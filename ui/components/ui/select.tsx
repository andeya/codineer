import { Check, ChevronDown } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";

interface SelectOption {
  value: string;
  label: string;
}

interface SelectProps {
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
  placeholder?: string;
  className?: string;
  /** Stretch trigger and menu to parent width (e.g. settings forms). */
  fullWidth?: boolean;
  /** Open the menu above the trigger (e.g. bottom status bar). */
  dropUp?: boolean;
  /** Horizontal alignment of the menu relative to the trigger. */
  menuAlign?: "start" | "end";
  /** Extra classes for the trigger button (compact / borderless variants). */
  triggerClassName?: string;
  /** Extra classes for the dropdown panel (min-width, max-height). */
  menuClassName?: string;
  /**
   * When set, used as the closed trigger text if `value` is non-empty
   * (options still show full `label`, e.g. status bar short model name).
   */
  triggerLabel?: string | null;
}

export function Select({
  value,
  options,
  onChange,
  placeholder,
  className,
  fullWidth,
  dropUp,
  menuAlign = "start",
  triggerClassName,
  menuClassName,
  triggerLabel,
}: SelectProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const selected = options.find((o) => o.value === value);

  const closedLabel = !value
    ? (placeholder ?? "Select...")
    : triggerLabel != null
      ? triggerLabel || selected?.label || value
      : (selected?.label ?? value);

  const handleSelect = useCallback(
    (v: string) => {
      onChange(v);
      setOpen(false);
    },
    [onChange],
  );

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open]);

  return (
    <div
      ref={ref}
      className={cn("relative", fullWidth ? "block w-full min-w-0" : "inline-block", className)}
    >
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className={cn(
          "flex cursor-pointer items-center justify-between gap-2 rounded-md border border-border bg-background px-2.5 py-1.5 text-xs text-foreground transition-colors",
          fullWidth ? "w-full min-w-0" : "min-w-[140px]",
          "hover:bg-accent focus:outline-none focus:ring-2 ring-focus",
          open && "ring-2 ring-focus",
          triggerClassName,
        )}
      >
        <span className={cn("min-w-0 truncate text-left", !value && "text-muted-foreground")}>
          {closedLabel}
        </span>
        <ChevronDown
          className={cn(
            "h-3.5 w-3.5 text-muted-foreground transition-transform",
            open && "rotate-180",
          )}
        />
      </button>

      {open && (
        <div
          className={cn(
            "absolute z-50 max-h-60 min-w-full overflow-y-auto rounded-md border border-border bg-popover shadow-md",
            dropUp ? "bottom-full mb-1" : "top-full mt-1",
            menuAlign === "end" ? "right-0" : "left-0",
            fullWidth && "w-full",
            menuClassName,
          )}
        >
          {options.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => handleSelect(opt.value)}
              className={cn(
                "flex w-full min-w-0 cursor-pointer items-center gap-2 px-2.5 py-1.5 text-left text-xs transition-colors",
                "hover:bg-accent hover:text-accent-foreground",
                opt.value === value && "bg-accent text-accent-foreground",
              )}
            >
              <Check
                className={cn(
                  "h-3 w-3 shrink-0",
                  opt.value === value ? "opacity-100" : "opacity-0",
                )}
              />
              <span className="min-w-0 truncate">{opt.label}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
