import { FileCode, FileDiff, MessageSquare, Pin, PinOff, Plus, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { Tab, TabType } from "@/hooks/useTabs";
import { useI18n } from "@/lib/i18n";
import { cn } from "@/lib/utils";

interface TabBarProps {
  tabs: Tab[];
  activeId: string;
  onActivate: (id: string) => void;
  onClose: (id: string) => void;
  onNewChat: () => void;
  onRename: (id: string, title: string) => void;
  onTogglePin: (id: string) => void;
}

const typeIcon: Record<TabType, typeof MessageSquare> = {
  chat: MessageSquare,
  file: FileCode,
  diff: FileDiff,
};

export function TabBar({
  tabs,
  activeId,
  onActivate,
  onClose,
  onNewChat,
  onRename,
  onTogglePin,
}: TabBarProps) {
  const { t } = useI18n();
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const [contextMenu, setContextMenu] = useState<{ id: string; x: number; y: number } | null>(null);

  const startRename = useCallback(
    (id: string) => {
      const tab = tabs.find((t) => t.id === id);
      if (!tab) return;
      setEditingId(id);
      setEditValue(tab.title);
    },
    [tabs],
  );

  const commitRename = useCallback(() => {
    if (editingId && editValue.trim()) {
      onRename(editingId, editValue.trim());
    }
    setEditingId(null);
  }, [editingId, editValue, onRename]);

  useEffect(() => {
    if (editingId && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [editingId]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (contextMenu && e.key === "Escape") {
        setContextMenu(null);
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "t") {
        e.preventDefault();
        onNewChat();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "w") {
        e.preventDefault();
        const active = tabs.find((t) => t.id === activeId);
        if (active && !active.pinned) {
          onClose(activeId);
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [tabs, activeId, onClose, onNewChat, contextMenu]);

  useEffect(() => {
    if (!contextMenu) return;
    const dismiss = () => setContextMenu(null);
    window.addEventListener("click", dismiss);
    return () => window.removeEventListener("click", dismiss);
  }, [contextMenu]);

  const handleContextMenu = useCallback((e: React.MouseEvent, id: string) => {
    e.preventDefault();
    setContextMenu({ id, x: e.clientX, y: e.clientY });
  }, []);

  return (
    <div className="flex h-9 shrink-0 items-end border-b border-border bg-card/30">
      <div className="flex min-w-0 flex-1 items-end overflow-x-auto">
        {tabs.map((tab) => {
          const Icon = typeIcon[tab.type];
          const isActive = tab.id === activeId;
          const isEditing = tab.id === editingId;

          return (
            <div
              key={tab.id}
              role="tab"
              tabIndex={0}
              aria-selected={isActive}
              onClick={() => onActivate(tab.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onActivate(tab.id);
                }
              }}
              onDoubleClick={() => startRename(tab.id)}
              onContextMenu={(e) => handleContextMenu(e, tab.id)}
              className={cn(
                "hover-reveal relative flex h-8 shrink-0 cursor-pointer items-center gap-1.5 border-r border-border px-3 text-xs transition-colors",
                tab.pinned && "max-w-[100px]",
                !tab.pinned && "max-w-[180px]",
                isActive
                  ? "bg-background text-foreground"
                  : tab.unread
                    ? "text-tab-unread"
                    : "bg-card/60 text-muted-foreground hover:bg-card hover:text-foreground",
              )}
            >
              {isActive && <span className="absolute inset-x-0 top-0 h-[2px] bg-primary" />}
              {tab.unread && !isActive && (
                <span className="absolute inset-x-0 top-0 h-[2px] bg-tab-unread-line" />
              )}

              <span className="relative shrink-0">
                <Icon className="h-3.5 w-3.5" />
                {tab.unread && !isActive && (
                  <span className="absolute -top-0.5 -right-0.5 h-1.5 w-1.5 rounded-full bg-tab-unread-dot" />
                )}
              </span>

              {isEditing ? (
                <input
                  ref={inputRef}
                  value={editValue}
                  onChange={(e) => setEditValue(e.target.value)}
                  onBlur={commitRename}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") commitRename();
                    if (e.key === "Escape") setEditingId(null);
                  }}
                  className="min-w-[40px] flex-1 border-none bg-transparent text-xs outline-none"
                />
              ) : (
                <span className="min-w-0 flex-1 truncate">
                  {tab.pinned && tab.type !== "chat" ? "" : tab.title}
                </span>
              )}

              {tab.pinned ? (
                <Pin className="h-3 w-3 shrink-0 rotate-45 text-muted-foreground" />
              ) : (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onClose(tab.id);
                  }}
                  className="hover-reveal-target flex h-4 w-4 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
                >
                  <X className="h-3 w-3" />
                </button>
              )}
            </div>
          );
        })}
      </div>

      <button
        type="button"
        onClick={onNewChat}
        title={t.tab.newChat}
        className="flex h-8 w-8 shrink-0 items-center justify-center text-muted-foreground transition-colors hover:bg-card hover:text-foreground"
      >
        <Plus className="h-3.5 w-3.5" />
      </button>

      {/* Context menu */}
      {contextMenu && (
        <div
          className="fixed z-50 min-w-[140px] rounded-md border border-border bg-popover py-1 text-xs shadow-md"
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          <ContextMenuItem
            onClick={() => {
              startRename(contextMenu.id);
              setContextMenu(null);
            }}
          >
            {t.tab.rename}
          </ContextMenuItem>
          <ContextMenuItem
            onClick={() => {
              onTogglePin(contextMenu.id);
              setContextMenu(null);
            }}
          >
            {tabs.find((tab) => tab.id === contextMenu.id)?.pinned ? (
              <>
                <PinOff className="h-3 w-3" /> {t.tab.unpin}
              </>
            ) : (
              <>
                <Pin className="h-3 w-3" /> {t.tab.pin}
              </>
            )}
          </ContextMenuItem>
          {!tabs.find((tab) => tab.id === contextMenu.id)?.pinned && (
            <ContextMenuItem
              onClick={() => {
                onClose(contextMenu.id);
                setContextMenu(null);
              }}
            >
              <X className="h-3 w-3" /> {t.tab.close}
            </ContextMenuItem>
          )}
        </div>
      )}
    </div>
  );
}

function ContextMenuItem({
  onClick,
  children,
}: {
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-popover-foreground hover:bg-accent"
    >
      {children}
    </button>
  );
}
