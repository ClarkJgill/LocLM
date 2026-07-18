//! Managed llama.cpp server sidecar.
//!
//! LocLM spawns the bundled `llama-server` binary as a child process on a
//! free localhost port and exposes start / stop / health to the UI.

use serde::Serialize;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ServerPhase {
    Stopped,
    Starting,
    Ready,
    Unhealthy,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub phase: ServerPhase,
    pub port: Option<u16>,
    pub pid: Option<u32>,
    pub base_url: Option<String>,
    pub binary_path: Option<String>,
    pub model_path: Option<String>,
    pub message: String,
}

impl Default for ServerStatus {
    fn default() -> Self {
        Self {
            phase: ServerPhase::Stopped,
            port: None,
            pid: None,
            base_url: None,
            binary_path: None,
            model_path: None,
            message: "Server idle".into(),
        }
    }
}

pub struct InferenceServer {
    inner: Mutex<ServerInner>,
}

struct ServerInner {
    child: Option<Child>,
    status: ServerStatus,
    /// Tail of sidecar stderr for failure diagnostics.
    last_stderr: String,
}

impl InferenceServer {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(ServerInner {
                child: None,
                status: ServerStatus::default(),
                last_stderr: String::new(),
            }),
        }
    }

    pub fn status(&self) -> ServerStatus {
        let mut guard = self.inner.lock().expect("server mutex");
        self.refresh_locked(&mut guard);
        guard.status.clone()
    }

    pub fn stop(&self) -> Result<ServerStatus, String> {
        let mut guard = self.inner.lock().expect("server mutex");
        Self::kill_locked(&mut guard)?;
        guard.status = ServerStatus {
            phase: ServerPhase::Stopped,
            message: "Server stopped".into(),
            ..ServerStatus::default()
        };
        Ok(guard.status.clone())
    }

    pub fn start(
        &self,
        app: &AppHandle,
        model_path: Option<String>,
        gpu_layers: Option<u32>,
        context_length: Option<u32>,
        thread_count: Option<usize>,
    ) -> Result<ServerStatus, String> {
        let mut guard = self.inner.lock().expect("server mutex");
        self.refresh_locked(&mut guard);

        // Switching models: if already running, stop first (no full app restart).
        if guard.child.is_some() {
            let same = guard.status.model_path == model_path
                && matches!(
                    guard.status.phase,
                    ServerPhase::Ready | ServerPhase::Starting | ServerPhase::Unhealthy
                );
            if same {
                return Ok(guard.status.clone());
            }
            Self::kill_locked(&mut guard)?;
            // Brief pause so the OS releases the previous port / GPU context.
            drop(guard);
            std::thread::sleep(Duration::from_millis(400));
            guard = self.inner.lock().expect("server mutex");
        }

        let binary = resolve_llama_server(app)?;
        if let Some(ref path) = model_path {
            if !Path::new(path).is_file() {
                return Err(format!("Model file not found: {path}"));
            }
        }

        let port = pick_free_port()?;
        let mut cmd = Command::new(&binary);
        cmd.current_dir(
            binary
                .parent()
                .ok_or_else(|| "llama-server has no parent directory".to_string())?,
        )
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .arg("--threads")
        .arg(
            thread_count
                .unwrap_or_else(|| {
                    std::thread::available_parallelism()
                        .map(|n| n.get())
                        .unwrap_or(4)
                })
                .to_string(),
        );

        if let Some(ctx) = context_length {
            cmd.arg("--ctx-size").arg(ctx.to_string());
        } else {
            cmd.arg("--ctx-size").arg("4096");
        }

        if let Some(layers) = gpu_layers {
            cmd.arg("--n-gpu-layers").arg(layers.to_string());
        }

        if let Some(ref path) = model_path {
            cmd.arg("--model").arg(path);
        }

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start llama-server: {e}"))?;

        let stderr_buf = Arc::new(Mutex::new(String::new()));
        if let Some(mut stderr) = child.stderr.take() {
            let buf = stderr_buf.clone();
            std::thread::spawn(move || {
                let mut chunk = [0u8; 4096];
                loop {
                    match stderr.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            if let Ok(mut g) = buf.lock() {
                                g.push_str(&String::from_utf8_lossy(&chunk[..n]));
                                const MAX: usize = 8_000;
                                if g.len() > MAX {
                                    let drain = g.len() - MAX;
                                    g.drain(..drain);
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        let pid = child.id();
        let base_url = format!("http://127.0.0.1:{port}");

        guard.status = ServerStatus {
            phase: ServerPhase::Starting,
            port: Some(port),
            pid: Some(pid),
            base_url: Some(base_url.clone()),
            binary_path: Some(binary.display().to_string()),
            model_path: model_path.clone(),
            message: if model_path.is_some() {
                "Loading model into memory…".into()
            } else {
                "Starting llama-server…".into()
            },
        };
        guard.last_stderr.clear();
        guard.child = Some(child);
        drop(guard);

        let timeout = if model_path.is_some() {
            Duration::from_secs(180)
        } else {
            Duration::from_secs(45)
        };
        let ready = wait_until_ready(&base_url, timeout);

        let stderr_tail = stderr_buf
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        let stderr_hint = summarize_stderr(&stderr_tail);

        let mut guard = self.inner.lock().expect("server mutex");
        guard.last_stderr = stderr_tail;
        self.refresh_locked(&mut guard);
        if guard.child.is_none() {
            guard.status.phase = ServerPhase::Error;
            guard.status.message = if stderr_hint.is_empty() {
                "Model failed to start. Check that Vulkan/GPU drivers are installed, then try Unload and retry.".into()
            } else {
                format!("Model failed to start. {stderr_hint}")
            };
            return Err(guard.status.message.clone());
        }

        match ready {
            Ok(()) => {
                guard.status.phase = ServerPhase::Ready;
                guard.status.message = if let Some(ref path) = model_path {
                    let name = Path::new(path)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(path);
                    format!("Ready · {name}")
                } else {
                    format!("Ready on {base_url} (router mode — no model loaded yet)")
                };
                Ok(guard.status.clone())
            }
            Err(msg) => {
                let extra = stderr_buf.lock().map(|g| g.clone()).unwrap_or_default();
                if !extra.is_empty() {
                    guard.last_stderr = extra;
                }
                let hint = summarize_stderr(&guard.last_stderr);
                Self::kill_locked(&mut guard)?;
                guard.status.phase = ServerPhase::Error;
                guard.status.message = if hint.is_empty() {
                    format!("{msg}. Try lowering GPU layers in Settings, or Unload and retry.")
                } else {
                    format!("{msg}. {hint}")
                };
                Err(guard.status.message.clone())
            }
        }
    }

    pub fn health_check(&self) -> Result<ServerStatus, String> {
        let mut guard = self.inner.lock().expect("server mutex");
        self.refresh_locked(&mut guard);

        let Some(ref url) = guard.status.base_url.clone() else {
            guard.status.phase = ServerPhase::Stopped;
            guard.status.message = "Server is not running".into();
            return Ok(guard.status.clone());
        };

        match http_get_status(&(url.clone() + "/health")) {
            Ok(code) if (200..300).contains(&code) => {
                guard.status.phase = ServerPhase::Ready;
                guard.status.message = format!("Healthy ({code})");
            }
            Ok(code) => {
                guard.status.phase = ServerPhase::Unhealthy;
                guard.status.message = format!("Unhealthy HTTP {code}");
            }
            Err(e) => {
                guard.status.phase = ServerPhase::Unhealthy;
                guard.status.message = format!("Health check failed: {e}");
            }
        }
        Ok(guard.status.clone())
    }

    fn refresh_locked(&self, inner: &mut ServerInner) {
        if let Some(child) = inner.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    inner.child = None;
                    inner.status.phase = ServerPhase::Error;
                    inner.status.pid = None;
                    let hint = summarize_stderr(&inner.last_stderr);
                    inner.status.message = if hint.is_empty() {
                        format!("llama-server exited ({status})")
                    } else {
                        format!("llama-server exited ({status}). {hint}")
                    };
                }
                Ok(None) => {}
                Err(e) => {
                    inner.child = None;
                    inner.status.phase = ServerPhase::Error;
                    inner.status.pid = None;
                    inner.status.message = format!("Failed to poll llama-server: {e}");
                }
            }
        }
    }

    fn kill_locked(inner: &mut ServerInner) -> Result<(), String> {
        if let Some(mut child) = inner.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }
}

impl Drop for InferenceServer {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.inner.lock() {
            let _ = Self::kill_locked(&mut guard);
        }
    }
}

fn pick_free_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Could not allocate a free port: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Could not read bound port: {e}"))?
        .port();
    // Drop listener so llama-server can bind the same port.
    drop(listener);
    Ok(port)
}

/// Resolve the bundled llama-server path for the current platform.
pub fn resolve_llama_server(app: &AppHandle) -> Result<PathBuf, String> {
    let exe_name = if cfg!(target_os = "windows") {
        "llama-server.exe"
    } else {
        "llama-server"
    };

    let platform_dir = if cfg!(target_os = "windows") {
        "windows-x86_64"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "macos-aarch64"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "macos-x86_64"
    } else if cfg!(target_os = "linux") {
        "linux-x86_64"
    } else {
        return Err("Unsupported platform for llama-server sidecar".into());
    };

    // 1) Packaged resource dir (release / installed app)
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidates = [
            resource_dir.join("resources").join(platform_dir).join(exe_name),
            resource_dir.join(platform_dir).join(exe_name),
            resource_dir.join("llama").join(exe_name),
            resource_dir.join(exe_name),
        ];
        for c in candidates {
            if c.is_file() {
                return Ok(c);
            }
        }
    }

    // 2) Dev / repo layout: <repo>/resources/<platform>/llama-server
    if let Ok(resource_dir) = app.path().resource_dir() {
        // In `tauri dev`, resource_dir often points near src-tauri; walk up for repo root.
        for ancestor in resource_dir.ancestors().take(6) {
            let candidate = ancestor
                .join("resources")
                .join(platform_dir)
                .join(exe_name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    // 3) Relative to the running executable (useful for bare `loclm.exe` next to resources)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidates = [
                exe_dir
                    .join("resources")
                    .join(platform_dir)
                    .join(exe_name),
                exe_dir
                    .join("..")
                    .join("resources")
                    .join(platform_dir)
                    .join(exe_name),
                exe_dir
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("resources")
                    .join(platform_dir)
                    .join(exe_name),
            ];
            for c in candidates {
                if let Ok(canonical) = c.canonicalize() {
                    if canonical.is_file() {
                        return Ok(canonical);
                    }
                }
            }
        }
    }

    // 4) CARGO_MANIFEST_DIR at compile time → repo resources (dev builds)
    let manifest_candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("resources")
        .join(platform_dir)
        .join(exe_name);
    if let Ok(canonical) = manifest_candidate.canonicalize() {
        if canonical.is_file() {
            return Ok(canonical);
        }
    }

    Err(format!(
        "Could not find {exe_name} for {platform_dir}. Run scripts/fetch-llama.ps1 to download it."
    ))
}

fn summarize_stderr(stderr: &str) -> String {
    let lower = stderr.to_lowercase();
    let plain = if lower.contains("out of memory")
        || lower.contains("failed to allocate")
        || lower.contains("insufficient memory")
    {
        "Ran out of GPU/system memory. Try a smaller model or lower GPU layers in Settings."
    } else if lower.contains("vulkan") && (lower.contains("error") || lower.contains("fail")) {
        "Vulkan/GPU driver issue. Update your GPU drivers, then retry."
    } else if lower.contains("cuda") && lower.contains("error") {
        "GPU backend error. Update drivers or lower GPU layers in Settings."
    } else if lower.contains("failed to load") || lower.contains("unable to load") {
        "Could not load the model file. Re-download it from the library."
    } else {
        ""
    };

    let snippet = stderr
        .lines()
        .rev()
        .find(|l| {
            let t = l.trim();
            !t.is_empty() && t.len() > 8
        })
        .unwrap_or("")
        .trim();
    let snippet = if snippet.len() > 180 {
        format!("{}…", &snippet[..177])
    } else {
        snippet.to_string()
    };

    match (plain.is_empty(), snippet.is_empty()) {
        (true, true) => String::new(),
        (false, true) => plain.to_string(),
        (true, false) => format!("Details: {snippet}"),
        (false, false) => format!("{plain} Details: {snippet}"),
    }
}

fn wait_until_ready(base_url: &str, timeout: Duration) -> Result<(), String> {
    let health = format!("{base_url}/health");
    let deadline = Instant::now() + timeout;
    let mut last_err = "not started".to_string();

    while Instant::now() < deadline {
        match http_get_json(&health) {
            Ok((code, body)) if (200..300).contains(&code) => {
                let status = body
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("ok");
                if status == "ok" {
                    return Ok(());
                }
                last_err = format!("status={status}");
            }
            Ok((code, _)) => last_err = format!("HTTP {code}"),
            Err(e) => last_err = e,
        }
        std::thread::sleep(Duration::from_millis(300));
    }

    Err(format!(
        "Timed out waiting for llama-server at {health} ({last_err})"
    ))
}

fn http_get_status(url: &str) -> Result<u16, String> {
    let response = ureq::get(url)
        .timeout(Duration::from_secs(2))
        .call()
        .map_err(|e| e.to_string())?;
    Ok(response.status())
}

fn http_get_json(url: &str) -> Result<(u16, serde_json::Value), String> {
    let response = ureq::get(url)
        .timeout(Duration::from_secs(3))
        .call()
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let text = response
        .into_string()
        .map_err(|e| format!("Could not read health body: {e}"))?;
    let body: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({}));
    Ok((status, body))
}

/// Tauri commands

#[tauri::command]
pub fn get_server_status(server: State<'_, InferenceServer>) -> ServerStatus {
    server.status()
}

#[tauri::command]
pub fn start_inference_server(
    app: AppHandle,
    server: State<'_, InferenceServer>,
    model_path: Option<String>,
    gpu_layers: Option<u32>,
    context_length: Option<u32>,
    thread_count: Option<usize>,
) -> Result<ServerStatus, String> {
    server.start(
        &app,
        model_path,
        gpu_layers,
        context_length,
        thread_count,
    )
}

#[tauri::command]
pub fn stop_inference_server(
    server: State<'_, InferenceServer>,
) -> Result<ServerStatus, String> {
    server.stop()
}

#[tauri::command]
pub fn check_inference_health(
    server: State<'_, InferenceServer>,
) -> Result<ServerStatus, String> {
    server.health_check()
}

#[tauri::command]
pub fn resolve_llama_binary(app: AppHandle) -> Result<String, String> {
    resolve_llama_server(&app).map(|p| p.display().to_string())
}
