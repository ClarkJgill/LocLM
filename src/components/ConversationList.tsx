import type { Conversation } from "../types/chat";

interface ConversationListProps {
  conversations: Conversation[];
  activeId: string | null;
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
  onNew: () => void;
}

function formatWhen(secs: number): string {
  const d = new Date(secs * 1000);
  const now = new Date();
  const sameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();
  if (sameDay) {
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  return d.toLocaleDateString([], { month: "short", day: "numeric" });
}

export function ConversationList({
  conversations,
  activeId,
  onSelect,
  onDelete,
  onNew,
}: ConversationListProps) {
  return (
    <aside className="flex w-52 shrink-0 flex-col border-r border-border bg-surface">
      <div className="flex items-center justify-between border-b border-border px-3 py-2">
        <span className="font-mono text-[10px] tracking-wider text-text-muted uppercase">
          Chats
        </span>
        <button
          type="button"
          onClick={onNew}
          className="border border-border px-1.5 py-0.5 font-mono text-[9px] tracking-wider text-text-primary uppercase"
        >
          New
        </button>
      </div>
      <ul className="flex flex-1 flex-col gap-0.5 overflow-y-auto p-1.5">
        {conversations.length === 0 ? (
          <li className="px-2 py-3 text-[11px] leading-snug text-text-muted">
            No saved chats yet.
          </li>
        ) : (
          conversations.map((c) => {
            const active = c.id === activeId;
            return (
              <li key={c.id} className="group relative">
                <button
                  type="button"
                  onClick={() => onSelect(c.id)}
                  className={`w-full rounded border px-2 py-1.5 text-left ${
                    active
                      ? "border-signal/40 bg-bg"
                      : "border-transparent hover:border-border hover:bg-bg/60"
                  }`}
                >
                  <span className="block truncate text-[12px] text-text-primary">
                    {c.title || "New chat"}
                  </span>
                  <span className="mt-0.5 block font-mono text-[9px] text-text-muted">
                    {formatWhen(c.updatedAt)}
                  </span>
                </button>
                <button
                  type="button"
                  title="Delete chat"
                  onClick={(e) => {
                    e.stopPropagation();
                    onDelete(c.id);
                  }}
                  className="absolute top-1 right-1 hidden border border-border bg-surface px-1 font-mono text-[9px] text-text-muted uppercase group-hover:block"
                >
                  Del
                </button>
              </li>
            );
          })
        )}
      </ul>
    </aside>
  );
}
