export type ConcreteThemeId = "github-light" | "solarized-light" | "one-dark-pro" | "dracula";

export interface ThemePreset {
  id: ConcreteThemeId;
  isDark: boolean;
  shikiTheme: string;
  /** Preview colors: [bg, sidebar, accent, text] */
  preview: [string, string, string, string];
}

export const LIGHT_THEMES: ThemePreset[] = [
  {
    id: "github-light",
    isDark: false,
    shikiTheme: "github-light",
    preview: ["#ffffff", "#f6f8fa", "#0969da", "#1f2328"],
  },
  {
    id: "solarized-light",
    isDark: false,
    shikiTheme: "solarized-light",
    preview: ["#fdf6e3", "#eee8d5", "#268bd2", "#073642"],
  },
];

export const DARK_THEMES: ThemePreset[] = [
  {
    id: "dracula",
    isDark: true,
    shikiTheme: "dracula",
    preview: ["#282a36", "#21222c", "#bd93f9", "#f8f8f2"],
  },
  {
    id: "one-dark-pro",
    isDark: true,
    shikiTheme: "one-dark-pro",
    preview: ["#282c34", "#21252b", "#61afef", "#abb2bf"],
  },
];

export const ALL_THEMES: ThemePreset[] = [...LIGHT_THEMES, ...DARK_THEMES];

export const THEME_MAP: Record<ConcreteThemeId, ThemePreset> = Object.fromEntries(
  ALL_THEMES.map((t) => [t.id, t]),
) as Record<ConcreteThemeId, ThemePreset>;

export type ThemeMode = "light" | "dark" | "system";

export interface ThemeConfig {
  mode: ThemeMode;
  light: ConcreteThemeId;
  dark: ConcreteThemeId;
}

const STORAGE_KEY = "aineer-theme-config";
const LEGACY_KEY = "aineer-theme";

const DEFAULT_CONFIG: ThemeConfig = {
  mode: "system",
  light: "github-light",
  dark: "dracula",
};

function getSystemPreference(): "light" | "dark" {
  if (typeof window === "undefined") return "dark";
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function resolveConcreteId(cfg: ThemeConfig): ConcreteThemeId {
  if (cfg.mode === "light") return cfg.light;
  if (cfg.mode === "dark") return cfg.dark;
  return getSystemPreference() === "dark" ? cfg.dark : cfg.light;
}

function applyFavicon(isDark: boolean) {
  const link = document.getElementById("favicon") as HTMLLinkElement | null;
  if (link) link.href = isDark ? "/logo-dark.svg" : "/logo-light.svg";
}

function applyTheme(id: ConcreteThemeId) {
  const root = document.documentElement;
  const preset = THEME_MAP[id];

  for (const t of ALL_THEMES) {
    root.classList.remove(`theme-${t.id}`);
  }
  root.classList.add(`theme-${id}`);

  if (preset.isDark) {
    root.classList.add("dark");
  } else {
    root.classList.remove("dark");
  }

  applyFavicon(preset.isDark);
}

export function getStoredConfig(): ThemeConfig {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return JSON.parse(raw) as ThemeConfig;

    const legacy = localStorage.getItem(LEGACY_KEY);
    if (legacy === "system") return DEFAULT_CONFIG;
    if (legacy && legacy in THEME_MAP) {
      const preset = THEME_MAP[legacy as ConcreteThemeId];
      return {
        mode: preset.isDark ? "dark" : "light",
        light: preset.isDark ? DEFAULT_CONFIG.light : (legacy as ConcreteThemeId),
        dark: preset.isDark ? (legacy as ConcreteThemeId) : DEFAULT_CONFIG.dark,
      };
    }
  } catch {
    /* noop */
  }
  return DEFAULT_CONFIG;
}

export function setThemeConfig(cfg: ThemeConfig) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(cfg));
  } catch {
    /* noop */
  }
  applyTheme(resolveConcreteId(cfg));
}

export function getActivePreset(): ThemePreset {
  const id = resolveConcreteId(getStoredConfig());
  return THEME_MAP[id];
}

export function initTheme() {
  const cfg = getStoredConfig();
  applyTheme(resolveConcreteId(cfg));

  if (cfg.mode === "system") {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => applyTheme(resolveConcreteId(getStoredConfig()));
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }
}

// Compat shims used by SettingsPanel.onSave({ theme: ... })
export type ThemeId = ConcreteThemeId | "system";
export type Theme = ThemeId;

export function getStoredTheme(): ThemeId {
  const cfg = getStoredConfig();
  return cfg.mode === "system" ? "system" : resolveConcreteId(cfg);
}

export function setTheme(theme: ThemeId) {
  const cfg = getStoredConfig();
  if (theme === "system") {
    setThemeConfig({ ...cfg, mode: "system" });
  } else {
    const preset = THEME_MAP[theme];
    setThemeConfig({
      ...cfg,
      mode: preset.isDark ? "dark" : "light",
      [preset.isDark ? "dark" : "light"]: theme,
    });
  }
}

export function resolveTheme(theme: ThemeId): ConcreteThemeId {
  if (theme === "system") return resolveConcreteId(getStoredConfig());
  return theme;
}
