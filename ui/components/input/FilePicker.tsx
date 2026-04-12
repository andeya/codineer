"use client";

import { ArrowLeft, Check, FileCode, Folder, FolderOpen } from "lucide-react";
import type { FileEntry } from "@/lib/tauri";
import { cn } from "@/lib/utils";

export function FilePicker({
  menuRef,
  pickerInputRef,
  filePickerQuery,
  onQueryChange,
  onQueryKeyDown,
  filePickerLoading,
  filteredEntries,
  selectedIdx,
  onHoverIndex,
  pickerBreadcrumb,
  filePickerPath,
  filePickerRoot,
  onBack,
  onSelectFolderShortcut,
  onCommitEntry,
  onExpandFolder,
  filterPlaceholder,
  loadingLabel,
  emptyLabel,
  selectFolderTitle,
  selectLabel,
  expandTitle,
}: {
  menuRef: React.RefObject<HTMLDivElement | null>;
  pickerInputRef: React.RefObject<HTMLInputElement | null>;
  filePickerQuery: string;
  onQueryChange: (q: string) => void;
  onQueryKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void;
  filePickerLoading: boolean;
  filteredEntries: FileEntry[];
  selectedIdx: number;
  onHoverIndex: (i: number) => void;
  pickerBreadcrumb: string;
  filePickerPath: string;
  filePickerRoot: string;
  onBack: () => void;
  onSelectFolderShortcut: () => void;
  onCommitEntry: (entry: FileEntry) => void;
  onExpandFolder: (entry: FileEntry) => void;
  filterPlaceholder: string;
  loadingLabel: string;
  emptyLabel: string;
  selectFolderTitle: string;
  selectLabel: string;
  expandTitle: string;
}) {
  return (
    <div className="absolute bottom-full left-4 right-4 mb-1 max-h-[380px] overflow-hidden rounded-lg border bg-popover shadow-lg">
      <div className="flex items-center gap-2 border-b px-2 py-1.5">
        <button
          type="button"
          onClick={onBack}
          className="flex h-6 w-6 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
        >
          <ArrowLeft className="h-3.5 w-3.5" />
        </button>
        <div className="flex min-w-0 flex-1 items-center gap-1.5">
          <FolderOpen className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
          <span className="truncate text-[11px] text-muted-foreground">{pickerBreadcrumb}</span>
        </div>
        {filePickerPath && filePickerPath !== filePickerRoot && (
          <button
            type="button"
            onClick={onSelectFolderShortcut}
            className="flex h-6 shrink-0 items-center gap-1 rounded bg-accent px-1.5 text-[11px] text-accent-foreground hover:bg-accent/80"
            title={selectFolderTitle}
          >
            <Check className="h-3 w-3" />
            {selectLabel}
          </button>
        )}
        <input
          ref={pickerInputRef}
          type="text"
          value={filePickerQuery}
          onChange={(e) => {
            onQueryChange(e.target.value);
            onHoverIndex(0);
          }}
          onKeyDown={onQueryKeyDown}
          placeholder={filterPlaceholder}
          className="h-6 w-28 rounded border bg-transparent px-1.5 text-xs text-foreground outline-none placeholder:text-muted-foreground focus-ring"
        />
      </div>

      <div ref={menuRef} className="max-h-[320px] overflow-y-auto p-1">
        {filePickerLoading ? (
          <div className="px-3 py-4 text-center text-xs text-muted-foreground">{loadingLabel}</div>
        ) : filteredEntries.length === 0 ? (
          <div className="px-3 py-4 text-center text-xs text-muted-foreground">{emptyLabel}</div>
        ) : (
          filteredEntries.map((entry, i) => (
            <div
              role="option"
              tabIndex={-1}
              aria-selected={i === selectedIdx}
              key={entry.path}
              onMouseEnter={() => onHoverIndex(i)}
              className={cn(
                "flex w-full items-center rounded-md text-sm transition-colors",
                i === selectedIdx
                  ? "bg-accent text-accent-foreground"
                  : "text-foreground hover:bg-accent/50",
              )}
            >
              <button
                type="button"
                onClick={() => onCommitEntry(entry)}
                className="flex min-w-0 flex-1 items-center gap-2.5 px-2.5 py-1.5 text-left"
              >
                {entry.is_dir ? (
                  <Folder className="h-4 w-4 shrink-0 text-muted-foreground" />
                ) : (
                  <FileCode className="h-4 w-4 shrink-0 text-muted-foreground" />
                )}
                <span className="flex-1 truncate font-medium">{entry.name}</span>
                <span className="max-w-[160px] truncate text-[11px] text-muted-foreground">
                  {filePickerRoot ? entry.path.replace(`${filePickerRoot}/`, "") : entry.name}
                </span>
              </button>
              {entry.is_dir && (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onExpandFolder(entry);
                  }}
                  className="flex h-full shrink-0 items-center px-2 text-muted-foreground hover:text-foreground"
                  title={expandTitle}
                >
                  <span className="text-sm">›</span>
                </button>
              )}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
