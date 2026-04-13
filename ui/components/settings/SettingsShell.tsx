import { useCallback, useEffect, useState } from "react";
import { useI18n } from "@/lib/i18n";
import type { AppSettings } from "@/lib/tauri";
import { getSettings, tryInvoke, updateSettings } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { AboutPage } from "./AboutPage";
import { AppearancePage } from "./AppearancePage";
import { CachePage } from "./CachePage";
import { NAV_ICONS, NAV_PAGES } from "./constants";
import { GatewayPage } from "./GatewayPage";
import { JsonPage } from "./JsonPage";
import { ModelsPage } from "./ModelsPage";
import { SafetyPage } from "./SafetyPage";
import { TerminalPage } from "./TerminalPage";
import type { SettingsPage } from "./types";
import { WebAiPage } from "./WebAiPage";

function useNavItems() {
  const { t } = useI18n();
  const labels: Record<string, string> = {
    appearance: t.settings.appearance,
    models: t.settings.modelsIntelligence,
    webai: t.settings.webAi,
    gateway: t.settings.clawGateway,
    terminal: t.settings.terminal,
    safety: t.settings.safety,
    cache: t.settings.cache,
    json: t.settings.json,
    about: t.settings.about,
  };
  return NAV_PAGES.map((id) => ({ id, label: labels[id] ?? id, icon: NAV_ICONS[id] }));
}

export function SettingsShell({
  page,
  setPage,
  dirty,
  settings,
  save,
}: {
  page: SettingsPage;
  setPage: (p: SettingsPage) => void;
  dirty: boolean;
  settings: AppSettings;
  save: (updates: Partial<AppSettings>) => Promise<void>;
}) {
  const { t } = useI18n();
  const navItems = useNavItems();

  return (
    <div className="flex h-full">
      <nav className="flex w-44 shrink-0 flex-col gap-0.5 border-r border-border p-2">
        {navItems.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            type="button"
            onClick={() => setPage(id)}
            className={cn(
              "flex items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors",
              page === id
                ? "bg-accent text-foreground"
                : "text-muted-foreground hover:bg-accent/50 hover:text-foreground",
            )}
          >
            <Icon className="h-3.5 w-3.5 shrink-0" />
            {label}
          </button>
        ))}
      </nav>

      <div className="flex-1 overflow-y-auto p-4">
        {dirty && (
          <div className="mb-3 rounded border bg-info-banner px-3 py-1.5 text-xs">
            {t.common.saving}
          </div>
        )}
        {page === "appearance" && <AppearancePage settings={settings} onSave={save} />}
        {page === "models" && <ModelsPage settings={settings} onSave={save} />}
        {page === "webai" && <WebAiPage />}
        {page === "gateway" && <GatewayPage settings={settings} onSave={save} />}
        {page === "terminal" && <TerminalPage settings={settings} onSave={save} />}
        {page === "safety" && <SafetyPage settings={settings} onSave={save} />}
        {page === "cache" && <CachePage />}
        {page === "json" && <JsonPage settings={settings} onSave={save} />}
        {page === "about" && <AboutPage />}
      </div>
    </div>
  );
}

export function SettingsPanel() {
  const [page, setPage] = useState<SettingsPage>("appearance");
  const [settings, setSettings] = useState<AppSettings>({});
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    tryInvoke(getSettings, {} as AppSettings).then(setSettings);
  }, []);

  const save = useCallback(
    async (updates: Partial<AppSettings>) => {
      const next = { ...settings, ...updates };
      setSettings(next);
      setDirty(true);
      try {
        await updateSettings(updates);
      } catch (err) {
        console.error("Failed to save settings:", err);
      } finally {
        setDirty(false);
      }
    },
    [settings],
  );

  return (
    <SettingsShell page={page} setPage={setPage} dirty={dirty} settings={settings} save={save} />
  );
}
