import { File, Folder, RefreshCw, Search } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useI18n } from "@/lib/i18n";
import { getProjectRoot, type SearchResult, searchFiles, tryInvoke } from "@/lib/tauri";
import { cn } from "@/lib/utils";

interface SearchPanelProps {
  onOpenFile: (path: string) => void;
}

export function SearchPanel({ onOpenFile }: SearchPanelProps) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [searchContent, setSearchContent] = useState(true);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [cwd, setCwd] = useState("");
  const [searched, setSearched] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const cwdRef = useRef("");

  useEffect(() => {
    tryInvoke(getProjectRoot, "").then((root) => {
      setCwd(root);
      cwdRef.current = root;
    });
  }, []);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const doSearch = useCallback(
    async (q: string) => {
      const dir = cwdRef.current || cwd;
      if (!q.trim() || !dir) {
        setResults([]);
        setSearched(false);
        setError(null);
        return;
      }
      setLoading(true);
      setSearched(true);
      setError(null);
      try {
        const res = await searchFiles(dir, q.trim(), searchContent);
        setResults(res);
      } catch (err) {
        console.error("Search failed:", err);
        setResults([]);
        setError(String(err));
      }
      setLoading(false);
    },
    [cwd, searchContent],
  );

  const handleInput = useCallback(
    (value: string) => {
      setQuery(value);
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => doSearch(value), 300);
    },
    [doSearch],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        if (timerRef.current) clearTimeout(timerRef.current);
        doSearch(query);
      }
    },
    [query, doSearch],
  );

  return (
    <div className="flex flex-col gap-2 p-2">
      {/* Search input */}
      <div className="flex items-center gap-1 rounded border border-border bg-background px-2 py-1">
        <Search className="h-3 w-3 shrink-0 text-muted-foreground" />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => handleInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t.searchPanel.placeholder}
          className="flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
        />
        {loading && <RefreshCw className="h-3 w-3 animate-spin text-muted-foreground" />}
      </div>

      {/* Options */}
      <label className="flex items-center gap-1.5 text-[10px] text-muted-foreground cursor-pointer select-none">
        <input
          type="checkbox"
          checked={searchContent}
          onChange={(e) => setSearchContent(e.target.checked)}
          className="accent-primary h-3 w-3"
        />
        {t.searchPanel.searchContent}
      </label>

      {/* Results */}
      <div className="overflow-y-auto">
        {error && <p className="py-2 text-center text-xs text-destructive">{error}</p>}
        {!error && searched && results.length === 0 && !loading && (
          <p className="py-4 text-center text-xs text-muted-foreground">{t.common.noResults}</p>
        )}
        {results.map((r) => (
          <SearchResultItem key={r.path} result={r} cwd={cwd} onOpenFile={onOpenFile} />
        ))}
        {results.length > 0 && (
          <p className="py-1 text-center text-[10px] text-muted-foreground">
            {t.searchPanel.resultCount.replace("{count}", String(results.length))}
          </p>
        )}
      </div>
    </div>
  );
}

function SearchResultItem({
  result,
  cwd,
  onOpenFile,
}: {
  result: SearchResult;
  cwd: string;
  onOpenFile: (path: string) => void;
}) {
  const fileName = result.path.split("/").pop() || result.path;
  const fullPath = result.path.startsWith("/") ? result.path : `${cwd}/${result.path}`;

  return (
    <div className="mb-0.5">
      <button
        type="button"
        className={cn(
          "flex w-full items-center gap-1.5 rounded px-1.5 py-1 text-left text-xs hover:bg-accent/50",
          result.is_dir ? "text-foreground" : "text-foreground",
        )}
        onClick={() => !result.is_dir && onOpenFile(fullPath)}
      >
        {result.is_dir ? (
          <Folder className="h-3.5 w-3.5 shrink-0 text-ai" />
        ) : (
          <File className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
        )}
        <div className="flex flex-col min-w-0">
          <span className="truncate font-medium">{fileName}</span>
          <span className="truncate text-[10px] text-muted-foreground">{result.path}</span>
        </div>
      </button>
      {result.matches.length > 0 && (
        <div className="ml-6 border-l border-border pl-2">
          {result.matches.map((m) => (
            <button
              type="button"
              key={`${result.path}:${m.line_number}`}
              className="flex w-full items-baseline gap-1 rounded px-1 py-0.5 text-left text-[11px] hover:bg-accent/30"
              onClick={() => onOpenFile(fullPath)}
            >
              <span className="shrink-0 text-muted-foreground/60">{m.line_number}</span>
              <span className="truncate font-mono text-muted-foreground">{m.line.trim()}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
