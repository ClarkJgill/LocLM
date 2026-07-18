import type { HardwareInfo } from "../types/hardware";
import type { ServerStatus } from "../types/server";

function formatGb(mb: number): string {
  return (mb / 1024).toFixed(1);
}

interface StatusStripProps {
  hw: HardwareInfo | null;
  hwReady: boolean;
  server: ServerStatus | null;
}

function serverDotClass(phase: ServerStatus["phase"] | undefined): string {
  switch (phase) {
    case "ready":
      return "ready-dot bg-signal";
    case "starting":
      return "ready-dot bg-signal-warn";
    case "unhealthy":
    case "error":
      return "bg-signal-warn";
    default:
      return "bg-text-muted";
  }
}

function serverLabel(phase: ServerStatus["phase"] | undefined): string {
  switch (phase) {
    case "ready":
      return "SRV READY";
    case "starting":
      return "SRV START";
    case "unhealthy":
      return "SRV WEAK";
    case "error":
      return "SRV ERR";
    default:
      return "SRV OFF";
  }
}

export function StatusStrip({ hw, hwReady, server }: StatusStripProps) {
  const ramUsedPct = hw
    ? Math.min(
        100,
        Math.round(
          ((hw.totalMemoryMb - hw.availableMemoryMb) / hw.totalMemoryMb) * 100,
        ),
      )
    : 0;

  return (
    <footer className="flex h-8 shrink-0 items-center gap-4 border-t border-border bg-surface px-4 font-mono text-[11px] tracking-wide text-text-muted">
      <div className="flex items-center gap-2">
        <span
          className={`inline-block h-1.5 w-1.5 rounded-full ${
            hwReady ? "ready-dot bg-signal" : "bg-text-muted"
          }`}
          aria-hidden
        />
        <span className="text-text-primary">
          {hwReady ? "HW READY" : "SCANNING"}
        </span>
      </div>

      <span className="text-border">│</span>

      <div className="flex items-center gap-2">
        <span
          className={`inline-block h-1.5 w-1.5 rounded-full ${serverDotClass(server?.phase)}`}
          aria-hidden
        />
        <span className="text-text-primary">{serverLabel(server?.phase)}</span>
        {server?.port != null ? (
          <span className="text-text-muted">:{server.port}</span>
        ) : null}
      </div>

      <span className="text-border">│</span>

      {hw ? (
        <>
          <span>
            RAM{" "}
            <span className="text-text-primary">
              {formatGb(hw.totalMemoryMb - hw.availableMemoryMb)}/
              {formatGb(hw.totalMemoryMb)} GB
            </span>{" "}
            <span className="text-text-muted">({ramUsedPct}%)</span>
          </span>
          <span className="text-border">│</span>
          <span>
            CPU{" "}
            <span className="text-text-primary">
              {hw.cpuCoresPhysical}C/{hw.cpuCoresLogical}T
            </span>
          </span>
          <span className="text-border">│</span>
          <span>
            GPU{" "}
            <span className="text-text-primary">
              {hw.gpus[0]?.name ?? "NONE"}
            </span>
          </span>
          <span className="text-border">│</span>
          <span>
            BACKEND{" "}
            <span className="text-signal uppercase">{hw.primaryBackend}</span>
          </span>
        </>
      ) : (
        <span>detecting hardware…</span>
      )}
    </footer>
  );
}
