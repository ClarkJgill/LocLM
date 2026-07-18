import type { DownloadProgress, FitLabel, ModelLibraryEntry } from "../types/models";
import { HardwareGauge } from "./HardwareGauge";

function formatBytes(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(0)} MB`;
  return `${bytes} B`;
}

function formatSpeed(bps: number): string {
  if (bps <= 0) return "";
  if (bps >= 1024 ** 2) return `${(bps / 1024 ** 2).toFixed(1)} MB/s`;
  return `${(bps / 1024).toFixed(0)} KB/s`;
}

function fitLabelText(fit: FitLabel): string {
  switch (fit) {
    case "runsWell":
      return "runs well";
    case "mightBeSlow":
      return "might be slow";
    case "notRecommended":
      return "not recommended";
  }
}

interface ModelLibraryProps {
  entries: ModelLibraryEntry[];
  progress: DownloadProgress | null;
  modelsDir: string | null;
  busyId: string | null;
  activeModelId: string | null;
  loadingModelId: string | null;
  recommendedId: string | null;
  onDownload: (id: string) => void;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
  onCancel: (id: string) => void;
  onDelete: (id: string) => void;
  onRun: (id: string) => void;
  onUnload: () => void;
}

export function ModelLibrary({
  entries,
  progress,
  modelsDir,
  busyId,
  activeModelId,
  loadingModelId,
  recommendedId,
  onDownload,
  onPause,
  onResume,
  onCancel,
  onDelete,
  onRun,
  onUnload,
}: ModelLibraryProps) {
  return (
    <aside className="flex w-72 shrink-0 flex-col border-r border-border bg-surface">
      <div className="border-b border-border px-3 py-2">
        <span className="font-mono text-[10px] tracking-wider text-text-muted uppercase">
          Model library
        </span>
        {modelsDir ? (
          <p
            className="mt-1 truncate font-mono text-[9px] text-text-muted"
            title={modelsDir}
          >
            {modelsDir}
          </p>
        ) : null}
      </div>

      <ul className="flex flex-1 flex-col gap-1 overflow-y-auto p-2">
        {entries.map((entry) => {
          const { model, fit, fitRatio, fitDetail, downloaded, partialBytes } =
            entry;
          const isActive = progress?.modelId === model.id;
          const phase = isActive ? progress.phase : null;
          const downloading = phase === "downloading" || phase === "verifying";
          const paused = phase === "paused";
          const pct = isActive
            ? progress.percent
            : partialBytes > 0
              ? (partialBytes / model.sizeBytes) * 100
              : downloaded
                ? 100
                : 0;
          const warn = fit !== "runsWell";
          const isRunning = activeModelId === model.id;
          const isLoading = loadingModelId === model.id;

          return (
            <li
              key={model.id}
              className={`rounded border bg-bg px-3 py-2.5 ${
                isRunning ? "border-signal/50" : "border-border"
              }`}
            >
              <div className="mb-1 flex items-baseline justify-between gap-2">
                <span className="flex items-center gap-1.5 text-[13px] font-medium leading-tight">
                  {isRunning && (
                    <span
                      className="ready-dot inline-block h-1.5 w-1.5 rounded-full bg-signal"
                      aria-hidden
                    />
                  )}
                  {model.name}
                  {recommendedId === model.id ? (
                    <span className="font-mono text-[9px] tracking-wider text-signal uppercase">
                      rec
                    </span>
                  ) : null}
                </span>
                <span className="font-mono text-[10px] text-text-muted">
                  {formatBytes(model.sizeBytes)}
                </span>
              </div>

              <p className="mb-2 font-mono text-[10px] text-text-muted">
                {model.paramsB}B · {model.quantization}
                {downloaded ? (
                  <span className="text-signal"> · on disk</span>
                ) : null}
              </p>

              <HardwareGauge
                value={fitRatio}
                warn={warn}
                label={fitLabelText(fit)}
              />

              <p className="mt-1.5 text-[11px] leading-snug text-text-muted">
                {fitDetail}
              </p>

              {(downloading || paused || (partialBytes > 0 && !downloaded)) && (
                <div className="mt-2">
                  <div className="h-1 overflow-hidden rounded-[1px] bg-border">
                    <div
                      className={`h-full transition-[width] duration-300 ease-out ${
                        warn ? "bg-signal-warn" : "bg-signal"
                      }`}
                      style={{ width: `${Math.min(100, pct)}%` }}
                    />
                  </div>
                  <div className="mt-1 flex justify-between font-mono text-[9px] text-text-muted">
                    <span>
                      {isActive ? progress.message : "Partial download"}
                    </span>
                    <span>
                      {pct.toFixed(0)}%
                      {isActive && progress.bytesPerSec > 0
                        ? ` · ${formatSpeed(progress.bytesPerSec)}`
                        : ""}
                    </span>
                  </div>
                </div>
              )}

              <div className="mt-2 flex flex-wrap gap-1.5">
                {downloaded && (
                  <>
                    <button
                      type="button"
                      disabled={isLoading || (loadingModelId !== null && !isRunning)}
                      onClick={() => onRun(model.id)}
                      className="border border-signal/50 px-2 py-1 font-mono text-[10px] tracking-wider text-signal uppercase disabled:opacity-40"
                    >
                      {isLoading ? "Loading…" : isRunning ? "Reload" : "Run"}
                    </button>
                  </>
                )}
                {isRunning && (
                  <button
                    type="button"
                    onClick={onUnload}
                    className="border border-border px-2 py-1 font-mono text-[10px] tracking-wider text-text-primary uppercase"
                  >
                    Stop
                  </button>
                )}
                {!downloaded && !downloading && !paused && (                  <button
                    type="button"
                    disabled={busyId !== null && busyId !== model.id}
                    onClick={() =>
                      partialBytes > 0
                        ? onResume(model.id)
                        : onDownload(model.id)
                    }
                    className="border border-signal/50 px-2 py-1 font-mono text-[10px] tracking-wider text-signal uppercase disabled:opacity-40"
                  >
                    {partialBytes > 0 ? "Resume" : "Download"}
                  </button>
                )}
                {downloading && (
                  <button
                    type="button"
                    onClick={() => onPause(model.id)}
                    className="border border-border px-2 py-1 font-mono text-[10px] tracking-wider text-text-primary uppercase"
                  >
                    Pause
                  </button>
                )}
                {paused && (
                  <button
                    type="button"
                    onClick={() => onResume(model.id)}
                    className="border border-signal/50 px-2 py-1 font-mono text-[10px] tracking-wider text-signal uppercase"
                  >
                    Resume
                  </button>
                )}
                {(downloading || paused) && (
                  <button
                    type="button"
                    onClick={() => onCancel(model.id)}
                    className="border border-border px-2 py-1 font-mono text-[10px] tracking-wider text-text-muted uppercase"
                  >
                    Cancel
                  </button>
                )}
                {downloaded && !isRunning && (
                  <button
                    type="button"
                    onClick={() => onDelete(model.id)}
                    className="border border-border px-2 py-1 font-mono text-[10px] tracking-wider text-text-muted uppercase"
                  >
                    Delete
                  </button>
                )}
              </div>
            </li>
          );
        })}
      </ul>
    </aside>
  );
}
