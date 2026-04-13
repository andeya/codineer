import { Check, ExternalLink, Globe, Loader2, LogIn, LogOut, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useI18n } from "@/lib/i18n";
import {
  type WebAiProviderInfo,
  webaiListAuthenticated,
  webaiListProviders,
  webaiLogout,
  webaiStartAuth,
} from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { Section } from "./shared";

function ProviderCard({
  provider,
  authenticated,
  loggingIn,
  onLogin,
  onLogout,
  disabled,
}: {
  provider: WebAiProviderInfo;
  authenticated: boolean;
  loggingIn: boolean;
  onLogin: () => void;
  onLogout: () => void;
  disabled: boolean;
}) {
  const { t } = useI18n();

  return (
    <div
      className={cn(
        "flex items-center gap-3 rounded-lg border px-3 py-2.5 transition-colors",
        loggingIn ? "border-primary/40 bg-primary/5" : "border-border",
      )}
    >
      <span
        className={cn(
          "flex h-7 w-7 items-center justify-center rounded-full text-xs font-bold",
          loggingIn
            ? "bg-primary/10 text-primary"
            : authenticated
              ? "bg-success/10 text-success"
              : "bg-muted-foreground/10 text-muted-foreground",
        )}
      >
        {loggingIn ? (
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
        ) : authenticated ? (
          <Check className="h-3.5 w-3.5" />
        ) : (
          <Globe className="h-3.5 w-3.5" />
        )}
      </span>

      <div className="flex-1">
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium">{provider.name}</span>
          <span className="text-[10px] text-muted-foreground">
            {t.settings.nModels.replace("{0}", String(provider.models.length))}
          </span>
        </div>
        <span
          className={cn(
            "text-[10px]",
            loggingIn ? "text-primary" : authenticated ? "text-success" : "text-muted-foreground",
          )}
        >
          {loggingIn
            ? t.settings.loggingIn
            : authenticated
              ? t.settings.loggedIn
              : t.settings.notLoggedIn}
        </span>
      </div>

      {loggingIn ? (
        <span className="flex items-center gap-1 text-[10px] text-primary">
          <ExternalLink className="h-3 w-3" />
          {t.settings.waitingForBrowser}
        </span>
      ) : authenticated ? (
        <button
          type="button"
          disabled={disabled}
          onClick={onLogout}
          className="flex items-center gap-1 rounded-md px-2.5 py-1 text-[10px] text-muted-foreground hover:bg-destructive/10 hover:text-destructive disabled:opacity-50"
        >
          <LogOut className="h-3 w-3" />
          {t.settings.logout}
        </button>
      ) : (
        <button
          type="button"
          disabled={disabled}
          onClick={onLogin}
          className="flex items-center gap-1 rounded-md bg-primary px-2.5 py-1 text-[10px] text-primary-foreground disabled:opacity-50"
        >
          <LogIn className="h-3 w-3" />
          {t.settings.login}
        </button>
      )}
    </div>
  );
}

export function WebAiPage() {
  const { t } = useI18n();
  const [providers, setProviders] = useState<WebAiProviderInfo[]>([]);
  const [authenticated, setAuthenticated] = useState<Set<string>>(new Set());
  const [loggingInId, setLoggingInId] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [pList, aList] = await Promise.all([webaiListProviders(), webaiListAuthenticated()]);
      setProviders(pList);
      setAuthenticated(new Set(aList));
    } catch {
      /* backend may not be ready */
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleLogin = useCallback(
    async (providerId: string) => {
      setLoggingInId(providerId);
      try {
        await webaiStartAuth(providerId);
      } catch (err) {
        console.error("WebAI login failed:", err);
      }
      await refresh();
      setLoggingInId(null);
    },
    [refresh],
  );

  const handleLogout = useCallback(
    async (providerId: string) => {
      setLoggingInId(providerId);
      try {
        await webaiLogout(providerId);
      } catch (err) {
        console.error("WebAI logout failed:", err);
      }
      await refresh();
      setLoggingInId(null);
    },
    [refresh],
  );

  const handleLoginAll = useCallback(async () => {
    for (const p of providers) {
      if (!authenticated.has(p.id)) {
        setLoggingInId(p.id);
        try {
          await webaiStartAuth(p.id);
        } catch {
          /* continue with next */
        }
        await refresh();
      }
    }
    setLoggingInId(null);
  }, [providers, authenticated, refresh]);

  const handleLogoutAll = useCallback(async () => {
    setLoggingInId("__all__");
    for (const id of authenticated) {
      try {
        await webaiLogout(id);
      } catch {
        /* continue */
      }
    }
    await refresh();
    setLoggingInId(null);
  }, [authenticated, refresh]);

  const busy = loggingInId !== null;
  const loggedCount = providers.filter((p) => authenticated.has(p.id)).length;

  return (
    <Section title={t.settings.webAi}>
      <p className="mb-4 text-[10px] text-muted-foreground">{t.settings.webAiDesc}</p>

      <div className="mb-4 flex items-center justify-between">
        <span className="text-xs text-muted-foreground">
          {t.settings.loggedIn}: {loggedCount}/{providers.length}
        </span>
        <div className="flex gap-2">
          <button
            type="button"
            disabled={busy || loggedCount === providers.length}
            onClick={handleLoginAll}
            className="flex items-center gap-1 rounded-md border border-border px-2.5 py-1 text-[10px] hover:bg-accent disabled:opacity-50"
          >
            <LogIn className="h-3 w-3" />
            {t.settings.loginAll}
          </button>
          <button
            type="button"
            disabled={busy || loggedCount === 0}
            onClick={handleLogoutAll}
            className="flex items-center gap-1 rounded-md border border-border px-2.5 py-1 text-[10px] text-muted-foreground hover:bg-destructive/10 hover:text-destructive disabled:opacity-50"
          >
            <X className="h-3 w-3" />
            {t.settings.logoutAll}
          </button>
        </div>
      </div>

      <div className="space-y-2">
        {providers.map((p) => (
          <ProviderCard
            key={p.id}
            provider={p}
            authenticated={authenticated.has(p.id)}
            loggingIn={loggingInId === p.id}
            onLogin={() => handleLogin(p.id)}
            onLogout={() => handleLogout(p.id)}
            disabled={busy}
          />
        ))}
        {providers.length === 0 && (
          <p className="py-4 text-center text-xs text-muted-foreground">{t.common.loading}</p>
        )}
      </div>
    </Section>
  );
}
