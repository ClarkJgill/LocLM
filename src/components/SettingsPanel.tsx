import type { ReactNode } from "react";
import type { HardwareInfo } from "../types/hardware";
import type { InferenceSettings } from "../types/settings";

interface SettingsPanelProps {
  open: boolean;
  settings: InferenceSettings;
  hw: HardwareInfo | null;
  onChange: (s: InferenceSettings) => void;
  onSave: () => void;
  onReset: () => void;
  onClose: () => void;
  saving: boolean;
}

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="font-mono text-[10px] tracking-wider text-text-muted uppercase">
        {label}
      </span>
      {children}
      {hint ? (
        <span className="text-[11px] leading-snug text-text-muted">{hint}</span>
      ) : null}
    </label>
  );
}

const inputClass =
  "border border-border bg-bg px-3 py-1.5 font-mono text-sm text-text-primary outline-none focus:border-signal/40";

export function SettingsPanel({
  open,
  settings,
  hw,
  onChange,
  onSave,
  onReset,
  onClose,
  saving,
}: SettingsPanelProps) {
  if (!open) return null;

  const rec = hw?.recommended;

  return (
    <div className="fixed inset-0 z-50 flex justify-end bg-bg/60">
      <button
        type="button"
        className="flex-1 cursor-default"
        aria-label="Close settings"
        onClick={onClose}
      />
      <aside className="flex h-full w-full max-w-md flex-col border-l border-border bg-surface shadow-none">
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <div>
            <h2 className="font-mono text-[11px] tracking-wider text-text-muted uppercase">
              Settings
            </h2>
            <p className="mt-0.5 text-sm text-text-primary">
              Inference overrides
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="border border-border px-2 py-1 font-mono text-[10px] tracking-wider text-text-muted uppercase"
          >
            Close
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-4 py-4">
          {rec && (
            <p className="mb-4 border border-border bg-bg px-3 py-2 font-mono text-[11px] leading-relaxed text-text-muted">
              Auto defaults from this machine: {rec.summary}
            </p>
          )}

          <div className="flex flex-col gap-4">
            <Field
              label="Context length"
              hint={
                rec
                  ? `Recommended: ${rec.contextLength}`
                  : "Tokens of conversation memory"
              }
            >
              <input
                type="number"
                min={512}
                max={131072}
                step={512}
                value={settings.contextLength}
                onChange={(e) =>
                  onChange({
                    ...settings,
                    contextLength: Number(e.target.value) || 512,
                  })
                }
                className={inputClass}
              />
            </Field>

            <Field
              label="Temperature"
              hint="0 = focused, 1 = creative. Affects chat only."
            >
              <input
                type="number"
                min={0}
                max={2}
                step={0.05}
                value={settings.temperature}
                onChange={(e) =>
                  onChange({
                    ...settings,
                    temperature: Number(e.target.value) || 0,
                  })
                }
                className={inputClass}
              />
            </Field>

            <Field
              label="GPU layers"
              hint={
                rec
                  ? `Recommended: ${rec.gpuLayers} (0 = CPU only)`
                  : "Layers offloaded to GPU"
              }
            >
              <input
                type="number"
                min={0}
                max={999}
                step={1}
                value={settings.gpuLayers}
                onChange={(e) =>
                  onChange({
                    ...settings,
                    gpuLayers: Number(e.target.value) || 0,
                  })
                }
                className={inputClass}
              />
            </Field>

            <Field
              label="Thread count"
              hint={
                rec
                  ? `Recommended: ${rec.threadCount} (physical cores)`
                  : "CPU threads for llama.cpp"
              }
            >
              <input
                type="number"
                min={1}
                max={256}
                step={1}
                value={settings.threadCount}
                onChange={(e) =>
                  onChange({
                    ...settings,
                    threadCount: Number(e.target.value) || 1,
                  })
                }
                className={inputClass}
              />
            </Field>

            <Field
              label="Max tokens"
              hint="Maximum tokens generated per reply"
            >
              <input
                type="number"
                min={16}
                max={16384}
                step={16}
                value={settings.maxTokens}
                onChange={(e) =>
                  onChange({
                    ...settings,
                    maxTokens: Number(e.target.value) || 16,
                  })
                }
                className={inputClass}
              />
            </Field>
          </div>

          <p className="mt-6 text-[11px] leading-relaxed text-text-muted">
            Context, GPU layers, and threads apply the next time you press{" "}
            <span className="text-text-primary">Run</span> on a model.
            Temperature and max tokens apply to new chat messages immediately.
          </p>
        </div>

        <div className="flex gap-2 border-t border-border px-4 py-3">
          <button
            type="button"
            onClick={onReset}
            disabled={saving}
            className="border border-border px-3 py-1.5 font-mono text-[11px] tracking-wider text-text-muted uppercase disabled:opacity-40"
          >
            Reset to auto
          </button>
          <button
            type="button"
            onClick={onSave}
            disabled={saving}
            className="ml-auto border border-signal/50 px-3 py-1.5 font-mono text-[11px] tracking-wider text-signal uppercase disabled:opacity-40"
          >
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </aside>
    </div>
  );
}
