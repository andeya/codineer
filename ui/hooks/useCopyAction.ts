import { useCallback, useState } from "react";

export function useCopyAction() {
  const [copied, setCopied] = useState<string | null>(null);
  const copy = useCallback(async (text: string, label: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(label);
      setTimeout(() => setCopied(null), 1500);
    } catch {
      /* noop */
    }
  }, []);
  return { copied, copy };
}
