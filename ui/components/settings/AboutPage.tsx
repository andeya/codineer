import { ExternalLink } from "lucide-react";
import { useEffect, useState } from "react";
import { Logo } from "@/components/Logo";
import type { Translations } from "@/lib/i18n";
import { useI18n } from "@/lib/i18n";
import type { AppInfo } from "@/lib/tauri";
import { getAppInfo, tryInvoke } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { Section } from "./shared";

function channelLabel(t: Translations, channel: string): string {
  switch (channel) {
    case "dev":
      return t.settings.channelDev;
    case "nightly":
      return t.settings.channelNightly;
    case "preview":
      return t.settings.channelPreview;
    case "stable":
      return t.settings.channelStable;
    default:
      return channel;
  }
}

export function AboutPage() {
  const { t } = useI18n();
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    tryInvoke(getAppInfo, null as unknown as AppInfo).then((v) => {
      if (v) setInfo(v);
    });
  }, []);

  const versionDisplay = info ? `v${info.version}${info.versionSuffix}` : "v0.1.0";

  const channelBadgeColor: Record<string, string> = {
    dev: "badge-channel-dev",
    nightly: "badge-channel-nightly",
    preview: "badge-channel-preview",
    stable: "badge-channel-stable",
  };

  return (
    <>
      <div className="mb-6 flex flex-col items-center gap-3 py-4">
        <Logo className="h-16 w-16 rounded-2xl shadow-sm" />
        <div className="text-center">
          <h2 className="text-lg font-semibold text-foreground">Aineer</h2>
          <p className="mt-0.5 text-sm text-muted-foreground">{versionDisplay}</p>
          {info && (
            <span
              className={cn(
                "mt-2 inline-block rounded-full border px-2.5 py-0.5 text-[10px] font-medium capitalize",
                channelBadgeColor[info.channel] ?? channelBadgeColor.dev,
              )}
            >
              {channelLabel(t, info.channel)}
            </span>
          )}
        </div>
      </div>

      <Section title={t.settings.application}>
        <div className="space-y-2">
          <AboutRow label={t.settings.version} value={versionDisplay} />
          <AboutRow
            label={t.settings.channel}
            value={channelLabel(t, info?.channel ?? "dev")}
            capitalize
          />
          <AboutRow
            label={t.settings.displayName}
            value={info?.displayName ?? t.settings.displayNameFallback}
          />
        </div>
      </Section>

      <Section title={t.settings.links}>
        <div className="space-y-2">
          {info?.githubUrl && <AboutLink label={t.settings.github} url={info.githubUrl} />}
          {info?.homepage && info.homepage !== info.githubUrl && (
            <AboutLink label={t.settings.homepage} url={info.homepage} />
          )}
          <AboutLink
            label={t.settings.reportIssue}
            url={`${info?.githubUrl ?? "https://github.com/andeya/aineer"}/issues`}
          />
          <AboutLink
            label={t.settings.releases}
            url={`${info?.githubUrl ?? "https://github.com/andeya/aineer"}/releases`}
          />
        </div>
      </Section>

      <Section title={t.settings.license}>
        <p className="text-xs text-muted-foreground">{t.settings.licenseValue}</p>
      </Section>
    </>
  );
}

export function AboutRow({
  label,
  value,
  capitalize,
}: {
  label: string;
  value: string;
  capitalize?: boolean;
}) {
  return (
    <div className="flex items-center justify-between rounded-md bg-card/50 px-3 py-2">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className={cn("text-xs font-medium text-foreground", capitalize && "capitalize")}>
        {value}
      </span>
    </div>
  );
}

export function AboutLink({ label, url }: { label: string; url: string }) {
  return (
    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      className="flex items-center justify-between rounded-md bg-card/50 px-3 py-2 text-xs transition-colors hover:bg-accent/50"
    >
      <span className="text-muted-foreground">{label}</span>
      <span className="flex items-center gap-1.5 text-primary">
        {url.replace(/^https?:\/\//, "")}
        <ExternalLink className="h-3 w-3" />
      </span>
    </a>
  );
}
