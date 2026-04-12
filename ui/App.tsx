import { useCallback, useEffect, useRef, useState } from "react";
import { ActivityBar, type SidebarPanel } from "@/components/ActivityBar";
import { ChatView } from "@/components/ChatView";
import { InputBar } from "@/components/InputBar";
import { PreviewPanel } from "@/components/PreviewPanel";
import { AboutPage } from "@/components/SettingsPanel";
import { Sidebar } from "@/components/Sidebar";
import { StatusBar } from "@/components/StatusBar";
import { TabBar } from "@/components/TabBar";
import { TaskQueueBar } from "@/components/TaskQueueBar";
import {
  preloadTerminalModules,
  TerminalPanel,
  type TerminalPanelHandle,
} from "@/components/TerminalPanel";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useChatExecution } from "@/hooks/useChatExecution";
import { useGlobalShortcuts } from "@/hooks/useGlobalShortcuts";
import { useTabs } from "@/hooks/useTabs";
import { useTaskQueue } from "@/hooks/useTaskQueue";
import { useWorkspace } from "@/hooks/useWorkspace";
import { I18nContext, useI18nState } from "@/lib/i18n";
import { gitDiff, readFile, tryInvoke } from "@/lib/tauri";
import { initTheme } from "@/lib/theme";

function StandaloneAboutPage() {
  return (
    <div className="flex h-screen w-full items-center justify-center bg-background p-6">
      <div className="w-full max-w-md">
        <AboutPage />
      </div>
    </div>
  );
}

export default function App() {
  const i18n = useI18nState();

  if (window.location.hash === "#about") {
    return (
      <I18nContext.Provider value={i18n}>
        <StandaloneAboutPage />
      </I18nContext.Provider>
    );
  }
  return (
    <I18nContext.Provider value={i18n}>
      <MainApp />
    </I18nContext.Provider>
  );
}

function MainApp() {
  const [sidebarPanel, setSidebarPanel] = useState<SidebarPanel>("explorer");
  const [sidebarVisible, setSidebarVisible] = useState(false);
  const [sidebarWidth, setSidebarWidth] = useState(280);
  const [terminalVisible, setTerminalVisible] = useState(false);
  const terminalRef = useRef<TerminalPanelHandle>(null);
  const termCommandActive = useRef(false);

  // ── Workspace (project root, git, models, slash commands) ──
  const workspace = useWorkspace();

  // ── Tab management ──
  const {
    tabs,
    activeTab,
    activeId: activeTabId,
    open: openTab,
    close: closeTab,
    closeAll: _closeAllTabs,
    rename: renameTab,
    togglePin: togglePinTab,
    newChat,
    activate: activateTab,
    markUnread,
  } = useTabs();

  // ── Unified task queue ──
  const {
    tasks: queuedTasks,
    enqueue,
    dequeue,
    cancel,
    cancelAll,
    updateContent,
    setEditing,
    moveUp,
  } = useTaskQueue();

  // ── Chat execution (messages, streaming, submit, slash) ──
  const chat = useChatExecution({
    projectRoot: workspace.projectRoot,
    modelName: workspace.modelName,
    dequeue,
    enqueue,
    tabs,
    activeTab,
    markUnread,
    terminalRef,
    termCommandActive,
    setTerminalVisible,
  });

  // ── Init ──
  useEffect(() => {
    initTheme();
    preloadTerminalModules();
  }, []);

  // ── Global shortcuts ──
  const handleToggleTerminal = useCallback(() => setTerminalVisible((v) => !v), []);

  useGlobalShortcuts({
    activeTab,
    closeTab,
    sidebarVisible,
    setSidebarVisible,
    onToggleTerminal: handleToggleTerminal,
  });

  // ── Sidebar ──
  const handlePanelClick = useCallback(
    (panel: SidebarPanel) => {
      if (sidebarVisible && panel === sidebarPanel) {
        setSidebarVisible(false);
      } else {
        setSidebarPanel(panel);
        setSidebarVisible(true);
      }
    },
    [sidebarVisible, sidebarPanel],
  );

  // ── File / Diff open ──
  const handleOpenFile = useCallback(
    async (path: string) => {
      const content = await tryInvoke(() => readFile(path), "");
      const name = path.split("/").pop() || path;
      openTab({ title: name, type: "file", data: { path, content } });
    },
    [openTab],
  );

  const handleOpenDiff = useCallback(
    async (cwd: string, path: string) => {
      const diff = await tryInvoke(() => gitDiff(cwd, path), "");
      const name = path.split("/").pop() || path;
      openTab({ title: `${name} (diff)`, type: "diff", data: { path, diff } });
    },
    [openTab],
  );

  // ── Terminal queue ──
  const handleTerminalCommandDone = useCallback(
    (_exitCode: number | null) => {
      const next = dequeue("terminal");
      if (next) {
        terminalRef.current?.runCommand(next.content);
      } else {
        termCommandActive.current = false;
        terminalRef.current?.resetShell();
      }
    },
    [dequeue],
  );

  const handleRunNextTerminal = useCallback(() => {
    const next = dequeue("terminal");
    if (!next) return;
    terminalRef.current?.runCommand(next.content);
    termCommandActive.current = true;
    setTerminalVisible(true);
  }, [dequeue]);

  const handleForceExecute = useCallback(
    (taskId: number) => {
      cancel(taskId);
      chat.handleForceExecute(taskId, queuedTasks);
    },
    [queuedTasks, cancel, chat],
  );

  return (
    <TooltipProvider>
      <div className="flex h-screen w-screen overflow-hidden">
        <ActivityBar
          activePanel={sidebarPanel}
          sidebarVisible={sidebarVisible}
          onPanelClick={handlePanelClick}
        />

        {sidebarVisible && (
          <Sidebar
            panel={sidebarPanel}
            visible={sidebarVisible}
            width={sidebarWidth}
            onWidthChange={setSidebarWidth}
            onOpenFile={handleOpenFile}
            onOpenDiff={handleOpenDiff}
          />
        )}

        <div className="flex min-w-0 flex-1 flex-col">
          <TabBar
            tabs={tabs}
            activeId={activeTabId}
            onActivate={activateTab}
            onClose={closeTab}
            onNewChat={newChat}
            onRename={renameTab}
            onTogglePin={togglePinTab}
          />
          <div className="flex min-h-0 flex-1 flex-col">
            {activeTab?.type === "file" && activeTab.data ? (
              <PreviewPanel
                tab={{
                  type: "file",
                  path: activeTab.data.path ?? "",
                  content: activeTab.data.content ?? "",
                }}
                onClose={() => closeTab(activeTab.id)}
              />
            ) : activeTab?.type === "diff" && activeTab.data ? (
              <PreviewPanel
                tab={{
                  type: "diff",
                  path: activeTab.data.path ?? "",
                  diff: activeTab.data.diff ?? "",
                }}
                onClose={() => closeTab(activeTab.id)}
              />
            ) : (
              <ChatView
                messages={chat.messages}
                isStreaming={chat.isStreaming}
                streamingMode={chat.inputMode}
              />
            )}
          </div>
          <TerminalPanel
            ref={terminalRef}
            visible={terminalVisible}
            cwd={chat.sessionCwd || workspace.projectRoot || undefined}
            onClose={handleToggleTerminal}
            onCommandDone={handleTerminalCommandDone}
          />
          <TaskQueueBar
            tasks={queuedTasks}
            onCancel={cancel}
            onCancelAll={() => cancelAll()}
            onRunNextTerminal={handleRunNextTerminal}
            onUpdateContent={updateContent}
            onSetEditing={setEditing}
            onMoveUp={moveUp}
            onForceExecute={handleForceExecute}
          />
          <InputBar
            mode={chat.inputMode}
            onModeChange={chat.setInputMode}
            onSubmit={chat.handleSubmit}
            onSlashCommand={chat.handleSlashCommand}
            onStop={chat.handleStop}
            isStreaming={chat.isStreaming}
            slashCommands={workspace.slashCommands}
            queueSize={queuedTasks.length}
            projectRoot={chat.sessionCwd || workspace.projectRoot}
          />
          <StatusBar
            cwd={chat.sessionCwd || workspace.projectRoot || "~"}
            gitBranch={workspace.gitBranchName || undefined}
            model={workspace.modelName || undefined}
            mode={chat.inputMode}
            terminalVisible={terminalVisible}
            onToggleTerminal={handleToggleTerminal}
            onListBranches={workspace.handleListBranches}
            onSwitchBranch={workspace.handleSwitchBranch}
            modelGroups={workspace.modelGroups}
            onSelectModel={workspace.handleSelectModel}
          />
        </div>
      </div>
    </TooltipProvider>
  );
}
