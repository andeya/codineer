import {
  Braces,
  Database,
  Globe,
  Info,
  Network,
  Palette,
  Shield,
  Sparkles,
  Terminal,
} from "lucide-react";
import type { SettingsPage } from "./types";

export const NAV_ICONS: Record<SettingsPage, typeof Palette> = {
  appearance: Palette,
  models: Sparkles,
  capabilities: Shield,
  channels: Globe,
  terminal: Terminal,
  memory: Database,
  safety: Shield,
  cache: Database,
  json: Braces,
  about: Info,
  webai: Globe,
  gateway: Network,
};

export const NAV_PAGES: SettingsPage[] = [
  "appearance",
  "models",
  "webai",
  "gateway",
  "terminal",
  "safety",
  "cache",
  "json",
  "about",
];

export const THEME_LABEL: Record<string, string> = {
  "github-light": "themeGithubLight",
  "solarized-light": "themeSolarizedLight",
  "one-dark-pro": "themeOneDarkPro",
  dracula: "themeDracula",
};

export const BUILTIN_PROVIDERS = [
  { id: "anthropic", name: "Anthropic", envKey: "ANTHROPIC_API_KEY" },
  { id: "openai", name: "OpenAI", envKey: "OPENAI_API_KEY" },
  { id: "google", name: "Google (Gemini)", envKey: "GEMINI_API_KEY" },
  { id: "xai", name: "xAI (Grok)", envKey: "XAI_API_KEY" },
  { id: "deepseek", name: "DeepSeek", envKey: "DEEPSEEK_API_KEY" },
  { id: "ollama", name: "Ollama", envKey: "" },
  { id: "openrouter", name: "OpenRouter", envKey: "OPENROUTER_API_KEY" },
  { id: "groq", name: "Groq", envKey: "GROQ_API_KEY" },
] as const;
