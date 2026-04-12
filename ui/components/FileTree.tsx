import { ChevronDown, ChevronRight, File, Folder, FolderOpen, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useI18n } from "@/lib/i18n";
import { type FileEntry, getProjectRoot, listDir, tryInvoke } from "@/lib/tauri";
import { cn } from "@/lib/utils";

interface TreeNodeState {
  entry: FileEntry;
  children?: TreeNodeState[];
  expanded: boolean;
  loading: boolean;
}

function FileTreeNode({
  node,
  depth,
  onToggle,
  onOpenFile,
}: {
  node: TreeNodeState;
  depth: number;
  onToggle: (path: string) => void;
  onOpenFile: (path: string) => void;
}) {
  const isDir = node.entry.is_dir;
  const paddingLeft = 8 + depth * 16;

  return (
    <>
      <button
        type="button"
        className={cn(
          "flex w-full items-center gap-1 py-0.5 text-left text-xs hover:bg-accent/50",
          isDir ? "text-foreground" : "text-muted-foreground",
        )}
        style={{ paddingLeft }}
        onClick={() => (isDir ? onToggle(node.entry.path) : onOpenFile(node.entry.path))}
      >
        {isDir ? (
          node.loading ? (
            <RefreshCw className="h-3 w-3 shrink-0 animate-spin text-muted-foreground" />
          ) : node.expanded ? (
            <ChevronDown className="h-3 w-3 shrink-0 text-muted-foreground" />
          ) : (
            <ChevronRight className="h-3 w-3 shrink-0 text-muted-foreground" />
          )
        ) : (
          <span className="w-3 shrink-0" />
        )}
        {isDir ? (
          node.expanded ? (
            <FolderOpen className="h-3.5 w-3.5 shrink-0 text-ai" />
          ) : (
            <Folder className="h-3.5 w-3.5 shrink-0 text-ai" />
          )
        ) : (
          <File className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
        )}
        <span className="truncate">{node.entry.name}</span>
      </button>
      {isDir && node.expanded && node.children && (
        <div>
          {node.children.map((child) => (
            <FileTreeNode
              key={child.entry.path}
              node={child}
              depth={depth + 1}
              onToggle={onToggle}
              onOpenFile={onOpenFile}
            />
          ))}
        </div>
      )}
    </>
  );
}

const HIDDEN_NAMES = new Set([".git", "node_modules", "target", "dist", ".DS_Store", "Thumbs.db"]);

interface FileTreeProps {
  onOpenFile: (path: string) => void;
}

export function FileTree({ onOpenFile }: FileTreeProps) {
  const { t } = useI18n();
  const [root, setRoot] = useState<string | null>(null);
  const [nodes, setNodes] = useState<TreeNodeState[]>([]);
  const [loading, setLoading] = useState(true);

  const loadChildren = useCallback(async (path: string): Promise<TreeNodeState[]> => {
    const entries = await tryInvoke(() => listDir(path), []);
    return entries
      .filter((e) => !HIDDEN_NAMES.has(e.name) && !e.name.startsWith("."))
      .map((entry) => ({
        entry,
        expanded: false,
        loading: false,
      }));
  }, []);

  useEffect(() => {
    (async () => {
      const projectRoot = await tryInvoke(getProjectRoot, null as unknown as string);
      if (!projectRoot) {
        setLoading(false);
        return;
      }
      setRoot(projectRoot);
      const children = await loadChildren(projectRoot);
      setNodes(children);
      setLoading(false);
    })();
  }, [loadChildren]);

  const handleToggle = useCallback(
    async (path: string) => {
      const toggleInTree = async (items: TreeNodeState[]): Promise<TreeNodeState[]> => {
        const result: TreeNodeState[] = [];
        for (const node of items) {
          if (node.entry.path === path) {
            if (node.expanded) {
              result.push({ ...node, expanded: false });
            } else {
              result.push({ ...node, loading: true, expanded: true });
              const idx = result.length - 1;
              const children = await loadChildren(path);
              result[idx] = { ...result[idx], children, loading: false };
            }
          } else if (node.entry.is_dir && node.expanded && node.children) {
            result.push({
              ...node,
              children: await toggleInTree(node.children),
            });
          } else {
            result.push(node);
          }
        }
        return result;
      };
      setNodes(await toggleInTree(nodes));
    },
    [nodes, loadChildren],
  );

  const handleRefresh = useCallback(async () => {
    if (!root) return;
    setLoading(true);
    const children = await loadChildren(root);
    setNodes(children);
    setLoading(false);
  }, [root, loadChildren]);

  if (loading) {
    return (
      <div className="flex items-center gap-2 p-2 text-xs text-muted-foreground">
        <RefreshCw className="h-3 w-3 animate-spin" />
        {t.fileTree.loading}
      </div>
    );
  }

  if (!root || nodes.length === 0) {
    return <p className="p-2 text-xs text-muted-foreground">{t.fileTree.noFiles}</p>;
  }

  const rootName = root.split("/").pop() || root;

  return (
    <div className="flex flex-col">
      <div className="flex items-center justify-between px-2 pb-1">
        <span className="truncate text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
          {rootName}
        </span>
        <button
          type="button"
          onClick={handleRefresh}
          className="rounded p-0.5 text-muted-foreground hover:bg-accent/50 hover:text-foreground"
        >
          <RefreshCw className="h-3 w-3" />
        </button>
      </div>
      <div className="overflow-y-auto">
        {nodes.map((node) => (
          <FileTreeNode
            key={node.entry.path}
            node={node}
            depth={0}
            onToggle={handleToggle}
            onOpenFile={onOpenFile}
          />
        ))}
      </div>
    </div>
  );
}
