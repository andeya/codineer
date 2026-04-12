import { File, FileDiff, X } from "lucide-react";
import { useEffect, useState } from "react";
import { getHighlighter, getShikiTheme, langFromPath } from "@/lib/highlighter";
import { useI18n } from "@/lib/i18n";
import type { PreviewTab } from "@/lib/types";
import { cn } from "@/lib/utils";

interface PreviewPanelProps {
  tab: PreviewTab;
  onClose: () => void;
}

function DiffLine({ line }: { line: string }) {
  let cls = "px-4 whitespace-pre font-mono text-[12px] leading-5";
  if (line.startsWith("+++") || line.startsWith("---")) {
    cls += " text-muted-foreground font-semibold";
  } else if (line.startsWith("+")) {
    cls += " bg-diff-add-line";
  } else if (line.startsWith("-")) {
    cls += " bg-diff-remove-line";
  } else if (line.startsWith("@@")) {
    cls += " bg-diff-info-line";
  } else if (line.startsWith("diff ") || line.startsWith("index ")) {
    cls += " text-muted-foreground";
  } else {
    cls += " text-muted-foreground";
  }
  return <div className={cls}>{line || " "}</div>;
}

export function PreviewPanel({ tab, onClose }: PreviewPanelProps) {
  const fileName = tab.path.split("/").pop() || tab.path;

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-card">
      {/* Header */}
      <div className="flex h-8 shrink-0 items-center justify-between border-b border-border px-3">
        <div className="flex items-center gap-2 text-xs">
          {tab.type === "diff" ? (
            <FileDiff className="h-3.5 w-3.5 text-warning" />
          ) : (
            <File className="h-3.5 w-3.5 text-muted-foreground" />
          )}
          <span className="font-medium text-foreground">{fileName}</span>
          <span className="text-muted-foreground">{tab.path}</span>
        </div>
        <button
          type="button"
          onClick={onClose}
          className="rounded p-0.5 text-muted-foreground hover:bg-accent/50 hover:text-foreground"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>

      {/* Content */}
      <div className="min-h-0 flex-1 overflow-auto">
        {tab.type === "file" ? (
          <FileContentView content={tab.content} path={tab.path} />
        ) : (
          <DiffContentView diff={tab.diff} />
        )}
      </div>
    </div>
  );
}

function FileContentView({ content, path }: { content: string; path: string }) {
  const { t } = useI18n();
  const [highlightedHtml, setHighlightedHtml] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getHighlighter()
      .then((hl) => {
        if (cancelled) return;
        const lang = langFromPath(path);
        const theme = getShikiTheme();
        const loadedLangs = hl.getLoadedLanguages();
        const effectiveLang = loadedLangs.includes(lang) ? lang : "plaintext";
        const html = hl.codeToHtml(content, { lang: effectiveLang, theme });
        setHighlightedHtml(html);
      })
      .catch(() => {
        /* fallback to plain text */
      });
    return () => {
      cancelled = true;
    };
  }, [content, path]);

  if (!content.trim()) {
    return (
      <p className="p-4 text-xs text-muted-foreground">
        {t.preview.emptyFile.replace("{path}", path)}
      </p>
    );
  }

  if (highlightedHtml) {
    return (
      <div
        className="shiki-preview overflow-x-auto font-mono text-[12px] leading-5 [&>pre]:!bg-transparent [&>pre]:p-0 [&_.line]:flex [&_.line]:min-h-5 [&_.line]:px-2 [&_.line:hover]:bg-accent/20"
        // biome-ignore lint/security/noDangerouslySetInnerHtml: shiki produces trusted HTML
        dangerouslySetInnerHTML={{ __html: highlightedHtml }}
      />
    );
  }

  const lines = content.split("\n");
  const gutterWidth = String(lines.length).length;

  return (
    <div className="font-mono text-[12px] leading-5">
      {lines.map((line, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: static line-numbered content never reorders
        <div key={i} className="flex hover:bg-accent/20">
          <span
            className="shrink-0 select-none text-right text-muted-foreground/50 pr-3 pl-2"
            style={{ minWidth: gutterWidth * 8 + 20 }}
          >
            {i + 1}
          </span>
          <span className={cn("whitespace-pre text-foreground", !line && "min-h-5")}>{line}</span>
        </div>
      ))}
    </div>
  );
}

function DiffContentView({ diff }: { diff: string }) {
  const { t } = useI18n();
  if (!diff.trim()) {
    return <p className="p-4 text-xs text-muted-foreground">{t.preview.noChanges}</p>;
  }

  const lines = diff.split("\n");
  return (
    <div>
      {lines.map((line, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: static diff lines never reorder
        <DiffLine key={i} line={line} />
      ))}
    </div>
  );
}
