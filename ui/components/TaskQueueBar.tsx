import {
  ArrowUp,
  Bot,
  Check,
  Pencil,
  Play,
  Sparkles,
  Terminal,
  Trash2,
  X,
  Zap,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { QueuedTask } from "@/hooks/useTaskQueue";
import { useI18n } from "@/lib/i18n";
import type { InputMode } from "@/lib/types";
import { cn } from "@/lib/utils";

interface TaskQueueBarProps {
  tasks: QueuedTask[];
  onCancel: (id: number) => void;
  onCancelAll: () => void;
  onRunNextTerminal?: () => void;
  onUpdateContent?: (id: number, content: string) => void;
  onSetEditing?: (id: number, editing: boolean) => void;
  onMoveUp?: (id: number) => void;
  onForceExecute?: (id: number) => void;
}

const modeIcon: Record<InputMode, typeof Terminal> = {
  shell: Terminal,
  ai: Sparkles,
  agent: Bot,
};

const modeStyle: Record<InputMode, string> = {
  shell: "bg-card text-card-foreground border-border",
  ai: "bg-card text-ai border-ai-subtle",
  agent: "bg-card text-agent border-agent-subtle",
};

export function TaskQueueBar({
  tasks,
  onCancel,
  onCancelAll,
  onRunNextTerminal,
  onUpdateContent,
  onSetEditing,
  onMoveUp,
  onForceExecute,
}: TaskQueueBarProps) {
  const { t } = useI18n();

  if (tasks.length === 0) return null;

  const terminalTasks = tasks.filter((task) => task.channel === "terminal");
  const chatTasks = tasks.filter((task) => task.channel === "chat");

  return (
    <div className="border-t border-border bg-background px-4 py-1.5">
      <div className="mb-1 flex items-center justify-between">
        <span className="text-[11px] font-medium text-muted-foreground">
          {t.taskQueue.queued.replace("{count}", String(tasks.length))}
        </span>
        {tasks.length > 1 && (
          <button
            type="button"
            onClick={onCancelAll}
            title={t.taskQueue.clearAll}
            className="flex items-center gap-1 text-[10px] text-muted-foreground transition-colors hover:text-destructive"
          >
            <Trash2 className="h-3 w-3" />
            <span>{t.taskQueue.clearAllBtn}</span>
          </button>
        )}
      </div>

      <div className="flex flex-col gap-1">
        {terminalTasks.map((task, i) => (
          <QueueItem
            key={task.id}
            task={task}
            isFirst={i === 0}
            showRunNext={i === 0 && !!onRunNextTerminal}
            onCancel={onCancel}
            onRunNext={onRunNextTerminal}
            onUpdateContent={onUpdateContent}
            onSetEditing={onSetEditing}
            onMoveUp={i > 0 ? onMoveUp : undefined}
            onForceExecute={onForceExecute}
          />
        ))}
        {chatTasks.map((task, i) => (
          <QueueItem
            key={task.id}
            task={task}
            isFirst={i === 0}
            showRunNext={false}
            onCancel={onCancel}
            onUpdateContent={onUpdateContent}
            onSetEditing={onSetEditing}
            onMoveUp={i > 0 ? onMoveUp : undefined}
            onForceExecute={onForceExecute}
          />
        ))}
      </div>
    </div>
  );
}

// ── Individual queue item ──

interface QueueItemProps {
  task: QueuedTask;
  isFirst: boolean;
  showRunNext: boolean;
  onCancel: (id: number) => void;
  onRunNext?: () => void;
  onUpdateContent?: (id: number, content: string) => void;
  onSetEditing?: (id: number, editing: boolean) => void;
  onMoveUp?: (id: number) => void;
  onForceExecute?: (id: number) => void;
}

function QueueItem({
  task,
  isFirst,
  showRunNext,
  onCancel,
  onRunNext,
  onUpdateContent,
  onSetEditing,
  onMoveUp,
  onForceExecute,
}: QueueItemProps) {
  const { t } = useI18n();
  const Icon = modeIcon[task.mode];
  const [editValue, setEditValue] = useState(task.content);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (task.editing) {
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [task.editing]);

  const startEdit = useCallback(() => {
    setEditValue(task.content);
    onSetEditing?.(task.id, true);
  }, [task.id, task.content, onSetEditing]);

  const commitEdit = useCallback(() => {
    const trimmed = editValue.trim();
    if (trimmed) {
      onUpdateContent?.(task.id, trimmed);
    }
    onSetEditing?.(task.id, false);
  }, [task.id, editValue, onUpdateContent, onSetEditing]);

  const cancelEdit = useCallback(() => {
    setEditValue(task.content);
    onSetEditing?.(task.id, false);
  }, [task.id, task.content, onSetEditing]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        commitEdit();
      } else if (e.key === "Escape") {
        e.preventDefault();
        cancelEdit();
      }
    },
    [commitEdit, cancelEdit],
  );

  return (
    <div
      className={cn(
        "flex items-center gap-1.5 rounded-lg border px-2 py-1",
        modeStyle[task.mode],
        isFirst && "ring-1 ring-primary/30",
        task.editing && "ring-2 ring-warning/50",
      )}
    >
      <Icon className="h-3 w-3 shrink-0 text-muted-foreground" />

      {/* Content: inline editor or static text */}
      {task.editing ? (
        <input
          ref={inputRef}
          type="text"
          value={editValue}
          onChange={(e) => setEditValue(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={commitEdit}
          className="min-w-0 flex-1 rounded bg-background/80 px-1.5 py-0.5 text-[11px] outline-none ring-1 ring-border"
        />
      ) : (
        <span className="min-w-0 flex-1 truncate text-[11px] text-foreground">{task.content}</span>
      )}

      {/* Action buttons */}
      <div className="flex shrink-0 items-center gap-0.5">
        {task.editing ? (
          <>
            <IconBtn title={t.taskQueue.confirmEdit} onClick={commitEdit}>
              <Check className="h-2.5 w-2.5 text-success" />
            </IconBtn>
            <IconBtn title={t.taskQueue.cancelEdit} onClick={cancelEdit}>
              <X className="h-2.5 w-2.5" />
            </IconBtn>
          </>
        ) : (
          <>
            {/* Run Next (terminal first item only) */}
            {showRunNext && onRunNext && (
              <IconBtn title={t.taskQueue.runNow} onClick={onRunNext}>
                <Play className="h-2.5 w-2.5 fill-current text-success" />
              </IconBtn>
            )}

            {/* Force execute (jump queue) */}
            {onForceExecute && (
              <IconBtn title={t.taskQueue.forceExecute} onClick={() => onForceExecute(task.id)}>
                <Zap className="h-2.5 w-2.5 text-warning" />
              </IconBtn>
            )}

            {/* Edit */}
            {onUpdateContent && (
              <IconBtn title={t.taskQueue.editCommand} onClick={startEdit}>
                <Pencil className="h-2.5 w-2.5" />
              </IconBtn>
            )}

            {/* Move up */}
            {onMoveUp && (
              <IconBtn title={t.taskQueue.moveUp} onClick={() => onMoveUp(task.id)}>
                <ArrowUp className="h-2.5 w-2.5" />
              </IconBtn>
            )}

            {/* Delete */}
            <IconBtn title={t.taskQueue.removeFromQueue} onClick={() => onCancel(task.id)}>
              <X className="h-2.5 w-2.5" />
            </IconBtn>
          </>
        )}
      </div>
    </div>
  );
}

function IconBtn({
  children,
  title,
  onClick,
}: {
  children: React.ReactNode;
  title: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      title={title}
      onClick={onClick}
      className="rounded p-0.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
    >
      {children}
    </button>
  );
}
