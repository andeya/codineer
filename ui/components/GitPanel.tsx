import {
  Check,
  ChevronDown,
  FileDiff,
  FileMinus,
  FilePlus,
  FileQuestion,
  GitBranch,
  RefreshCw,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { type Translations, useI18n } from "@/lib/i18n";
import {
  type GitBranchInfo,
  type GitFileStatus,
  type GitStatus,
  getProjectRoot,
  gitCheckout,
  gitListBranches,
  gitStatus,
  tryInvoke,
} from "@/lib/tauri";
import { cn } from "@/lib/utils";

function statusIcon(status: string) {
  switch (status) {
    case "M":
    case "MM":
      return <FileDiff className="h-3.5 w-3.5 shrink-0 text-warning" />;
    case "A":
      return <FilePlus className="h-3.5 w-3.5 shrink-0 text-success" />;
    case "D":
      return <FileMinus className="h-3.5 w-3.5 shrink-0 text-destructive" />;
    case "??":
      return <FileQuestion className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />;
    default:
      return <FileDiff className="h-3.5 w-3.5 shrink-0 text-warning" />;
  }
}

function statusLabel(status: string, t: Translations): string {
  switch (status) {
    case "M":
    case "MM":
      return t.git.modified;
    case "A":
      return t.git.added;
    case "D":
      return t.git.deleted;
    case "R":
      return t.git.renamed;
    case "??":
      return t.git.untracked;
    default:
      return status;
  }
}

interface GitPanelProps {
  onOpenDiff: (cwd: string, path: string) => void;
}

export function GitPanel({ onOpenDiff }: GitPanelProps) {
  const { t } = useI18n();
  const [status, setStatus] = useState<GitStatus | null>(null);
  const [cwd, setCwd] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showBranches, setShowBranches] = useState(false);
  const [branches, setBranches] = useState<GitBranchInfo[]>([]);
  const [branchLoading, setBranchLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    const root = await tryInvoke(getProjectRoot, "");
    if (!root) {
      setError(t.git.noProjectRoot);
      setLoading(false);
      return;
    }
    setCwd(root);
    try {
      const result = await gitStatus(root);
      setStatus(result);
    } catch (err) {
      setError(String(err));
    }
    setLoading(false);
  }, [t]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleToggleBranches = useCallback(async () => {
    if (showBranches) {
      setShowBranches(false);
      return;
    }
    if (!cwd) return;
    setBranchLoading(true);
    setShowBranches(true);
    const list = await tryInvoke(() => gitListBranches(cwd), []);
    setBranches(list);
    setBranchLoading(false);
  }, [showBranches, cwd]);

  const handleCheckout = useCallback(
    async (branch: string) => {
      if (!cwd) return;
      setBranchLoading(true);
      try {
        await gitCheckout(cwd, branch);
        setShowBranches(false);
        await refresh();
      } catch (err) {
        setError(String(err));
      }
      setBranchLoading(false);
    },
    [cwd, refresh],
  );

  if (loading) {
    return (
      <div className="flex items-center gap-2 p-3 text-xs text-muted-foreground">
        <RefreshCw className="h-3 w-3 animate-spin" />
        {t.git.loadingGit}
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-3">
        <p className="text-xs text-destructive">{error}</p>
        <button
          type="button"
          onClick={refresh}
          className="mt-2 text-xs text-primary hover:underline"
        >
          {t.common.retry}
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-col">
      {/* Branch header */}
      <div className="flex items-center justify-between border-b border-border px-2 py-1.5">
        <button
          type="button"
          onClick={handleToggleBranches}
          className="flex items-center gap-1.5 rounded px-1 py-0.5 text-xs text-foreground hover:bg-accent/50"
        >
          <GitBranch className="h-3.5 w-3.5 text-ai" />
          <span className="font-medium">{status?.branch ?? t.common.unknown}</span>
          <ChevronDown
            className={cn(
              "h-3 w-3 text-muted-foreground transition-transform",
              showBranches && "rotate-180",
            )}
          />
        </button>
        <button
          type="button"
          onClick={refresh}
          className="rounded p-0.5 text-muted-foreground hover:bg-accent/50 hover:text-foreground"
        >
          <RefreshCw className="h-3 w-3" />
        </button>
      </div>

      {/* Branch picker dropdown */}
      {showBranches && (
        <div className="border-b border-border bg-popover">
          {branchLoading ? (
            <div className="flex items-center gap-1 px-3 py-2 text-xs text-muted-foreground">
              <RefreshCw className="h-3 w-3 animate-spin" />
              {t.git.loadingBranches}
            </div>
          ) : (
            <div className="max-h-40 overflow-y-auto py-1">
              {branches.map((b) => (
                <button
                  key={b.name}
                  type="button"
                  disabled={b.is_current}
                  onClick={() => handleCheckout(b.name)}
                  className={cn(
                    "flex w-full items-center gap-2 px-3 py-1 text-left text-xs hover:bg-accent/50",
                    b.is_current && "font-medium text-ai",
                    !b.is_current && "text-foreground",
                  )}
                >
                  <GitBranch className="h-3 w-3 shrink-0" />
                  {b.name}
                  {b.is_current && (
                    <span className="ml-auto text-[10px] text-ai">{t.common.current}</span>
                  )}
                </button>
              ))}
              {branches.length === 0 && (
                <p className="px-3 py-2 text-xs text-muted-foreground">{t.git.noBranches}</p>
              )}
            </div>
          )}
        </div>
      )}

      {/* File list */}
      {status && status.changed_files.length > 0 ? (
        <div className="py-1">
          <div className="px-2 pb-1 text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
            {t.git.changesWithCount.replace("{count}", String(status.changed_files.length))}
          </div>
          {status.changed_files.map((file) => (
            <GitFileItem
              key={file.path}
              file={file}
              t={t}
              onClick={() => onOpenDiff(cwd, file.path)}
            />
          ))}
        </div>
      ) : (
        <div className="flex flex-col items-center gap-2 p-6 text-center">
          <Check className="h-6 w-6 text-success" />
          <p className="text-xs text-muted-foreground">{t.git.workingTreeClean}</p>
        </div>
      )}
    </div>
  );
}

function GitFileItem({
  file,
  t,
  onClick,
}: {
  file: GitFileStatus;
  t: Translations;
  onClick: () => void;
}) {
  const fileName = file.path.split("/").pop() || file.path;

  return (
    <button
      type="button"
      className="flex w-full items-center gap-1.5 px-2 py-1 text-left text-xs hover:bg-accent/50"
      onClick={onClick}
    >
      {statusIcon(file.status)}
      <span className="flex-1 truncate text-foreground">{fileName}</span>
      <span
        className={cn(
          "shrink-0 rounded px-1 text-[10px]",
          file.status === "??" && "text-muted-foreground",
          (file.status === "M" || file.status === "MM") && "text-git-modified",
          file.status === "A" && "text-git-added",
          file.status === "D" && "text-git-deleted",
        )}
      >
        {statusLabel(file.status, t)}
      </span>
    </button>
  );
}
