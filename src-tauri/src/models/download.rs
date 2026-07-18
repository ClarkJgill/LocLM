//! Hugging Face GGUF download manager with pause/resume and SHA-256 verify.

use super::catalog::{
    find_catalog_model, model_file_path, models_dir, partial_file_path, CatalogModel,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadPhase {
    Idle,
    Downloading,
    Paused,
    Verifying,
    Completed,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub model_id: String,
    pub phase: DownloadPhase,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub bytes_per_sec: u64,
    pub percent: f32,
    pub message: String,
}

impl DownloadProgress {
    fn new(model_id: &str, phase: DownloadPhase, downloaded: u64, total: u64, bps: u64, message: impl Into<String>) -> Self {
        let percent = if total > 0 {
            (downloaded as f64 / total as f64 * 100.0).clamp(0.0, 100.0) as f32
        } else {
            0.0
        };
        Self {
            model_id: model_id.into(),
            phase,
            downloaded_bytes: downloaded,
            total_bytes: total,
            bytes_per_sec: bps,
            percent,
            message: message.into(),
        }
    }
}

struct ActiveDownload {
    model_id: String,
    pause: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

pub struct DownloadManager {
    active: Mutex<Option<ActiveDownload>>,
    last: Mutex<Option<DownloadProgress>>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(None),
            last: Mutex::new(None),
        }
    }

    pub fn last_progress(&self) -> Option<DownloadProgress> {
        self.last.lock().expect("download mutex").clone()
    }

    pub fn pause(&self, model_id: &str) -> Result<(), String> {
        let guard = self.active.lock().expect("download mutex");
        match guard.as_ref() {
            Some(active) if active.model_id == model_id => {
                active.pause.store(true, Ordering::SeqCst);
                Ok(())
            }
            Some(_) => Err("A different download is in progress".into()),
            None => Err("No active download to pause".into()),
        }
    }

    pub fn resume(&self, app: AppHandle, model_id: String) -> Result<(), String> {
        // Resume = start again; Range request picks up the .partial file.
        self.start(app, model_id)
    }

    pub fn cancel(&self, model_id: &str) -> Result<(), String> {
        let guard = self.active.lock().expect("download mutex");
        match guard.as_ref() {
            Some(active) if active.model_id == model_id => {
                active.cancel.store(true, Ordering::SeqCst);
                active.pause.store(false, Ordering::SeqCst);
                Ok(())
            }
            Some(_) => Err("A different download is in progress".into()),
            None => Err("No active download to cancel".into()),
        }
    }

    pub fn start(&self, app: AppHandle, model_id: String) -> Result<(), String> {
        {
            let guard = self.active.lock().expect("download mutex");
            if let Some(active) = guard.as_ref() {
                if active.model_id == model_id && !active.cancel.load(Ordering::SeqCst) {
                    // Unpause if paused
                    if active.pause.load(Ordering::SeqCst) {
                        active.pause.store(false, Ordering::SeqCst);
                        return Ok(());
                    }
                    return Err("This model is already downloading".into());
                }
                if !active.cancel.load(Ordering::SeqCst) {
                    return Err("Another download is already in progress — pause or cancel it first".into());
                }
            }
        }

        let model = find_catalog_model(&model_id)
            .ok_or_else(|| format!("Unknown model id: {model_id}"))?;
        let root = models_dir(&app)?;
        let final_path = model_file_path(&root, &model);
        if final_path.is_file() {
            return Err("Model is already downloaded".into());
        }

        let pause = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));

        {
            let mut guard = self.active.lock().expect("download mutex");
            *guard = Some(ActiveDownload {
                model_id: model_id.clone(),
                pause: pause.clone(),
                cancel: cancel.clone(),
            });
        }

        let app2 = app.clone();
        let model2 = model.clone();
        std::thread::Builder::new()
            .name(format!("loclm-dl-{model_id}"))
            .spawn(move || {
                let result = run_download(&app2, &model2, &pause, &cancel);
                if let Err(e) = result {
                    let prog = DownloadProgress::new(
                        &model2.id,
                        DownloadPhase::Error,
                        0,
                        model2.size_bytes,
                        0,
                        e,
                    );
                    emit(&app2, prog);
                }
            })
            .map_err(|e| format!("Failed to spawn download thread: {e}"))?;

        let initial = DownloadProgress::new(
            &model_id,
            DownloadPhase::Downloading,
            0,
            model.size_bytes,
            0,
            "Starting download…",
        );
        self.set_last(initial.clone());
        let _ = app.emit("download-progress", &initial);

        Ok(())
    }

    pub fn clear_active_if(&self, model_id: &str) {
        let mut guard = self.active.lock().expect("download mutex");
        if guard.as_ref().is_some_and(|a| a.model_id == model_id) {
            *guard = None;
        }
    }

    pub fn set_last(&self, progress: DownloadProgress) {
        *self.last.lock().expect("download mutex") = Some(progress);
    }
}

fn run_download(
    app: &AppHandle,
    model: &CatalogModel,
    pause: &AtomicBool,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let cleanup = |app: &AppHandle, model_id: &str| {
        if let Some(mgr) = app.try_state::<DownloadManager>() {
            mgr.clear_active_if(model_id);
        }
    };

    let result = run_download_inner(app, model, pause, cancel);
    cleanup(app, &model.id);
    result
}

fn run_download_inner(
    app: &AppHandle,
    model: &CatalogModel,
    pause: &AtomicBool,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let root = models_dir(app)?;
    let partial = partial_file_path(&root, model);
    let final_path = model_file_path(&root, model);

    let mut downloaded = if partial.is_file() {
        std::fs::metadata(&partial).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    let total = model.size_bytes;
    emit(
        app,
        DownloadProgress::new(
            &model.id,
            DownloadPhase::Downloading,
            downloaded,
            total,
            0,
            if downloaded > 0 {
                "Resuming download…"
            } else {
                "Connecting to Hugging Face…"
            },
        ),
    );

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(30))
        .timeout_read(std::time::Duration::from_secs(60))
        .user_agent("LocLM/0.1 (local LLM desktop app)")
        .build();

    let mut request = agent.get(&model.download_url);
    if downloaded > 0 {
        request = request.set("Range", &format!("bytes={downloaded}-"));
    }

    let response = request
        .call()
        .map_err(|e| format!("Download request failed: {e}"))?;
    let status = response.status();

    if downloaded > 0 && status == 200 {
        // Server ignored Range — restart from scratch.
        downloaded = 0;
        let _ = std::fs::remove_file(&partial);
    } else if downloaded > 0 && status != 206 {
        return Err(format!("Unexpected HTTP status while resuming: {status}"));
    } else if downloaded == 0 && status != 200 {
        return Err(format!("Unexpected HTTP status: {status}"));
    }

    let mut file = if downloaded > 0 {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&partial)
            .map_err(|e| format!("Could not open partial file: {e}"))?
    } else {
        File::create(&partial).map_err(|e| format!("Could not create partial file: {e}"))?
    };

    let mut reader = response.into_reader();
    let mut buf = vec![0u8; 256 * 1024];
    let mut window_start = Instant::now();
    let mut window_bytes: u64 = 0;

    loop {
        if cancel.load(Ordering::SeqCst) {
            emit(
                app,
                DownloadProgress::new(
                    &model.id,
                    DownloadPhase::Idle,
                    downloaded,
                    total,
                    0,
                    "Download cancelled",
                ),
            );
            return Ok(());
        }

        while pause.load(Ordering::SeqCst) {
            if cancel.load(Ordering::SeqCst) {
                emit(
                    app,
                    DownloadProgress::new(
                        &model.id,
                        DownloadPhase::Idle,
                        downloaded,
                        total,
                        0,
                        "Download cancelled",
                    ),
                );
                return Ok(());
            }
            emit(
                app,
                DownloadProgress::new(
                    &model.id,
                    DownloadPhase::Paused,
                    downloaded,
                    total,
                    0,
                    "Paused — click Resume to continue",
                ),
            );
            std::thread::sleep(std::time::Duration::from_millis(250));
        }

        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Read error: {e}"))?;
        if n == 0 {
            break;
        }

        file.write_all(&buf[..n])
            .map_err(|e| format!("Write error: {e}"))?;
        downloaded += n as u64;
        window_bytes += n as u64;

        let elapsed = window_start.elapsed().as_secs_f64();
        if elapsed >= 0.4 {
            let bps = (window_bytes as f64 / elapsed) as u64;
            window_start = Instant::now();
            window_bytes = 0;
            emit(
                app,
                DownloadProgress::new(
                    &model.id,
                    DownloadPhase::Downloading,
                    downloaded,
                    total,
                    bps,
                    format!(
                        "Downloading {:.1}%",
                        downloaded as f64 / total.max(1) as f64 * 100.0
                    ),
                ),
            );
        }
    }

    file.flush().map_err(|e| format!("Flush error: {e}"))?;
    drop(file);

    if downloaded + 1024 < total {
        return Err(format!(
            "Download incomplete ({downloaded} / {total} bytes). Resume to retry."
        ));
    }

    emit(
        app,
        DownloadProgress::new(
            &model.id,
            DownloadPhase::Verifying,
            downloaded.max(total),
            total,
            0,
            "Verifying checksum…",
        ),
    );

    let digest = sha256_file(&partial)?;
    if !digest.eq_ignore_ascii_case(&model.sha256) {
        let _ = std::fs::remove_file(&partial);
        return Err(format!(
            "Checksum mismatch. Expected {}, got {digest}",
            model.sha256
        ));
    }

    std::fs::rename(&partial, &final_path)
        .map_err(|e| format!("Could not finalize model file: {e}"))?;

    emit(
        app,
        DownloadProgress::new(
            &model.id,
            DownloadPhase::Completed,
            total,
            total,
            0,
            format!("Ready — {}", final_path.display()),
        ),
    );

    Ok(())
}

fn emit(app: &AppHandle, progress: DownloadProgress) {
    if let Some(mgr) = app.try_state::<DownloadManager>() {
        mgr.set_last(progress.clone());
    }
    let _ = app.emit("download-progress", &progress);
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("Could not open file for hashing: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("Hash read error: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

// --- Tauri commands ---

#[tauri::command]
pub fn list_model_library(
    app: AppHandle,
) -> Result<Vec<super::catalog::ModelLibraryEntry>, String> {
    let hw = crate::hardware::detect();
    super::catalog::build_library(&app, &hw)
}

#[tauri::command]
pub fn get_models_dir(app: AppHandle) -> Result<String, String> {
    models_dir(&app).map(|p| p.display().to_string())
}

#[tauri::command]
pub fn start_model_download(
    app: AppHandle,
    manager: State<'_, DownloadManager>,
    model_id: String,
) -> Result<(), String> {
    manager.start(app, model_id)
}

#[tauri::command]
pub fn pause_model_download(
    manager: State<'_, DownloadManager>,
    model_id: String,
) -> Result<(), String> {
    manager.pause(&model_id)
}

#[tauri::command]
pub fn resume_model_download(
    app: AppHandle,
    manager: State<'_, DownloadManager>,
    model_id: String,
) -> Result<(), String> {
    manager.resume(app, model_id)
}

#[tauri::command]
pub fn cancel_model_download(
    manager: State<'_, DownloadManager>,
    model_id: String,
) -> Result<(), String> {
    manager.cancel(&model_id)
}

#[tauri::command]
pub fn get_download_progress(
    manager: State<'_, DownloadManager>,
) -> Option<DownloadProgress> {
    manager.last_progress()
}

#[tauri::command]
pub fn delete_local_model(app: AppHandle, model_id: String) -> Result<(), String> {
    let model = find_catalog_model(&model_id)
        .ok_or_else(|| format!("Unknown model id: {model_id}"))?;
    let root = models_dir(&app)?;
    let final_path = model_file_path(&root, &model);
    let partial = partial_file_path(&root, &model);
    if final_path.is_file() {
        std::fs::remove_file(&final_path).map_err(|e| e.to_string())?;
    }
    if partial.is_file() {
        std::fs::remove_file(&partial).map_err(|e| e.to_string())?;
    }
    Ok(())
}
