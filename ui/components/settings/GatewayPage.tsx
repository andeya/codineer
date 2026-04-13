import { Check, Copy } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Select } from "@/components/ui/select";
import { useI18n } from "@/lib/i18n";
import { modelGroupsToSelectOptions, withCurrentModelOption } from "@/lib/model-options";
import {
  type AppSettings,
  type GatewayStatusInfo,
  getGatewayStatus,
  listModelGroups,
  type ModelGroupData,
  startGateway,
  stopGateway,
} from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { Field, Section, TextInput, Toggle } from "./shared";

export function GatewayPage({
  settings,
  onSave,
}: {
  settings: AppSettings;
  onSave: (updates: Partial<AppSettings>) => void;
}) {
  const { t } = useI18n();
  const gw = settings.gateway ?? {};
  const enabled = gw.enabled ?? false;
  const listenAddr = gw.listenAddr ?? "127.0.0.1:8090";

  const [status, setStatus] = useState<GatewayStatusInfo | null>(null);
  const [copied, setCopied] = useState(false);
  const [modelGroups, setModelGroups] = useState<ModelGroupData[]>([]);

  const baseUrl = `http://${listenAddr}/v1`;

  const refreshStatus = useCallback(async () => {
    try {
      const s = await getGatewayStatus();
      setStatus(s);
    } catch {
      setStatus(null);
    }
  }, []);

  useEffect(() => {
    refreshStatus();
    listModelGroups()
      .then(setModelGroups)
      .catch(() => setModelGroups([]));
  }, [refreshStatus]);

  const catalogModelOptions = useMemo(() => modelGroupsToSelectOptions(modelGroups), [modelGroups]);
  const modelSelectOptions = useMemo(
    () => withCurrentModelOption(catalogModelOptions, gw.defaultModel),
    [catalogModelOptions, gw.defaultModel],
  );

  const handleToggle = useCallback(
    async (on: boolean) => {
      const next = { ...gw, enabled: on };
      onSave({ gateway: next });
      try {
        if (on) {
          await startGateway();
        } else {
          await stopGateway();
        }
        await refreshStatus();
      } catch (err) {
        console.error("Gateway toggle failed:", err);
      }
    },
    [gw, onSave, refreshStatus],
  );

  const handleAddrChange = useCallback(
    (addr: string) => {
      onSave({ gateway: { ...gw, listenAddr: addr } });
    },
    [gw, onSave],
  );

  const handleModelChange = useCallback(
    (model: string) => {
      onSave({ gateway: { ...gw, defaultModel: model || undefined } });
    },
    [gw, onSave],
  );

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(baseUrl);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [baseUrl]);

  const isRunning = status?.running ?? false;

  return (
    <Section title={t.settings.clawGateway}>
      <p className="mb-4 text-[10px] text-muted-foreground">{t.settings.gatewayDesc}</p>

      <Field label={t.settings.enableGateway}>
        <div className="flex items-center gap-3">
          <Toggle checked={enabled} onChange={handleToggle} />
          <span
            className={cn(
              "flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[10px] font-medium",
              isRunning
                ? "bg-success/10 text-success"
                : "bg-muted-foreground/10 text-muted-foreground",
            )}
          >
            <span
              className={cn(
                "h-1.5 w-1.5 rounded-full",
                isRunning ? "bg-success" : "bg-muted-foreground",
              )}
            />
            {isRunning ? t.settings.gatewayRunning : t.settings.gatewayStopped}
          </span>
        </div>
      </Field>

      <Field label={t.settings.listenAddress} hint={t.settings.listenAddressHint}>
        <TextInput value={listenAddr} onChange={handleAddrChange} placeholder="127.0.0.1:8090" />
      </Field>

      <Field label={t.settings.gatewayDefaultModel}>
        {catalogModelOptions.length > 0 ? (
          <Select
            fullWidth
            value={gw.defaultModel ?? ""}
            options={modelSelectOptions}
            onChange={handleModelChange}
            placeholder={t.settings.modelPlaceholder}
          />
        ) : (
          <TextInput
            value={gw.defaultModel ?? ""}
            onChange={handleModelChange}
            placeholder={t.settings.modelPlaceholder}
          />
        )}
      </Field>

      <Field label={t.settings.baseUrl}>
        <div className="flex items-center gap-2">
          <code className="flex-1 rounded-md border border-border bg-muted px-2.5 py-1.5 font-mono text-xs">
            {baseUrl}
          </code>
          <button
            type="button"
            onClick={handleCopy}
            className={cn(
              "flex items-center gap-1 rounded-md border border-border px-2.5 py-1.5 text-[10px] transition-colors",
              copied
                ? "border-success text-success"
                : "text-muted-foreground hover:bg-accent hover:text-foreground",
            )}
          >
            {copied ? (
              <>
                <Check className="h-3 w-3" />
                {t.settings.copied}
              </>
            ) : (
              <>
                <Copy className="h-3 w-3" />
                {t.settings.copyUrl}
              </>
            )}
          </button>
        </div>
        <p className="mt-1.5 text-[10px] text-muted-foreground">{t.settings.gatewayTip}</p>
      </Field>
    </Section>
  );
}
