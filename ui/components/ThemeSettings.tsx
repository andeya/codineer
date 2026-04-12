import { Monitor, Moon, Sun } from "lucide-react";
import { useCallback, useState } from "react";
import { useI18n } from "@/lib/i18n";
import {
  DARK_THEMES,
  getStoredConfig,
  LIGHT_THEMES,
  setThemeConfig,
  type ThemeConfig,
  type ThemeMode,
} from "@/lib/theme";
import { cn } from "@/lib/utils";

const THEME_LABEL: Record<string, string> = {
  "github-light": "themeGithubLight",
  "solarized-light": "themeSolarizedLight",
  "one-dark-pro": "themeOneDarkPro",
  dracula: "themeDracula",
};

export function ThemeSettings() {
  const { t } = useI18n();
  const [cfg, setCfg] = useState<ThemeConfig>(getStoredConfig);

  const update = useCallback((patch: Partial<ThemeConfig>) => {
    setCfg((prev) => {
      const next = { ...prev, ...patch };
      setThemeConfig(next);
      return next;
    });
  }, []);

  const modes: { id: ThemeMode; label: string; icon: typeof Monitor }[] = [
    { id: "light", label: t.settings.themeLight, icon: Sun },
    { id: "dark", label: t.settings.themeDark, icon: Moon },
    { id: "system", label: t.settings.themeSystem, icon: Monitor },
  ];

  return (
    <div className="space-y-3 p-3">
      <h3 className="text-xs font-semibold uppercase tracking-wider text-foreground">
        {t.settings.appearance}
      </h3>

      <div className="flex gap-1 rounded-lg border border-border p-0.5">
        {modes.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            type="button"
            onClick={() => update({ mode: id })}
            className={cn(
              "flex flex-1 items-center justify-center gap-1.5 rounded-md px-2 py-1.5 text-[11px] transition-colors",
              cfg.mode === id
                ? "bg-accent font-medium text-foreground"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            <Icon className="h-3.5 w-3.5" />
            {label}
          </button>
        ))}
      </div>

      <div>
        <h4 className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          {t.settings.themeLight}
        </h4>
        <div className="grid grid-cols-2 gap-1.5">
          {LIGHT_THEMES.map((th) => (
            <button
              key={th.id}
              type="button"
              onClick={() => update({ light: th.id })}
              className={cn(
                "flex flex-col items-center gap-1 overflow-hidden rounded-lg border text-xs transition-all",
                cfg.light === th.id
                  ? "border-primary ring-2 ring-primary/30"
                  : "border-border hover:border-muted-foreground/50",
              )}
            >
              <div
                className="flex h-10 w-full items-end gap-0.5 p-1"
                style={{ background: th.preview[0] }}
              >
                <div className="h-full w-2.5 rounded-sm" style={{ background: th.preview[1] }} />
                <div className="flex flex-1 flex-col gap-0.5">
                  <div className="h-1 w-3/4 rounded-full" style={{ background: th.preview[2] }} />
                  <div
                    className="h-0.5 w-1/2 rounded-full opacity-40"
                    style={{ background: th.preview[3] }}
                  />
                </div>
              </div>
              <span className="pb-1 text-[10px] text-muted-foreground">
                {(t.settings as Record<string, string>)[THEME_LABEL[th.id]] ?? th.id}
              </span>
            </button>
          ))}
        </div>
      </div>

      <div>
        <h4 className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          {t.settings.themeDark}
        </h4>
        <div className="grid grid-cols-2 gap-1.5">
          {DARK_THEMES.map((th) => (
            <button
              key={th.id}
              type="button"
              onClick={() => update({ dark: th.id })}
              className={cn(
                "flex flex-col items-center gap-1 overflow-hidden rounded-lg border text-xs transition-all",
                cfg.dark === th.id
                  ? "border-primary ring-2 ring-primary/30"
                  : "border-border hover:border-muted-foreground/50",
              )}
            >
              <div
                className="flex h-10 w-full items-end gap-0.5 p-1"
                style={{ background: th.preview[0] }}
              >
                <div className="h-full w-2.5 rounded-sm" style={{ background: th.preview[1] }} />
                <div className="flex flex-1 flex-col gap-0.5">
                  <div className="h-1 w-3/4 rounded-full" style={{ background: th.preview[2] }} />
                  <div
                    className="h-0.5 w-1/2 rounded-full opacity-40"
                    style={{ background: th.preview[3] }}
                  />
                </div>
              </div>
              <span className="pb-1 text-[10px] text-muted-foreground">
                {(t.settings as Record<string, string>)[THEME_LABEL[th.id]] ?? th.id}
              </span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
