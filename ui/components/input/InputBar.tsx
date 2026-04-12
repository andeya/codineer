"use client";

import { ArrowUp, Bot, Paperclip, Sparkles, Square, Terminal } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  PromptInput,
  PromptInputAction,
  PromptInputActions,
  PromptInputTextarea,
} from "@/components/ui/prompt-input";
import { useI18n } from "@/lib/i18n";
import type { CompletionItem, FileEntry } from "@/lib/tauri";
import {
  AT_MENTIONS,
  type AtMention,
  type Attachment,
  type InputMode,
  SLASH_COMMANDS,
} from "@/lib/types";
import { cn } from "@/lib/utils";
import { AttachmentLightbox, AttachmentStrip } from "./AttachmentStrip";
import { FilePicker } from "./FilePicker";
import { InlineChip } from "./InlineChip";
import { MentionMenu } from "./MentionMenu";
import { ShellCompletionMenu, SlashMenu } from "./SlashMenu";
import type { InputBarProps, InputChip, ModeDraft, PendingAttachment } from "./types";

let _nextChipId = 1;
function nextChipId(): string {
  return `chip-${_nextChipId++}`;
}

let _nextAttId = 1;
function nextAttachmentId(): string {
  return `att-${_nextAttId++}`;
}

const emptyDraft: ModeDraft = { value: "", chips: [], attachments: [] };

export function InputBar({
  mode,
  onModeChange,
  onSubmit,
  onSlashCommand,
  onStop,
  isStreaming,
  slashCommands,
  queueSize = 0,
  projectRoot,
}: InputBarProps) {
  const { t } = useI18n();

  const modes: { id: InputMode; icon: typeof Terminal; label: string; color: string }[] = useMemo(
    () => [
      { id: "shell", icon: Terminal, label: t.mode.shell, color: "text-foreground" },
      {
        id: "ai",
        icon: Sparkles,
        label: t.mode.chat,
        color: "text-ai",
      },
      { id: "agent", icon: Bot, label: t.mode.agent, color: "text-agent" },
    ],
    [t.mode.shell, t.mode.chat, t.mode.agent],
  );

  const [drafts, setDrafts] = useState<Record<InputMode, ModeDraft>>({
    shell: { ...emptyDraft },
    ai: { ...emptyDraft },
    agent: { ...emptyDraft },
  });

  const draft = drafts[mode];
  const value = draft.value;
  const chips = draft.chips;
  const attachments = draft.attachments;

  const updateDraft = useCallback(
    (patch: Partial<ModeDraft>) => {
      setDrafts((prev) => ({
        ...prev,
        [mode]: { ...prev[mode], ...patch },
      }));
    },
    [mode],
  );

  const setValue = useCallback((v: string) => updateDraft({ value: v }), [updateDraft]);
  const setChips = useCallback(
    (fn: InputChip[] | ((prev: InputChip[]) => InputChip[])) => {
      setDrafts((prev) => ({
        ...prev,
        [mode]: {
          ...prev[mode],
          chips: typeof fn === "function" ? fn(prev[mode].chips) : fn,
        },
      }));
    },
    [mode],
  );
  const setAttachments = useCallback(
    (fn: PendingAttachment[] | ((prev: PendingAttachment[]) => PendingAttachment[])) => {
      setDrafts((prev) => ({
        ...prev,
        [mode]: {
          ...prev[mode],
          attachments: typeof fn === "function" ? fn(prev[mode].attachments) : fn,
        },
      }));
    },
    [mode],
  );

  const [showSlash, setShowSlash] = useState(false);
  const [showMentions, setShowMentions] = useState(false);
  const [showFilePicker, setShowFilePicker] = useState(false);
  const [showShellComplete, setShowShellComplete] = useState(false);
  const [shellCompletions, setShellCompletions] = useState<CompletionItem[]>([]);
  const shellPrefixRef = useRef("");

  const [filePickerQuery, setFilePickerQuery] = useState("");
  const [filePickerEntries, setFilePickerEntries] = useState<FileEntry[]>([]);
  const [filePickerPath, setFilePickerPath] = useState<string>("");
  const [filePickerRoot, setFilePickerRoot] = useState<string>("");
  const [filePickerLoading, setFilePickerLoading] = useState(false);

  const [selectedIdx, setSelectedIdx] = useState(0);
  const [lightboxUrl, setLightboxUrl] = useState<string | null>(null);
  const [dragAttId, setDragAttId] = useState<string | null>(null);

  const menuRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const pickerInputRef = useRef<HTMLInputElement>(null);

  const prevModeRef = useRef(mode);
  useEffect(() => {
    if (prevModeRef.current !== mode) {
      prevModeRef.current = mode;
      setShowSlash(false);
      setShowMentions(false);
      setShowFilePicker(false);
      setShowShellComplete(false);
      setShellCompletions([]);
      setSelectedIdx(0);
    }
  }, [mode]);

  const canAttach = mode !== "shell";

  const addImageFiles = useCallback(
    (files: File[]) => {
      const newAtts: PendingAttachment[] = files.map((f) => ({
        id: nextAttachmentId(),
        type: f.type.startsWith("image/") ? ("image" as const) : ("file" as const),
        name: f.name,
        size: f.size,
        previewUrl: f.type.startsWith("image/") ? URL.createObjectURL(f) : undefined,
        file: f,
      }));
      setAttachments((prev) => [...prev, ...newAtts]);
    },
    [setAttachments],
  );

  const removeAttachment = useCallback(
    (id: string) => {
      setAttachments((prev) => {
        const att = prev.find((a) => a.id === id);
        if (att?.previewUrl) URL.revokeObjectURL(att.previewUrl);
        return prev.filter((a) => a.id !== id);
      });
    },
    [setAttachments],
  );

  const moveAttachment = useCallback(
    (fromId: string, toId: string) => {
      if (fromId === toId) return;
      setAttachments((prev) => {
        const arr = [...prev];
        const fromIdx = arr.findIndex((a) => a.id === fromId);
        const toIdx = arr.findIndex((a) => a.id === toId);
        if (fromIdx < 0 || toIdx < 0) return prev;
        const [item] = arr.splice(fromIdx, 1);
        arr.splice(toIdx, 0, item);
        return arr;
      });
    },
    [setAttachments],
  );

  const extractPastedImages = useCallback((dt: DataTransfer): File[] => {
    const fromFiles = Array.from(dt.files).filter((f) => f.type.startsWith("image/"));
    if (fromFiles.length > 0) return fromFiles;
    const fromItems: File[] = [];
    for (const item of Array.from(dt.items)) {
      if (item.kind === "file" && item.type.startsWith("image/")) {
        const f = item.getAsFile();
        if (f) fromItems.push(f);
      }
    }
    return fromItems;
  }, []);

  const handlePaste = useCallback(
    (e: React.ClipboardEvent) => {
      if (!canAttach) return;
      const dt = e.clipboardData;
      if (!dt) return;
      const images = extractPastedImages(dt);
      if (images.length > 0) {
        e.preventDefault();
        addImageFiles(images);
      }
    },
    [canAttach, addImageFiles, extractPastedImages],
  );

  const handleFileSelect = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = Array.from(e.target.files ?? []);
      if (files.length > 0) addImageFiles(files);
      e.target.value = "";
    },
    [addImageFiles],
  );

  const removeChip = useCallback(
    (id: string) => {
      setChips((prev) => prev.filter((c) => c.id !== id));
    },
    [setChips],
  );

  const addCommandChip = useCallback(
    (name: string) => {
      setChips((prev) => [
        ...prev,
        { id: nextChipId(), type: "command", label: `/${name}`, value: name },
      ]);
    },
    [setChips],
  );

  const addMentionChip = useCallback(
    (label: string, rawValue: string, icon?: string) => {
      setChips((prev) => [
        ...prev,
        { id: nextChipId(), type: "mention", label, value: rawValue, icon },
      ]);
    },
    [setChips],
  );

  const loadDirectory = useCallback(
    async (dirPath: string) => {
      setFilePickerLoading(true);
      try {
        const { listDir, getProjectRoot } = await import("@/lib/tauri");
        let root = filePickerRoot;
        if (!root) {
          root = await getProjectRoot();
          setFilePickerRoot(root);
        }
        const target = dirPath || root;
        const entries = await listDir(target);
        const sorted = [...entries].sort((a, b) => {
          if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
          return a.name.localeCompare(b.name);
        });
        setFilePickerEntries(sorted);
        setFilePickerPath(target);
        setSelectedIdx(0);
        setFilePickerQuery("");
      } catch {
        setFilePickerEntries([]);
      } finally {
        setFilePickerLoading(false);
      }
    },
    [filePickerRoot],
  );

  const openFilePicker = useCallback(() => {
    setShowMentions(false);
    setShowFilePicker(true);
    setFilePickerQuery("");
    setSelectedIdx(0);
    loadDirectory("");
    setTimeout(() => pickerInputRef.current?.focus(), 50);
  }, [loadDirectory]);

  const closeFilePicker = useCallback(() => {
    setShowFilePicker(false);
    setFilePickerQuery("");
    setFilePickerEntries([]);
  }, []);

  const commitEntry = useCallback(
    (entry: FileEntry) => {
      const relativePath = filePickerRoot
        ? entry.path.replace(`${filePickerRoot}/`, "")
        : entry.name;
      const atIdx = value.lastIndexOf("@");
      const before = atIdx >= 0 ? value.slice(0, atIdx) : value;
      const displayLabel = entry.is_dir ? `${relativePath}/` : relativePath;
      addMentionChip(displayLabel, entry.path, entry.is_dir ? "folder" : "file");
      setValue(before);
      closeFilePicker();
    },
    [value, filePickerRoot, addMentionChip, closeFilePicker, setValue],
  );

  const expandFolder = useCallback(
    (entry: FileEntry) => {
      if (entry.is_dir) loadDirectory(entry.path);
    },
    [loadDirectory],
  );

  const filePickerGoUp = useCallback(() => {
    if (!filePickerPath || filePickerPath === filePickerRoot) return;
    const parent = filePickerPath.replace(/\/[^/]+$/, "") || filePickerRoot;
    loadDirectory(parent);
  }, [filePickerPath, filePickerRoot, loadDirectory]);

  const filteredPickerEntries = filePickerQuery
    ? filePickerEntries.filter((e) => e.name.toLowerCase().includes(filePickerQuery.toLowerCase()))
    : filePickerEntries;

  const handleValueChange = useCallback(
    (v: string) => {
      setValue(v);
      setShowShellComplete(false);
      if (showFilePicker) return;

      if (mode !== "shell" && (v === "/" || v.startsWith("/"))) {
        setShowSlash(true);
        setShowMentions(false);
        setSelectedIdx(0);
      } else if (mode !== "shell") {
        const atIdx = v.lastIndexOf("@");
        if (atIdx >= 0) {
          const beforeAt = atIdx === 0 ? "" : v[atIdx - 1];
          const afterAt = v.slice(atIdx + 1);
          const isWordStart = atIdx === 0 || beforeAt === " " || beforeAt === "\n";
          const hasNoSpace = !afterAt.includes(" ");
          if (isWordStart && hasNoSpace) {
            setShowMentions(true);
            setShowSlash(false);
            setSelectedIdx(0);
          } else {
            setShowMentions(false);
          }
        } else {
          setShowSlash(false);
          setShowMentions(false);
        }
      } else {
        setShowSlash(false);
        setShowMentions(false);
      }
    },
    [mode, showFilePicker, setValue],
  );

  const allSlash = useMemo(
    () =>
      slashCommands && slashCommands.length > 0
        ? slashCommands.map((s) => ({ name: s.name, description: s.description }))
        : SLASH_COMMANDS,
    [slashCommands],
  );
  const slashQuery = value.startsWith("/") ? value.slice(1).toLowerCase() : "";
  const filteredSlash = allSlash.filter((c) => c.name.startsWith(slashQuery));
  const mentionQuery = value.slice(value.lastIndexOf("@") + 1).toLowerCase();
  const filteredMentions = AT_MENTIONS.filter(
    (m) =>
      m.name.toLowerCase().includes(mentionQuery) ||
      m.description.toLowerCase().includes(mentionQuery),
  );

  const selectSlashCommand = useCallback(
    (name: string) => {
      addCommandChip(name);
      setValue("");
      setShowSlash(false);
    },
    [addCommandChip, setValue],
  );

  const selectMention = useCallback(
    (item: AtMention) => {
      if (item.hasPicker && item.name === "Files & Folders") {
        openFilePicker();
        return;
      }
      const atIdx = value.lastIndexOf("@");
      const before = atIdx > 0 ? value.slice(0, atIdx) : "";
      addMentionChip(item.name, item.name, item.icon);
      setValue(before);
      setShowMentions(false);
    },
    [value, addMentionChip, openFilePicker, setValue],
  );

  const handleSubmit = useCallback(() => {
    if (showFilePicker) return;
    const trimmed = value.trim();
    const hasChips = chips.length > 0;
    if (!trimmed && !hasChips && attachments.length === 0) return;

    const commandChip = chips.find((c) => c.type === "command");
    if (commandChip) {
      const cmd = commandChip.value;
      onSlashCommand(cmd);
      updateDraft({ value: "", chips: [], attachments: [] });
      setShowSlash(false);
      return;
    }

    if (!hasChips && trimmed.startsWith("/")) {
      const parts = trimmed.slice(1).split(/\s+/);
      const cmdName = parts[0];
      if (cmdName && allSlash.some((c) => c.name === cmdName)) {
        onSlashCommand(cmdName);
        updateDraft({ value: "", chips: [], attachments: [] });
        setShowSlash(false);
        return;
      }
    }

    const mentionValues = chips.filter((c) => c.type === "mention").map((c) => c.value);
    const contextPrefix = mentionValues.length > 0 ? mentionValues.join(" ") : "";
    const fullText = contextPrefix ? `${contextPrefix} ${trimmed}`.trim() : trimmed;

    const atts: Attachment[] | undefined =
      attachments.length > 0
        ? attachments.map((a) => ({
            id: a.id,
            type: a.type,
            name: a.name,
            size: a.size,
            previewUrl: a.previewUrl,
          }))
        : undefined;

    onSubmit(fullText, mode, atts);
    updateDraft({ value: "", chips: [], attachments: [] });
    setShowSlash(false);
    setShowMentions(false);
    setShowShellComplete(false);
  }, [
    value,
    chips,
    mode,
    attachments,
    onSubmit,
    onSlashCommand,
    showFilePicker,
    updateDraft,
    allSlash,
  ]);

  const requestShellComplete = useCallback(
    async (input: string) => {
      if (!input.trim()) return;
      const lastSpace = input.lastIndexOf(" ");
      const prefix = lastSpace >= 0 ? input.substring(0, lastSpace + 1) : "";
      const word = lastSpace >= 0 ? input.substring(lastSpace + 1) : input;
      const isFirstWord = lastSpace < 0;
      shellPrefixRef.current = prefix;
      try {
        const { shellComplete } = await import("@/lib/tauri");
        const results = await shellComplete(word, projectRoot || undefined, isFirstWord);
        if (results.length === 0) return;

        if (results.length === 1) {
          const r = results[0];
          const suffix = r.isDir ? "/" : " ";
          setValue(`${prefix}${r.value}${suffix}`);
          setShowShellComplete(false);
          return;
        }

        // Auto-fill the longest common prefix before showing the menu
        const values = results.map((r) => r.value);
        let lcp = values[0];
        for (const v of values) {
          while (lcp && !v.startsWith(lcp)) lcp = lcp.slice(0, -1);
        }
        if (lcp.length > word.length) {
          setValue(`${prefix}${lcp}`);
        }

        setShellCompletions(results);
        setShowShellComplete(true);
        setSelectedIdx(0);
      } catch {
        /* silently ignore */
      }
    },
    [setValue, projectRoot],
  );

  const handlePickerKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      const items = filteredPickerEntries;

      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIdx((i) => Math.min(i + 1, items.length - 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIdx((i) => Math.max(i - 1, 0));
        return;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        const item = items[selectedIdx];
        if (item) commitEntry(item);
        return;
      }
      if (e.key === "ArrowRight") {
        const item = items[selectedIdx];
        if (item?.is_dir) {
          e.preventDefault();
          expandFolder(item);
        }
        return;
      }
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        if (filePickerPath && filePickerPath !== filePickerRoot) {
          filePickerGoUp();
        }
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        closeFilePicker();
        return;
      }
      if (e.key === "Backspace" && !filePickerQuery) {
        e.preventDefault();
        if (filePickerPath && filePickerPath !== filePickerRoot) {
          filePickerGoUp();
        } else {
          closeFilePicker();
          setShowMentions(true);
        }
      }
    },
    [
      filteredPickerEntries,
      selectedIdx,
      commitEntry,
      expandFolder,
      closeFilePicker,
      filePickerQuery,
      filePickerPath,
      filePickerRoot,
      filePickerGoUp,
    ],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.nativeEvent.isComposing || e.keyCode === 229) return;
      if (showFilePicker) return;

      const activePopup = showSlash || showMentions || showShellComplete;

      if (e.key === "Backspace" && !value && chips.length > 0 && !activePopup) {
        e.preventDefault();
        setChips((prev) => prev.slice(0, -1));
        return;
      }

      if (activePopup) {
        const itemCount = showSlash
          ? filteredSlash.length
          : showMentions
            ? filteredMentions.length
            : shellCompletions.length;

        if (e.key === "ArrowDown") {
          e.preventDefault();
          setSelectedIdx((i) => Math.min(i + 1, itemCount - 1));
          return;
        }
        if (e.key === "ArrowUp") {
          e.preventDefault();
          setSelectedIdx((i) => Math.max(i - 1, 0));
          return;
        }
        if (e.key === "Tab" || (e.key === "Enter" && !e.shiftKey)) {
          e.preventDefault();
          if (showSlash) {
            const item = filteredSlash[selectedIdx];
            if (item) selectSlashCommand(item.name);
          } else if (showMentions) {
            const item = filteredMentions[selectedIdx];
            if (item) selectMention(item as unknown as AtMention);
          } else {
            const comp = shellCompletions[selectedIdx];
            if (comp) {
              const suffix = comp.isDir ? "/" : " ";
              setValue(`${shellPrefixRef.current}${comp.value}${suffix}`);
              setShowShellComplete(false);
            }
          }
          return;
        }
        if (e.key === "Escape") {
          setShowSlash(false);
          setShowMentions(false);
          setShowShellComplete(false);
          return;
        }
      }

      if (e.key === "Tab" && !e.shiftKey && !activePopup) {
        e.preventDefault();
        if (mode === "shell" && value.trim()) {
          requestShellComplete(value);
        } else if (!value.trim() && chips.length === 0) {
          const currentIndex = modes.findIndex((m) => m.id === mode);
          onModeChange(modes[(currentIndex + 1) % modes.length].id);
        }
      }
    },
    [
      showFilePicker,
      showSlash,
      showMentions,
      showShellComplete,
      filteredSlash,
      filteredMentions,
      shellCompletions,
      selectedIdx,
      selectSlashCommand,
      selectMention,
      requestShellComplete,
      value,
      chips,
      mode,
      onModeChange,
      modes,
      setValue,
      setChips,
    ],
  );

  useEffect(() => {
    if (!menuRef.current) return;
    const headerOffset = showMentions && !showFilePicker ? 1 : 0;
    const el = menuRef.current.children[selectedIdx + headerOffset] as HTMLElement | undefined;
    el?.scrollIntoView({ block: "nearest" });
  }, [selectedIdx, showMentions, showFilePicker]);

  const attachmentsRef = useRef(attachments);
  attachmentsRef.current = attachments;
  useEffect(() => {
    return () => {
      for (const att of attachmentsRef.current) {
        if (att.previewUrl) URL.revokeObjectURL(att.previewUrl);
      }
    };
  }, []);

  const pickerBreadcrumb =
    filePickerRoot && filePickerPath
      ? filePickerPath.replace(filePickerRoot, "").replace(/^\//, "") || "/"
      : "/";

  const handleFilePickerBack = useCallback(() => {
    if (filePickerPath && filePickerPath !== filePickerRoot) {
      filePickerGoUp();
    } else {
      closeFilePicker();
      setShowMentions(true);
    }
  }, [filePickerPath, filePickerRoot, filePickerGoUp, closeFilePicker]);

  const handleSelectCurrentFolder = useCallback(() => {
    const relativePath = filePickerRoot
      ? filePickerPath.replace(`${filePickerRoot}/`, "")
      : filePickerPath;
    const atIdx = value.lastIndexOf("@");
    const before = atIdx >= 0 ? value.slice(0, atIdx) : value;
    addMentionChip(`${relativePath}/`, filePickerPath, "folder");
    setValue(before);
    closeFilePicker();
  }, [filePickerRoot, filePickerPath, value, addMentionChip, setValue, closeFilePicker]);

  const showSlashPopup = showSlash && filteredSlash.length > 0;
  const showMentionPopup = showMentions && !showFilePicker && filteredMentions.length > 0;

  return (
    <div className="relative border-t border-border bg-background px-4 py-2">
      {showSlashPopup && (
        <SlashMenu
          menuRef={menuRef}
          items={filteredSlash}
          selectedIdx={selectedIdx}
          onHoverIndex={setSelectedIdx}
          onSelect={selectSlashCommand}
        />
      )}

      {showMentionPopup && (
        <MentionMenu
          menuRef={menuRef}
          items={filteredMentions}
          selectedIdx={selectedIdx}
          sectionTitle={t.input.contextSection}
          onHoverIndex={setSelectedIdx}
          onSelect={selectMention}
        />
      )}

      {showFilePicker && (
        <FilePicker
          menuRef={menuRef}
          pickerInputRef={pickerInputRef}
          filePickerQuery={filePickerQuery}
          onQueryChange={setFilePickerQuery}
          onQueryKeyDown={handlePickerKeyDown}
          filePickerLoading={filePickerLoading}
          filteredEntries={filteredPickerEntries}
          selectedIdx={selectedIdx}
          onHoverIndex={setSelectedIdx}
          pickerBreadcrumb={pickerBreadcrumb}
          filePickerPath={filePickerPath}
          filePickerRoot={filePickerRoot}
          onBack={handleFilePickerBack}
          onSelectFolderShortcut={handleSelectCurrentFolder}
          onCommitEntry={commitEntry}
          onExpandFolder={expandFolder}
          filterPlaceholder={t.common.filter}
          loadingLabel={t.common.loading}
          emptyLabel={t.input.noMatchingFiles}
          selectFolderTitle={t.input.selectFolder}
          selectLabel={t.common.select}
          expandTitle={t.common.expand}
        />
      )}

      {showShellComplete && (
        <ShellCompletionMenu
          menuRef={menuRef}
          completions={shellCompletions}
          selectedIdx={selectedIdx}
          onHoverIndex={setSelectedIdx}
          onSelect={(comp) => {
            const suffix = comp.isDir ? "/" : " ";
            setValue(`${shellPrefixRef.current}${comp.value}${suffix}`);
            setShowShellComplete(false);
          }}
        />
      )}

      <div className="mb-1.5 flex items-center gap-1">
        {modes.map((m) => {
          const Icon = m.icon;
          return (
            <button
              key={m.id}
              type="button"
              onClick={() => onModeChange(m.id)}
              className={cn(
                "flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs transition-colors",
                mode === m.id
                  ? `bg-secondary ${m.color} font-medium`
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              <Icon className="h-3 w-3" />
              {m.label}
            </button>
          );
        })}
        <Badge variant="outline" className="ml-auto text-[10px] text-muted-foreground">
          {!value.trim()
            ? t.input.emptyTabToSwitch
            : mode === "shell"
              ? t.input.tabToComplete
              : null}
        </Badge>
      </div>

      <AttachmentStrip
        attachments={attachments}
        dragAttId={dragAttId}
        units={t.units}
        onRemove={removeAttachment}
        onPreview={setLightboxUrl}
        onDragStart={setDragAttId}
        onDragEnd={() => setDragAttId(null)}
        onDragOver={moveAttachment}
      />

      {lightboxUrl && (
        <AttachmentLightbox
          url={lightboxUrl}
          alt={t.input.preview}
          onClose={() => setLightboxUrl(null)}
        />
      )}

      <PromptInput
        value={value}
        onValueChange={handleValueChange}
        onSubmit={handleSubmit}
        isLoading={isStreaming}
        className={cn(
          "rounded-2xl border-2 transition-colors",
          mode === "ai" && "border-ai-focus",
          mode === "agent" && "border-agent-focus",
          mode === "shell" && "border-border focus-within:border-primary",
        )}
      >
        <PromptInputTextarea
          placeholder={
            chips.length > 0
              ? t.input.placeholderWithChips
              : mode === "shell"
                ? t.input.placeholderShell
                : mode === "ai"
                  ? t.input.placeholderChat
                  : t.input.placeholderAgent
          }
          className="min-h-[40px] text-sm"
          onKeyDown={handleKeyDown}
          onPaste={handlePaste}
          inlinePrefix={
            chips.length > 0
              ? chips.map((chip) => <InlineChip key={chip.id} chip={chip} onRemove={removeChip} />)
              : undefined
          }
        />
        <PromptInputActions className="justify-end gap-1 px-2 pb-2">
          {canAttach && (
            <PromptInputAction tooltip={t.input.attachTooltip}>
              <Button
                size="sm"
                variant="ghost"
                className="h-8 w-8 rounded-full p-0 text-muted-foreground hover:text-foreground"
                onClick={() => fileInputRef.current?.click()}
              >
                <Paperclip className="h-4 w-4" />
              </Button>
            </PromptInputAction>
          )}
          {isStreaming && (
            <PromptInputAction tooltip={t.input.stopTooltip}>
              <Button
                size="sm"
                variant="destructive"
                className="h-8 w-8 rounded-full p-0"
                onClick={onStop}
              >
                <Square className="h-3.5 w-3.5" />
              </Button>
            </PromptInputAction>
          )}
          <PromptInputAction tooltip={isStreaming && value.trim() ? t.common.queue : t.common.send}>
            <div className="relative">
              <Button
                size="sm"
                variant="default"
                className={cn(
                  "h-8 w-8 rounded-full p-0",
                  mode === "ai" && "bg-ai-solid hover:opacity-90",
                  mode === "agent" && "bg-agent-solid hover:opacity-90",
                )}
                disabled={!value.trim() && chips.length === 0 && attachments.length === 0}
                onClick={handleSubmit}
              >
                <ArrowUp className="h-4 w-4" />
              </Button>
              {queueSize > 0 && (
                <span className="absolute -top-1.5 -right-1.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-primary px-1 text-[9px] font-bold text-primary-foreground">
                  {queueSize}
                </span>
              )}
            </div>
          </PromptInputAction>
        </PromptInputActions>
      </PromptInput>

      <input
        ref={fileInputRef}
        type="file"
        multiple
        accept="image/*,.pdf,.txt,.md,.json,.csv,.yaml,.yml,.xml,.html,.css,.js,.ts,.jsx,.tsx,.py,.rs,.go,.java,.c,.cpp,.h,.hpp"
        className="hidden"
        onChange={handleFileSelect}
      />
    </div>
  );
}
