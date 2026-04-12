import { Check, ChevronDown, ChevronRight, Cpu, GitBranch, TerminalSquare } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useClickOutside } from "@/hooks/useClickOutside";
import { useI18n } from "@/lib/i18n";
import type { GitBranchInfo, ModelGroupData } from "@/lib/tauri";
import type { InputMode } from "@/lib/types";
import { cn } from "@/lib/utils";

interface StatusBarProps {
  cwd: string;
  gitBranch?: string;
  model?: string;
  mode: InputMode;
  terminalVisible?: boolean;
  onToggleTerminal?: () => void;
  onListBranches?: () => Promise<GitBranchInfo[]>;
  onSwitchBranch?: (branch: string) => void;
  modelGroups?: ModelGroupData[];
  onSelectModel?: (model: string) => void;
}

export function StatusBar({
  cwd,
  gitBranch,
  model,
  mode,
  terminalVisible,
  onToggleTerminal,
  onListBranches,
  onSwitchBranch,
  modelGroups,
  onSelectModel,
}: StatusBarProps) {
  const { t } = useI18n();

  return (
    <div className="flex h-6 shrink-0 items-center gap-4 border-t border-border bg-card/50 px-3 text-[11px] text-muted-foreground">
      <span className="truncate font-mono">{cwd}</span>
      {gitBranch && (
        <BranchPicker
          gitBranch={gitBranch}
          onListBranches={onListBranches}
          onSwitchBranch={onSwitchBranch}
        />
      )}
      <div className="flex-1" />
      {onToggleTerminal && (
        <button
          type="button"
          onClick={onToggleTerminal}
          title={t.status.toggleTerminal}
          className={cn(
            "flex items-center gap-1 transition-colors hover:text-foreground",
            terminalVisible && "text-primary",
          )}
        >
          <TerminalSquare className="h-3 w-3" />
          <span className="hidden sm:inline">{t.status.terminal}</span>
        </button>
      )}
      <ModelPicker model={model} groups={modelGroups} onSelect={onSelectModel} />
      <span
        className={cn(
          "rounded-sm px-1.5 py-0.5 text-[10px] font-medium",
          mode === "shell" && "bg-muted text-foreground",
          mode === "ai" && "bg-ai-solid",
          mode === "agent" && "bg-agent-solid",
        )}
      >
        {mode === "shell"
          ? t.status.modeShell
          : mode === "ai"
            ? t.status.modeChat
            : t.status.modeAgent}
      </span>
    </div>
  );
}

// ────────────────────────────────────────────────────────
// Branch picker (extracted)
// ────────────────────────────────────────────────────────

function BranchPicker({
  gitBranch,
  onListBranches,
  onSwitchBranch,
}: {
  gitBranch: string;
  onListBranches?: () => Promise<GitBranchInfo[]>;
  onSwitchBranch?: (branch: string) => void;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [branches, setBranches] = useState<GitBranchInfo[]>([]);
  const ref = useRef<HTMLDivElement>(null);

  const toggle = useCallback(async () => {
    if (open) {
      setOpen(false);
      return;
    }
    if (onListBranches) {
      try {
        setBranches(await onListBranches());
      } catch {
        setBranches([]);
      }
    }
    setOpen(true);
  }, [open, onListBranches]);

  useClickOutside(ref, open, () => setOpen(false));

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        onClick={toggle}
        className="flex items-center gap-1 rounded px-1 transition-colors hover:bg-accent hover:text-foreground"
      >
        <GitBranch className="h-3 w-3" />
        {gitBranch}
        <ChevronDown className="h-2.5 w-2.5 opacity-60" />
      </button>
      {open && (
        <div className="absolute bottom-full left-0 z-50 mb-1 max-h-60 w-48 overflow-y-auto rounded-md border border-border bg-popover py-1 shadow-lg">
          {branches.length === 0 ? (
            <div className="px-2 py-1.5 text-[10px] text-muted-foreground">{t.common.loading}</div>
          ) : (
            branches.map((b) => (
              <button
                key={b.name}
                type="button"
                onClick={() => {
                  setOpen(false);
                  onSwitchBranch?.(b.name);
                }}
                className={cn(
                  "flex w-full items-center gap-2 px-2 py-1 text-left text-[11px] transition-colors hover:bg-accent hover:text-foreground",
                  b.is_current && "font-medium text-foreground",
                )}
              >
                {b.is_current ? (
                  <Check className="h-3 w-3 shrink-0 text-success" />
                ) : (
                  <span className="h-3 w-3 shrink-0" />
                )}
                <span className="truncate">{b.name}</span>
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
}

// ────────────────────────────────────────────────────────
// Model picker
// ────────────────────────────────────────────────────────

function ModelPicker({
  model,
  groups,
  onSelect,
}: {
  model?: string;
  groups?: ModelGroupData[];
  onSelect?: (model: string) => void;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [focusIdx, setFocusIdx] = useState(-1);
  const ref = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const close = useCallback(() => {
    setOpen(false);
    setCollapsed(new Set());
    setFocusIdx(-1);
  }, []);

  useClickOutside(ref, open, close);

  // Flat list of visible model IDs for keyboard navigation
  const flatItems = useMemo(() => {
    if (!groups) return [];
    const items: string[] = [];
    for (const g of groups) {
      if (collapsed.has(g.provider)) continue;
      for (const m of g.models) {
        items.push(`${g.provider}/${m}`);
      }
    }
    return items;
  }, [groups, collapsed]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!open) return;
      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          setFocusIdx((prev) => (prev + 1 < flatItems.length ? prev + 1 : 0));
          break;
        case "ArrowUp":
          e.preventDefault();
          setFocusIdx((prev) => (prev - 1 >= 0 ? prev - 1 : flatItems.length - 1));
          break;
        case "Enter": {
          e.preventDefault();
          const id = flatItems[focusIdx];
          if (id) {
            onSelect?.(id);
            close();
          }
          break;
        }
        case "Escape":
          e.preventDefault();
          close();
          break;
      }
    },
    [open, flatItems, focusIdx, onSelect, close],
  );

  // Scroll focused item into view
  useEffect(() => {
    if (focusIdx < 0 || !listRef.current) return;
    const el = listRef.current.querySelector(`[data-idx="${focusIdx}"]`);
    if (el) el.scrollIntoView({ block: "nearest" });
  }, [focusIdx]);

  const displayName = model ? (model.includes("/") ? model.split("/").pop() : model) : "";

  const hasGroups = groups && groups.length > 0;

  let itemCounter = 0;

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: keyboard nav on wrapper
    <div ref={ref} className="relative" onKeyDown={handleKeyDown}>
      <button
        type="button"
        onClick={() => {
          if (hasGroups) {
            setOpen((v) => !v);
            setFocusIdx(-1);
          }
        }}
        className={cn(
          "flex items-center gap-1 rounded px-1 transition-colors",
          hasGroups && "hover:bg-accent hover:text-foreground",
          !hasGroups && "cursor-default",
        )}
      >
        <Cpu className="h-3 w-3 text-muted-foreground" />
        {displayName ? (
          <span className="max-w-[180px] truncate">{displayName}</span>
        ) : (
          <span className="italic opacity-50">{t.status.noModel}</span>
        )}
        {hasGroups && <ChevronDown className="h-2.5 w-2.5 opacity-60" />}
      </button>

      {open && hasGroups && (
        <div className="absolute bottom-full right-0 z-50 mb-1 w-64 overflow-hidden rounded-md border border-border bg-popover shadow-lg">
          <div ref={listRef} className="max-h-72 overflow-y-auto py-1">
            {groups.map((g) => {
              const isGroupCollapsed = collapsed.has(g.provider);
              return (
                <div key={g.provider}>
                  <button
                    type="button"
                    onClick={() =>
                      setCollapsed((prev) => {
                        const next = new Set(prev);
                        if (next.has(g.provider)) next.delete(g.provider);
                        else next.add(g.provider);
                        return next;
                      })
                    }
                    className="flex w-full items-center gap-1.5 px-2 py-1.5 text-left text-[11px] font-medium text-foreground transition-colors hover:bg-accent"
                  >
                    <ChevronRight
                      className={cn(
                        "h-3 w-3 shrink-0 transition-transform",
                        !isGroupCollapsed && "rotate-90",
                      )}
                    />
                    {g.provider}
                    <span className="ml-auto text-[10px] text-muted-foreground">
                      {g.models.length}
                    </span>
                  </button>
                  {!isGroupCollapsed &&
                    g.models.map((m) => {
                      const idx = itemCounter++;
                      const fullId = `${g.provider}/${m}`;
                      const isCurrent = model === fullId || model === m;
                      const isFocused = idx === focusIdx;
                      return (
                        <button
                          key={m}
                          type="button"
                          data-idx={idx}
                          onClick={() => {
                            onSelect?.(fullId);
                            close();
                          }}
                          onMouseEnter={() => setFocusIdx(idx)}
                          className={cn(
                            "flex w-full items-center gap-2 pl-6 pr-2 py-1 text-left text-[11px] transition-colors",
                            isFocused
                              ? "bg-accent text-accent-foreground"
                              : "hover:bg-accent hover:text-foreground",
                            isCurrent && !isFocused && "font-medium text-primary",
                          )}
                        >
                          {isCurrent ? (
                            <Check className="h-3 w-3 shrink-0 text-primary" />
                          ) : (
                            <span className="h-3 w-3 shrink-0" />
                          )}
                          <span className="truncate">{m}</span>
                        </button>
                      );
                    })}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

// ────────────────────────────────────────────────────────
// Shared click-outside hook
// ────────────────────────────────────────────────────────
