//! Curated GGUF model catalog and local library state.

use crate::hardware::{HardwareInfo, InferenceBackend};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    pub family: String,
    pub params_b: f32,
    pub quantization: String,
    pub size_bytes: u64,
    pub hf_repo: String,
    pub hf_filename: String,
    pub download_url: String,
    pub sha256: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FitLabel {
    RunsWell,
    MightBeSlow,
    NotRecommended,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelLibraryEntry {
    pub model: CatalogModel,
    pub fit: FitLabel,
    pub fit_ratio: f32,
    pub fit_detail: String,
    pub local_path: Option<String>,
    pub downloaded: bool,
    pub partial_bytes: u64,
}

pub fn curated_catalog() -> Vec<CatalogModel> {
    vec![
        CatalogModel {
            id: "smollm2-360m-q4".into(),
            name: "SmolLM2 360M".into(),
            family: "SmolLM".into(),
            params_b: 0.36,
            quantization: "Q4_K_M".into(),
            size_bytes: 270_590_880,
            hf_repo: "bartowski/SmolLM2-360M-Instruct-GGUF".into(),
            hf_filename: "SmolLM2-360M-Instruct-Q4_K_M.gguf".into(),
            download_url: "https://huggingface.co/bartowski/SmolLM2-360M-Instruct-GGUF/resolve/main/SmolLM2-360M-Instruct-Q4_K_M.gguf".into(),
            sha256: "2fa3f013dcdd7b99f9b237717fa0b12d75bbb89984cc1274be1471a465bac9c2".into(),
            description: "Tiny starter model — great for verifying LocLM on any machine.".into(),
        },
        CatalogModel {
            id: "llama-3.2-3b-q4".into(),
            name: "Llama 3.2 3B".into(),
            family: "Llama".into(),
            params_b: 3.2,
            quantization: "Q4_K_M".into(),
            size_bytes: 2_019_377_696,
            hf_repo: "bartowski/Llama-3.2-3B-Instruct-GGUF".into(),
            hf_filename: "Llama-3.2-3B-Instruct-Q4_K_M.gguf".into(),
            download_url: "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf".into(),
            sha256: "6c1a2b41161032677be168d354123594c0e6e67d2b9227c84f296ad037c728ff".into(),
            description: "Meta's compact instruct model — solid everyday chat.".into(),
        },
        CatalogModel {
            id: "phi-3.5-mini-q4".into(),
            name: "Phi-3.5 Mini".into(),
            family: "Phi".into(),
            params_b: 3.8,
            quantization: "Q4_K_M".into(),
            size_bytes: 2_393_232_672,
            hf_repo: "bartowski/Phi-3.5-mini-instruct-GGUF".into(),
            hf_filename: "Phi-3.5-mini-instruct-Q4_K_M.gguf".into(),
            download_url: "https://huggingface.co/bartowski/Phi-3.5-mini-instruct-GGUF/resolve/main/Phi-3.5-mini-instruct-Q4_K_M.gguf".into(),
            sha256: "e4165e3a71af97f1b4820da61079826d8752a2088e313af0c7d346796c38eff5".into(),
            description: "Microsoft Phi — strong reasoning for its size.".into(),
        },
        CatalogModel {
            id: "qwen2.5-3b-q4".into(),
            name: "Qwen2.5 3B".into(),
            family: "Qwen".into(),
            params_b: 3.0,
            quantization: "Q4_K_M".into(),
            size_bytes: 2_104_932_768,
            hf_repo: "Qwen/Qwen2.5-3B-Instruct-GGUF".into(),
            hf_filename: "qwen2.5-3b-instruct-q4_k_m.gguf".into(),
            download_url: "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf".into(),
            sha256: "626b4a6678b86442240e33df819e00132d3ba7dddfe1cdc4fbb18e0a9615c62d".into(),
            description: "Alibaba Qwen — multilingual and capable at 3B.".into(),
        },
        CatalogModel {
            id: "mistral-7b-q4".into(),
            name: "Mistral 7B".into(),
            family: "Mistral".into(),
            params_b: 7.0,
            quantization: "Q4_K_M".into(),
            size_bytes: 4_372_812_000,
            hf_repo: "bartowski/Mistral-7B-Instruct-v0.3-GGUF".into(),
            hf_filename: "Mistral-7B-Instruct-v0.3-Q4_K_M.gguf".into(),
            download_url: "https://huggingface.co/bartowski/Mistral-7B-Instruct-v0.3-GGUF/resolve/main/Mistral-7B-Instruct-v0.3-Q4_K_M.gguf".into(),
            sha256: "1270d22c0fbb3d092fb725d4d96c457b7b687a5f5a715abe1e818da303e562b6".into(),
            description: "Classic 7B instruct — needs more RAM/VRAM than the 3B options.".into(),
        },
    ]
}

pub fn find_catalog_model(id: &str) -> Option<CatalogModel> {
    curated_catalog().into_iter().find(|m| m.id == id)
}

pub fn models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Could not resolve app data dir: {e}"))?;
    let dir = base.join("models");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create models dir: {e}"))?;
    Ok(dir)
}

pub fn model_file_path(models_root: &Path, model: &CatalogModel) -> PathBuf {
    models_root.join(&model.hf_filename)
}

pub fn partial_file_path(models_root: &Path, model: &CatalogModel) -> PathBuf {
    models_root.join(format!("{}.partial", model.hf_filename))
}

pub fn assess_fit(model: &CatalogModel, hw: &HardwareInfo) -> (FitLabel, f32, String) {
    let max = hw.recommended.max_model_params_b.max(0.1);
    let ratio = (model.params_b / max).clamp(0.0, 1.5);
    let size_mb = model.size_bytes / (1024 * 1024);
    // Leave headroom for KV cache + OS (~1.5× weights as a rough floor).
    let needed_mb = size_mb.saturating_mul(3) / 2 + 1024;
    let usable_mb = hw.total_memory_mb.saturating_sub(3072);
    let vram_mb = hw.gpus.iter().filter_map(|g| g.vram_mb).max().unwrap_or(0);
    let has_accel = !matches!(hw.primary_backend, InferenceBackend::Cpu) && vram_mb > 0;

    let (label, detail) = if model.params_b <= max * 0.65 && needed_mb <= usable_mb {
        let detail = if has_accel {
            "Runs well on this machine".into()
        } else {
            "Runs well (CPU — expect slower tokens)".into()
        };
        (FitLabel::RunsWell, detail)
    } else if model.params_b <= max && needed_mb <= usable_mb.saturating_add(2048) {
        (
            FitLabel::MightBeSlow,
            "Might be slow or tight on memory".into(),
        )
    } else {
        (
            FitLabel::NotRecommended,
            format!(
                "Not recommended — needs ~{:.0}B class / {} MB free",
                model.params_b, needed_mb
            ),
        )
    };

    (label, ratio.min(1.0), detail)
}

pub fn build_library(app: &AppHandle, hw: &HardwareInfo) -> Result<Vec<ModelLibraryEntry>, String> {
    let root = models_dir(app)?;
    let mut entries = Vec::new();

    for model in curated_catalog() {
        let final_path = model_file_path(&root, &model);
        let partial_path = partial_file_path(&root, &model);
        let downloaded = final_path.is_file();
        let partial_bytes = if downloaded {
            0
        } else {
            std::fs::metadata(&partial_path)
                .map(|m| m.len())
                .unwrap_or(0)
        };
        let (fit, fit_ratio, fit_detail) = assess_fit(&model, hw);

        entries.push(ModelLibraryEntry {
            model,
            fit,
            fit_ratio,
            fit_detail,
            local_path: if downloaded {
                Some(final_path.display().to_string())
            } else {
                None
            },
            downloaded,
            partial_bytes,
        });
    }

    Ok(entries)
}
