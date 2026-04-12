import { lazy, Suspense, useCallback, useEffect, useState } from "react";
import { useI18n } from "@/lib/i18n";
import { Section } from "./shared";
import type { PageProps } from "./types";

const LazyJsonEditor = lazy(() =>
  import("@/components/ui/json-editor").then((m) => ({ default: m.JsonEditor })),
);

export function JsonPage({ settings, onSave }: PageProps) {
  const { t } = useI18n();
  const [jsonText, setJsonText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    try {
      setJsonText(JSON.stringify(settings, null, 2));
      setError(null);
    } catch {
      setJsonText("{}");
    }
  }, [settings]);

  const handleSave = useCallback(() => {
    try {
      const parsed = JSON.parse(jsonText);
      setError(null);
      onSave(parsed);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (err) {
      setError(String(err));
    }
  }, [jsonText, onSave]);

  return (
    <Section title={t.settings.jsonTitle}>
      <p className="mb-2 text-[10px] text-muted-foreground">{t.settings.jsonDesc}</p>
      <Suspense
        fallback={
          <div className="flex min-h-[200px] items-center justify-center rounded-md border border-border bg-background text-xs text-muted-foreground">
            Loading editor…
          </div>
        }
      >
        <LazyJsonEditor
          value={jsonText}
          height="min(55vh, 440px)"
          onChange={(next) => {
            setJsonText(next);
            try {
              JSON.parse(next);
              setError(null);
            } catch (err) {
              setError(String(err));
            }
          }}
        />
      </Suspense>
      {error && <p className="mt-1 text-[10px] text-destructive">{error}</p>}
      <div className="mt-2 flex items-center gap-2">
        <button
          type="button"
          onClick={handleSave}
          disabled={!!error}
          className="rounded bg-primary px-3 py-1.5 text-xs text-primary-foreground disabled:opacity-50"
        >
          {t.common.save}
        </button>
        {saved && <span className="text-[10px] text-success">{t.common.saved}</span>}
      </div>
    </Section>
  );
}
