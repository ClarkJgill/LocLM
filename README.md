# LocLM

Run local LLMs on your own machine with zero setup friction.

Download the Windows installer, click a few times, and chat with an open model — no terminal, no Python, no hunting for GGUF files.

## Download (Windows)

Grab the latest **`LocLM-Setup.exe`** from [Releases](https://github.com/ClarkJgill/LocLM/releases).

1. Run the installer  
2. Open LocLM  
3. Use **Download & run recommended model** (or pick one from the library)  
4. Chat — returning users can **Resume last model** and reopen past conversations  

After models are downloaded, LocLM works fully offline (fonts are bundled). The only network use is Hugging Face model downloads.

### SmartScreen / code signing

The installer may be **unsigned** until an Authenticode certificate is configured. Windows SmartScreen can warn on first run — choose **More info → Run anyway**. LocLM itself does not phone home; this is a distribution-trust issue, not an app privacy issue.

To enable signing in CI, add certificate secrets and wire them into [`.github/workflows/release.yml`](.github/workflows/release.yml), then push a `v*` tag.

## Features

- First-run onboarding with one recommended model and auto-run after download  
- Hardware auto-detect (CPU / RAM / GPU) with plain-language model fit gauges  
- Curated GGUF library from Hugging Face (SmolLM, Llama, Phi, Qwen, Mistral)  
- Download with progress, pause/resume, and SHA-256 verification  
- Bundled [llama.cpp](https://github.com/ggml-org/llama.cpp) server (Vulkan) as a sidecar  
- Streaming chat, stop generation, conversation list, restore last model  
- Live status strip (RAM / CPU / VRAM while a model is running)  
- Clear load / fail messages (including sidecar stderr hints)  
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

### Release (CI)

Push a version tag (`v0.2.0`) to trigger [`.github/workflows/release.yml`](.github/workflows/release.yml), which builds the Windows NSIS installer and attaches it to the GitHub Release.

If `git push` rejects `.github/workflows`, refresh GitHub CLI scopes: `gh auth refresh -s workflow`.

## Project layout

```
loclm/
  src-tauri/     # Rust: hardware, downloads, sidecar, settings
  src/           # React frontend
  resources/     # Bundled llama.cpp binaries (fetched, not in git)
  scripts/       # fetch-llama.ps1
  .github/       # Release workflow
  README.md
```

## License

MIT (application code). Bundled llama.cpp and downloaded models have their own licenses — see upstream projects and Hugging Face model cards.

## Privacy

No telemetry. Conversations and models stay in your app-data folder (`%APPDATA%\com.loclm.desktop\` on Windows).
