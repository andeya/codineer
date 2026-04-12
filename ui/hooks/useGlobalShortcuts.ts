import { useEffect } from "react";

interface UseGlobalShortcutsOptions {
  activeTab: { id: string; type: string } | undefined;
  closeTab: (id: string) => void;
  sidebarVisible: boolean;
  setSidebarVisible: (v: boolean) => void;
  onToggleTerminal: () => void;
}

export function useGlobalShortcuts({
  activeTab,
  closeTab,
  sidebarVisible,
  setSidebarVisible,
  onToggleTerminal,
}: UseGlobalShortcutsOptions) {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "`" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        onToggleTerminal();
        return;
      }

      if (e.key !== "Escape") return;
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

      if (activeTab && activeTab.type !== "chat") {
        closeTab(activeTab.id);
      } else if (sidebarVisible) {
        setSidebarVisible(false);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeTab, closeTab, sidebarVisible, setSidebarVisible, onToggleTerminal]);
}
