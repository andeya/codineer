import { Brain, Database, Files, GitBranch, Search, Settings } from "lucide-react";
import { type ElementType, useMemo } from "react";
import { useI18n } from "@/lib/i18n";
import { cn } from "@/lib/utils";

export type SidebarPanel = "explorer" | "search" | "git" | "context" | "memory" | "settings";

interface ActivityBarProps {
  activePanel: SidebarPanel;
  sidebarVisible: boolean;
  onPanelClick: (panel: SidebarPanel) => void;
}

export function ActivityBar({ activePanel, sidebarVisible, onPanelClick }: ActivityBarProps) {
  const { t } = useI18n();

  const panels = useMemo(
    () =>
      [
        { id: "explorer" as const, icon: Files, label: t.nav.explorer },
        { id: "search" as const, icon: Search, label: t.nav.search },
        { id: "git" as const, icon: GitBranch, label: t.nav.git },
        { id: "context" as const, icon: Brain, label: t.nav.context },
        { id: "memory" as const, icon: Database, label: t.nav.memory },
        { id: "settings" as const, icon: Settings, label: t.nav.settings },
      ] satisfies { id: SidebarPanel; icon: ElementType; label: string }[],
    [t],
  );

  return (
    <div className="flex h-full w-12 flex-col items-center border-r border-border bg-sidebar py-2">
      <div className="flex flex-1 flex-col gap-1">
        {panels.map(({ id, icon: Icon, label }) => (
          <button
            key={id}
            type="button"
            title={label}
            onClick={() => onPanelClick(id)}
            className={cn(
              "flex h-10 w-10 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground",
              sidebarVisible && activePanel === id && "bg-accent text-foreground",
            )}
          >
            <Icon size={20} />
          </button>
        ))}
      </div>
    </div>
  );
}
