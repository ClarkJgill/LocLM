//! Persisted inference settings — auto-filled from hardware, user-overridable.

use crate::hardware::{self, HardwareInfo};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceSettings {
    pub context_length: u32,
    pub temperature: f32,
    pub gpu_layers: u32,
    pub thread_count: usize,
    pub max_tokens: u32,
    /// True once the user has saved custom values (so we don't overwrite on next launch).
    pub user_customized: bool,
}

impl InferenceSettings {
    pub fn from_hardware(hw: &HardwareInfo) -> Self {
        Self {
            context_length: hw.recommended.context_length,
            temperature: 0.7,
            gpu_layers: hw.recommended.gpu_layers,
            thread_count: hw.recommended.thread_count,
            max_tokens: 1024,
            user_customized: false,
        }
    }
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Could not resolve app data dir: {e}"))?;
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    Ok(base.join("settings.json"))
}

pub fn load_or_init(app: &AppHandle) -> Result<InferenceSettings, String> {
    let path = settings_path(app)?;
    if path.is_file() {
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let settings: InferenceSettings =
            serde_json::from_str(&text).map_err(|e| format!("Invalid settings.json: {e}"))?;
        return Ok(settings);
    }
    let hw = hardware::detect();
    let settings = InferenceSettings::from_hardware(&hw);
    save(app, &settings)?;
    Ok(settings)
}

pub fn save(app: &AppHandle, settings: &InferenceSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_inference_settings(app: AppHandle) -> Result<InferenceSettings, String> {
    load_or_init(&app)
}

#[tauri::command]
pub fn save_inference_settings(
    app: AppHandle,
    mut settings: InferenceSettings,
) -> Result<InferenceSettings, String> {
    settings.user_customized = true;
    // Clamp to sane ranges
    settings.context_length = settings.context_length.clamp(512, 131_072);
    settings.temperature = settings.temperature.clamp(0.0, 2.0);
    settings.gpu_layers = settings.gpu_layers.min(999);
    settings.thread_count = settings.thread_count.clamp(1, 256);
    settings.max_tokens = settings.max_tokens.clamp(16, 16_384);
    save(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn reset_inference_settings(app: AppHandle) -> Result<InferenceSettings, String> {
    let hw = hardware::detect();
    let settings = InferenceSettings::from_hardware(&hw);
    save(&app, &settings)?;
    Ok(settings)
}
