use crate::{
    download::{self, Job},
    model::{Instance, Loader},
    store,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashSet,
    io::Read,
    path::{Path, PathBuf},
};

const MODRINTH: &str = "https://api.modrinth.com/v2";
const CURSEFORGE: &str = "https://api.curseforge.com/v1";
const MAX_ARCHIVE: u64 = 300 * 1024 * 1024;
const MAX_EXTRACTED: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Modrinth,
    Curseforge,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Mod,
    Modpack,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub provider: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub id: String,
    pub name: String,
    pub minecraft: Vec<String>,
    pub loaders: Vec<String>,
    pub date: String,
}

fn cf_key() -> Result<String, String> {
    keyring::Entry::new("app.blockyard.launcher", "curseforge-api-key")
        .map_err(|e| e.to_string())?
        .get_password()
        .map_err(|_| "CurseForge API key is missing. Add it in Settings.".to_owned())
}
pub fn key_is_set() -> bool {
    cf_key().is_ok()
}
pub fn set_key(value: &str) -> Result<(), String> {
    let entry = keyring::Entry::new("app.blockyard.launcher", "curseforge-api-key")
        .map_err(|e| e.to_string())?;
    if value.trim().is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    } else {
        entry.set_password(value.trim()).map_err(|e| e.to_string())
    }
}
async fn json(
    client: &reqwest::Client,
    provider: &Provider,
    url: reqwest::Url,
) -> Result<Value, String> {
    let mut request = client.get(url);
    if matches!(provider, Provider::Curseforge) {
        request = request.header("x-api-key", cf_key()?);
    }
    let response = request.send().await.map_err(|e| e.to_string())?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED
        || response.status() == reqwest::StatusCode::FORBIDDEN
    {
        return Err("CurseForge API key was rejected or lacks permission".into());
    }
    response
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}
fn url(base: &str, path: &str) -> Result<reqwest::Url, String> {
    reqwest::Url::parse(&format!("{base}{path}")).map_err(|e| e.to_string())
}
fn str_field<'a>(v: &'a Value, field: &str) -> &'a str {
    v.get(field).and_then(Value::as_str).unwrap_or("")
}
fn as_strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}
fn loader_name(loader: &Loader) -> &'static str {
    match loader {
        Loader::Vanilla => "",
        Loader::Fabric(_) => "fabric",
        Loader::Neoforge(_) => "neoforge",
    }
}
fn cf_loader(loader: &Loader) -> Option<&'static str> {
    match loader {
        Loader::Vanilla => None,
        Loader::Fabric(_) => Some("4"),
        Loader::Neoforge(_) => Some("6"),
    }
}

pub async fn search(
    client: &reqwest::Client,
    provider: Provider,
    kind: Kind,
    query: &str,
    instance: Option<&Instance>,
) -> Result<Vec<Project>, String> {
    if query.len() > 120 {
        return Err("Search is too long".into());
    }
    let mut u = match provider {
        Provider::Modrinth => url(MODRINTH, "/search")?,
        Provider::Curseforge => url(CURSEFORGE, "/mods/search")?,
    };
    match provider {
        Provider::Modrinth => {
            let mut facets = vec![vec![format!(
                "project_type:{}",
                if matches!(kind, Kind::Mod) {
                    "mod"
                } else {
                    "modpack"
                }
            )]];
            if let Some(i) = instance {
                facets.push(vec![format!("versions:{}", i.version)]);
                if !loader_name(&i.loader).is_empty() {
                    facets.push(vec![format!("categories:{}", loader_name(&i.loader))]);
                }
            }
            u.query_pairs_mut()
                .append_pair("query", query)
                .append_pair("limit", "30")
                .append_pair(
                    "facets",
                    &serde_json::to_string(&facets).map_err(|e| e.to_string())?,
                );
            let v = json(client, &provider, u).await?;
            Ok(v["hits"]
                .as_array()
                .ok_or("Invalid Modrinth search response")?
                .iter()
                .map(|p| Project {
                    id: str_field(p, "project_id").to_owned(),
                    title: str_field(p, "title").to_owned(),
                    description: str_field(p, "description").to_owned(),
                    icon_url: p["icon_url"].as_str().map(str::to_owned),
                    downloads: p["downloads"].as_u64().unwrap_or(0),
                    provider: "modrinth".into(),
                })
                .collect())
        }
        Provider::Curseforge => {
            {
                let mut pairs = u.query_pairs_mut();
                pairs
                    .append_pair("gameId", "432")
                    .append_pair(
                        "classId",
                        if matches!(kind, Kind::Mod) {
                            "6"
                        } else {
                            "4471"
                        },
                    )
                    .append_pair("searchFilter", query)
                    .append_pair("pageSize", "30");
                if let Some(i) = instance {
                    pairs.append_pair("gameVersion", &i.version);
                    if let Some(loader) = cf_loader(&i.loader) {
                        pairs.append_pair("modLoaderType", loader);
                    }
                }
            }
            let v = json(client, &provider, u).await?;
            Ok(v["data"]
                .as_array()
                .ok_or("Invalid CurseForge search response")?
                .iter()
                .map(|p| Project {
                    id: p["id"].to_string(),
                    title: str_field(p, "name").to_owned(),
                    description: str_field(p, "summary").to_owned(),
                    icon_url: p["logo"]["thumbnailUrl"].as_str().map(str::to_owned),
                    downloads: p["downloadCount"].as_f64().unwrap_or(0.0) as u64,
                    provider: "curseforge".into(),
                })
                .collect())
        }
    }
}
pub async fn releases(
    client: &reqwest::Client,
    provider: Provider,
    project: &str,
    instance: Option<&Instance>,
) -> Result<Vec<Release>, String> {
    let mut u = match provider {
        Provider::Modrinth => url(MODRINTH, &format!("/project/{project}/version"))?,
        Provider::Curseforge => url(CURSEFORGE, &format!("/mods/{project}/files"))?,
    };
    match provider {
        Provider::Modrinth => {
            if let Some(i) = instance {
                u.query_pairs_mut().append_pair(
                    "game_versions",
                    &serde_json::to_string(&vec![&i.version]).map_err(|e| e.to_string())?,
                );
                if !loader_name(&i.loader).is_empty() {
                    u.query_pairs_mut().append_pair(
                        "loaders",
                        &serde_json::to_string(&vec![loader_name(&i.loader)])
                            .map_err(|e| e.to_string())?,
                    );
                }
            }
            let v = json(client, &provider, u).await?;
            Ok(v.as_array()
                .ok_or("Invalid Modrinth version response")?
                .iter()
                .map(|p| Release {
                    id: str_field(p, "id").to_owned(),
                    name: str_field(p, "name").to_owned(),
                    minecraft: as_strings(&p["game_versions"]),
                    loaders: as_strings(&p["loaders"]),
                    date: str_field(p, "date_published").to_owned(),
                })
                .collect())
        }
        Provider::Curseforge => {
            {
                let mut pairs = u.query_pairs_mut();
                pairs.append_pair("pageSize", "50");
                if let Some(i) = instance {
                    pairs.append_pair("gameVersion", &i.version);
                    if let Some(loader) = cf_loader(&i.loader) {
                        pairs.append_pair("modLoaderType", loader);
                    }
                }
            }
            let v = json(client, &provider, u).await?;
            Ok(v["data"]
                .as_array()
                .ok_or("Invalid CurseForge files response")?
                .iter()
                .map(|p| Release {
                    id: p["id"].to_string(),
                    name: str_field(p, "displayName").to_owned(),
                    minecraft: as_strings(&p["gameVersions"]),
                    loaders: Vec::new(),
                    date: str_field(p, "fileDate").to_owned(),
                })
                .collect())
        }
    }
}
async fn version(
    client: &reqwest::Client,
    provider: &Provider,
    project: &str,
    release: &str,
) -> Result<Value, String> {
    let u = match provider {
        Provider::Modrinth => url(MODRINTH, &format!("/version/{release}"))?,
        Provider::Curseforge => url(CURSEFORGE, &format!("/mods/{project}/files/{release}"))?,
    };
    let v = json(client, provider, u).await?;
    if matches!(provider, Provider::Curseforge) {
        Ok(v["data"].clone())
    } else {
        Ok(v)
    }
}
fn chosen_file(v: &Value, provider: &Provider, extension: &str) -> Result<(String, Job), String> {
    let file = match provider {
        Provider::Modrinth => v["files"].as_array().and_then(|a| {
            a.iter()
                .find(|f| f["primary"] == true && str_field(f, "filename").ends_with(extension))
                .or_else(|| {
                    a.iter()
                        .find(|f| str_field(f, "filename").ends_with(extension))
                })
        }),
        Provider::Curseforge => Some(v),
    }
    .ok_or("No compatible downloadable file")?;
    let name = str_field(
        file,
        if matches!(provider, Provider::Modrinth) {
            "filename"
        } else {
            "fileName"
        },
    );
    if !store::safe_filename(name) || name.contains(':') || !name.ends_with(extension) {
        return Err("Unsafe or unsupported file name".into());
    }
    let download_url = if matches!(provider, Provider::Modrinth) {
        file["url"].as_str()
    } else {
        file["downloadUrl"].as_str()
    }
    .ok_or("Publisher has disabled third-party downloads for this file")?;
    let sha1 = if matches!(provider, Provider::Modrinth) {
        file["hashes"]["sha1"].as_str().map(str::to_owned)
    } else {
        file["hashes"]
            .as_array()
            .and_then(|a| a.iter().find(|h| h["algo"] == 1))
            .and_then(|h| h["value"].as_str())
            .map(str::to_owned)
    };
    let size = if matches!(provider, Provider::Modrinth) {
        file["size"].as_u64()
    } else {
        file["fileLength"].as_u64()
    };
    Ok((
        name.to_owned(),
        Job {
            url: download_url.into(),
            path: PathBuf::new(),
            sha1,
            size,
        },
    ))
}
async fn fetch_retry(client: &reqwest::Client, job: &Job) -> Result<(), String> {
    let mut last = String::new();
    for attempt in 0..3 {
        match download::fetch(client, job).await {
            Ok(()) => return Ok(()),
            Err(error) => last = error,
        }
        if attempt < 2 {
            tokio::time::sleep(std::time::Duration::from_millis(500 * (1 << attempt))).await;
        }
    }
    Err(last)
}
pub async fn install_mod(
    client: &reqwest::Client,
    root: &Path,
    instance: &Instance,
    provider: Provider,
    project: &str,
    release: &str,
) -> Result<(), String> {
    if matches!(instance.loader, Loader::Vanilla) {
        return Err("Choose a Fabric or NeoForge instance for mods".into());
    }
    let v = version(client, &provider, project, release).await?;
    if matches!(provider, Provider::Modrinth)
        && (!as_strings(&v["game_versions"]).contains(&instance.version)
            || !as_strings(&v["loaders"])
                .iter()
                .any(|l| l == loader_name(&instance.loader)))
    {
        return Err("Mod version does not match this instance".into());
    }
    let mut queue = vec![(project.to_owned(), v)];
    let mut seen = HashSet::new();
    while let Some((project_id, current)) = queue.pop() {
        if !seen.insert(project_id) {
            continue;
        }
        let (name, mut job) = chosen_file(&current, &provider, ".jar")?;
        job.path = root
            .join("instances")
            .join(&instance.id)
            .join("game/mods")
            .join(name);
        fetch_retry(client, &job).await?;
        if matches!(provider, Provider::Modrinth) {
            if let Some(deps) = current["dependencies"].as_array() {
                for dep in deps.iter().filter(|d| d["dependency_type"] == "required") {
                    let id = str_field(dep, "project_id");
                    let pinned = str_field(dep, "version_id");
                    if id.is_empty() && pinned.is_empty() {
                        return Err(
                            "Required Modrinth dependency has no project or version ID".into()
                        );
                    }
                    let key = if id.is_empty() { pinned } else { id };
                    if seen.contains(key) {
                        continue;
                    }
                    if !pinned.is_empty() {
                        let selected = version(client, &Provider::Modrinth, id, pinned).await?;
                        if !as_strings(&selected["game_versions"]).contains(&instance.version)
                            || !as_strings(&selected["loaders"])
                                .iter()
                                .any(|l| l == loader_name(&instance.loader))
                        {
                            return Err(format!(
                                "Required dependency {key} is incompatible with this instance"
                            ));
                        }
                        queue.push((key.to_owned(), selected));
                        continue;
                    }
                    let mut u = url(MODRINTH, &format!("/project/{id}/version"))?;
                    u.query_pairs_mut()
                        .append_pair(
                            "game_versions",
                            &serde_json::to_string(&vec![&instance.version])
                                .map_err(|e| e.to_string())?,
                        )
                        .append_pair(
                            "loaders",
                            &serde_json::to_string(&vec![loader_name(&instance.loader)])
                                .map_err(|e| e.to_string())?,
                        );
                    let versions = json(client, &provider, u).await?;
                    let selected = versions
                        .as_array()
                        .and_then(|a| a.first())
                        .ok_or_else(|| {
                            format!("Required dependency {id} has no compatible version")
                        })?
                        .clone();
                    queue.push((id.to_owned(), selected));
                }
            }
        } else if let Some(deps) = current["dependencies"].as_array() {
            for dep in deps.iter().filter(|d| d["relationType"] == 3) {
                let id = dep["modId"]
                    .as_u64()
                    .ok_or("Invalid CurseForge dependency")?
                    .to_string();
                if seen.contains(&id) {
                    continue;
                }
                let choices = releases(client, Provider::Curseforge, &id, Some(instance)).await?;
                let choice = choices
                    .first()
                    .ok_or_else(|| format!("Required dependency {id} has no compatible version"))?;
                queue.push((
                    id.clone(),
                    version(client, &Provider::Curseforge, &id, &choice.id).await?,
                ));
            }
        }
    }
    Ok(())
}
fn allowed_pack_url(raw: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(raw) else {
        return false;
    };
    url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("cdn.modrinth.com" | "github.com" | "raw.githubusercontent.com" | "gitlab.com")
        )
}
fn safe_pack_path(raw: &str) -> Result<&Path, String> {
    if raw.contains('\\') || raw.contains(':') {
        return Err(format!("Unsafe pack path: {raw}"));
    }
    download::relative_path(raw)
}
fn pack_loader(deps: &Value) -> Result<(String, Loader), String> {
    let mc = str_field(deps, "minecraft");
    if mc.is_empty() {
        return Err("Pack has no Minecraft version".into());
    }
    let unsupported = ["forge", "quilt-loader"]
        .iter()
        .any(|k| deps.get(k).is_some());
    if unsupported {
        return Err("This pack requires Forge or Quilt, which Blockyard does not support".into());
    }
    let loader = if let Some(v) = deps["fabric-loader"].as_str() {
        Loader::Fabric(v.into())
    } else if let Some(v) = deps["neoforge"].as_str() {
        Loader::Neoforge(v.into())
    } else {
        Loader::Vanilla
    };
    Ok((mc.into(), loader))
}
fn extract_overrides(
    zip: &mut zip::ZipArchive<std::fs::File>,
    game: &Path,
    prefixes: &[&str],
) -> Result<(), String> {
    let mut total = 0u64;
    for prefix in prefixes {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            let Some(relative) = entry.name().strip_prefix(prefix) else {
                continue;
            };
            if relative.is_empty() || entry.is_dir() {
                continue;
            }
            if entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
                return Err("Symlinks in pack are not supported".into());
            }
            total = total.checked_add(entry.size()).ok_or("Pack is too large")?;
            if total > MAX_EXTRACTED {
                return Err("Pack overrides exceed size limit".into());
            }
            let path = game.join(safe_pack_path(relative)?);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut target = std::fs::File::create(path).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
pub async fn install_pack(
    client: &reqwest::Client,
    root: &Path,
    settings: &crate::model::Settings,
    provider: Provider,
    project: &str,
    release: &str,
) -> Result<Instance, String> {
    let v = version(client, &provider, project, release).await?;
    let extension = if matches!(provider, Provider::Modrinth) {
        ".mrpack"
    } else {
        ".zip"
    };
    let (_, mut archive_job) = chosen_file(&v, &provider, extension)?;
    if archive_job.size.is_some_and(|n| n > MAX_ARCHIVE) {
        return Err("Pack archive exceeds size limit".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let folder = root.join("instances").join(&id);
    let game = folder.join("game");
    std::fs::create_dir_all(&game).map_err(|e| e.to_string())?;
    archive_job.path = folder.join(format!("pack{extension}"));
    let result = async {
        fetch_retry(client, &archive_job).await?;
        let meta = std::fs::metadata(&archive_job.path).map_err(|e| e.to_string())?;
        if meta.len() > MAX_ARCHIVE {
            return Err("Pack archive exceeds size limit".into());
        }
        let file = std::fs::File::open(&archive_job.path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        if zip.len() > 10000 {
            return Err("Pack contains too many files".into());
        }
        let manifest_name = if matches!(provider, Provider::Modrinth) {
            "modrinth.index.json"
        } else {
            "manifest.json"
        };
        let mut manifest = String::new();
        zip.by_name(manifest_name)
            .map_err(|e| e.to_string())?
            .take(2_000_000)
            .read_to_string(&mut manifest)
            .map_err(|e| e.to_string())?;
        let index: Value = serde_json::from_str(&manifest).map_err(|e| e.to_string())?;
        let (name, mc, loader, jobs, prefixes) = match provider {
            Provider::Modrinth => {
                if index["formatVersion"] != 1 || index["game"] != "minecraft" {
                    return Err("Unsupported Modrinth pack format".into());
                }
                let (mc, loader) = pack_loader(&index["dependencies"])?;
                let mut jobs = Vec::new();
                for f in index["files"].as_array().ok_or("Pack files are missing")? {
                    if f["env"]["client"] == "unsupported" {
                        continue;
                    }
                    let path = safe_pack_path(str_field(f, "path"))?;
                    let source = f["downloads"]
                        .as_array()
                        .and_then(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .find(|s| allowed_pack_url(s))
                        })
                        .ok_or("Pack file has no permitted HTTPS download")?;
                    let sha1 = f["hashes"]["sha1"]
                        .as_str()
                        .ok_or("Pack file has no SHA-1 hash")?;
                    jobs.push(Job {
                        url: source.into(),
                        path: game.join(path),
                        sha1: Some(sha1.into()),
                        size: f["fileSize"].as_u64(),
                    });
                }
                (
                    str_field(&index, "name").to_owned(),
                    mc,
                    loader,
                    jobs,
                    vec!["overrides/".to_owned(), "client-overrides/".to_owned()],
                )
            }
            Provider::Curseforge => {
                if index["manifestType"] != "minecraftModpack" {
                    return Err("Invalid CurseForge modpack manifest".into());
                }
                let mc = str_field(&index["minecraft"], "version").to_owned();
                if mc.is_empty() {
                    return Err("Pack has no Minecraft version".into());
                }
                let loader_id = index["minecraft"]["modLoaders"]
                    .as_array()
                    .and_then(|a| {
                        a.iter()
                            .find(|l| l["primary"] == true)
                            .or_else(|| a.first())
                    })
                    .and_then(|l| l["id"].as_str())
                    .unwrap_or("");
                let loader = if let Some(v) = loader_id.strip_prefix("fabric-") {
                    Loader::Fabric(v.into())
                } else if let Some(v) = loader_id.strip_prefix("neoforge-") {
                    Loader::Neoforge(v.into())
                } else if loader_id.is_empty() {
                    Loader::Vanilla
                } else {
                    return Err(format!("Unsupported pack loader: {loader_id}"));
                };
                let mut jobs = Vec::new();
                for f in index["files"].as_array().ok_or("Pack files are missing")? {
                    if f["required"] == false {
                        continue;
                    }
                    let pid = f["projectID"].as_u64().ok_or("Invalid project ID")?;
                    let fid = f["fileID"].as_u64().ok_or("Invalid file ID")?;
                    let data = version(
                        client,
                        &Provider::Curseforge,
                        &pid.to_string(),
                        &fid.to_string(),
                    )
                    .await?;
                    let (name, mut job) = chosen_file(&data, &Provider::Curseforge, ".jar")?;
                    job.path = game.join("mods").join(name);
                    jobs.push(job);
                }
                let override_dir = str_field(&index, "overrides");
                let prefix = if override_dir.is_empty() {
                    "overrides/".to_owned()
                } else {
                    format!("{override_dir}/")
                };
                if safe_pack_path(&prefix).is_err() {
                    return Err("Unsafe overrides path".into());
                }
                (
                    str_field(&index, "name").to_owned(),
                    mc,
                    loader,
                    jobs,
                    vec![prefix],
                )
            }
        };
        if name.trim().is_empty() || name.len() > 80 {
            return Err("Invalid pack name".into());
        }
        if jobs.len() > 10000 {
            return Err("Pack contains too many downloads".into());
        }
        let mut paths = HashSet::new();
        if jobs.iter().any(|job| !paths.insert(job.path.clone())) {
            return Err("Pack contains duplicate file destinations".into());
        }
        extract_overrides(
            &mut zip,
            &game,
            &prefixes.iter().map(String::as_str).collect::<Vec<_>>(),
        )?;
        let results = futures::stream::iter(
            jobs.into_iter()
                .map(|j| async move { fetch_retry(client, &j).await }),
        )
        .buffer_unordered(settings.download_concurrency.clamp(1, 32))
        .collect::<Vec<_>>()
        .await;
        for r in results {
            r?;
        }
        let _ = std::fs::remove_file(&archive_job.path);
        Ok(Instance {
            id,
            name,
            version: mc,
            loader,
            java_path: None,
            ram_mb: settings.ram_mb,
            jvm_args: String::new(),
            width: settings.width,
            height: settings.height,
            icon: "cube".into(),
            last_played: None,
            playtime_seconds: 0,
        })
    }
    .await;
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&folder);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths() {
        assert!(allowed_pack_url("https://cdn.modrinth.com/data/x"));
        assert!(!allowed_pack_url("https://cdn.modrinth.com.evil.test/x"));
        assert!(!allowed_pack_url("http://github.com/x"));
        assert!(safe_pack_path("mods/a.jar").is_ok());
        assert!(safe_pack_path("../bad").is_err());
        assert!(safe_pack_path("C:/bad").is_err());
        assert!(safe_pack_path("a\\b").is_err());
    }
}
