import { useCallback, useRef, useState } from "react";
import { isInteractiveCommand } from "@/lib/constants";
import {
  executeCommand,
  executeSlashCommand,
  isTauri,
  sendAiMessage,
  startAgent,
} from "@/lib/tauri";
import type { Attachment, ChatMessage, InputMode } from "@/lib/types";

interface UseChatExecutionOptions {
  projectRoot: string;
  dequeue: (channel: "chat" | "terminal") => { content: string; mode: InputMode } | undefined;
  enqueue: (channel: "chat" | "terminal", mode: InputMode, content: string) => void;
  tabs: { id: string; type: string }[];
  activeTab: { id: string; type: string } | undefined;
  markUnread: (id: string) => void;
  terminalRef: React.RefObject<{
    runCommand: (cmd: string) => void;
    resetShell: () => void;
  } | null>;
  termCommandActive: React.MutableRefObject<boolean>;
  setTerminalVisible: (v: boolean) => void;
}

export function useChatExecution({
  projectRoot,
  dequeue,
  enqueue,
  tabs,
  activeTab,
  markUnread,
  terminalRef,
  termCommandActive,
  setTerminalVisible,
}: UseChatExecutionOptions) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [isStreaming, setIsStreaming] = useState(false);
  const nextIdRef = useRef(1);
  const abortRef = useRef<AbortController | null>(null);
  const pendingAttachmentsRef = useRef<Attachment[] | null>(null);
  const [inputMode, setInputMode] = useState<InputMode>("shell");

  const allocId = useCallback(() => {
    const id = nextIdRef.current;
    nextIdRef.current += 1;
    return id;
  }, []);

  const notifyChatUpdate = useCallback(() => {
    if (activeTab && activeTab.type !== "chat") {
      const chatTab = tabs.find((t) => t.type === "chat");
      if (chatTab) markUnread(chatTab.id);
    }
  }, [activeTab, tabs, markUnread]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: circular dependency between executeChatTask ↔ processChatQueue ↔ execute*Task
  const executeChatTask = useCallback(async (text: string, mode: InputMode) => {
    if (mode === "shell") {
      await _executeShell(text);
    } else if (mode === "ai") {
      await _executeAi(text);
    } else {
      await _executeAgent(text);
    }
  }, []);

  const processChatQueue = useCallback(() => {
    notifyChatUpdate();
    const next = dequeue("chat");
    if (next) {
      executeChatTask(next.content, next.mode);
    } else {
      setIsStreaming(false);
    }
  }, [dequeue, executeChatTask, notifyChatUpdate]);

  const _executeShell = useCallback(
    async (text: string) => {
      const userId = allocId();
      setMessages((prev) => [
        ...prev,
        {
          id: userId,
          role: "user",
          mode: "shell",
          content: text,
          timestamp: Date.now(),
          attachments: pendingAttachmentsRef.current ?? undefined,
        },
      ]);
      pendingAttachmentsRef.current = null;
      setIsStreaming(true);

      const abort = new AbortController();
      abortRef.current = abort;

      let output: string;
      let exitCode: number;
      let durationMs: number;
      let timedOut = false;
      const cwd = projectRoot || undefined;
      const startMs = Date.now();

      if (isTauri()) {
        try {
          const resultPromise = executeCommand({ command: text, cwd });
          const abortPromise = new Promise<"aborted">((resolve) => {
            abort.signal.addEventListener("abort", () => resolve("aborted"), { once: true });
          });
          const race = await Promise.race([resultPromise, abortPromise]);

          if (race === "aborted") {
            output = "[Command stopped by user]";
            exitCode = 130;
            durationMs = Date.now() - startMs;
          } else {
            const result = race;
            const parts: string[] = [];
            if (result.stdout) parts.push(result.stdout);
            if (result.stderr) parts.push(result.stderr);
            output = parts.join("\n") || "(no output)";
            exitCode = result.exit_code;
            durationMs = result.duration_ms;
            timedOut = result.timed_out;
          }
        } catch (err) {
          if (abort.signal.aborted) {
            output = "[Command stopped by user]";
            exitCode = 130;
            durationMs = Date.now() - startMs;
          } else {
            output = `Error: ${err}`;
            exitCode = 1;
            durationMs = 0;
          }
        }
      } else {
        await new Promise<void>((resolve) => {
          const timer = setTimeout(resolve, Math.floor(Math.random() * 500) + 50);
          abort.signal.addEventListener(
            "abort",
            () => {
              clearTimeout(timer);
              resolve();
            },
            { once: true },
          );
        });
        if (abort.signal.aborted) {
          output = "[Command stopped by user]";
          exitCode = 130;
          durationMs = Date.now() - startMs;
        } else {
          output = simulateShellOutput(text);
          exitCode = text.includes("fail") ? 1 : 0;
          durationMs = Date.now() - startMs;
        }
      }

      abortRef.current = null;

      const outId = allocId();
      setMessages((prev) => [
        ...prev,
        {
          id: outId,
          role: "assistant",
          mode: "shell",
          content: text,
          timestamp: Date.now(),
          shell: {
            command: text,
            cwd: cwd || "~",
            output: output.replace(/\n$/, ""),
            exitCode,
            durationMs,
            timedOut,
          },
        },
      ]);
      processChatQueue();
    },
    [allocId, projectRoot, processChatQueue],
  );

  const _executeAi = useCallback(
    async (text: string) => {
      const userId = allocId();
      const atts = pendingAttachmentsRef.current ?? undefined;
      pendingAttachmentsRef.current = null;
      setMessages((prev) => [
        ...prev,
        {
          id: userId,
          role: "user",
          mode: "ai",
          content: text,
          timestamp: Date.now(),
          attachments: atts,
        },
      ]);
      setIsStreaming(true);

      if (isTauri()) {
        try {
          await sendAiMessage({ message: text, context_block_ids: [] });
          processChatQueue();
          return;
        } catch {
          // fall through to mock
        }
      }

      setTimeout(() => {
        setMessages((prev) => [
          ...prev,
          {
            id: allocId(),
            role: "assistant",
            mode: "ai",
            content: simulateAIResponse(text),
            model: "claude-sonnet-4",
            timestamp: Date.now(),
          },
        ]);
        processChatQueue();
      }, 1200);
    },
    [allocId, processChatQueue],
  );

  const _executeAgent = useCallback(
    async (text: string) => {
      const userId = allocId();
      const atts = pendingAttachmentsRef.current ?? undefined;
      pendingAttachmentsRef.current = null;
      setMessages((prev) => [
        ...prev,
        {
          id: userId,
          role: "user",
          mode: "agent",
          content: text,
          timestamp: Date.now(),
          attachments: atts,
        },
      ]);
      setIsStreaming(true);

      if (isTauri()) {
        try {
          await startAgent({ goal: text, context_block_ids: [] });
          processChatQueue();
          return;
        } catch {
          // fall through to mock
        }
      }

      setTimeout(() => {
        setMessages((prev) => [
          ...prev,
          {
            id: allocId(),
            role: "assistant",
            mode: "agent",
            content: `I'll help you with: **${text}**`,
            model: "claude-sonnet-4",
            timestamp: Date.now(),
            thinking:
              "Analyzing the request...\nBreaking down into steps:\n1. Understand the goal\n2. Search relevant files\n3. Make changes\n4. Verify results",
            agentSteps: [
              { name: "Analyzing codebase", status: "completed" as const },
              { name: "Searching for relevant files", status: "completed" as const },
              { name: "Planning changes", status: "running" as const },
            ],
            toolCalls: [
              {
                type: "search_files",
                state: "output-available" as const,
                input: { query: text, path: "." },
                output: { matches: 3, files: ["src/main.rs", "lib.rs", "Cargo.toml"] },
              },
            ],
          },
        ]);
        processChatQueue();
      }, 1800);
    },
    [allocId, processChatQueue],
  );

  const handleSubmit = useCallback(
    (text: string, mode: InputMode, attachments?: Attachment[]) => {
      const interactive = mode === "shell" && isInteractiveCommand(text);

      if (interactive) {
        if (termCommandActive.current) {
          enqueue("terminal", mode, text);
        } else {
          termCommandActive.current = true;
          setTerminalVisible(true);
          requestAnimationFrame(() => {
            terminalRef.current?.runCommand(text);
          });
        }
        return;
      }

      if (isStreaming) {
        enqueue("chat", mode, text);
        return;
      }
      pendingAttachmentsRef.current = attachments ?? null;
      executeChatTask(text, mode);
    },
    [isStreaming, enqueue, executeChatTask, termCommandActive, terminalRef, setTerminalVisible],
  );

  const handleStop = useCallback(() => {
    abortRef.current?.abort();
  }, []);

  const handleSlashCommand = useCallback(
    async (cmd: string) => {
      if (cmd === "clear") {
        setMessages([]);
        return;
      }

      const userMsg: ChatMessage = {
        id: nextIdRef.current++,
        role: "user",
        mode: inputMode,
        content: `/${cmd}`,
        timestamp: Date.now(),
      };
      setMessages((prev) => [...prev, userMsg]);

      let result: string;
      try {
        result = await executeSlashCommand(cmd);
      } catch (err) {
        result = `Error: ${err}`;
      }

      const sysMsg: ChatMessage = {
        id: nextIdRef.current++,
        role: "assistant",
        mode: inputMode,
        content: result,
        timestamp: Date.now(),
      };
      setMessages((prev) => [...prev, sysMsg]);
    },
    [inputMode],
  );

  const handleForceExecute = useCallback(
    (
      taskId: number,
      queuedTasks: { id: number; channel: string; content: string; mode: InputMode }[],
    ) => {
      const task = queuedTasks.find((t) => t.id === taskId);
      if (!task) return;

      if (task.channel === "terminal") {
        terminalRef.current?.runCommand(task.content);
        termCommandActive.current = true;
        setTerminalVisible(true);
      } else {
        abortRef.current?.abort();
        setIsStreaming(false);
        executeChatTask(task.content, task.mode);
      }
    },
    [executeChatTask, terminalRef, termCommandActive, setTerminalVisible],
  );

  return {
    messages,
    isStreaming,
    inputMode,
    setInputMode,
    handleSubmit,
    handleStop,
    handleSlashCommand,
    handleForceExecute,
    executeChatTask,
  };
}

function simulateShellOutput(cmd: string): string {
  const c = cmd.trim().split(/\s+/)[0];
  const outputs: Record<string, string> = {
    ls: "Cargo.toml  Cargo.lock  app/  crates/  ui/  scripts/  package.json  README.md",
    pwd: "/Users/demo/projects/aineer",
    echo: cmd.replace(/^echo\s+/, ""),
    date: new Date().toString(),
    whoami: "developer",
    cargo:
      "   Compiling aineer v0.1.0\n    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.34s",
    git: "On branch main\nYour branch is up to date with 'origin/main'.\n\nnothing to commit, working tree clean",
    bun: "bun v1.3.11\n✓ 127 packages installed",
  };
  return outputs[c] ?? `$ ${cmd}\nCommand executed successfully.`;
}

function simulateAIResponse(query: string): string {
  if (query.toLowerCase().includes("explain")) {
    return "Here's an explanation:\n\nThis code uses a **modular architecture** with clear separation of concerns:\n\n1. `app/` — Tauri desktop entry point with IPC commands\n2. `crates/` — Reusable Rust business logic\n3. `ui/` — React frontend with shadcn components\n\n```rust\nfn main() {\n    aineer_lib::run_desktop();\n}\n```\n\nThe Tauri IPC bridge connects frontend to Rust backend via `invoke()`.";
  }
  return `I'd be happy to help with that.\n\nBased on your question about **"${query}"**, here's what I think:\n\nThe approach involves analyzing the current codebase structure and making targeted changes. Would you like me to elaborate on any specific aspect?`;
}
