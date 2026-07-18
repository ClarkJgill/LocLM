import { useEffect, useState } from "react";
import { getLiveMetrics } from "../lib/api";
import type { HardwareInfo, LiveMetrics } from "../types/hardware";
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
      return "SRV LOAD";
    case "unhealthy":
      return "SRV WEAK";
    case "error":
      return "SRV FAIL";
    default:
      return "SRV OFF";
  }
}

export function StatusStrip({ hw, hwReady, server }: StatusStripProps) {
  const [live, setLive] = useState<LiveMetrics | null>(null);
  const polling =
    server?.phase === "ready" ||
    server?.phase === "starting" ||
    server?.phase === "unhealthy";

  useEffect(() => {
    if (!polling) {
      setLive(null);
      return;
    }
    let cancelled = false;
    const tick = () => {
      getLiveMetrics()
        .then((m) => {
          if (!cancelled) setLive(m);
        })
        .catch(() => undefined);
    };
    tick();
    const id = window.setInterval(tick, 2000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [polling]);

  const ramUsedMb = live?.ramUsedMb ?? (hw ? hw.totalMemoryMb - hw.availableMemoryMb : 0);
  const ramTotalMb = live?.ramTotalMb ?? hw?.totalMemoryMb ?? 0;
  const ramUsedPct =
    ramTotalMb > 0 ? Math.min(100, Math.round((ramUsedMb / ramTotalMb) * 100)) : 0;

  return (
    <footer className="flex h-8 shrink-0 items-center gap-4 overflow-x-auto border-t border-border bg-surface px-4 font-mono text-[11px] tracking-wide text-text-muted">
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
              {formatGb(ramUsedMb)}/{formatGb(ramTotalMb)} GB
            </span>{" "}
            <span className="text-text-muted">({ramUsedPct}%)</span>
          </span>
          <span className="text-border">│</span>
          <span>
            CPU{" "}
            <span className="text-text-primary">
              {live
                ? `${Math.round(live.cpuUsagePct)}%`
                : `${hw.cpuCoresPhysical}C/${hw.cpuCoresLogical}T`}
            </span>
          </span>
          {live?.vramTotalMb != null && live.vramUsedMb != null ? (
            <>
              <span className="text-border">│</span>
              <span>
                VRAM{" "}
                <span className="text-text-primary">
                  {formatGb(live.vramUsedMb)}/{formatGb(live.vramTotalMb)} GB
                </span>
              </span>
            </>
          ) : (
            <>
              <span className="text-border">│</span>
              <span>
                GPU{" "}
                <span className="text-text-primary">
                  {hw.gpus[0]?.name ?? "NONE"}
                </span>
              </span>
            </>
          )}
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
