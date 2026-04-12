import { useCallback, useEffect, useState } from "react";
import { useI18n } from "@/lib/i18n";
import { Section } from "./shared";
import type { PageProps } from "./types";

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
      <textarea
        value={jsonText}
        onChange={(e) => {
          setJsonText(e.target.value);
          try {
            JSON.parse(e.target.value);
            setError(null);
          } catch (err) {
            setError(String(err));
          }
        }}
        className="min-h-[300px] w-full rounded-md border border-border bg-background p-3 font-mono text-xs text-foreground focus:border-primary focus:outline-none"
        spellCheck={false}
      />
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
