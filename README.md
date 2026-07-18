# LocLM

Run local LLMs on your own machine with zero setup friction.

Download the Windows installer, click a few times, and chat with an open model — no terminal, no Python, no hunting for GGUF files.

## Download (Windows)

Grab the latest **`LocLM_*-x64-setup.exe`** from [Releases](../../releases).

1. Run the installer  
2. Open LocLM  
3. Download a starter model (or use one already on disk)  
4. Click **Run**, then chat  

After models are downloaded, LocLM works fully offline. The only network use is Hugging Face model downloads.

> **Note:** The installer is currently unsigned. Windows SmartScreen may warn on first run — choose *More info → Run anyway*.

## Features

- Hardware auto-detect (CPU / RAM / GPU) with plain-language model fit gauges  
- Curated GGUF library from Hugging Face (SmolLM, Llama, Phi, Qwen, Mistral)  
- Download with progress, pause/resume, and SHA-256 verification  
- Bundled [llama.cpp](https://github.com/ggml-org/llama.cpp) server (Vulkan) as a sidecar  
- Streaming chat, stop generation, local conversation history  
- Advanced settings: context, temperature, GPU layers, threads  

## Stack

- **Shell:** [Tauri 2](https://tauri.app/) (Rust + WebView)  
- **UI:** React + TypeScript + Tailwind  
- **Inference:** llama.cpp `llama-server` over localhost HTTP  

## Develop

Prerequisites: Node 20+, Rust stable, Visual Studio Build Tools (Windows), WebView2.

```bash
npm install
powershell -ExecutionPolicy Bypass -File scripts/fetch-llama.ps1
npm run tauri dev
```

### Build installer

```bash
npm run tauri build
```

NSIS installer: `src-tauri/target/release/bundle/nsis/`.

## Project layout

```
loclm/
  src-tauri/     # Rust: hardware, downloads, sidecar, settings
  src/           # React frontend
  resources/     # Bundled llama.cpp binaries (fetched, not in git)
  scripts/       # fetch-llama.ps1
  README.md
```

## License

MIT (application code). Bundled llama.cpp and downloaded models have their own licenses — see upstream projects and Hugging Face model cards.

## Privacy

No telemetry. Conversations and models stay in your app-data folder (`%APPDATA%\com.loclm.desktop\` on Windows).
