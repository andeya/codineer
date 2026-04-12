import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { isInteractiveCommand } from "@/lib/constants";
import type { ShellContextSnippet } from "@/lib/tauri";
import {
  executeCommand,
  executeSlashCommand,
  isTauri,
  sendAiMessage,
  startAgent,
  stopAgent,
  stopAiStream,
} from "@/lib/tauri";
import type { Attachment, ChatMessage, InputMode } from "@/lib/types";

interface AiStreamPayload {
  blockId: number;
  delta: string;
  /** `"text"` for formal output, `"thinking"` for model reasoning */
  kind: string;
  done: boolean;
}

interface AgentStreamPayload {
  blockId: number;
  kind: string;
  data: string;
}

interface StreamBinding {
  backendBlockId: number;
  assistantMsgId: number;
  safetyTimer: ReturnType<typeof setTimeout>;
}

interface UseChatExecutionOptions {
  projectRoot: string;
  modelName: string;
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

const STREAM_SAFETY_TIMEOUT_MS = 120_000;

function recentShellContext(msgs: ChatMessage[], max: number): ShellContextSnippet[] {
  const out: ShellContextSnippet[] = [];
  for (let i = msgs.length - 1; i >= 0 && out.length < max; i--) {
    const m = msgs[i];
    if (m.mode === "shell" && m.shell) {
      out.push({ command: m.shell.command, output: m.shell.output });
    }
  }
  return out.reverse();
}

export function useChatExecution({
  projectRoot,
  modelName,
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

  // Session CWD persists across shell commands; initialized from projectRoot.
  const [sessionCwd, setSessionCwd] = useState(projectRoot);
  const sessionCwdRef = useRef(sessionCwd);
  sessionCwdRef.current = sessionCwd;

  // Sync sessionCwd when projectRoot first becomes available
  const projectRootInitialized = useRef(false);
  useEffect(() => {
    if (projectRoot && !projectRootInitialized.current) {
      projectRootInitialized.current = true;
      setSessionCwd(projectRoot);
    }
  }, [projectRoot]);

  const messagesRef = useRef(messages);
  messagesRef.current = messages;

  const modelNameRef = useRef(modelName);
  modelNameRef.current = modelName;

  const projectRootRef = useRef(projectRoot);
  projectRootRef.current = projectRoot;

  const aiStreamRef = useRef<StreamBinding | null>(null);
  const agentStreamRef = useRef<StreamBinding | null>(null);
  const aiStopBlockRef = useRef<number | null>(null);

  const clearAiStream = useCallback(() => {
    const cur = aiStreamRef.current;
    if (cur) {
      clearTimeout(cur.safetyTimer);
      aiStreamRef.current = null;
    }
    aiStopBlockRef.current = null;
  }, []);

  const clearAgentStream = useCallback(() => {
    const cur = agentStreamRef.current;
    if (cur) {
      clearTimeout(cur.safetyTimer);
      agentStreamRef.current = null;
    }
  }, []);

  const processChatQueue = useCallback(() => {
    if (activeTab && activeTab.type !== "chat") {
      const chatTab = tabs.find((t) => t.type === "chat");
      if (chatTab) markUnread(chatTab.id);
    }
    const next = dequeue("chat");
    if (next) {
      executeChatTaskRef.current(next.content, next.mode);
    } else {
      setIsStreaming(false);
    }
  }, [activeTab, tabs, markUnread, dequeue]);

  const processChatQueueRef = useRef(processChatQueue);
  processChatQueueRef.current = processChatQueue;

  const allocId = useCallback(() => {
    const id = nextIdRef.current;
    nextIdRef.current += 1;
    return id;
  }, []);

  const executeChatTaskRef = useRef((_text: string, _mode: InputMode) => {
    /* set below */
  });

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
      const cwd = sessionCwdRef.current || projectRootRef.current || undefined;
      const startMs = Date.now();

      if (isTauri()) {
        try {
          const resultPromise = executeCommand({ command: text, cwd, track_cwd: true });
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
            const newCwd = result.final_cwd && result.final_cwd !== cwd ? result.final_cwd : null;
            if (newCwd) setSessionCwd(newCwd);
            output = parts.join("\n") || newCwd || "(no output)";
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
      const displayCwd = sessionCwdRef.current || cwd || "~";
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
            cwd: displayCwd,
            output: output.replace(/\n$/, ""),
            exitCode,
            durationMs,
            timedOut,
          },
        },
      ]);
      processChatQueueRef.current();
    },
    [allocId],
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
          const shellContext = recentShellContext(messagesRef.current, 5);
          const assistantId = allocId();
          const blockId = await sendAiMessage({
            message: text,
            model: modelNameRef.current || undefined,
            cwd: sessionCwdRef.current || projectRootRef.current || undefined,
            shell_context: shellContext.length > 0 ? shellContext : undefined,
          });
          const safetyTimer = setTimeout(() => {
            if (aiStreamRef.current?.backendBlockId === blockId) {
              clearAiStream();
              processChatQueueRef.current();
            }
          }, STREAM_SAFETY_TIMEOUT_MS);
          aiStreamRef.current = {
            backendBlockId: blockId,
            assistantMsgId: assistantId,
            safetyTimer,
          };
          aiStopBlockRef.current = blockId;
          setMessages((prev) => [
            ...prev,
            {
              id: assistantId,
              role: "assistant",
              mode: "ai",
              content: "",
              timestamp: Date.now(),
              model: modelNameRef.current || undefined,
            },
          ]);
          return;
        } catch {
          clearAiStream();
          setIsStreaming(false);
          processChatQueueRef.current();
          return;
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
        processChatQueueRef.current();
      }, 1200);
    },
    [allocId, clearAiStream],
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
          const shellContext = recentShellContext(messagesRef.current, 5);
          const assistantId = allocId();
          const blockId = await startAgent({
            goal: text,
            cwd: sessionCwdRef.current || projectRootRef.current || undefined,
            model: modelNameRef.current || undefined,
            shell_context: shellContext.length > 0 ? shellContext : undefined,
          });
          const safetyTimer = setTimeout(() => {
            if (agentStreamRef.current?.backendBlockId === blockId) {
              clearAgentStream();
              processChatQueueRef.current();
            }
          }, STREAM_SAFETY_TIMEOUT_MS);
          agentStreamRef.current = {
            backendBlockId: blockId,
            assistantMsgId: assistantId,
            safetyTimer,
          };
          setMessages((prev) => [
            ...prev,
            {
              id: assistantId,
              role: "assistant",
              mode: "agent",
              content: "",
              timestamp: Date.now(),
              model: modelNameRef.current || undefined,
            },
          ]);
          return;
        } catch {
          clearAgentStream();
          setIsStreaming(false);
          processChatQueueRef.current();
          return;
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
        processChatQueueRef.current();
      }, 1800);
    },
    [allocId, clearAgentStream],
  );

  const executeChatTask = useCallback(
    async (text: string, mode: InputMode) => {
      if (mode === "shell") {
        await _executeShell(text);
      } else if (mode === "ai") {
        await _executeAi(text);
      } else {
        await _executeAgent(text);
      }
    },
    [_executeAgent, _executeAi, _executeShell],
  );

  executeChatTaskRef.current = executeChatTask;

  useEffect(() => {
    if (!isTauri()) return;
    let unlistenAi: UnlistenFn | undefined;
    let unlistenAgent: UnlistenFn | undefined;
    const setup = async () => {
      unlistenAi = await listen<AiStreamPayload>("ai_stream_delta", (event) => {
        const p = event.payload;
        const cur = aiStreamRef.current;
        if (!cur || p.blockId !== cur.backendBlockId) return;
        if (p.done) {
          clearTimeout(cur.safetyTimer);
          aiStreamRef.current = null;
          aiStopBlockRef.current = null;
          processChatQueueRef.current();
          return;
        }
        if (p.delta) {
          if (p.kind === "thinking") {
            setMessages((prev) =>
              prev.map((m) =>
                m.id === cur.assistantMsgId ? { ...m, thinking: (m.thinking || "") + p.delta } : m,
              ),
            );
          } else {
            setMessages((prev) =>
              prev.map((m) =>
                m.id === cur.assistantMsgId ? { ...m, content: m.content + p.delta } : m,
              ),
            );
          }
        }
      });
      unlistenAgent = await listen<AgentStreamPayload>("agent_event", (event) => {
        const p = event.payload;
        const cur = agentStreamRef.current;
        if (!cur || p.blockId !== cur.backendBlockId) return;
        if (p.kind === "done") {
          clearTimeout(cur.safetyTimer);
          agentStreamRef.current = null;
          processChatQueueRef.current();
          return;
        }
        if (p.kind === "error" && p.data) {
          setMessages((prev) =>
            prev.map((m) =>
              m.id === cur.assistantMsgId ? { ...m, content: `**Error:** ${p.data}` } : m,
            ),
          );
          return;
        }
        if (p.kind === "text" && p.data) {
          setMessages((prev) =>
            prev.map((m) =>
              m.id === cur.assistantMsgId ? { ...m, content: m.content + p.data } : m,
            ),
          );
        }
      });
    };
    setup().catch((err) => {
      console.error("Failed to register Tauri event listeners:", err);
    });
    return () => {
      void unlistenAi?.();
      void unlistenAgent?.();
    };
  }, []);

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
    const aiBid = aiStopBlockRef.current;
    if (aiBid != null) {
      void stopAiStream(aiBid);
    }
    const agentBid = agentStreamRef.current?.backendBlockId;
    if (agentBid != null) {
      void stopAgent(agentBid);
    }
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
    sessionCwd,
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
