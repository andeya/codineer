import { useCallback } from "react";
import { useI18n } from "@/lib/i18n";
import type { AppSettings } from "@/lib/tauri";
import { Field, NumberInput, Section, SettingsSelect, TextInput } from "./shared";
import type { PageProps } from "./types";

export function TerminalPage({ settings, onSave }: PageProps) {
  const { t } = useI18n();
  const term = settings.terminal ?? {};

  const update = useCallback(
    (patch: Partial<NonNullable<AppSettings["terminal"]>>) => {
      onSave({ terminal: { ...term, ...patch } });
    },
    [term, onSave],
  );

  return (
    <>
      <Section title={t.settings.shellSection}>
        <Field label={t.settings.shellPath} hint={t.settings.shellPathHint}>
          <TextInput
            value={term.shellPath ?? ""}
            onChange={(v) => update({ shellPath: v || undefined })}
            placeholder={t.settings.shellPathPlaceholder}
          />
        </Field>

        <Field label={t.settings.shellArgs}>
          <TextInput
            value={(term.shellArgs ?? []).join(" ")}
            onChange={(v) => update({ shellArgs: v.trim() ? v.split(/\s+/) : undefined })}
            placeholder={t.settings.shellArgsPlaceholder}
          />
        </Field>
      </Section>

      <Section title={t.settings.termAppearance}>
        <Field label={t.settings.fontFamily}>
          <TextInput
            value={term.fontFamily ?? ""}
            onChange={(v) => update({ fontFamily: v || undefined })}
            placeholder={t.settings.fontFamilyPlaceholder}
          />
        </Field>

        <Field label={t.settings.termFontSize}>
          <NumberInput
            value={term.fontSize ?? 13}
            onChange={(v) => update({ fontSize: v })}
            min={8}
            max={28}
            step={1}
          />
        </Field>

        <Field label={t.settings.cursorShape}>
          <SettingsSelect
            value={term.cursorShape ?? "block"}
            options={[
              { value: "block", label: t.settings.cursorBlock },
              { value: "bar", label: t.settings.cursorBar },
              { value: "underline", label: t.settings.cursorUnderline },
            ]}
            onChange={(v) => update({ cursorShape: v })}
          />
        </Field>
      </Section>

      <Section title={t.settings.buffer}>
        <Field label={t.settings.scrollbackLines} hint={t.settings.scrollbackHint}>
          <NumberInput
            value={term.scrollbackLines ?? 10000}
            onChange={(v) => update({ scrollbackLines: v })}
            min={100}
            max={100000}
            step={1000}
          />
        </Field>
      </Section>
    </>
  );
}
