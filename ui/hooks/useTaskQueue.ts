import { useCallback, useRef, useState } from "react";
import type { InputMode } from "@/lib/types";

export type TaskChannel = "chat" | "terminal";

export interface QueuedTask {
  id: number;
  channel: TaskChannel;
  mode: InputMode;
  content: string;
  queuedAt: number;
  /** When true, the queue blocks at this item until the user finishes editing. */
  editing: boolean;
}

/**
 * Unified task queue with two independent channels.
 *
 * Features:
 *   - Per-item inline editing — blocks auto-consumption while editing
 *   - Reorder (move up) and delete
 *   - Priority insert (jump queue) via `enqueuePriority`
 *
 * The ref-backed state ensures synchronous access for dequeue while driving re-renders.
 */
export function useTaskQueue() {
  const [tasks, setTasks] = useState<QueuedTask[]>([]);
  const tasksRef = useRef<QueuedTask[]>([]);
  const idRef = useRef(0);

  const sync = useCallback((next: QueuedTask[]) => {
    tasksRef.current = next;
    setTasks(next);
  }, []);

  /** Append a task to the end of the queue. */
  const enqueue = useCallback(
    (channel: TaskChannel, mode: InputMode, content: string): number => {
      const id = ++idRef.current;
      sync([
        ...tasksRef.current,
        { id, channel, mode, content, queuedAt: Date.now(), editing: false },
      ]);
      return id;
    },
    [sync],
  );

  /**
   * Insert a task at the front of its channel (priority / jump-queue).
   * Returns the new task id.
   */
  const enqueuePriority = useCallback(
    (channel: TaskChannel, mode: InputMode, content: string): number => {
      const id = ++idRef.current;
      const task: QueuedTask = { id, channel, mode, content, queuedAt: Date.now(), editing: false };
      const firstIdx = tasksRef.current.findIndex((t) => t.channel === channel);
      if (firstIdx === -1) {
        sync([...tasksRef.current, task]);
      } else {
        const copy = [...tasksRef.current];
        copy.splice(firstIdx, 0, task);
        sync(copy);
      }
      return id;
    },
    [sync],
  );

  /**
   * Dequeue the first non-editing task for a channel.
   * If the head item is being edited, returns undefined (queue is blocked).
   */
  const dequeue = useCallback(
    (channel: TaskChannel): QueuedTask | undefined => {
      const idx = tasksRef.current.findIndex((t) => t.channel === channel);
      if (idx === -1) return undefined;
      const task = tasksRef.current[idx];
      if (task.editing) return undefined; // blocked by editing
      sync([...tasksRef.current.slice(0, idx), ...tasksRef.current.slice(idx + 1)]);
      return task;
    },
    [sync],
  );

  /** Remove a task by id. */
  const cancel = useCallback(
    (id: number) => {
      sync(tasksRef.current.filter((t) => t.id !== id));
    },
    [sync],
  );

  /** Remove all tasks, optionally filtered by channel. */
  const cancelAll = useCallback(
    (channel?: TaskChannel) => {
      if (channel) {
        sync(tasksRef.current.filter((t) => t.channel !== channel));
      } else {
        sync([]);
      }
    },
    [sync],
  );

  /** Update the content of a queued task (inline edit). */
  const updateContent = useCallback(
    (id: number, content: string) => {
      sync(tasksRef.current.map((t) => (t.id === id ? { ...t, content } : t)));
    },
    [sync],
  );

  /** Toggle editing state for a task. While editing, the queue blocks at this item. */
  const setEditing = useCallback(
    (id: number, editing: boolean) => {
      sync(tasksRef.current.map((t) => (t.id === id ? { ...t, editing } : t)));
    },
    [sync],
  );

  /** Move a task one position earlier within the same channel. */
  const moveUp = useCallback(
    (id: number) => {
      const copy = [...tasksRef.current];
      const idx = copy.findIndex((t) => t.id === id);
      if (idx <= 0) return;
      // Find the previous task in the same channel
      const task = copy[idx];
      let swapIdx = -1;
      for (let i = idx - 1; i >= 0; i--) {
        if (copy[i].channel === task.channel) {
          swapIdx = i;
          break;
        }
      }
      if (swapIdx === -1) return;
      // Swap positions
      [copy[swapIdx], copy[idx]] = [copy[idx], copy[swapIdx]];
      sync(copy);
    },
    [sync],
  );

  const countByChannel = useCallback(
    (channel: TaskChannel): number => tasksRef.current.filter((t) => t.channel === channel).length,
    [],
  );

  const peekChannel = useCallback(
    (channel: TaskChannel): QueuedTask | undefined =>
      tasksRef.current.find((t) => t.channel === channel),
    [],
  );

  /** Check if the head of a channel is blocked (being edited). */
  const isChannelBlocked = useCallback((channel: TaskChannel): boolean => {
    const head = tasksRef.current.find((t) => t.channel === channel);
    return head?.editing ?? false;
  }, []);

  return {
    tasks,
    enqueue,
    enqueuePriority,
    dequeue,
    cancel,
    cancelAll,
    updateContent,
    setEditing,
    moveUp,
    countByChannel,
    peekChannel,
    isChannelBlocked,
  };
}
