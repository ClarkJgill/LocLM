//! Hardware detection for LocLM.
//!
//! Detects CPU, RAM, and GPU so the UI can recommend model sizes
//! and fill in sane llama.cpp defaults. GPU probing is OS-specific
//! with stubs ready for macOS/Linux.

use serde::Serialize;
use sysinfo::System;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub vendor: GpuVendor,
    pub vram_mb: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Apple,
    Intel,
    Other,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    pub cpu_brand: String,
    pub cpu_cores_logical: usize,
    pub cpu_cores_physical: usize,
    pub total_memory_mb: u64,
    pub available_memory_mb: u64,
    pub gpus: Vec<GpuInfo>,
    pub primary_backend: InferenceBackend,
    pub recommended: RecommendedSettings,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InferenceBackend {
    Cuda,
    Vulkan,
    Metal,
    Cpu,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedSettings {
    /// Suggested max model size in billions of parameters (approx).
    pub max_model_params_b: f32,
    /// Suggested GGUF quantization label, e.g. "Q4_K_M".
    pub quantization: String,
    pub context_length: u32,
    pub gpu_layers: u32,
    pub thread_count: usize,
    pub summary: String,
}

pub fn detect() -> HardwareInfo {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();

    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown CPU".into());

    let cpu_cores_logical = sys.cpus().len().max(1);
    let cpu_cores_physical = System::physical_core_count().unwrap_or(cpu_cores_logical);

    let total_memory_mb = sys.total_memory() / (1024 * 1024);
    let available_memory_mb = sys.available_memory() / (1024 * 1024);

    let gpus = detect_gpus();
    let primary_backend = pick_backend(&gpus);
    let recommended = recommend(
        total_memory_mb,
        available_memory_mb,
        cpu_cores_physical,
        &gpus,
        &primary_backend,
    );

    HardwareInfo {
        cpu_brand,
        cpu_cores_logical,
        cpu_cores_physical,
        total_memory_mb,
        available_memory_mb,
        gpus,
        primary_backend,
        recommended,
    }
}

fn pick_backend(gpus: &[GpuInfo]) -> InferenceBackend {
    if gpus.iter().any(|g| g.vendor == GpuVendor::Nvidia) {
        return InferenceBackend::Cuda;
    }
    if gpus.iter().any(|g| g.vendor == GpuVendor::Apple) {
        return InferenceBackend::Metal;
    }
    if gpus
        .iter()
        .any(|g| matches!(g.vendor, GpuVendor::Amd | GpuVendor::Intel | GpuVendor::Other))
    {
        return InferenceBackend::Vulkan;
    }
    InferenceBackend::Cpu
}

fn recommend(
    total_mb: u64,
    available_mb: u64,
    physical_cores: usize,
    gpus: &[GpuInfo],
    backend: &InferenceBackend,
) -> RecommendedSettings {
    // Leave ~3 GB for OS + LocLM itself when estimating model fit.
    let usable_ram_mb = total_mb.saturating_sub(3072).max(available_mb.min(total_mb));
    let vram_mb = gpus
        .iter()
        .filter_map(|g| g.vram_mb)
        .max()
        .unwrap_or(0);

    // Rough rule: ~0.6 GB RAM per billion params at Q4_K_M, plus KV cache headroom.
    let (max_params_b, quantization) = if usable_ram_mb >= 48_000 {
        (70.0, "Q4_K_M")
    } else if usable_ram_mb >= 24_000 {
        (32.0, "Q4_K_M")
    } else if usable_ram_mb >= 14_000 {
        (13.0, "Q4_K_M")
    } else if usable_ram_mb >= 8_000 {
        (7.0, "Q4_K_M")
    } else if usable_ram_mb >= 5_000 {
        (3.0, "Q4_K_M")
    } else {
        (1.5, "Q4_K_S")
    };

    let gpu_layers = match backend {
        InferenceBackend::Cpu => 0,
        InferenceBackend::Cuda | InferenceBackend::Metal | InferenceBackend::Vulkan => {
            if vram_mb >= 16_000 {
                99
            } else if vram_mb >= 8_000 {
                40
            } else if vram_mb >= 4_000 {
                24
            } else if vram_mb > 0 {
                12
            } else {
                // Discrete GPU reported but VRAM unknown — offload moderately.
                20
            }
        }
    };

    let context_length = if usable_ram_mb >= 16_000 { 8192 } else { 4096 };
    let thread_count = physical_cores.max(1);

    let gpu_label = gpus
        .first()
        .map(|g| g.name.as_str())
        .unwrap_or("CPU only");

    let summary = format!(
        "{:.0}B @ {} · {} threads · {} GPU layers · {}",
        max_params_b,
        quantization,
        thread_count,
        gpu_layers,
        match backend {
            InferenceBackend::Cuda => format!("CUDA ({gpu_label})"),
            InferenceBackend::Vulkan => format!("Vulkan ({gpu_label})"),
            InferenceBackend::Metal => format!("Metal ({gpu_label})"),
            InferenceBackend::Cpu => "CPU".into(),
        }
    );

    RecommendedSettings {
        max_model_params_b: max_params_b,
        quantization: quantization.into(),
        context_length,
        gpu_layers,
        thread_count,
        summary,
    }
}

fn detect_gpus() -> Vec<GpuInfo> {
    #[cfg(target_os = "windows")]
    {
        detect_gpus_windows()
    }
    #[cfg(target_os = "macos")]
    {
        detect_gpus_macos()
    }
    #[cfg(target_os = "linux")]
    {
        detect_gpus_linux()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "windows")]
fn detect_gpus_windows() -> Vec<GpuInfo> {
    // Prefer CIM over legacy WMIC (removed on newer Windows builds).
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_VideoController | Select-Object Name, AdapterRAM, AdapterCompatibility | ConvertTo-Json -Compress",
        ])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Vec::new();
    }

    #[derive(serde::Deserialize)]
    struct WinGpu {
        #[serde(rename = "Name")]
        name: Option<String>,
        #[serde(rename = "AdapterRAM")]
        adapter_ram: Option<u64>,
        #[serde(rename = "AdapterCompatibility")]
        adapter_compatibility: Option<String>,
    }

    let parsed: Result<Vec<WinGpu>, _> = serde_json::from_str(trimmed);
    let gpus: Vec<WinGpu> = match parsed {
        Ok(list) => list,
        Err(_) => serde_json::from_str::<WinGpu>(trimmed)
            .map(|g| vec![g])
            .unwrap_or_default(),
    };

    let nvidia_vram = query_nvidia_smi_vram_mb();

    let mut gpus: Vec<GpuInfo> = gpus
        .into_iter()
        .filter_map(|g| {
            let name = g.name?.trim().to_string();
            if name.is_empty() || is_virtual_display(&name) {
                return None;
            }
            let vendor = classify_vendor(&name, g.adapter_compatibility.as_deref());
            // Win32_VideoController.AdapterRAM is a 32-bit field — cards with >4 GB VRAM
            // often report ~4095 MB. Prefer nvidia-smi for NVIDIA; otherwise treat the
            // overflow sentinel as unknown.
            let mut vram_mb = g.adapter_ram.and_then(|bytes| {
                if bytes == 0 || bytes >= u32::MAX as u64 {
                    None
                } else {
                    let mb = bytes / (1024 * 1024);
                    if mb >= 4095 {
                        None
                    } else {
                        Some(mb)
                    }
                }
            });
            if vendor == GpuVendor::Nvidia {
                if let Some(nv) = nvidia_vram {
                    vram_mb = Some(nv);
                }
            }
            Some(GpuInfo {
                name,
                vendor,
                vram_mb,
            })
        })
        .collect();

    // Discrete accelerators first so the UI/status strip show the useful GPU.
    gpus.sort_by_key(|g| match g.vendor {
        GpuVendor::Nvidia => 0,
        GpuVendor::Amd => 1,
        GpuVendor::Apple => 2,
        GpuVendor::Intel => 3,
        GpuVendor::Other => 4,
    });

    gpus
}

#[cfg(target_os = "windows")]
fn query_nvidia_smi_vram_mb() -> Option<u64> {
    let output = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u64>().ok())
        .max()
}

fn is_virtual_display(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("basic display")
        || n.contains("virtual")
        || n.contains("remote desktop")
        || n.contains("parsec")
        || n.contains("meta virtual")
        || n.contains("microsoft remote")
        || n.contains("indirect display")
}

#[cfg(target_os = "macos")]
fn detect_gpus_macos() -> Vec<GpuInfo> {
    // Apple Silicon / discrete GPU via system_profiler — filled in when we ship macOS builds.
    let output = std::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-json"])
        .output();

    let Ok(output) = output else {
        return vec![GpuInfo {
            name: "Apple GPU".into(),
            vendor: GpuVendor::Apple,
            vram_mb: None,
        }];
    };

    let text = String::from_utf8_lossy(&output.stdout);
    if text.to_lowercase().contains("apple") {
        vec![GpuInfo {
            name: "Apple Silicon GPU".into(),
            vendor: GpuVendor::Apple,
            vram_mb: None, // unified memory — sized via system RAM
        }]
    } else {
        vec![GpuInfo {
            name: "macOS GPU".into(),
            vendor: GpuVendor::Other,
            vram_mb: None,
        }]
    }
}

#[cfg(target_os = "linux")]
fn detect_gpus_linux() -> Vec<GpuInfo> {
    // Prefer lspci when available; otherwise empty (CPU-only path).
    let output = std::process::Command::new("lspci")
        .args(["-nn"])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .filter(|line| {
            let l = line.to_lowercase();
            l.contains("vga") || l.contains("3d") || l.contains("display")
        })
        .map(|line| {
            let name = line.split(": ").nth(1).unwrap_or(line).trim().to_string();
            let vendor = classify_vendor(&name, None);
            GpuInfo {
                name,
                vendor,
                vram_mb: None,
            }
        })
        .collect()
}

fn classify_vendor(name: &str, compatibility: Option<&str>) -> GpuVendor {
    let hay = format!(
        "{} {}",
        name.to_lowercase(),
        compatibility.unwrap_or("").to_lowercase()
    );
    if hay.contains("nvidia") || hay.contains("geforce") || hay.contains("quadro") || hay.contains("rtx") || hay.contains("gtx")
    {
        GpuVendor::Nvidia
    } else if hay.contains("amd") || hay.contains("radeon") || hay.contains("advanced micro devices")
    {
        GpuVendor::Amd
    } else if hay.contains("apple") {
        GpuVendor::Apple
    } else if hay.contains("intel") {
        GpuVendor::Intel
    } else {
        GpuVendor::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_nvidia() {
        assert_eq!(
            classify_vendor("NVIDIA GeForce RTX 3060", Some("NVIDIA")),
            GpuVendor::Nvidia
        );
    }

    #[test]
    fn recommend_small_machine() {
        let rec = recommend(8192, 4096, 4, &[], &InferenceBackend::Cpu);
        assert!(rec.max_model_params_b <= 7.0);
        assert_eq!(rec.gpu_layers, 0);
        assert_eq!(rec.thread_count, 4);
    }

    #[test]
    fn detect_returns_real_machine_info() {
        let hw = detect();
        assert!(hw.cpu_cores_logical >= 1);
        assert!(hw.total_memory_mb >= 1024);
        assert!(!hw.cpu_brand.is_empty());
        assert!(!hw.recommended.summary.is_empty());
        eprintln!(
            "detected: {} | {}C/{}T | {} MB RAM | {:?} | {}",
            hw.cpu_brand,
            hw.cpu_cores_physical,
            hw.cpu_cores_logical,
            hw.total_memory_mb,
            hw.primary_backend,
            hw.recommended.summary
        );
        for gpu in &hw.gpus {
            eprintln!("  gpu: {} ({:?}, {:?} MB)", gpu.name, gpu.vendor, gpu.vram_mb);
        }
    }
}
