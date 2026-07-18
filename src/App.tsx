import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChatPanel } from "./components/ChatPanel";
import { ConversationList } from "./components/ConversationList";
import { ModelLibrary } from "./components/ModelLibrary";
import { SettingsPanel } from "./components/SettingsPanel";
import { StatusStrip } from "./components/StatusStrip";
import {
  cancelModelDownload,
  deleteConversation,
  deleteLocalModel,
  getHardwareInfo,
  getInferenceSettings,
  getModelsDir,
  getServerStatus,
  listConversations,
  listModelLibrary,
  loadConversation,
  newConversation,
  pauseModelDownload,
  resetInferenceSettings,
  resolveLlamaBinary,
  resumeModelDownload,
  saveConversation,
  saveInferenceSettings,
  startInferenceServer,
  startModelDownload,
  stopInferenceServer,
} from "./lib/api";
import { pickRecommendedModel } from "./lib/recommend";
import type { Conversation } from "./types/chat";
import type { HardwareInfo } from "./types/hardware";
import type { DownloadProgress, ModelLibraryEntry } from "./types/models";
import type { ServerStatus } from "./types/server";
import type { InferenceSettings } from "./types/settings";

export default function App() {
  const [hw, setHw] = useState<HardwareInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [server, setServer] = useState<ServerStatus | null>(null);
  const [binaryPath, setBinaryPath] = useState<string | null>(null);

  const [library, setLibrary] = useState<ModelLibraryEntry[]>([]);
  const [modelsDir, setModelsDir] = useState<string | null>(null);
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [dlError, setDlError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  const [conversation, setConversation] = useState<Conversation | null>(null);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [activeModelId, setActiveModelId] = useState<string | null>(null);
  const [loadingModelId, setLoadingModelId] = useState<string | null>(null);
  const [runError, setRunError] = useState<string | null>(null);

  const [settings, setSettings] = useState<InferenceSettings | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsDraft, setSettingsDraft] = useState<InferenceSettings | null>(
    null,
  );
  const [settingsSaving, setSettingsSaving] = useState(false);

  /** After download of this id completes, auto-run it. */
  const pendingAutoRunRef = useRef<string | null>(null);
  const runModelRef = useRef<(id: string) => Promise<void>>(async () => {});

  const recommended = useMemo(
    () => pickRecommendedModel(library),
    [library],
  );

  const refreshLibrary = useCallback(async () => {
    const [entries, dir] = await Promise.all([
      listModelLibrary(),
      getModelsDir(),
    ]);
    setLibrary(entries);
    setModelsDir(dir);
  }, []);

  const refreshConversations = useCallback(async () => {
    try {
      const list = await listConversations();
      setConversations(list);
    } catch {
      // ignore
    }
  }, []);

  const persistSettings = useCallback(
    async (next: InferenceSettings) => {
      const saved = await saveInferenceSettings(next);
      setSettings(saved);
      return saved;
    },
    [],
  );

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [info, status, binary, conv, infSettings] = await Promise.all([
          getHardwareInfo(),
          getServerStatus(),
          resolveLlamaBinary().catch(() => null),
          newConversation(null),
          getInferenceSettings(),
        ]);
        if (cancelled) return;
        setHw(info);
        setServer(status);
        setBinaryPath(binary);
        setConversation(conv);
        setSettings(infSettings);
        await Promise.all([refreshLibrary(), refreshConversations()]);
        setError(null);
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : String(e));
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [refreshLibrary, refreshConversations]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<DownloadProgress>("download-progress", (event) => {
      setProgress(event.payload);
      if (
        event.payload.phase === "completed" ||
        event.payload.phase === "error" ||
        event.payload.phase === "idle"
      ) {
        setBusyId(null);
        void refreshLibrary();
      }
      if (event.payload.phase === "error") {
        setDlError(event.payload.message);
        if (pendingAutoRunRef.current === event.payload.modelId) {
          pendingAutoRunRef.current = null;
        }
      }
      if (
        event.payload.phase === "completed" &&
        pendingAutoRunRef.current === event.payload.modelId
      ) {
        const id = event.payload.modelId;
        pendingAutoRunRef.current = null;
        window.setTimeout(() => {
          void runModelRef.current(id);
        }, 150);
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [refreshLibrary]);

  useEffect(() => {
    if (!server || server.phase === "stopped" || server.phase === "error") {
      return;
    }
    const id = window.setInterval(() => {
      getServerStatus()
        .then(setServer)
        .catch(() => undefined);
    }, 2000);
    return () => window.clearInterval(id);
  }, [server?.phase]);

  useEffect(() => {
    if (!server?.modelPath) return;
    const match = library.find(
      (e) =>
        e.localPath === server.modelPath ||
        server.modelPath?.endsWith(e.model.hfFilename),
    );
    if (match) setActiveModelId(match.model.id);
  }, [server?.modelPath, library]);

  const runModelById = useCallback(
    async (id: string) => {
      if (!settings) return;
      // Prefer freshest library snapshot
      let entries = library;
      try {
        entries = await listModelLibrary();
        setLibrary(entries);
      } catch {
        // keep existing
      }
      const entry = entries.find((e) => e.model.id === id);
      if (!entry?.localPath) {
        setRunError("Model file not found on disk");
        return;
      }
      setRunError(null);
      setLoadingModelId(id);
      try {
        const status = await startInferenceServer({
          modelPath: entry.localPath,
          gpuLayers: settings.gpuLayers,
          contextLength: settings.contextLength,
          threadCount: settings.threadCount,
        });
        setServer(status);
        setActiveModelId(id);
        const nextSettings = {
          ...settings,
          lastModelId: id,
          onboardingComplete: true,
        };
        await persistSettings(nextSettings);
        if (conversation) {
          const updated = { ...conversation, modelId: id };
          setConversation(updated);
          await saveConversation(updated);
          await refreshConversations();
        }
      } catch (e) {
        setRunError(e instanceof Error ? e.message : String(e));
        const status = await getServerStatus().catch(() => null);
        if (status) setServer(status);
      } finally {
        setLoadingModelId(null);
      }
    },
    [settings, library, conversation, persistSettings, refreshConversations],
  );

  useEffect(() => {
    runModelRef.current = runModelById;
  }, [runModelById]);

  const onDownload = useCallback(async (id: string) => {
    setDlError(null);
    setBusyId(id);
    try {
      await startModelDownload(id);
    } catch (e) {
      setBusyId(null);
      setDlError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const onDownloadAndRun = useCallback(
    async (id: string) => {
      const entry = library.find((e) => e.model.id === id);
      if (entry?.downloaded) {
        await runModelById(id);
        return;
      }
      pendingAutoRunRef.current = id;
      setDlError(null);
      setBusyId(id);
      try {
        await startModelDownload(id);
      } catch (e) {
        pendingAutoRunRef.current = null;
        setBusyId(null);
        setDlError(e instanceof Error ? e.message : String(e));
      }
    },
    [library, runModelById],
  );

  const onPause = useCallback(async (id: string) => {
    setDlError(null);
    try {
      await pauseModelDownload(id);
    } catch (e) {
      setDlError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const onResume = useCallback(async (id: string) => {
    setDlError(null);
    setBusyId(id);
    try {
      await resumeModelDownload(id);
    } catch (e) {
      setBusyId(null);
      setDlError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const onCancel = useCallback(
    async (id: string) => {
      setDlError(null);
      if (pendingAutoRunRef.current === id) {
        pendingAutoRunRef.current = null;
      }
      try {
        await cancelModelDownload(id);
        setBusyId(null);
        await refreshLibrary();
      } catch (e) {
        setDlError(e instanceof Error ? e.message : String(e));
      }
    },
    [refreshLibrary],
  );

  const onDelete = useCallback(
    async (id: string) => {
      setDlError(null);
      try {
        await deleteLocalModel(id);
        await refreshLibrary();
      } catch (e) {
        setDlError(e instanceof Error ? e.message : String(e));
      }
    },
    [refreshLibrary],
  );

  const onRun = useCallback(
    (id: string) => {
      void runModelById(id);
    },
    [runModelById],
  );

  const onUnload = useCallback(async () => {
    setRunError(null);
    setLoadingModelId(null);
    try {
      const status = await stopInferenceServer();
      setServer(status);
      setActiveModelId(null);
    } catch (e) {
      setRunError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const onPersist = useCallback(
    async (c: Conversation) => {
      try {
        const saved = await saveConversation(c);
        setConversation(saved);
        await refreshConversations();
      } catch {
        // keep local state even if disk write fails
      }
    },
    [refreshConversations],
  );

  const onNewChat = useCallback(async () => {
    const conv = await newConversation(activeModelId);
    setConversation(conv);
  }, [activeModelId]);

  const onSelectChat = useCallback(async (id: string) => {
    try {
      const conv = await loadConversation(id);
      setConversation(conv);
    } catch (e) {
      setRunError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const onDeleteChat = useCallback(
    async (id: string) => {
      try {
        await deleteConversation(id);
        await refreshConversations();
        if (conversation?.id === id) {
          const conv = await newConversation(activeModelId);
          setConversation(conv);
        }
      } catch (e) {
        setRunError(e instanceof Error ? e.message : String(e));
      }
    },
    [conversation?.id, activeModelId, refreshConversations],
  );

  const dismissOnboarding = useCallback(async () => {
    if (!settings) return;
    await persistSettings({ ...settings, onboardingComplete: true });
  }, [settings, persistSettings]);

  const ackSmartscreen = useCallback(async () => {
    if (!settings) return;
    await persistSettings({ ...settings, smartscreenAcked: true });
  }, [settings, persistSettings]);

  const openSettings = () => {
    if (settings) {
      setSettingsDraft({ ...settings });
      setSettingsOpen(true);
    }
  };

  const onSaveSettings = async () => {
    if (!settingsDraft) return;
    setSettingsSaving(true);
    try {
      const saved = await persistSettings({
        ...settingsDraft,
        userCustomized: true,
      });
      setSettingsDraft(saved);
      setSettingsOpen(false);
    } catch (e) {
      setRunError(e instanceof Error ? e.message : String(e));
    } finally {
      setSettingsSaving(false);
    }
  };

  const onResetSettings = async () => {
    setSettingsSaving(true);
    try {
      const saved = await resetInferenceSettings();
      setSettings(saved);
      setSettingsDraft(saved);
    } catch (e) {
      setRunError(e instanceof Error ? e.message : String(e));
    } finally {
      setSettingsSaving(false);
    }
  };

  const lastModelEntry = settings?.lastModelId
    ? library.find((e) => e.model.id === settings.lastModelId)
    : null;
  const showResume =
    !!lastModelEntry?.downloaded &&
    activeModelId !== lastModelEntry.model.id &&
    server?.phase !== "ready" &&
    server?.phase !== "starting" &&
    loadingModelId === null;

  const showOnboarding =
    !!settings &&
    !settings.onboardingComplete &&
    !!recommended &&
    server?.phase !== "ready";

  return (
    <div className="flex h-full flex-col bg-bg text-text-primary">
      <header className="flex h-11 shrink-0 items-center justify-between border-b border-border px-4">
        <div className="flex items-center gap-2.5">
          <img
            src="/loclm-icon.png"
            alt=""
            width={22}
            height={22}
            className="rounded-[4px]"
          />
          <div className="flex items-baseline gap-3">
            <h1 className="font-mono text-sm font-semibold tracking-[0.14em] text-text-primary">
              LocLM
            </h1>
            <span className="font-mono text-[10px] tracking-wider text-text-muted uppercase">
              local inference
            </span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {binaryPath ? (
            <span
              className="hidden max-w-[180px] truncate font-mono text-[9px] text-text-muted sm:inline"
              title={binaryPath}
            >
              llama-server
            </span>
          ) : null}
          <button
            type="button"
            onClick={openSettings}
            disabled={!settings}
            className="border border-border px-2 py-1 font-mono text-[10px] tracking-wider text-text-muted uppercase disabled:opacity-40"
          >
            Settings
          </button>
          <span className="font-mono text-[10px] text-text-muted">v0.2.0</span>
        </div>
      </header>

      {settings && !settings.smartscreenAcked ? (
        <div className="flex items-start justify-between gap-3 border-b border-signal-warn/30 bg-surface px-4 py-2">
          <p className="text-[12px] leading-snug text-text-muted">
            <span className="font-mono text-signal-warn uppercase">Note · </span>
            Windows may show a SmartScreen warning because the installer is not
            code-signed yet. Choose <em>More info → Run anyway</em> — LocLM stays
            fully local.
          </p>
          <button
            type="button"
            onClick={() => void ackSmartscreen()}
            className="shrink-0 border border-border px-2 py-1 font-mono text-[10px] tracking-wider text-text-primary uppercase"
          >
            Got it
          </button>
        </div>
      ) : null}

      {(dlError || runError || error) && (
        <div className="border-b border-signal-warn/30 bg-surface px-4 py-1.5 font-mono text-[11px] text-signal-warn">
          {runError || dlError || error}
        </div>
      )}

      {showResume && lastModelEntry ? (
        <div className="flex items-center justify-between gap-3 border-b border-border bg-surface px-4 py-1.5">
          <p className="text-[12px] text-text-muted">
            Last model:{" "}
            <span className="text-text-primary">{lastModelEntry.model.name}</span>
          </p>
          <button
            type="button"
            onClick={() => onRun(lastModelEntry.model.id)}
            className="border border-signal/50 px-2 py-1 font-mono text-[10px] tracking-wider text-signal uppercase"
          >
            Resume last model
          </button>
        </div>
      ) : null}

      <div className="flex min-h-0 flex-1">
        <ModelLibrary
          entries={library}
          progress={progress}
          modelsDir={modelsDir}
          busyId={busyId}
          activeModelId={activeModelId}
          loadingModelId={loadingModelId}
          recommendedId={recommended?.model.id ?? null}
          onDownload={onDownload}
          onPause={onPause}
          onResume={onResume}
          onCancel={onCancel}
          onDelete={onDelete}
          onRun={onRun}
          onUnload={() => void onUnload()}
        />

        <ConversationList
          conversations={conversations}
          activeId={conversation?.id ?? null}
          onSelect={(id) => void onSelectChat(id)}
          onDelete={(id) => void onDeleteChat(id)}
          onNew={() => void onNewChat()}
        />

        {loading || !conversation || !settings ? (
          <main className="flex flex-1 items-center justify-center">
            <p className="font-mono text-sm text-text-muted">
              probing hardware…
            </p>
          </main>
        ) : (
          <ChatPanel
            conversation={conversation}
            server={server}
            library={library}
            activeModelId={activeModelId}
            loadingModel={loadingModelId !== null}
            temperature={settings.temperature}
            maxTokens={settings.maxTokens}
            recommended={recommended}
            showOnboarding={showOnboarding}
            onboardingBusy={
              busyId === recommended?.model.id ||
              loadingModelId === recommended?.model.id
            }
            onDownloadAndRun={(id) => void onDownloadAndRun(id)}
            onDismissOnboarding={() => void dismissOnboarding()}
            onConversationChange={setConversation}
            onPersist={onPersist}
            onNewChat={() => void onNewChat()}
          />
        )}
      </div>

      <StatusStrip
        hw={hw}
        hwReady={!loading && !!hw && !error}
        server={server}
      />

      {settingsDraft && (
        <SettingsPanel
          open={settingsOpen}
          settings={settingsDraft}
          hw={hw}
          onChange={setSettingsDraft}
          onSave={() => void onSaveSettings()}
          onReset={() => void onResetSettings()}
          onClose={() => setSettingsOpen(false)}
          saving={settingsSaving}
        />
      )}
    </div>
  );
}
