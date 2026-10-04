use crate::model::Account;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const SCOPE: &str = "XboxLive.signin offline_access";
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    #[serde(alias = "device_code")]
    pub device_code: String,
    #[serde(alias = "user_code")]
    pub user_code: String,
    #[serde(alias = "verification_uri")]
    pub verification_uri: String,
    pub message: String,
    pub interval: u64,
    #[serde(alias = "expires_in")]
    pub expires_in: u64,
}
#[derive(Deserialize)]
struct MsToken {
    access_token: String,
    refresh_token: String,
}
#[derive(Deserialize)]
struct XboxToken {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    claims: Value,
}
#[derive(Deserialize)]
struct McToken {
    access_token: String,
    expires_in: i64,
}
#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
}
#[derive(Serialize, Deserialize)]
struct Secret {
    refresh_token: String,
    access_token: String,
    expires_at: i64,
}
fn entry(id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new("app.blockyard.launcher", id).map_err(|e| e.to_string())
}
fn client_id(id: &str) -> Result<&str, String> {
    if id.trim().is_empty() {
        Err("Microsoft client ID is not configured in Settings".into())
    } else {
        Ok(id)
    }
}
pub async fn start(client: &reqwest::Client, id: &str) -> Result<DeviceCode, String> {
    let response = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[("client_id", client_id(id)?), ("scope", SCOPE)])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    response.json().await.map_err(|e| e.to_string())
}
async fn exchange(
    client: &reqwest::Client,
    id: &str,
    fields: &[(&str, &str)],
) -> Result<Value, String> {
    let mut form = vec![("client_id", client_id(id)?)];
    form.extend_from_slice(fields);
    let response = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .form(&form)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let value: Value = response.json().await.map_err(|e| e.to_string())?;
    if !status.is_success()
        && !matches!(
            value.get("error").and_then(Value::as_str),
            Some("authorization_pending" | "slow_down")
        )
    {
        return Err(format!(
            "Microsoft sign-in: {}",
            value
                .get("error_description")
                .and_then(Value::as_str)
                .unwrap_or("request failed")
        ));
    }
    Ok(value)
}
async fn minecraft(
    client: &reqwest::Client,
    ms: &str,
) -> Result<(String, i64, String, Profile), String> {
    let xbox: XboxToken=client.post("https://user.auth.xboxlive.com/user/authenticate").json(&json!({"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":format!("d={ms}")},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"})).send().await.map_err(|e|e.to_string())?.error_for_status().map_err(|e|format!("Xbox Live sign-in failed: {e}"))?.json().await.map_err(|e|e.to_string())?;
    let user_hash = xbox
        .claims
        .pointer("/xui/0/uhs")
        .and_then(Value::as_str)
        .ok_or("Xbox user hash missing")?
        .to_owned();
    let xuid = xbox
        .claims
        .pointer("/xui/0/xid")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let xsts: XboxToken=client.post("https://xsts.auth.xboxlive.com/xsts/authorize").json(&json!({"Properties":{"SandboxId":"RETAIL","UserTokens":[xbox.token]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"})).send().await.map_err(|e|e.to_string())?.error_for_status().map_err(|e|format!("Xbox XSTS failed: {e}"))?.json().await.map_err(|e|e.to_string())?;
    let mc: McToken = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({"identityToken":format!("XBL3.0 x={user_hash};{}",xsts.token)}))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| format!("Minecraft sign-in failed: {e}"))?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let profile: Profile = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| format!("Minecraft Java profile unavailable. Confirm account ownership: {e}"))?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok((
        mc.access_token,
        chrono::Utc::now().timestamp() + mc.expires_in,
        xuid,
        profile,
    ))
}
pub async fn finish(
    client: &reqwest::Client,
    id: &str,
    code: &DeviceCode,
) -> Result<Account, String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(code.expires_in);
    loop {
        if std::time::Instant::now() > deadline {
            return Err("Sign-in timed out".into());
        }
        let value = exchange(
            client,
            id,
            &[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", &code.device_code),
            ],
        )
        .await?;
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            let delay = if error == "slow_down" {
                code.interval.max(5) + 5
            } else {
                code.interval.max(5)
            };
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            continue;
        }
        let token: MsToken = serde_json::from_value(value)
            .map_err(|e| format!("Invalid Microsoft token response: {e}"))?;
        let (access_token, expires_at, xuid, profile) =
            minecraft(client, &token.access_token).await?;
        let secret = Secret {
            refresh_token: token.refresh_token,
            access_token,
            expires_at,
        };
        entry(&profile.id)?
            .set_password(&serde_json::to_string(&secret).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Could not save token in system keyring: {e}"))?;
        return Ok(Account {
            id: profile.id,
            name: profile.name,
            kind: crate::model::AccountKind::Microsoft,
            xuid,
            expires_at,
        });
    }
}
pub async fn access(
    client: &reqwest::Client,
    client_id: &str,
    account: &mut Account,
) -> Result<String, String> {
    let mut secret: Secret = serde_json::from_str(
        &entry(&account.id)?
            .get_password()
            .map_err(|e| format!("Account token unavailable: {e}"))?,
    )
    .map_err(|e| e.to_string())?;
    if secret.expires_at > chrono::Utc::now().timestamp() + 120 {
        return Ok(secret.access_token);
    }
    let value = exchange(
        client,
        client_id,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", &secret.refresh_token),
        ],
    )
    .await?;
    let ms: MsToken =
        serde_json::from_value(value).map_err(|e| format!("Microsoft refresh failed: {e}"))?;
    let (access_token, expires_at, xuid, profile) = minecraft(client, &ms.access_token).await?;
    secret = Secret {
        refresh_token: ms.refresh_token,
        access_token,
        expires_at,
    };
    entry(&account.id)?
        .set_password(&serde_json::to_string(&secret).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    account.name = profile.name;
    account.xuid = xuid;
    account.expires_at = expires_at;
    Ok(secret.access_token)
}
pub fn logout(id: &str) -> Result<(), String> {
    match entry(id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
