import { ChevronRight, Globe, Key, Plus, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Select } from "@/components/ui/select";
import { useI18n } from "@/lib/i18n";
import { modelGroupsToSelectOptions, withCurrentModelOption } from "@/lib/model-options";
import { getApiKey, listModelGroups, type ModelGroupData, setApiKey } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { BUILTIN_PROVIDERS } from "./constants";
import { Field, NumberInput, Section, TextInput, Toggle } from "./shared";
import type { PageProps } from "./types";

function ProviderRow({
  id,
  name,
  envKey,
  apiKeyInput,
  onStartEdit,
  onCancelEdit,
  onSaveKey,
  onChangeInput,
}: {
  id: string;
  name: string;
  envKey: string;
  apiKeyInput: { provider: string; value: string } | null;
  onStartEdit: () => void;
  onCancelEdit: () => void;
  onSaveKey: (provider: string, key: string) => void;
  onChangeInput: (v: string) => void;
}) {
  const { t } = useI18n();
  const [status, setStatus] = useState<"unknown" | "configured" | "missing">("unknown");
  const isEditing = apiKeyInput?.provider === id;

  useEffect(() => {
    if (!envKey) {
      setStatus("configured");
      return;
    }
    getApiKey(id)
      .then((k) => setStatus(k ? "configured" : "missing"))
      .catch(() => setStatus("unknown"));
  }, [id, envKey]);

  return (
    <div className="flex items-center gap-2 rounded-md border border-border px-2.5 py-2 text-xs">
      <span
        className={cn(
          "h-2 w-2 rounded-full",
          status === "configured" && "bg-success",
          status === "missing" && "bg-muted",
          status === "unknown" && "bg-warning",
        )}
      />
      <span className="w-28 font-medium">{name}</span>

      {isEditing ? (
        <div className="flex flex-1 items-center gap-1">
          <input
            type="password"
            value={apiKeyInput.value}
            onChange={(e) => onChangeInput(e.target.value)}
            placeholder={t.settings.pasteApiKey}
            className="flex-1 rounded border border-border bg-background px-2 py-1 text-xs focus:border-primary focus:outline-none"
            ref={(el) => el?.focus()}
          />
          <button
            type="button"
            onClick={() => onSaveKey(id, apiKeyInput.value)}
            disabled={!apiKeyInput.value.trim()}
            className="rounded bg-primary px-2 py-1 text-[10px] text-primary-foreground disabled:opacity-50"
          >
            {t.common.save}
          </button>
          <button
            type="button"
            onClick={onCancelEdit}
            className="rounded px-1.5 py-1 text-muted-foreground hover:text-foreground"
          >
            <X className="h-3 w-3" />
          </button>
        </div>
      ) : (
        <div className="flex flex-1 items-center justify-end gap-2">
          {status === "configured" && envKey && (
            <span className="font-mono text-muted-foreground">****</span>
          )}
          {envKey && (
            <button
              type="button"
              onClick={onStartEdit}
              className="flex items-center gap-1 rounded px-2 py-0.5 text-[10px] text-muted-foreground hover:bg-accent hover:text-foreground"
            >
              <Key className="h-3 w-3" />
              {status === "configured" ? t.common.change : t.settings.setKey}
            </button>
          )}
          {!envKey && id === "ollama" && (
            <span className="text-[10px] text-success">{t.settings.localNoKey}</span>
          )}
        </div>
      )}
    </div>
  );
}

export function ModelsPage({ settings, onSave }: PageProps) {
  const { t } = useI18n();
  const [newFallback, setNewFallback] = useState("");
  const [apiKeyInput, setApiKeyInput] = useState<{ provider: string; value: string } | null>(null);
  const [modelGroups, setModelGroups] = useState<ModelGroupData[]>([]);

  const fallbackModels = settings.fallbackModels ?? [];
  const aliases = settings.modelAliases ?? {};
  const providers = settings.providers ?? {};

  useEffect(() => {
    listModelGroups()
      .then(setModelGroups)
      .catch(() => setModelGroups([]));
  }, []);

  const catalogModelOptions = useMemo(() => modelGroupsToSelectOptions(modelGroups), [modelGroups]);
  const modelSelectOptions = useMemo(
    () => withCurrentModelOption(catalogModelOptions, settings.model),
    [catalogModelOptions, settings.model],
  );

  const showModelSelect = catalogModelOptions.length > 0;

  const handleSetKey = useCallback(async (provider: string, key: string) => {
    try {
      await setApiKey(provider, key);
      setApiKeyInput(null);
    } catch (err) {
      console.error("Failed to set API key:", err);
    }
  }, []);

  return (
    <>
      <Section title={t.settings.defaultModel}>
        <Field label={t.settings.model} hint={t.settings.modelHint}>
          {showModelSelect ? (
            <Select
              fullWidth
              value={settings.model ?? ""}
              options={modelSelectOptions}
              onChange={(v) => onSave({ model: v || undefined })}
              placeholder={t.settings.modelPlaceholder}
            />
          ) : (
            <TextInput
              value={settings.model ?? ""}
              onChange={(v) => onSave({ model: v || undefined })}
              placeholder={t.settings.modelPlaceholder}
            />
          )}
        </Field>

        <Field label={t.settings.thinkingMode}>
          <div className="flex items-center gap-2">
            <Toggle
              checked={settings.thinkingMode ?? false}
              onChange={(v) => onSave({ thinkingMode: v })}
            />
            <span className="text-[10px] text-muted-foreground">{t.settings.thinkingModeHint}</span>
          </div>
        </Field>

        <Field label={t.settings.maxContextTokens}>
          <NumberInput
            value={settings.maxContextTokens ?? 200000}
            onChange={(v) => onSave({ maxContextTokens: v })}
            min={1000}
            max={2000000}
            step={1000}
          />
        </Field>
      </Section>

      <Section title={t.settings.modelAliases}>
        <div className="space-y-1">
          {Object.entries(aliases).map(([alias, model]) => (
            <div key={alias} className="flex items-center gap-2 text-xs">
              <span className="font-mono text-primary">{alias}</span>
              <ChevronRight className="h-3 w-3 text-muted-foreground" />
              <span className="flex-1 truncate font-mono text-muted-foreground">{model}</span>
            </div>
          ))}
          {Object.keys(aliases).length === 0 && (
            <p className="text-[10px] text-muted-foreground">{t.settings.modelAliasesEmpty}</p>
          )}
        </div>
      </Section>

      <Section title={t.settings.fallbackModels}>
        <div className="space-y-1">
          {fallbackModels.map((m, i) => (
            <div key={m} className="flex items-center gap-2 text-xs">
              <span className="w-4 text-muted-foreground">{i + 1}.</span>
              <span className="flex-1 truncate font-mono">{m}</span>
              <button
                type="button"
                onClick={() => {
                  const next = fallbackModels.filter((_, j) => j !== i);
                  onSave({ fallbackModels: next });
                }}
                className="text-muted-foreground hover:text-destructive"
              >
                <X className="h-3 w-3" />
              </button>
            </div>
          ))}
          <div className="flex items-center gap-1">
            <TextInput
              value={newFallback}
              onChange={setNewFallback}
              placeholder={t.settings.fallbackPlaceholder}
            />
            <button
              type="button"
              disabled={!newFallback.trim()}
              onClick={() => {
                onSave({ fallbackModels: [...fallbackModels, newFallback.trim()] });
                setNewFallback("");
              }}
              className="rounded bg-primary px-2 py-1.5 text-[10px] text-primary-foreground disabled:opacity-50"
            >
              <Plus className="h-3 w-3" />
            </button>
          </div>
        </div>
      </Section>

      <Section title={t.settings.providers}>
        <div className="space-y-2">
          {BUILTIN_PROVIDERS.map((bp) => (
            <ProviderRow
              key={bp.id}
              id={bp.id}
              name={bp.name}
              envKey={bp.envKey}
              apiKeyInput={apiKeyInput}
              onStartEdit={() => setApiKeyInput({ provider: bp.id, value: "" })}
              onCancelEdit={() => setApiKeyInput(null)}
              onSaveKey={handleSetKey}
              onChangeInput={(v) => setApiKeyInput((prev) => (prev ? { ...prev, value: v } : null))}
            />
          ))}
        </div>

        {Object.keys(providers).length > 0 && (
          <div className="mt-4">
            <h4 className="mb-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
              {t.settings.customProviders}
            </h4>
            {Object.entries(providers).map(([id, cfg]) => (
              <div
                key={id}
                className="flex items-center gap-2 rounded border border-border px-2 py-1.5 text-xs"
              >
                <Globe className="h-3 w-3 text-muted-foreground" />
                <span className="font-medium">{id}</span>
                <span className="flex-1 truncate text-muted-foreground">{cfg.baseUrl}</span>
                <span className="text-[10px] text-muted-foreground">
                  {cfg.models?.length ?? 0} {t.settings.modelsCount}
                </span>
              </div>
            ))}
          </div>
        )}
      </Section>
    </>
  );
}
