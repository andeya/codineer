"use client";

import { CheckCircle, ChevronDown, Loader2, Settings, XCircle } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { useI18n } from "@/lib/i18n";
import type { ToolPart } from "@/lib/types";
import { cn } from "@/lib/utils";

export type { ToolPart };

export type ToolProps = {
  toolPart: ToolPart;
  defaultOpen?: boolean;
  className?: string;
};

const Tool = ({ toolPart, defaultOpen = false, className }: ToolProps) => {
  const { t } = useI18n();
  const [isOpen, setIsOpen] = useState(defaultOpen);

  const { state, input, output, toolCallId } = toolPart;

  const getStateIcon = () => {
    switch (state) {
      case "input-streaming":
        return <Loader2 className="h-4 w-4 animate-spin text-info" />;
      case "input-available":
        return <Settings className="h-4 w-4 text-warning" />;
      case "output-available":
        return <CheckCircle className="h-4 w-4 text-success" />;
      case "output-error":
        return <XCircle className="h-4 w-4 text-destructive" />;
      default:
        return <Settings className="h-4 w-4 text-muted-foreground" />;
    }
  };

  const getStateBadge = () => {
    const baseClasses = "px-2 py-1 rounded-full text-xs font-medium";
    switch (state) {
      case "input-streaming":
        return <span className={cn(baseClasses, "badge-info")}>{t.tool.processing}</span>;
      case "input-available":
        return <span className={cn(baseClasses, "badge-warning")}>{t.tool.ready}</span>;
      case "output-available":
        return <span className={cn(baseClasses, "badge-success")}>{t.tool.completed}</span>;
      case "output-error":
        return <span className={cn(baseClasses, "badge-error")}>{t.tool.error}</span>;
      default:
        return (
          <span className={cn(baseClasses, "bg-muted text-muted-foreground")}>
            {t.tool.pending}
          </span>
        );
    }
  };

  const formatValue = (value: unknown): string => {
    if (value === null) return "null";
    if (value === undefined) return "undefined";
    if (typeof value === "string") return value;
    if (typeof value === "object") {
      return JSON.stringify(value, null, 2);
    }
    return String(value);
  };

  return (
    <div className={cn("border-border mt-3 overflow-hidden rounded-lg border", className)}>
      <Collapsible open={isOpen} onOpenChange={setIsOpen}>
        <CollapsibleTrigger asChild>
          <Button
            variant="ghost"
            className="bg-background h-auto w-full justify-between rounded-b-none px-3 py-2 font-normal"
          >
            <div className="flex items-center gap-2">
              {getStateIcon()}
              <span className="font-mono text-sm font-medium">{toolPart.type}</span>
              {getStateBadge()}
            </div>
            <ChevronDown className={cn("h-4 w-4", isOpen && "rotate-180")} />
          </Button>
        </CollapsibleTrigger>
        <CollapsibleContent
          className={cn(
            "border-border border-t",
            "data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down overflow-hidden",
          )}
        >
          <div className="bg-background space-y-3 p-3">
            {input && Object.keys(input).length > 0 && (
              <div>
                <h4 className="text-muted-foreground mb-2 text-sm font-medium">{t.tool.input}</h4>
                <div className="bg-background rounded border p-2 font-mono text-sm">
                  {Object.entries(input).map(([key, value]) => (
                    <div key={key} className="mb-1">
                      <span className="text-muted-foreground">{key}:</span>{" "}
                      <span>{formatValue(value)}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {output && (
              <div>
                <h4 className="text-muted-foreground mb-2 text-sm font-medium">{t.tool.output}</h4>
                <div className="bg-background max-h-60 overflow-auto rounded border p-2 font-mono text-sm">
                  <pre className="whitespace-pre-wrap">{formatValue(output)}</pre>
                </div>
              </div>
            )}

            {state === "output-error" && toolPart.errorText && (
              <div>
                <h4 className="mb-2 text-sm font-medium text-error">{t.tool.error}</h4>
                <div className="rounded border bg-error-block p-2 text-sm">
                  {toolPart.errorText}
                </div>
              </div>
            )}

            {state === "input-streaming" && (
              <div className="text-muted-foreground text-sm">{t.tool.processingToolCall}</div>
            )}

            {toolCallId && (
              <div className="text-muted-foreground border-t border-border pt-2 text-xs">
                <span className="font-mono">{t.tool.callId.replace("{id}", toolCallId)}</span>
              </div>
            )}
          </div>
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
};

export { Tool };
