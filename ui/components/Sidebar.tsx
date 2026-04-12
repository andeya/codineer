import { useCallback, useRef } from "react";
import { type Translations, useI18n } from "@/lib/i18n";
import type { SidebarPanel } from "./ActivityBar";
import { FileTree } from "./FileTree";
import { GitPanel } from "./GitPanel";
import { SearchPanel } from "./SearchPanel";
import { SettingsPanel } from "./SettingsPanel";

interface SidebarProps {
  panel: SidebarPanel;
  visible: boolean;
  width: number;
  onWidthChange?: (w: number) => void;
  onOpenFile: (path: string) => void;
  onOpenDiff: (cwd: string, path: string) => void;
}

function panelTitle(panel: SidebarPanel, t: Translations): string {
  switch (panel) {
    case "explorer":
      return t.nav.explorer;
    case "search":
      return t.nav.search;
    case "git":
      return t.nav.git;
    case "context":
      return t.nav.context;
    case "memory":
      return t.nav.memory;
    case "settings":
      return t.nav.settings;
  }
}

function panelContent(
  panel: SidebarPanel,
  onOpenFile: (path: string) => void,
  onOpenDiff: (cwd: string, path: string) => void,
  t: Translations,
) {
  switch (panel) {
    case "explorer":
      return <FileTree onOpenFile={onOpenFile} />;
    case "search":
      return <SearchPanel onOpenFile={onOpenFile} />;
    case "git":
      return <GitPanel onOpenDiff={onOpenDiff} />;
    case "context":
      return <p className="p-3">{t.nav.contextHint}</p>;
    case "memory":
      return <p className="p-3">{t.nav.memoryHint}</p>;
    case "settings":
      return <SettingsPanel />;
  }
}

export function Sidebar({
  panel,
  visible,
  width,
  onWidthChange,
  onOpenFile,
  onOpenDiff,
}: SidebarProps) {
  const { t } = useI18n();
  const dragging = useRef(false);

  const isSettings = panel === "settings";
  const minW = isSettings ? 480 : 200;
  const maxW = isSettings ? 800 : 500;
  const effectiveWidth = isSettings ? Math.max(width, minW) : width;

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (!onWidthChange) return;
      e.preventDefault();
      dragging.current = true;
      const startX = e.clientX;
      const startW = effectiveWidth;

      const onMove = (ev: MouseEvent) => {
        if (!dragging.current) return;
        const delta = ev.clientX - startX;
        const next = Math.min(maxW, Math.max(minW, startW + delta));
        onWidthChange(next);
      };

      const onUp = () => {
        dragging.current = false;
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
      };

      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", onUp);
    },
    [onWidthChange, effectiveWidth, minW, maxW],
  );

  if (!visible) return null;

  return (
    <div className="relative flex h-full shrink-0" style={{ width: effectiveWidth }}>
      <div
        className="flex h-full flex-1 flex-col border-r border-border bg-sidebar"
        style={{ width: effectiveWidth }}
      >
        {!isSettings && (
          <div className="flex h-8 items-center border-b border-border px-3 text-xs font-medium text-foreground">
            {panelTitle(panel, t)}
          </div>
        )}
        <div className="flex-1 overflow-y-auto text-xs text-muted-foreground">
          {panelContent(panel, onOpenFile, onOpenDiff, t)}
        </div>
      </div>
      <div
        aria-hidden="true"
        className="absolute top-0 right-0 z-10 h-full w-1 cursor-col-resize transition-colors hover:bg-primary/40 active:bg-primary/60"
        onMouseDown={handleMouseDown}
      />
    </div>
  );
}
