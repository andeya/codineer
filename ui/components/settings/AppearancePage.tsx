import { Monitor, Moon, Sun } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Select } from "@/components/ui/select";
import type { Locale } from "@/lib/i18n";
import { useI18n } from "@/lib/i18n";
import type { AppSettings } from "@/lib/tauri";
import { getCloseToTray, setCloseToTray, tryInvoke } from "@/lib/tauri";
import {
  DARK_THEMES,
  getStoredConfig,
  LIGHT_THEMES,
  setThemeConfig,
  type ThemeConfig,
  type ThemeMode,
} from "@/lib/theme";
import { cn } from "@/lib/utils";
import { THEME_LABEL } from "./constants";
import { Field, NumberInput, Section, Toggle } from "./shared";
import type { PageProps } from "./types";

function LanguageField({ settings, onSave }: PageProps) {
  const { t, setLocale } = useI18n();
  return (
    <Field label={t.settings.language}>
      <Select
        value={settings.language ?? "en"}
        options={[
          { value: "en", label: t.settings.langEn },
          { value: "zh-CN", label: t.settings.langZh },
        ]}
        onChange={(v) => {
          onSave({ language: v });
          setLocale(v as Locale);
        }}
      />
    </Field>
  );
}

function CloseToTrayToggle() {
  const [enabled, setEnabled] = useState(true);

  useEffect(() => {
    tryInvoke(getCloseToTray, true).then(setEnabled);
  }, []);

  const handleChange = useCallback(async (v: boolean) => {
    setEnabled(v);
    try {
      await setCloseToTray(v);
    } catch (err) {
      console.error("Failed to set close_to_tray:", err);
    }
  }, []);

  return <Toggle checked={enabled} onChange={handleChange} />;
}

function ThemePreviewCard({
  preview,
  label,
  selected,
  onClick,
}: {
  preview: [string, string, string, string];
  label: string;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex flex-col items-center gap-1 overflow-hidden rounded-lg border text-xs transition-all",
        selected
          ? "border-primary ring-2 ring-primary/30"
          : "border-border hover:border-muted-foreground/50",
      )}
    >
      <div className="flex h-12 w-full items-end gap-0.5 p-1.5" style={{ background: preview[0] }}>
        <div className="h-full w-3 rounded-sm" style={{ background: preview[1] }} />
        <div className="flex flex-1 flex-col gap-0.5">
          <div className="h-1.5 w-3/4 rounded-full" style={{ background: preview[2] }} />
          <div className="h-1 w-1/2 rounded-full opacity-40" style={{ background: preview[3] }} />
          <div className="h-1 w-2/3 rounded-full opacity-25" style={{ background: preview[3] }} />
        </div>
      </div>
      <span
        className={cn(
          "pb-1 text-[10px]",
          selected ? "font-medium text-foreground" : "text-muted-foreground",
        )}
      >
        {label}
      </span>
    </button>
  );
}

function ThemePickerInline({ onSave }: { onSave: (patch: Partial<AppSettings>) => void }) {
  const { t } = useI18n();
  const [cfg, setCfg] = useState<ThemeConfig>(getStoredConfig);

  const update = useCallback(
    (patch: Partial<ThemeConfig>) => {
      const next = { ...cfg, ...patch };
      setCfg(next);
      setThemeConfig(next);
      onSave({
        theme: next.mode === "system" ? "system" : (patch.light ?? patch.dark ?? cfg.light),
      });
    },
    [cfg, onSave],
  );

  const modes: { id: ThemeMode; label: string; icon: typeof Monitor }[] = [
    { id: "light", label: t.settings.themeLight, icon: Sun },
    { id: "dark", label: t.settings.themeDark, icon: Moon },
    { id: "system", label: t.settings.themeSystem, icon: Monitor },
  ];

  return (
    <div className="space-y-3">
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
          {cfg.mode === "system" && (
            <span className="ml-1 font-normal normal-case text-muted-foreground/70">
              ({t.settings.themeSystemDay})
            </span>
          )}
        </h4>
        <div className="grid grid-cols-2 gap-2">
          {LIGHT_THEMES.map((th) => (
            <ThemePreviewCard
              key={th.id}
              preview={th.preview}
              label={(t.settings as Record<string, string>)[THEME_LABEL[th.id]] ?? th.id}
              selected={cfg.light === th.id}
              onClick={() => update({ light: th.id })}
            />
          ))}
        </div>
      </div>

      <div>
        <h4 className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          {t.settings.themeDark}
          {cfg.mode === "system" && (
            <span className="ml-1 font-normal normal-case text-muted-foreground/70">
              ({t.settings.themeSystemNight})
            </span>
          )}
        </h4>
        <div className="grid grid-cols-2 gap-2">
          {DARK_THEMES.map((th) => (
            <ThemePreviewCard
              key={th.id}
              preview={th.preview}
              label={(t.settings as Record<string, string>)[THEME_LABEL[th.id]] ?? th.id}
              selected={cfg.dark === th.id}
              onClick={() => update({ dark: th.id })}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

export function AppearancePage({ settings, onSave }: PageProps) {
  const { t } = useI18n();

  return (
    <>
      <Section title={t.settings.theme}>
        <ThemePickerInline onSave={onSave} />
      </Section>

      <Section title={t.settings.interface}>
        <Field label={t.settings.fontSize} hint={t.settings.fontSizeHint}>
          <NumberInput
            value={settings.fontSize ?? 13}
            onChange={(v) => onSave({ fontSize: v })}
            min={10}
            max={24}
            step={1}
          />
        </Field>

        <LanguageField settings={settings} onSave={onSave} />

        <Field label={t.settings.restoreSession}>
          <Toggle
            checked={settings.sessionRestore ?? true}
            onChange={(v) => onSave({ sessionRestore: v })}
          />
        </Field>
      </Section>

      <Section title={t.settings.systemTray}>
        <Field label={t.settings.closeToTray} hint={t.settings.closeToTrayHint}>
          <CloseToTrayToggle />
        </Field>
      </Section>
    </>
  );
}
