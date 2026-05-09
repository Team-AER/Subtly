//! HTTP model downloads with streaming progress + atomic rename.

use crate::events::Event;
use crate::models::{find_descriptor, ModelDescriptor};
use crate::paths::ensure_models_directory;
use anyhow::{anyhow, Result};
use futures::StreamExt;
use serde::Serialize;
use std::path::PathBuf;
use tokio::fs as tfs;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub model_id: String,
    pub progress: u8,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

/// Download a model into the user data dir; emits `Event::Progress` until done.
pub async fn download_model(
    model_id: &str,
    events: mpsc::Sender<Event>,
    cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<PathBuf> {
    let descriptor: &ModelDescriptor =
        find_descriptor(model_id).ok_or_else(|| anyhow!("Unknown model: {model_id}"))?;
    let dir = ensure_models_directory()?;
    let dest = dir.join(descriptor.filename);
    let temp = dir.join(format!("{}.download", descriptor.filename));

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;
    let response = client
        .get(descriptor.url)
        .send()
        .await?
        .error_for_status()?;
    let total = response.content_length().unwrap_or(descriptor.size_bytes);

    let mut file = tfs::File::create(&temp).await?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_percent: u8 = u8::MAX;

    while let Some(chunk) = stream.next().await {
        if *cancel.borrow() {
            drop(file);
            let _ = tfs::remove_file(&temp).await;
            return Err(anyhow!("Download cancelled"));
        }
        let bytes = chunk?;
        file.write_all(&bytes).await?;
        downloaded += bytes.len() as u64;
        let pct = if total == 0 {
            0
        } else {
            ((downloaded as f64 / total as f64) * 100.0)
                .round()
                .min(100.0) as u8
        };
        if pct != last_percent {
            last_percent = pct;
            let _ = events
                .send(Event::Progress(crate::events::ProgressUpdate {
                    progress: Some(pct),
                    current: Some(downloaded),
                    total: Some(total),
                    phase: Some(format!("Downloading {}", descriptor.name)),
                }))
                .await;
        }
    }
    file.flush().await?;
    drop(file);

    tfs::rename(&temp, &dest).await?;
    let _ = events.send(Event::Done).await;
    Ok(dest)
}

pub fn delete_model(model_id: &str) -> Result<bool> {
    let descriptor =
        find_descriptor(model_id).ok_or_else(|| anyhow!("Unknown model: {model_id}"))?;
    let path = crate::paths::models_directory().join(descriptor.filename);
    if path.exists() {
        std::fs::remove_file(path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn model_path_if_present(model_id: &str) -> Option<PathBuf> {
    let descriptor = find_descriptor(model_id)?;
    let path = crate::paths::models_directory().join(descriptor.filename);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}
