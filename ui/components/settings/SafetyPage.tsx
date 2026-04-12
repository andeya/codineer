import { useI18n } from "@/lib/i18n";
import { Field, Section, SettingsSelect, Toggle } from "./shared";
import { type PageProps, PERMISSION_MODE_LABELS } from "./types";

export function SafetyPage({ settings, onSave }: PageProps) {
  const { t, locale } = useI18n();
  const sandbox = settings.sandbox ?? {};
  const permissionLabels = PERMISSION_MODE_LABELS[locale];

  return (
    <>
      <Section title={t.settings.permissionMode}>
        <Field label={t.settings.defaultPermission} hint={t.settings.defaultPermissionHint}>
          <SettingsSelect
            value={settings.permissionMode ?? "workspace-write"}
            options={[
              { value: "read-only", label: permissionLabels["read-only"] },
              { value: "workspace-write", label: permissionLabels["workspace-write"] },
              { value: "auto-edit", label: permissionLabels["auto-edit"] },
              { value: "full-auto", label: permissionLabels["full-auto"] },
            ]}
            onChange={(v) => onSave({ permissionMode: v })}
          />
        </Field>
      </Section>

      <Section title={t.settings.sandbox}>
        <Field label={t.settings.enableSandbox}>
          <div className="flex items-center gap-2">
            <Toggle
              checked={sandbox.enabled ?? false}
              onChange={(v) => onSave({ sandbox: { ...sandbox, enabled: v } })}
            />
            <span className="text-[10px] text-muted-foreground">
              {t.settings.enableSandboxHint}
            </span>
          </div>
        </Field>

        {sandbox.enabled && (
          <>
            <Field label={t.settings.filesystemMode}>
              <SettingsSelect
                value={sandbox.filesystemMode ?? "workspace-only"}
                options={[
                  { value: "workspace-only", label: t.settings.workspaceOnly },
                  { value: "full", label: t.settings.fullAccess },
                ]}
                onChange={(v) => onSave({ sandbox: { ...sandbox, filesystemMode: v } })}
              />
            </Field>

            <Field label={t.settings.networkIsolation}>
              <Toggle
                checked={sandbox.networkIsolation ?? false}
                onChange={(v) => onSave({ sandbox: { ...sandbox, networkIsolation: v } })}
              />
            </Field>
          </>
        )}
      </Section>

      <Section title={t.settings.credentials}>
        <Field label={t.settings.autoDiscoverCreds}>
          <Toggle
            checked={settings.credentials?.autoDiscover ?? true}
            onChange={(v) =>
              onSave({
                credentials: {
                  ...(settings.credentials ?? {}),
                  autoDiscover: v,
                },
              })
            }
          />
        </Field>
      </Section>
    </>
  );
}
