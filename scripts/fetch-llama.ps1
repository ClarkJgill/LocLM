# Downloads the bundled llama.cpp server for Windows (Vulkan build).
# Usage: powershell -ExecutionPolicy Bypass -File scripts/fetch-llama.ps1

param(
  [string]$Version = "b10064"
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Dest = Join-Path $Root "resources\windows-x86_64"
$Tmp = Join-Path $env:TEMP "loclm-llama-$Version"
$ZipUrl = "https://github.com/ggml-org/llama.cpp/releases/download/$Version/llama-$Version-bin-win-vulkan-x64.zip"
$ZipPath = Join-Path $Tmp "llama-vulkan.zip"

Write-Host "LocLM: fetching llama.cpp $Version (Windows Vulkan x64)"
New-Item -ItemType Directory -Force -Path $Tmp, $Dest | Out-Null

if (-not (Test-Path $ZipPath)) {
  Write-Host "Downloading $ZipUrl"
  Invoke-WebRequest -Uri $ZipUrl -OutFile $ZipPath
}

$Extract = Join-Path $Tmp "extract"
if (Test-Path $Extract) { Remove-Item $Extract -Recurse -Force }
Expand-Archive -Path $ZipPath -DestinationPath $Extract -Force

$Keep = @(
  "llama-server.exe",
  "llama-server-impl.dll",
  "llama.dll",
  "llama-common.dll",
  "ggml.dll",
  "ggml-base.dll",
  "ggml-vulkan.dll",
  "ggml-rpc.dll",
  "libomp140.x86_64.dll"
) + (Get-ChildItem $Extract -Filter "ggml-cpu-*.dll" | ForEach-Object { $_.Name })

foreach ($f in $Keep) {
  $src = Join-Path $Extract $f
  if (-not (Test-Path $src)) { throw "Missing expected file in zip: $f" }
  Copy-Item $src (Join-Path $Dest $f) -Force
}

@"
llama.cpp $Version
backend=vulkan
platform=windows-x86_64
source=https://github.com/ggml-org/llama.cpp/releases/tag/$Version
"@ | Set-Content (Join-Path $Dest "VERSION.txt")

$totalMb = [math]::Round((Get-ChildItem $Dest | Measure-Object Length -Sum).Sum / 1MB, 1)
Write-Host "Installed to $Dest ($totalMb MB)"
& (Join-Path $Dest "llama-server.exe") --version
