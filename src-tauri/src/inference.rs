//! Managed llama.cpp server sidecar.
//!
//! LocLM spawns the bundled `llama-server` binary as a child process on a
//! free localhost port and exposes start / stop / health to the UI.

use serde::Serialize;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
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
}

impl InferenceServer {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(ServerInner {
                child: None,
                status: ServerStatus::default(),
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

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start llama-server: {e}"))?;

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
                "Loading model…".into()
            } else {
                "Starting llama-server…".into()
            },
        };
        guard.child = Some(child);
        drop(guard);

        let timeout = if model_path.is_some() {
            Duration::from_secs(180)
        } else {
            Duration::from_secs(45)
        };
        let ready = wait_until_ready(&base_url, timeout);

        let mut guard = self.inner.lock().expect("server mutex");
        self.refresh_locked(&mut guard);
        if guard.child.is_none() {
            guard.status.phase = ServerPhase::Error;
            guard.status.message =
                "llama-server exited before becoming ready. Check that Vulkan/GPU drivers are installed.".into();
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
                    format!("Ready · {name} · {base_url}")
                } else {
                    format!("Ready on {base_url} (router mode — no model loaded yet)")
                };
                Ok(guard.status.clone())
            }
            Err(msg) => {
                guard.status.phase = ServerPhase::Unhealthy;
                guard.status.message = msg.clone();
                Err(msg)
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
                    inner.status.message = format!("llama-server exited ({status})");
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
