import { call } from "./call";

export interface TerminalSettings {
  shellPath?: string;
  shellArgs?: string[];
  env?: Record<string, string>;
  fontFamily?: string;
  fontSize?: number;
  cursorShape?: string;
  scrollbackLines?: number;
}

export interface CustomProviderConfig {
  baseUrl: string;
  apiVersion?: string;
  apiKey?: string;
  apiKeyEnv?: string;
  models: string[];
  defaultModel?: string;
  headers?: Record<string, string>;
}

export interface GatewaySettings {
  enabled?: boolean;
  listenAddr?: string;
  defaultModel?: string;
}

export interface SandboxConfig {
  enabled?: boolean;
  namespaceRestrictions?: boolean;
  networkIsolation?: boolean;
  filesystemMode?: string;
  allowedMounts?: string[];
}

export interface HooksConfig {
  preToolUse?: string[];
  postToolUse?: string[];
}

export interface CredentialConfig {
  defaultSource?: string;
  autoDiscover?: boolean;
  claudeCode?: { enabled?: boolean; configPath?: string };
}

export interface RulesConfig {
  autoInjectBudget?: number;
  rulesDir?: string;
  specsDir?: string;
  disableAutoInject?: boolean;
  disabledRules?: string[];
}

export interface AppSettings {
  theme?: string;
  fontSize?: number;
  language?: string;
  sessionRestore?: boolean;
  closeToTray?: boolean;
  terminal?: TerminalSettings;
  model?: string;
  modelAliases?: Record<string, string>;
  fallbackModels?: string[];
  thinkingMode?: boolean;
  providers?: Record<string, CustomProviderConfig>;
  gateway?: GatewaySettings;
  env?: Record<string, string>;
  credentials?: CredentialConfig;
  permissionMode?: string;
  hooks?: HooksConfig;
  enabledPlugins?: Record<string, boolean>;
  sandbox?: SandboxConfig;
  rules?: RulesConfig;
  autoCompact?: boolean;
  maxContextTokens?: number;
}

export interface AppInfo {
  name: string;
  version: string;
  versionSuffix: string;
  channel: string;
  displayName: string;
  githubUrl: string;
  homepage: string;
}

export interface ModelGroupData {
  provider: string;
  models: string[];
}

export const getAppInfo = () => call<AppInfo>("get_app_info");
export const getSettings = () => call<AppSettings>("get_settings");
export const getCloseToTray = () => call<boolean>("get_close_to_tray");
export const setCloseToTray = (enabled: boolean) => call<void>("set_close_to_tray", { enabled });
export const updateSettings = (updates: Partial<AppSettings>) =>
  call<void>("update_settings", { updates });
export const getApiKey = (provider: string) => call<string | null>("get_api_key", { provider });
export const setApiKey = (provider: string, key: string) =>
  call<void>("set_api_key", { provider, key });
export const listModelGroups = () => call<ModelGroupData[]>("list_model_groups");

// ── WebAI ──

export interface WebAiProviderInfo {
  id: string;
  name: string;
  models: WebAiModelInfo[];
}

export interface WebAiModelInfo {
  id: string;
  name: string;
  default: boolean;
}

export const webaiListProviders = () => call<WebAiProviderInfo[]>("webai_list_providers");
export const webaiListAuthenticated = () => call<string[]>("webai_list_authenticated");
export const webaiStartAuth = (providerId: string) =>
  call<string>("webai_start_auth", { providerId });
export const webaiLogout = (providerId: string) => call<void>("webai_logout", { providerId });
