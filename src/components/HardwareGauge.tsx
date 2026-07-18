import { useEffect, useState } from "react";

interface HardwareGaugeProps {
  /** 0–1 predicted or live utilization against machine capacity */
  value: number;
  label?: string;
  warn?: boolean;
  className?: string;
}

/** Compact VU-meter sliver — predictive fit and live load share this shape. */
export function HardwareGauge({
  value,
  label,
  warn = false,
  className = "",
}: HardwareGaugeProps) {
  const [shown, setShown] = useState(0);
  const clamped = Math.max(0, Math.min(1, value));

  useEffect(() => {
    const id = requestAnimationFrame(() => setShown(clamped));
    return () => cancelAnimationFrame(id);
  }, [clamped]);

  const fill = warn ? "bg-signal-warn" : "bg-signal";

  return (
    <div className={`flex items-center gap-2 ${className}`}>
      <div
        className="h-1.5 w-16 overflow-hidden rounded-[1px] bg-border"
        role="meter"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(shown * 100)}
        aria-label={label ?? "Hardware fit"}
      >
        <div
          className={`h-full ${fill} transition-[width] duration-500 ease-out`}
          style={{ width: `${shown * 100}%` }}
        />
      </div>
      {label ? (
        <span className="font-mono text-[10px] tracking-wide text-text-muted uppercase">
          {label}
        </span>
      ) : null}
    </div>
  );
}
