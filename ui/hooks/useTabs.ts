import { useCallback, useState } from "react";

export type TabType = "chat" | "file" | "diff";

export interface Tab {
  id: string;
  title: string;
  type: TabType;
  pinned: boolean;
  /** True when this tab has new content the user hasn't seen yet */
  unread: boolean;
  data?: {
    path?: string;
    content?: string;
    diff?: string;
  };
}

let _tabSeq = 0;
function nextTabId(): string {
  return `tab-${++_tabSeq}-${Date.now()}`;
}

const DEFAULT_TAB: Tab = {
  id: "tab-default-chat",
  title: "Chat",
  type: "chat",
  pinned: false,
  unread: false,
};

export function useTabs() {
  const [tabs, setTabs] = useState<Tab[]>([DEFAULT_TAB]);
  const [activeId, setActiveId] = useState(DEFAULT_TAB.id);

  const activeTab = tabs.find((t) => t.id === activeId) ?? tabs[0] ?? null;

  const open = useCallback(
    (tab: Omit<Tab, "id" | "pinned" | "unread">): string => {
      let existingId: string | undefined;

      if (tab.type === "file" && tab.data?.path) {
        const existing = tabs.find((t) => t.type === "file" && t.data?.path === tab.data?.path);
        existingId = existing?.id;
      } else if (tab.type === "diff" && tab.data?.path) {
        const existing = tabs.find((t) => t.type === "diff" && t.data?.path === tab.data?.path);
        existingId = existing?.id;
      }

      if (existingId) {
        setActiveId(existingId);
        setTabs((prev) =>
          prev.map((t) => (t.id === existingId ? { ...t, data: tab.data, title: tab.title } : t)),
        );
        return existingId;
      }

      const id = nextTabId();
      const newTab: Tab = { ...tab, id, pinned: false, unread: false };
      setTabs((prev) => [...prev, newTab]);
      setActiveId(id);
      return id;
    },
    [tabs],
  );

  const close = useCallback(
    (id: string) => {
      setTabs((prev) => {
        const tab = prev.find((t) => t.id === id);
        if (!tab || tab.pinned) return prev;

        const next = prev.filter((t) => t.id !== id);
        if (next.length === 0) {
          const fresh: Tab = { ...DEFAULT_TAB, id: nextTabId() };
          setActiveId(fresh.id);
          return [fresh];
        }

        if (activeId === id) {
          const idx = prev.findIndex((t) => t.id === id);
          const newActive = next[Math.min(idx, next.length - 1)];
          setActiveId(newActive.id);
        }

        return next;
      });
    },
    [activeId],
  );

  const closeAll = useCallback(() => {
    setTabs((prev) => {
      const pinned = prev.filter((t) => t.pinned);
      if (pinned.length === 0) {
        const fresh: Tab = { ...DEFAULT_TAB, id: nextTabId() };
        setActiveId(fresh.id);
        return [fresh];
      }
      setActiveId(pinned[0].id);
      return pinned;
    });
  }, []);

  const rename = useCallback((id: string, title: string) => {
    setTabs((prev) => prev.map((t) => (t.id === id ? { ...t, title } : t)));
  }, []);

  const togglePin = useCallback((id: string) => {
    setTabs((prev) => {
      const updated = prev.map((t) => (t.id === id ? { ...t, pinned: !t.pinned } : t));
      const pinned = updated.filter((t) => t.pinned);
      const unpinned = updated.filter((t) => !t.pinned);
      return [...pinned, ...unpinned];
    });
  }, []);

  const newChat = useCallback((): string => {
    const id = nextTabId();
    const tab: Tab = { id, title: "Chat", type: "chat", pinned: false, unread: false };
    setTabs((prev) => [...prev, tab]);
    setActiveId(id);
    return id;
  }, []);

  const activate = useCallback((id: string) => {
    setActiveId(id);
    setTabs((prev) => prev.map((t) => (t.id === id ? { ...t, unread: false } : t)));
  }, []);

  const markUnread = useCallback(
    (id: string) => {
      if (id === activeId) return;
      setTabs((prev) => prev.map((t) => (t.id === id ? { ...t, unread: true } : t)));
    },
    [activeId],
  );

  return {
    tabs,
    activeTab,
    activeId,
    open,
    close,
    closeAll,
    rename,
    togglePin,
    newChat,
    activate,
    markUnread,
  };
}
