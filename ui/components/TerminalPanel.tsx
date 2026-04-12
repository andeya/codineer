import { ChevronDown, ChevronUp, GripHorizontal, TerminalSquare, X } from "lucide-react";
import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from "react";
import { useI18n } from "@/lib/i18n";
import {
  isTauri,
  killPty,
  listenPtyExit,
  listenPtyOutput,
  resizePty,
  spawnPty,
  writePty,
} from "@/lib/tauri";
import { cn } from "@/lib/utils";

// ── Module preload cache ──────────────────────────────────────────
let _xtermP: Promise<typeof import("@xterm/xterm")> | undefined;
let _fitP: Promise<typeof import("@xterm/addon-fit")> | undefined;
let _webglP: Promise<typeof import("@xterm/addon-webgl")> | undefined;

function ensurePreloaded() {
  _xtermP ??= import("@xterm/xterm");
  _fitP ??= import("@xterm/addon-fit");
  _webglP ??= import("@xterm/addon-webgl").catch(() => null as never);
}

export function preloadTerminalModules() {
  ensurePreloaded();
}

function getXtermTheme() {
  const style = getComputedStyle(document.documentElement);
  const v = (name: string) => style.getPropertyValue(name).trim() || undefined;
  return {
    background: v("--terminal-bg") || v("--background") || "#1e2127",
    foreground: v("--terminal-fg") || v("--foreground") || "#abb2bf",
    cursor: v("--terminal-cursor") || v("--primary") || "#61afef",
    selectionBackground: v("--terminal-selection") || v("--accent") || "#3a3f4b",
  };
}

// ── Component ─────────────────────────────────────────────────────

const MIN_HEIGHT = 120;
const DEFAULT_HEIGHT = 200;
const HEADER_HEIGHT = 32;

interface TerminalPanelProps {
  visible: boolean;
  cwd?: string;
  onClose: () => void;
  onCommandDone?: (exitCode: number | null) => void;
}

export interface TerminalPanelHandle {
  /** Write raw text to the current shell PTY (for manual terminal usage). */
  writeCommand: (cmd: string) => void;
  /** Spawn a dedicated PTY for a command. PTY exits when command finishes. */
  runCommand: (cmd: string) => void;
  /** Kill current PTY and respawn a fresh interactive shell. */
  resetShell: () => void;
}

export const TerminalPanel = forwardRef<TerminalPanelHandle, TerminalPanelProps>(
  function TerminalPanel({ visible, cwd, onClose, onCommandDone }, ref) {
    const { t } = useI18n();
    const tRef = useRef(t);
    tRef.current = t;

    const containerRef = useRef<HTMLDivElement>(null);
    const termRef = useRef<import("@xterm/xterm").Terminal | null>(null);
    const fitRef = useRef<import("@xterm/addon-fit").FitAddon | null>(null);
    const ptyIdRef = useRef<number | null>(null);
    const outputBuf = useRef<Uint8Array[]>([]);
    const xtermReady = useRef(false);
    const xtermCreated = useRef(false);
    const listenersReady = useRef(false);
    const commandModeRef = useRef(false);
    const onCommandDoneRef = useRef(onCommandDone);
    const pendingRunRef = useRef<string | null>(null);
    const [exited, setExited] = useState(false);
    const [runningCmd, setRunningCmd] = useState<string | null>(null);
    const [collapsed, setCollapsed] = useState(false);
    const [height, setHeight] = useState(DEFAULT_HEIGHT);
    const dragStartRef = useRef<{ y: number; h: number } | null>(null);

    useEffect(() => {
      onCommandDoneRef.current = onCommandDone;
    }, [onCommandDone]);

    const toggleCollapsed = useCallback(() => {
      setCollapsed((c) => {
        if (c) {
          requestAnimationFrame(() => {
            fitRef.current?.fit();
            termRef.current?.focus();
            if (ptyIdRef.current != null && termRef.current) {
              resizePty(ptyIdRef.current, termRef.current.cols, termRef.current.rows).catch(
                () => {},
              );
            }
          });
        }
        return !c;
      });
    }, []);

    const handleDragStart = useCallback(
      (e: React.MouseEvent) => {
        e.preventDefault();
        dragStartRef.current = { y: e.clientY, h: height };

        const onMove = (ev: MouseEvent) => {
          if (!dragStartRef.current) return;
          const delta = dragStartRef.current.y - ev.clientY;
          const newH = Math.max(MIN_HEIGHT, dragStartRef.current.h + delta);
          setHeight(newH);
          setCollapsed(false);
        };

        const onUp = () => {
          dragStartRef.current = null;
          window.removeEventListener("mousemove", onMove);
          window.removeEventListener("mouseup", onUp);
          requestAnimationFrame(() => {
            fitRef.current?.fit();
            if (ptyIdRef.current != null && termRef.current) {
              resizePty(ptyIdRef.current, termRef.current.cols, termRef.current.rows).catch(
                () => {},
              );
            }
          });
        };

        window.addEventListener("mousemove", onMove);
        window.addEventListener("mouseup", onUp);
      },
      [height],
    );

    const spawnAndTrack = useCallback(
      async (command?: string) => {
        const cols = termRef.current?.cols ?? 80;
        const rows = termRef.current?.rows ?? 24;
        const { id } = await spawnPty({ command, cwd, cols, rows });
        ptyIdRef.current = id;
        setExited(false);
        if (termRef.current) {
          resizePty(id, termRef.current.cols, termRef.current.rows).catch(() => {});
        }
        return id;
      },
      [cwd],
    );

    const doRunCommand = useCallback(
      async (cmd: string) => {
        if (ptyIdRef.current != null) {
          killPty(ptyIdRef.current).catch(() => {});
          ptyIdRef.current = null;
        }
        commandModeRef.current = true;
        setRunningCmd(cmd);
        setExited(false);

        if (termRef.current) {
          termRef.current.write(`\r\n\x1b[36m\u276f ${cmd}\x1b[0m\r\n`);
        }

        try {
          await spawnAndTrack(cmd);
        } catch (err) {
          if (termRef.current) {
            const msg = tRef.current.terminalPanel.failedToStart.replace("{error}", String(err));
            termRef.current.write(`\r\n\x1b[31m${msg}\x1b[0m\r\n`);
          }
          commandModeRef.current = false;
          setRunningCmd(null);
          onCommandDoneRef.current?.(1);
        }
      },
      [spawnAndTrack],
    );

    const doResetShell = useCallback(async () => {
      if (ptyIdRef.current != null) {
        killPty(ptyIdRef.current).catch(() => {});
        ptyIdRef.current = null;
      }
      commandModeRef.current = false;
      setRunningCmd(null);
      setExited(false);

      try {
        await spawnAndTrack();
      } catch (err) {
        if (termRef.current) {
          const msg = tRef.current.terminalPanel.failedToStartShell.replace("{error}", String(err));
          termRef.current.write(`\r\n\x1b[31m${msg}\x1b[0m\r\n`);
        }
      }
    }, [spawnAndTrack]);

    useImperativeHandle(ref, () => ({
      writeCommand(cmd: string) {
        if (ptyIdRef.current != null && !commandModeRef.current) {
          writePty(ptyIdRef.current, Array.from(new TextEncoder().encode(`${cmd}\n`))).catch(
            () => {},
          );
        }
      },
      runCommand(cmd: string) {
        if (!listenersReady.current) {
          pendingRunRef.current = cmd;
          return;
        }
        doRunCommand(cmd);
      },
      resetShell() {
        if (!listenersReady.current) return;
        doResetShell();
      },
    }));

    // ── Phase 1: Set up global PTY listeners + spawn initial PTY ──
    useEffect(() => {
      if (!isTauri()) return;

      let cancelled = false;
      const cleanups: (() => void)[] = [];

      (async () => {
        ensurePreloaded();

        const [unlistenOut, unlistenExit] = await Promise.all([
          listenPtyOutput((p) => {
            if (p.id !== ptyIdRef.current) return;
            const data = new Uint8Array(p.data);
            if (xtermReady.current && termRef.current) {
              termRef.current.write(data);
            } else {
              outputBuf.current.push(data);
            }
          }),
          listenPtyExit((p) => {
            if (p.id !== ptyIdRef.current) return;

            if (commandModeRef.current) {
              const code = p.exit_code;
              const tp = tRef.current.terminalPanel;
              const exitMsg =
                code != null
                  ? tp.exitedBracketWithCode.replace("{code}", String(code))
                  : tp.exitedBracket;
              termRef.current?.write(`\r\n\x1b[90m${exitMsg}\x1b[0m\r\n`);
              commandModeRef.current = false;
              setRunningCmd(null);
              onCommandDoneRef.current?.(code);
            } else {
              const msg = tRef.current.terminalPanel.processExited;
              termRef.current?.write(`\r\n\x1b[90m${msg}\x1b[0m\r\n`);
              setExited(true);
            }
          }),
        ]);

        if (cancelled) {
          unlistenOut();
          unlistenExit();
          return;
        }

        cleanups.push(unlistenOut, unlistenExit);
        listenersReady.current = true;

        // Process pending command or spawn default shell
        if (pendingRunRef.current) {
          const cmd = pendingRunRef.current;
          pendingRunRef.current = null;
          commandModeRef.current = true;
          setRunningCmd(cmd);
          try {
            await spawnAndTrack(cmd);
          } catch {
            commandModeRef.current = false;
            setRunningCmd(null);
            onCommandDoneRef.current?.(1);
          }
        } else {
          try {
            await spawnAndTrack();
          } catch {
            /* shell spawn failed — user can retry via resetShell */
          }
        }
      })();

      return () => {
        cancelled = true;
        for (const fn of cleanups) fn();
        if (ptyIdRef.current != null) {
          killPty(ptyIdRef.current).catch(() => {});
          ptyIdRef.current = null;
        }
        outputBuf.current = [];
        xtermReady.current = false;
        listenersReady.current = false;
        commandModeRef.current = false;
        setExited(false);
        setRunningCmd(null);
      };
    }, [spawnAndTrack]);

    // ── Phase 2: Create xterm on first visible ──
    const initXterm = useCallback(async () => {
      if (xtermCreated.current || !containerRef.current) return;
      xtermCreated.current = true;

      ensurePreloaded();
      if (!_xtermP || !_fitP) return;
      const [{ Terminal }, { FitAddon }] = await Promise.all([_xtermP, _fitP]);

      if (!containerRef.current) return;

      const term = new Terminal({
        cursorBlink: true,
        fontSize: 13,
        fontFamily: '"Berkeley Mono", "JetBrains Mono", Menlo, Consolas, monospace',
        theme: getXtermTheme(),
        allowProposedApi: true,
      });

      const fitAddon = new FitAddon();
      term.loadAddon(fitAddon);
      term.open(containerRef.current);
      fitAddon.fit();

      termRef.current = term;
      fitRef.current = fitAddon;

      // Replay buffered output
      for (const chunk of outputBuf.current) {
        term.write(chunk);
      }
      outputBuf.current = [];
      xtermReady.current = true;

      // Forward keyboard to PTY
      if (isTauri()) {
        term.onData((data) => {
          if (ptyIdRef.current != null) {
            writePty(ptyIdRef.current, Array.from(new TextEncoder().encode(data))).catch(() => {});
          }
        });
      } else {
        term.write(`${tRef.current.terminalPanel.mockMode}\r\n$ `);
        term.onData((data) => term.write(data));
      }

      // Sync size to PTY
      if (ptyIdRef.current != null) {
        resizePty(ptyIdRef.current, term.cols, term.rows).catch(() => {});
      }

      // WebGL non-blocking
      _webglP?.then((mod) => {
        if (!mod || !termRef.current) return;
        try {
          const webgl = new mod.WebglAddon();
          webgl.onContextLoss(() => webgl.dispose());
          termRef.current.loadAddon(webgl);
        } catch {
          /* fallback to canvas */
        }
      });

      term.focus();
    }, []);

    // When visible changes, init xterm (first time) or re-fit + focus
    useEffect(() => {
      if (!visible) return;
      if (!xtermCreated.current) {
        initXterm();
      } else {
        requestAnimationFrame(() => {
          fitRef.current?.fit();
          termRef.current?.focus();
          if (ptyIdRef.current != null && termRef.current) {
            resizePty(ptyIdRef.current, termRef.current.cols, termRef.current.rows).catch(() => {});
          }
        });
      }
    }, [visible, initXterm]);

    // Resize observer (active when visible)
    useEffect(() => {
      if (!visible || !containerRef.current) return;
      const container = containerRef.current;
      const ro = new ResizeObserver(() => {
        const term = termRef.current;
        const fit = fitRef.current;
        if (!term || !fit) return;
        try {
          fit.fit();
          if (ptyIdRef.current != null) {
            resizePty(ptyIdRef.current, term.cols, term.rows).catch(() => {});
          }
        } catch {
          /* ignore */
        }
      });
      ro.observe(container);
      return () => ro.disconnect();
    }, [visible]);

    // Cleanup xterm on component unmount
    useEffect(() => {
      return () => {
        termRef.current?.dispose();
        termRef.current = null;
        fitRef.current = null;
        xtermCreated.current = false;
        xtermReady.current = false;
      };
    }, []);

    return (
      <div
        style={{ display: visible ? undefined : "none" }}
        className="flex flex-col border-t border-border"
      >
        {/* Drag handle */}
        {!collapsed && (
          // biome-ignore lint/a11y/noStaticElementInteractions: drag-resize handle
          <div
            onMouseDown={handleDragStart}
            className="group flex h-1.5 cursor-row-resize items-center justify-center drag-handle-hover"
          >
            <GripHorizontal className="h-3 w-3 text-transparent transition-colors group-hover:text-muted-foreground" />
          </div>
        )}

        {/* Header bar — click to toggle collapse */}
        <div
          className="flex shrink-0 items-center gap-2 border-b border-border bg-card/50 px-3"
          style={{ height: HEADER_HEIGHT }}
        >
          <button
            type="button"
            onClick={toggleCollapsed}
            className="flex items-center gap-2 text-muted-foreground hover:text-foreground"
          >
            {collapsed ? (
              <ChevronUp className="h-3.5 w-3.5" />
            ) : (
              <ChevronDown className="h-3.5 w-3.5" />
            )}
            <TerminalSquare className="h-3.5 w-3.5" />
            <span className="text-xs font-medium">
              {runningCmd
                ? t.terminalPanel.running.replace("{cmd}", runningCmd)
                : t.terminalPanel.terminal}
            </span>
          </button>
          {exited && <span className="text-[10px] text-destructive">{t.terminalPanel.exited}</span>}
          <div className="flex-1" />
          <button
            type="button"
            onClick={onClose}
            className="flex h-5 w-5 items-center justify-center rounded text-muted-foreground hover:bg-accent hover:text-foreground"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>

        {/* Terminal body — hidden when collapsed */}
        <div
          ref={containerRef}
          className={cn("bg-background px-1 py-1", collapsed && "hidden")}
          style={{ height }}
        />
      </div>
    );
  },
);
