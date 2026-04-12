import { json } from "@codemirror/lang-json";
import { indentUnit } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { vscodeDark, vscodeLight } from "@uiw/codemirror-theme-vscode";
import CodeMirror from "@uiw/react-codemirror";
import { useMemo } from "react";
import { useIsDark } from "@/hooks/useIsDark";
import { cn } from "@/lib/utils";

export interface JsonEditorProps {
  value: string;
  onChange: (value: string) => void;
  /** Editor content height, e.g. `320px` or `50vh` */
  height?: string;
  className?: string;
  readOnly?: boolean;
}

export function JsonEditor({
  value,
  onChange,
  height = "min(55vh, 440px)",
  className,
  readOnly = false,
}: JsonEditorProps) {
  const dark = useIsDark();

  const extensions = useMemo(
    () => [
      json(),
      EditorState.tabSize.of(2),
      indentUnit.of("  "),
      EditorView.lineWrapping,
      dark ? vscodeDark : vscodeLight,
    ],
    [dark],
  );

  return (
    <div
      className={cn(
        "json-editor-shell overflow-hidden rounded-md border border-border text-left shadow-sm",
        "focus-within:border-primary focus-within:ring-2 focus-within:ring-ring/40",
        className,
      )}
    >
      <CodeMirror
        value={value}
        height={height}
        theme="none"
        extensions={extensions}
        editable={!readOnly}
        basicSetup={{
          lineNumbers: true,
          foldGutter: true,
          dropCursor: false,
          allowMultipleSelections: false,
          indentOnInput: true,
          bracketMatching: true,
          closeBrackets: true,
          autocompletion: false,
          highlightSelectionMatches: true,
        }}
        onChange={onChange}
        className="text-xs [&_.cm-editor]:min-h-[200px] [&_.cm-scroller]:font-mono"
      />
    </div>
  );
}
