use crate::model::GameStatus;
use futures::{stream, StreamExt};
use sha1::{Digest, Sha1};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tauri::Emitter;
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
};

#[derive(Clone)]
pub struct Job {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}
pub fn relative_path(path: &str) -> Result<&Path, String> {
    let p = Path::new(path);
    if path.is_empty()
        || p.is_absolute()
        || p.components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(format!("Unsafe metadata path: {path}"));
    }
    Ok(p)
}
pub async fn valid(path: &Path, hash: Option<&str>, size: Option<u64>) -> bool {
    let Ok(meta) = fs::metadata(path).await else {
        return false;
    };
    if size.is_some_and(|s| s != meta.len()) {
        return false;
    }
    if let Some(expected) = hash {
        let Ok(mut file) = fs::File::open(path).await else {
            return false;
        };
        let mut digest = Sha1::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = match file.read(&mut buffer).await {
                Ok(count) => count,
                Err(_) => return false,
            };
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
        }
        return hex::encode(digest.finalize()).eq_ignore_ascii_case(expected);
    }
    true
}
pub async fn fetch(client: &reqwest::Client, job: &Job) -> Result<(), String> {
    fetch_inner(client, job, None).await
}
pub async fn fetch_with_key(client: &reqwest::Client, job: &Job, key: &str) -> Result<(), String> {
    fetch_inner(client, job, Some(key)).await
}
async fn fetch_inner(client: &reqwest::Client, job: &Job, key: Option<&str>) -> Result<(), String> {
    if valid(&job.path, job.sha1.as_deref(), job.size).await {
        return Ok(());
    }
    if !job.url.starts_with("https://") {
        return Err(format!("Non HTTPS download rejected: {}", job.url));
    }
    if let Some(parent) = job.path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    let tmp = job
        .path
        .with_extension(format!("part-{}", uuid::Uuid::new_v4()));
    let result = async {
        let mut request = client.get(&job.url);
        if let Some(key) = key {
            request = request.header("x-api-key", key);
        }
        let response = request
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        if response.url().scheme() != "https" {
            return Err("Download redirected to non HTTPS URL".into());
        }
        let mut file = fs::File::create(&tmp).await.map_err(|e| e.to_string())?;
        let mut stream = response.bytes_stream();
        let mut digest = Sha1::new();
        let mut bytes = 0u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            digest.update(&chunk);
            bytes += chunk.len() as u64;
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())?;
        if job.size.is_some_and(|s| s != bytes) {
            return Err(format!("Incorrect size: {}", job.url));
        }
        if job
            .sha1
            .as_ref()
            .is_some_and(|h| !hex::encode(digest.finalize()).eq_ignore_ascii_case(h))
        {
            return Err(format!("SHA-1 mismatch: {}", job.url));
        }
        #[cfg(windows)]
        if job.path.exists() {
            fs::remove_file(&job.path)
                .await
                .map_err(|e| e.to_string())?;
        }
        fs::rename(&tmp, &job.path).await.map_err(|e| e.to_string())
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&tmp).await;
    }
    result
}
pub async fn fetch_all(
    client: &reqwest::Client,
    jobs: Vec<Job>,
    concurrency: usize,
    app: &tauri::AppHandle,
    id: &str,
    phase: &str,
) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    let jobs = jobs
        .into_iter()
        .filter(|job| seen.insert(job.path.clone()))
        .collect::<Vec<_>>();
    let total = jobs.len();
    let progress_step = (total / 100).max(1);
    let done = Arc::new(AtomicUsize::new(0));
    let results = stream::iter(jobs.into_iter().map(|job| {
        let client = client.clone();
        let done = done.clone();
        let app = app.clone();
        let id = id.to_owned();
        let phase = phase.to_owned();
        async move {
            let mut error = String::new();
            for attempt in 0..3 {
                match fetch(&client, &job).await {
                    Ok(()) => {
                        error.clear();
                        break;
                    }
                    Err(e) => {
                        error = e;
                        if attempt < 2 {
                            tokio::time::sleep(std::time::Duration::from_millis(
                                500 * (1 << attempt),
                            ))
                            .await;
                        }
                    }
                }
            }
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            if n == total || n % progress_step == 0 {
                let _ = app.emit(
                    "game-status",
                    GameStatus {
                        instance_id: id,
                        phase,
                        progress: n as f64 / total as f64,
                        message: format!("{n} / {total}"),
                        exit_code: None,
                    },
                );
            }
            if error.is_empty() {
                Ok(())
            } else {
                Err(error)
            }
        }
    }))
    .buffer_unordered(concurrency.clamp(1, 32))
    .collect::<Vec<_>>()
    .await;
    let errors: Vec<String> = results.into_iter().filter_map(Result::err).collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} downloads failed; first: {}",
            errors.len(),
            errors[0]
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn traversal() {
        assert!(relative_path("../escape").is_err());
        assert!(relative_path("a/b.jar").is_ok());
        assert!(relative_path("/tmp/x").is_err());
    }
    #[tokio::test]
    async fn cached_file_requires_correct_hash_and_size() {
        let path = std::env::temp_dir().join(format!("blockyard-hash-{}", uuid::Uuid::new_v4()));
        fs::write(&path, b"cached asset").await.unwrap();
        let hash = hex::encode(Sha1::digest(b"cached asset"));
        assert!(valid(&path, Some(&hash), Some(12)).await);
        assert!(!valid(&path, Some(&hash), Some(11)).await);
        assert!(!valid(&path, Some(&"0".repeat(40)), Some(12)).await);
        fs::remove_file(path).await.unwrap();
    }
}
