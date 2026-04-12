import type { Locale } from "@/lib/i18n";
import type { AppSettings } from "@/lib/tauri";

export type SettingsPage =
  | "appearance"
  | "models"
  | "capabilities"
  | "channels"
  | "terminal"
  | "memory"
  | "safety"
  | "cache"
  | "json"
  | "about";

export interface PageProps {
  settings: AppSettings;
  onSave: (updates: Partial<AppSettings>) => void;
}

export const PERMISSION_MODE_LABELS: Record<Locale, Record<string, string>> = {
  en: {
    "read-only": "Read Only — no file writes",
    "workspace-write": "Workspace Write — edit project files (default)",
    "auto-edit": "Auto Edit — apply changes without asking",
    "full-auto": "Full Auto — unrestricted (dangerous)",
  },
  "zh-CN": {
    "read-only": "只读 — 不可写文件",
    "workspace-write": "工作区写入 — 可编辑项目文件（默认）",
    "auto-edit": "自动编辑 — 不询问直接应用更改",
    "full-auto": "完全自动 — 无限制（危险）",
  },
};
