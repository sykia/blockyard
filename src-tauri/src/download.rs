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
use tokio::{fs, io::AsyncWriteExt};

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
        let Ok(data) = fs::read(path).await else {
            return false;
        };
        return hex::encode(Sha1::digest(&data)).eq_ignore_ascii_case(expected);
    }
    true
}
pub async fn fetch(client: &reqwest::Client, job: &Job) -> Result<(), String> {
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
        let response = client
            .get(&job.url)
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
            let _ = app.emit(
                "game-status",
                GameStatus {
                    instance_id: id,
                    phase,
                    progress: if total == 0 {
                        1.0
                    } else {
                        n as f64 / total as f64
                    },
                    message: format!("{n} / {total}"),
                    exit_code: None,
                },
            );
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
}
