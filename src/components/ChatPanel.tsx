import { useEffect, useRef, useState } from "react";
import type { ChatMessage, Conversation } from "../types/chat";
import type { ModelLibraryEntry } from "../types/models";
import type { ServerStatus } from "../types/server";
import { streamChatCompletion } from "../lib/chatStream";

function uid(): string {
  return crypto.randomUUID();
}

function now(): number {
  return Math.floor(Date.now() / 1000);
}

interface ChatPanelProps {
  conversation: Conversation;
  server: ServerStatus | null;
  library: ModelLibraryEntry[];
  activeModelId: string | null;
  loadingModel: boolean;
  temperature: number;
  maxTokens: number;
  onConversationChange: (c: Conversation) => void;
  onPersist: (c: Conversation) => void;
  onNewChat: () => void;
}

export function ChatPanel({
  conversation,
  server,
  library,
  activeModelId,
  loadingModel,
  temperature,
  maxTokens,
  onConversationChange,
  onPersist,
  onNewChat,
}: ChatPanelProps) {
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showSystem, setShowSystem] = useState(false);
  const abortRef = useRef<AbortController | null>(null);
  const bottomRef = useRef<HTMLDivElement | null>(null);

  const ready =
    server?.phase === "ready" && !!server.baseUrl && !!server.modelPath;
  const activeName =
    library.find((e) => e.model.id === activeModelId)?.model.name ??
    (server?.modelPath
      ? server.modelPath.split(/[/\\]/).pop()
      : null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [conversation.messages, streaming]);

  async function send() {
    const text = input.trim();
    if (!text || streaming || !ready || !server?.baseUrl) return;

    setError(null);
    setInput("");

    const userMsg: ChatMessage = {
      id: uid(),
      role: "user",
      content: text,
      createdAt: now(),
    };
    const assistantMsg: ChatMessage = {
      id: uid(),
      role: "assistant",
      content: "",
      createdAt: now(),
    };

    let next: Conversation = {
      ...conversation,
      messages: [...conversation.messages, userMsg, assistantMsg],
    };
    onConversationChange(next);
    setStreaming(true);

    const apiMessages = [
      ...(next.systemPrompt.trim()
        ? [{ role: "system" as const, content: next.systemPrompt.trim() }]
        : []),
      ...next.messages
        .filter((m) => m.role === "user" || m.role === "assistant")
        .filter((m) => m.id !== assistantMsg.id)
        .map((m) => ({
          role: m.role as "user" | "assistant",
          content: m.content,
        })),
    ];

    const abort = new AbortController();
    abortRef.current = abort;

    try {
      await streamChatCompletion({
        baseUrl: server.baseUrl,
        messages: apiMessages,
        temperature,
        maxTokens,
        signal: abort.signal,
        onToken: (token) => {
          next = {
            ...next,
            messages: next.messages.map((m) =>
              m.id === assistantMsg.id
                ? { ...m, content: m.content + token }
                : m,
            ),
          };
          onConversationChange(next);
        },
      });
      onPersist(next);
    } catch (e) {
      if ((e as Error).name === "AbortError") {
        onPersist(next);
      } else {
        setError(e instanceof Error ? e.message : String(e));
        onPersist(next);
      }
    } finally {
      setStreaming(false);
      abortRef.current = null;
    }
  }

  function stop() {
    abortRef.current?.abort();
  }

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-border px-4 py-2.5">
        <div className="min-w-0">
          <h2 className="truncate font-mono text-[11px] tracking-wider text-text-muted uppercase">
            Chat
          </h2>
          <p className="truncate text-sm text-text-primary">
            {loadingModel
              ? "Loading model…"
              : activeName
                ? activeName
                : "No model running — click Run on a downloaded model"}
          </p>
        </div>
        <div className="flex shrink-0 gap-2">
          <button
            type="button"
            onClick={() => setShowSystem((v) => !v)}
            className="border border-border px-2 py-1 font-mono text-[10px] tracking-wider text-text-muted uppercase"
          >
            {showSystem ? "Hide system" : "System"}
          </button>
          <button
            type="button"
            onClick={onNewChat}
            className="border border-border px-2 py-1 font-mono text-[10px] tracking-wider text-text-primary uppercase"
          >
            New
          </button>
        </div>
      </div>

      {showSystem && (
        <div className="border-b border-border bg-surface px-4 py-3">
          <label className="font-mono text-[10px] tracking-wider text-text-muted uppercase">
            System prompt
          </label>
          <textarea
            value={conversation.systemPrompt}
            onChange={(e) =>
              onConversationChange({
                ...conversation,
                systemPrompt: e.target.value,
              })
            }
            onBlur={() => onPersist(conversation)}
            rows={3}
            placeholder="Optional instructions for the model…"
            className="mt-1 w-full resize-y border border-border bg-bg px-3 py-2 text-sm leading-relaxed text-text-primary outline-none placeholder:text-text-muted focus:border-signal/40"
          />
        </div>
      )}

      <div className="flex-1 overflow-y-auto px-4 py-4">
        {conversation.messages.length === 0 ? (
          <p className="text-sm leading-relaxed text-text-muted">
            {ready
              ? "Ask anything. Tokens stream from your local model — nothing leaves this machine."
              : "Download a starter model, then press Run. Chat appears here once the sidecar is ready."}
          </p>
        ) : (
          <ul className="mx-auto flex max-w-2xl flex-col gap-4">
            {conversation.messages.map((m) => (
              <li key={m.id} className="flex flex-col gap-1">
                <span className="font-mono text-[10px] tracking-wider text-text-muted uppercase">
                  {m.role}
                </span>
                <div
                  className={`border px-3 py-2.5 text-[15px] leading-relaxed whitespace-pre-wrap ${
                    m.role === "user"
                      ? "border-border text-text-primary"
                      : "border-border/80 text-text-primary"
                  }`}
                >
                  {m.content ||
                    (streaming && m.role === "assistant" ? (
                      <span className="font-mono text-text-muted">▍</span>
                    ) : (
                      ""
                    ))}
                </div>
              </li>
            ))}
          </ul>
        )}
        <div ref={bottomRef} />
      </div>

      {error && (
        <p className="border-t border-border px-4 py-2 font-mono text-[11px] text-signal-warn">
          {error}
        </p>
      )}

      <div className="border-t border-border bg-surface px-4 py-3">
        <div className="mx-auto flex max-w-2xl gap-2">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                void send();
              }
            }}
            rows={2}
            disabled={!ready || loadingModel}
            placeholder={
              ready ? "Message… (Enter to send)" : "Start a model to chat"
            }
            className="min-h-[52px] flex-1 resize-none border border-border bg-bg px-3 py-2 text-sm leading-relaxed text-text-primary outline-none placeholder:text-text-muted focus:border-signal/40 disabled:opacity-50"
          />
          {streaming ? (
            <button
              type="button"
              onClick={stop}
              className="self-end border border-signal-warn/50 px-3 py-2 font-mono text-[11px] tracking-wider text-signal-warn uppercase"
            >
              Stop
            </button>
          ) : (
            <button
              type="button"
              disabled={!ready || !input.trim() || loadingModel}
              onClick={() => void send()}
              className="self-end border border-signal/50 px-3 py-2 font-mono text-[11px] tracking-wider text-signal uppercase disabled:opacity-40"
            >
              Send
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
