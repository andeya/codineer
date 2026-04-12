export type { AgentRequest, AiMessageRequest, ShellContextSnippet } from "./ai";
export {
  approveTool,
  denyTool,
  sendAiMessage,
  startAgent,
  stopAgent,
  stopAiStream,
} from "./ai";
export type { AutoCleanupConfig, CacheStats, ChatHistoryEntry } from "./cache";
export {
  clearCache,
  deleteChatHistory,
  getAutoCleanup,
  getCacheStats,
  listChatHistory,
  saveAttachment,
  setAutoCleanup,
} from "./cache";
export { isTauri, tryInvoke } from "./call";
export type { ContentMatch, FileEntry, SearchResult } from "./files";
export { getProjectRoot, listDir, readFile, searchFiles } from "./files";
export type { GitBranchInfo, GitFileStatus, GitStatus } from "./git";
export { gitBranch, gitCheckout, gitDiff, gitListBranches, gitStatus } from "./git";
export type {
  ChannelAdapterInfo,
  GatewayStatusInfo,
  LspCompletionItem,
  LspDiagnosticItem,
  LspHoverInfo,
  McpServerInfo,
  McpToolCallRequest,
  MemoryEntryInfo,
  PluginInfo,
  SessionInfo,
  SlashCommandDef,
  UpdateCheckResult,
} from "./misc";
export {
  callMcpTool,
  checkForUpdate,
  executeSlashCommand,
  forget,
  getGatewayStatus,
  getSlashCommands,
  getUpdateChannel,
  installPlugin,
  listChannelAdapters,
  listMcpServers,
  listPlugins,
  listSessions,
  loadSession,
  lspCompletions,
  lspDiagnostics,
  lspHover,
  remember,
  saveSession,
  searchMemory,
  startGateway,
  startMcpServer,
  stopGateway,
  stopMcpServer,
  uninstallPlugin,
} from "./misc";
export type {
  AppInfo,
  AppSettings,
  CredentialConfig,
  CustomProviderConfig,
  GatewaySettings,
  HooksConfig,
  ModelGroupData,
  RulesConfig,
  SandboxConfig,
  TerminalSettings,
} from "./settings";
export {
  getApiKey,
  getAppInfo,
  getCloseToTray,
  getSettings,
  listModelGroups,
  setApiKey,
  setCloseToTray,
  updateSettings,
} from "./settings";
export type {
  CommandOutput,
  CompletionItem,
  ExecuteCommandRequest,
  PtyExitPayload,
  PtyId,
  PtyOutputPayload,
  SpawnPtyRequest,
} from "./shell";
export {
  executeCommand,
  killPty,
  listenPtyExit,
  listenPtyOutput,
  resizePty,
  shellComplete,
  spawnPty,
  writePty,
} from "./shell";
