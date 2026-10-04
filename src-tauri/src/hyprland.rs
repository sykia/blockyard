//! Request a normal floating launcher window from Hyprland without changing
//! the user's compositor configuration. Other desktop environments are untouched.

use serde::Deserialize;
use tauri::Manager;
use tokio::{
    process::Command,
    time::{sleep, Duration},
};

#[derive(Deserialize)]
struct Client {
    address: String,
    title: String,
    pid: u32,
    floating: bool,
}

fn selector(address: &str) -> Option<String> {
    let hex = address.strip_prefix("0x")?;
    if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("address:{address}"))
}

async fn set_floating(address: &str) -> Result<(), String> {
    let selector = selector(address).ok_or("Invalid Hyprland window address")?;
    // Hyprland 0.55+ uses Lua dispatchers. Older releases use positional
    // dispatcher arguments. Both commands receive an exact window address.
    let lua = format!("hl.dsp.window.float({{ action = \"set\", window = \"{selector}\" }})");
    let modern = Command::new("hyprctl")
        .args(["dispatch", &lua])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if modern.status.success() {
        return Ok(());
    }
    let legacy = Command::new("hyprctl")
        .args(["dispatch", "setfloating", &selector])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if legacy.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} {}",
            String::from_utf8_lossy(&modern.stdout).trim(),
            String::from_utf8_lossy(&legacy.stdout).trim()
        ))
    }
}

pub fn float_main_window(app: &tauri::AppHandle) {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return;
    }
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        let pid = std::process::id();
        for _ in 0..30 {
            sleep(Duration::from_millis(100)).await;
            let output = match Command::new("hyprctl")
                .args(["clients", "-j"])
                .output()
                .await
            {
                Ok(output) if output.status.success() => output,
                _ => return,
            };
            let Ok(clients) = serde_json::from_slice::<Vec<Client>>(&output.stdout) else {
                return;
            };
            let Some(client) = clients
                .iter()
                .find(|c| c.pid == pid && c.title == "Blockyard")
            else {
                continue;
            };
            if !client.floating {
                if let Err(error) = set_floating(&client.address).await {
                    eprintln!("Could not float Blockyard window: {error}");
                    return;
                }
            }
            if let Err(error) = window.center() {
                eprintln!("Could not center Blockyard window: {error}");
            }
            return;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::selector;

    #[test]
    fn hyprland_address_is_safe_for_lua_dispatch() {
        assert_eq!(selector("0x1aBc"), Some("address:0x1aBc".into()));
        assert_eq!(selector("0x1\" })"), None);
        assert_eq!(selector("0x"), None);
    }
}
