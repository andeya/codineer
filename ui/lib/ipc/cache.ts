import { call } from "./call";

export interface CacheStats {
  attachmentsCount: number;
  attachmentsSizeBytes: number;
  historyCount: number;
  historySizeBytes: number;
  cachePath: string;
}

export interface ChatHistoryEntry {
  sessionId: string;
  sizeBytes: number;
  modifiedAt: number;
}

export const getCacheStats = () => call<CacheStats>("get_cache_stats");
export const saveAttachment = (name: string, dataBase64: string) =>
  call<string>("save_attachment", { name, dataBase64 });
export const clearCache = (target: "attachments" | "history" | "all") =>
  call<void>("clear_cache", { target });
export const listChatHistory = () => call<ChatHistoryEntry[]>("list_chat_history");
export const deleteChatHistory = (sessionId: string) =>
  call<void>("delete_chat_history", { sessionId });

export interface AutoCleanupConfig {
  interval: string;
  target: string;
  lastRunMs: number;
}

export const getAutoCleanup = () => call<AutoCleanupConfig>("get_auto_cleanup");
export const setAutoCleanup = (interval: string, target: string) =>
  call<void>("set_auto_cleanup", { interval, target });
