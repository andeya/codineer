import { call } from "./call";

export interface SpawnPtyRequest {
  shell?: string;
  command?: string;
  cwd?: string;
  cols: number;
  rows: number;
}

export interface PtyId {
  id: number;
}

export interface PtyOutputPayload {
  id: number;
  data: number[];
}

export interface PtyExitPayload {
  id: number;
  exit_code: number | null;
}

export interface ExecuteCommandRequest {
  command: string;
  cwd?: string;
  timeout_ms?: number;
}

export interface CommandOutput {
  stdout: string;
  stderr: string;
  exit_code: number;
  duration_ms: number;
  timed_out: boolean;
}

export const spawnPty = (req: SpawnPtyRequest) => call<PtyId>("spawn_pty", { request: req });
export const writePty = (id: number, data: number[]) => call<void>("write_pty", { id, data });
export const resizePty = (id: number, cols: number, rows: number) =>
  call<void>("resize_pty", { id, cols, rows });
export const killPty = (id: number) => call<void>("kill_pty", { id });

export async function listenPtyOutput(
  cb: (payload: PtyOutputPayload) => void,
): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<PtyOutputPayload>("pty_output", (e) => cb(e.payload));
}

export async function listenPtyExit(cb: (payload: PtyExitPayload) => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<PtyExitPayload>("pty_exit", (e) => cb(e.payload));
}

export const executeCommand = (req: ExecuteCommandRequest) =>
  call<CommandOutput>("execute_command", { request: req });

export interface CompletionItem {
  value: string;
  isDir: boolean;
}

export const shellComplete = (partial: string, cwd?: string, isFirstWord?: boolean) =>
  call<CompletionItem[]>("shell_complete", { partial, cwd, isFirstWord });
