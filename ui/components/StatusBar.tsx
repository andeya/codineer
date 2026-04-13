import { Check, ChevronDown, Cpu, GitBranch, TerminalSquare } from "lucide-react";
import { useCallback, useMemo, useRef, useState } from "react";
import { Select } from "@/components/ui/select";
import { useClickOutside } from "@/hooks/useClickOutside";
import { useI18n } from "@/lib/i18n";
import {
  modelGroupsToSelectOptions,
  shortModelDisplay,
  withCurrentModelOption,
} from "@/lib/model-options";
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
      <StatusBarModelSelect model={model} groups={modelGroups} onSelect={onSelectModel} />
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
// Model select (same flat list as Settings → Models)
// ────────────────────────────────────────────────────────

function StatusBarModelSelect({
  model,
  groups,
  onSelect,
}: {
  model?: string;
  groups?: ModelGroupData[];
  onSelect?: (model: string) => void;
}) {
  const { t } = useI18n();

  const options = useMemo(() => {
    const base = modelGroupsToSelectOptions(groups ?? [], true);
    return withCurrentModelOption(base, model);
  }, [groups, model]);

  const short = model?.trim() ? shortModelDisplay(model) : "";

  if (options.length === 0) {
    return (
      <div className="flex max-w-[220px] items-center gap-1 text-[11px] text-muted-foreground">
        <Cpu className="h-3 w-3 shrink-0" />
        {short ? (
          <span className="truncate text-foreground">{short}</span>
        ) : (
          <span className="italic opacity-50">{t.status.noModel}</span>
        )}
      </div>
    );
  }

  return (
    <div className="flex max-w-[240px] min-w-0 items-center gap-0.5">
      <Cpu className="h-3 w-3 shrink-0 text-muted-foreground" />
      <Select
        value={model ?? ""}
        options={options}
        onChange={(v) => onSelect?.(v)}
        placeholder={t.status.noModel}
        triggerLabel={short || null}
        dropUp
        menuAlign="end"
        menuClassName="min-w-64 max-h-72"
        triggerClassName={cn(
          "max-w-[180px] min-w-0 border-0 bg-transparent px-1 py-0.5 text-[11px] shadow-none",
          "hover:bg-accent hover:text-foreground",
          "focus:ring-1 focus:ring-ring",
        )}
        className="min-w-0 flex-1"
      />
    </div>
  );
}
