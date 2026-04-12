import { Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Select } from "@/components/ui/select";
import { formatBytes } from "@/lib/format";
import { useI18n } from "@/lib/i18n";
import type { AutoCleanupConfig, CacheStats, ChatHistoryEntry } from "@/lib/tauri";
import {
  clearCache,
  deleteChatHistory,
  getAutoCleanup,
  getCacheStats,
  listChatHistory,
  setAutoCleanup,
  tryInvoke,
} from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { Field, Section } from "./shared";

function formatDate(timestampMs: number): string {
  if (!timestampMs) return "";
  const d = new Date(timestampMs);
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
}

function StatCard({ label, count, size }: { label: string; count: number; size: string }) {
  const { t } = useI18n();
  return (
    <div className="rounded-lg border border-border bg-card px-3 py-2">
      <div className="text-xs font-medium text-foreground">{label}</div>
      <div className="mt-1 flex items-baseline gap-2">
        <span className="text-lg font-bold text-foreground">{count}</span>
        <span className="text-[10px] text-muted-foreground">
          {t.settings.items} · {size}
        </span>
      </div>
    </div>
  );
}

function ClearButton({
  label,
  target,
  clearing,
  onClick,
  destructive,
}: {
  label: string;
  target: "attachments" | "history" | "all";
  clearing: string | null;
  onClick: (target: "attachments" | "history" | "all") => void;
  destructive?: boolean;
}) {
  const { t } = useI18n();
  const isClearing = clearing === target;
  return (
    <button
      type="button"
      onClick={() => {
        if (window.confirm(t.settings.confirmClear)) {
          onClick(target);
        }
      }}
      disabled={clearing !== null}
      className={cn(
        "flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs transition-colors disabled:opacity-50",
        destructive
          ? "border btn-destructive-outline"
          : "border border-border bg-card text-foreground hover:bg-accent",
      )}
    >
      <Trash2 className="h-3 w-3" />
      {isClearing ? t.common.clearing : label}
    </button>
  );
}

function formatRelativeDate(ms: number, t: ReturnType<typeof useI18n>["t"]): string {
  if (!ms) return t.settings.cleanupNever;
  const d = new Date(ms);
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function CachePage() {
  const { t } = useI18n();
  const [stats, setStats] = useState<CacheStats | null>(null);
  const [history, setHistory] = useState<ChatHistoryEntry[]>([]);
  const [clearing, setClearing] = useState<string | null>(null);
  const [cleanup, setCleanup] = useState<AutoCleanupConfig | null>(null);

  const refresh = useCallback(async () => {
    const [s, h, ac] = await Promise.all([
      tryInvoke(getCacheStats, null),
      tryInvoke(listChatHistory, []),
      tryInvoke(getAutoCleanup, null),
    ]);
    if (s) setStats(s);
    setHistory(h);
    if (ac) setCleanup(ac);
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleClear = useCallback(
    async (target: "attachments" | "history" | "all") => {
      setClearing(target);
      try {
        await clearCache(target);
        await refresh();
      } catch (err) {
        console.error("Failed to clear cache:", err);
      } finally {
        setClearing(null);
      }
    },
    [refresh],
  );

  const handleDeleteSession = useCallback(
    async (sessionId: string) => {
      try {
        await deleteChatHistory(sessionId);
        await refresh();
      } catch (err) {
        console.error("Failed to delete history:", err);
      }
    },
    [refresh],
  );

  return (
    <>
      <Section title={t.settings.storageOverview}>
        {stats ? (
          <div className="space-y-2">
            <div className="grid grid-cols-2 gap-3">
              <StatCard
                label={t.settings.attachments}
                count={stats.attachmentsCount}
                size={formatBytes(stats.attachmentsSizeBytes, t.units)}
              />
              <StatCard
                label={t.settings.chatHistory}
                count={stats.historyCount}
                size={formatBytes(stats.historySizeBytes, t.units)}
              />
            </div>
            <p className="text-[10px] text-muted-foreground">
              {t.settings.cachePath}
              {stats.cachePath}
            </p>
          </div>
        ) : (
          <p className="text-xs text-muted-foreground">{t.settings.loadingCacheInfo}</p>
        )}
      </Section>

      <Section title={t.settings.clearCache}>
        <div className="flex flex-wrap gap-2">
          <ClearButton
            label={t.settings.clearAttachments}
            target="attachments"
            clearing={clearing}
            onClick={handleClear}
          />
          <ClearButton
            label={t.settings.clearChatHistory}
            target="history"
            clearing={clearing}
            onClick={handleClear}
          />
          <ClearButton
            label={t.settings.clearAll}
            target="all"
            clearing={clearing}
            onClick={handleClear}
            destructive
          />
        </div>
      </Section>

      <Section title={t.settings.autoCleanup}>
        <p className="mb-2 text-[10px] text-muted-foreground">{t.settings.autoCleanupHint}</p>
        <div className="grid grid-cols-2 gap-3">
          <Field label={t.settings.cleanupInterval}>
            <Select
              fullWidth
              value={cleanup?.interval ?? "off"}
              options={[
                { value: "off", label: t.settings.cleanupOff },
                { value: "daily", label: t.settings.cleanupDaily },
                { value: "weekly", label: t.settings.cleanupWeekly },
                { value: "monthly", label: t.settings.cleanupMonthly },
              ]}
              onChange={(v) => {
                const target = cleanup?.target ?? "all";
                setCleanup((prev) =>
                  prev ? { ...prev, interval: v } : { interval: v, target, lastRunMs: 0 },
                );
                setAutoCleanup(v, target).catch(console.error);
              }}
            />
          </Field>
          <Field label={t.settings.cleanupTarget}>
            <Select
              fullWidth
              value={cleanup?.target ?? "all"}
              options={[
                { value: "all", label: t.settings.cleanupTargetAll },
                { value: "attachments", label: t.settings.cleanupTargetAttachments },
                { value: "history", label: t.settings.cleanupTargetHistory },
              ]}
              onChange={(v) => {
                const interval = cleanup?.interval ?? "off";
                setCleanup((prev) =>
                  prev ? { ...prev, target: v } : { interval, target: v, lastRunMs: 0 },
                );
                setAutoCleanup(interval, v).catch(console.error);
              }}
            />
          </Field>
        </div>
        {cleanup && cleanup.interval !== "off" && (
          <p className="mt-1.5 text-[10px] text-muted-foreground">
            {t.settings.cleanupLastRun} {formatRelativeDate(cleanup.lastRunMs, t)}
          </p>
        )}
      </Section>

      {history.length > 0 && (
        <Section title={t.settings.chatSessions}>
          <div className="max-h-60 space-y-1 overflow-y-auto">
            {history.map((h) => (
              <div
                key={h.sessionId}
                className="flex items-center gap-2 rounded border border-border px-2 py-1.5 text-xs"
              >
                <span className="min-w-0 flex-1 truncate font-mono text-muted-foreground">
                  {h.sessionId}
                </span>
                <span className="shrink-0 text-[10px] text-muted-foreground">
                  {formatBytes(h.sizeBytes, t.units)}
                </span>
                <span className="shrink-0 text-[10px] text-muted-foreground">
                  {formatDate(h.modifiedAt)}
                </span>
                <button
                  type="button"
                  onClick={() => handleDeleteSession(h.sessionId)}
                  className="shrink-0 text-muted-foreground hover:text-destructive"
                  title={t.settings.deleteSession}
                >
                  <Trash2 className="h-3 w-3" />
                </button>
              </div>
            ))}
          </div>
        </Section>
      )}
    </>
  );
}
