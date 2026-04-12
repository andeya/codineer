const en = {
  // ── Common ──
  common: {
    save: "Save",
    saved: "Saved!",
    cancel: "Cancel",
    close: "Close",
    delete: "Delete",
    edit: "Edit",
    retry: "Retry",
    loading: "Loading...",
    confirm: "Confirm",
    clear: "Clear",
    select: "Select",
    search: "Search",
    filter: "Filter...",
    noResults: "No results found.",
    send: "Send",
    queue: "Queue",
    stop: "Stop",
    expand: "Expand",
    clearing: "Clearing...",
    saving: "Saving...",
    copy: "Copy",
    change: "Change",
    unknown: "unknown",
    current: "current",
    thinking: "Thinking...",
  },

  // ── Modes ──
  mode: {
    shell: "Shell",
    chat: "Chat",
    agent: "Agent",
  },

  // ── Activity bar / Sidebar ──
  nav: {
    explorer: "Explorer",
    search: "Search",
    git: "Source Control",
    context: "Context",
    memory: "Memory",
    settings: "Settings",
    contextHint: "Project context and @ mentions.",
    memoryHint: "Stored project memory.",
  },

  // ── Tab bar ──
  tab: {
    newChat: "New Chat (Ctrl+T)",
    rename: "Rename",
    pin: "Pin",
    unpin: "Unpin",
    close: "Close",
    chat: "Chat",
    diffSuffix: "(diff)",
  },

  // ── Input bar ──
  input: {
    placeholderShell: "Enter a command...",
    placeholderChat: "Ask anything... (/ commands, @ context)",
    placeholderAgent: "Describe a goal for the agent...",
    placeholderWithChips: "Type your message...",
    tabToComplete: "Tab to complete",
    emptyTabToSwitch: "Empty + Tab to switch",
    attachTooltip: "Attach files or images",
    stopTooltip: "Stop current task",
    selectFolder: "Select this folder",
    noMatchingFiles: "No matching files",
    filesAndFolders: "Files & Folders",
    contextSection: "Context",
    preview: "Preview",
  },

  // ── Status bar ──
  status: {
    terminal: "Terminal",
    toggleTerminal: "Toggle Terminal (Ctrl+`)",
    modeShell: "SHELL",
    modeChat: "CHAT",
    modeAgent: "AGENT",
    noModel: "No model",
  },

  // ── Chat view ──
  chat: {
    welcome: "Welcome to Aineer",
    subtitle: "Agentic Development Environment",
    shellDesc: "Run commands directly",
    chatDesc: "Ask anything, get answers",
    agentDesc: "Autonomous multi-step tasks",
    hintSend: "send",
    hintSwitchMode: "switch mode",
    hintCommands: "commands",
    hintMentions: "mentions",
    showMarkdown: "Show rendered markdown",
    showRaw: "Show raw text",
    markdownBtn: "MD",
    rawBtn: "Raw",
    copyQ: "Q",
    copyA: "A",
    copyQA: "QA",
    timeout: "timeout",
    steps: "steps",
    stoppedByUser: "[Command stopped by user]",
    noOutput: "(no output)",
  },

  // ── Settings panel ──
  settings: {
    appearance: "Appearance",
    modelsIntelligence: "Models & Intelligence",
    terminal: "Terminal",
    safety: "Safety",
    cache: "Cache",
    json: "JSON",
    about: "About",

    // Appearance
    theme: "Theme",
    themeLight: "Light",
    themeDark: "Dark",
    themeSystem: "System",
    themeGithubLight: "GitHub Light",
    themeSolarizedLight: "Solarized Light",
    themeOneDarkPro: "One Dark Pro",
    themeDracula: "Dracula",
    themeSystemDay: "daytime",
    themeSystemNight: "nighttime",
    interface: "Interface",
    fontSize: "UI Font Size",
    fontSizeHint: "Global font size (10-24)",
    language: "Language",
    langEn: "English",
    langZh: "简体中文",
    restoreSession: "Restore Session on Startup",

    // System tray
    systemTray: "System Tray",
    closeToTray: "Minimize to Tray on Close",
    closeToTrayHint:
      "When enabled, closing the window will minimize to system tray instead of quitting.",

    // Models
    defaultModel: "Default Model",
    model: "Model",
    modelHint: "The AI model to use for chat (e.g., claude-sonnet-4-20250514).",
    modelPlaceholder: "auto",
    thinkingMode: "Thinking Mode",
    thinkingModeHint: "Enable extended thinking for complex tasks",
    maxContextTokens: "Max Context Tokens",
    modelAliases: "Model Aliases",
    modelAliasesEmpty: "No aliases configured. Edit JSON to add model aliases.",
    fallbackModels: "Fallback Models",
    fallbackPlaceholder: "ollama/qwen3-coder",
    providers: "Providers",
    customProviders: "Custom Providers",
    modelsCount: "models",
    pasteApiKey: "Paste API key...",
    setKey: "Set Key",
    localNoKey: "Local (no key needed)",

    // Terminal
    shellSection: "Shell",
    shellPath: "Shell Path",
    shellPathHint: "The shell executable to use (leave empty for auto-detection).",
    shellPathPlaceholder: "Auto-detect",
    shellArgs: "Shell Arguments",
    shellArgsPlaceholder: "--login",
    termAppearance: "Appearance",
    fontFamily: "Font Family",
    fontFamilyPlaceholder: "Berkeley Mono, JetBrains Mono, Menlo",
    termFontSize: "Font Size",
    cursorShape: "Cursor Shape",
    cursorBlock: "Block",
    cursorBar: "Bar",
    cursorUnderline: "Underline",
    buffer: "Buffer",
    scrollbackLines: "Scrollback Lines",
    scrollbackHint: "Number of lines to keep in terminal scrollback buffer.",

    // Safety
    permissionMode: "Permission Mode",
    defaultPermission: "Default Permission Mode",
    defaultPermissionHint: "Controls how the AI interacts with your system.",
    sandbox: "Sandbox",
    enableSandbox: "Enable Sandbox",
    enableSandboxHint: "Isolate AI tool execution in a sandboxed environment",
    filesystemMode: "Filesystem Mode",
    workspaceOnly: "Workspace Only",
    fullAccess: "Full Access",
    networkIsolation: "Network Isolation",
    credentials: "Credentials",
    autoDiscoverCreds: "Auto-discover Claude Code Credentials",

    // Cache
    storageOverview: "Storage Overview",
    attachments: "Attachments",
    chatHistory: "Chat History",
    cachePath: "Cache path: ",
    loadingCacheInfo: "Loading cache info...",
    clearCache: "Clear Cache",
    clearAttachments: "Clear Attachments",
    clearChatHistory: "Clear Chat History",
    clearAll: "Clear All",
    chatSessions: "Chat Sessions",
    deleteSession: "Delete session",
    confirmClear: "Are you sure? This cannot be undone.",
    items: "items",
    autoCleanup: "Scheduled Cleanup",
    autoCleanupHint: "Automatically clear cache on a recurring schedule when the app starts.",
    cleanupInterval: "Interval",
    cleanupTarget: "Target",
    cleanupOff: "Off",
    cleanupDaily: "Daily",
    cleanupWeekly: "Weekly",
    cleanupMonthly: "Monthly",
    cleanupTargetAttachments: "Attachments Only",
    cleanupTargetHistory: "Chat History Only",
    cleanupTargetAll: "All Cache",
    cleanupLastRun: "Last cleanup:",
    cleanupNever: "Never",

    // JSON
    jsonTitle: "Raw JSON Configuration",
    jsonDesc: "Edit the raw settings JSON directly. Changes are saved when you click Save.",

    // About
    application: "Application",
    version: "Version",
    channel: "Channel",
    displayName: "Display Name",
    links: "Links",
    github: "GitHub",
    homepage: "Homepage",
    reportIssue: "Report Issue",
    releases: "Releases",
    license: "License",
    licenseValue: "Apache-2.0",
    channelDev: "dev",
    channelNightly: "nightly",
    channelPreview: "preview",
    channelStable: "stable",
    displayNameFallback: "Aineer Dev",
  },

  // ── File tree ──
  fileTree: {
    loading: "Loading...",
    noFiles: "No files found.",
  },

  // ── Search panel ──
  searchPanel: {
    placeholder: "Search files...",
    searchContent: "Search file content",
    resultCount: "{count} result(s)",
  },

  // ── Git panel ──
  git: {
    modified: "Modified",
    added: "Added",
    deleted: "Deleted",
    renamed: "Renamed",
    untracked: "Untracked",
    noProjectRoot: "Could not determine project root.",
    loadingGit: "Loading git status...",
    loadingBranches: "Loading branches...",
    noBranches: "No branches found.",
    changes: "Changes",
    changesWithCount: "Changes ({count})",
    workingTreeClean: "Working tree clean",
  },

  // ── Preview panel ──
  preview: {
    emptyFile: "Empty file: {path}",
    noChanges: "No changes detected.",
  },

  // ── Task queue ──
  taskQueue: {
    queued: "{count} queued",
    clearAll: "Clear all queued tasks",
    clearAllBtn: "Clear all",
    confirmEdit: "Confirm edit",
    cancelEdit: "Cancel edit",
    runNow: "Run this command now (ensure previous finished)",
    forceExecute: "Force execute: stop current and run this immediately",
    editCommand: "Edit command",
    moveUp: "Move up in queue",
    removeFromQueue: "Remove from queue",
  },

  // ── Terminal panel ──
  terminalPanel: {
    running: "Running: {cmd}",
    terminal: "Terminal",
    exited: "(exited)",
    processExited: "[Process exited]",
    exitedBracket: "[exited]",
    exitedBracketWithCode: "[exited ({code})]",
    failedToStart: "Failed to start: {error}",
    failedToStartShell: "Failed to start shell: {error}",
    mockMode: "Terminal (mock mode)",
  },

  // ── Tool component ──
  tool: {
    processing: "Processing",
    ready: "Ready",
    completed: "Completed",
    error: "Error",
    pending: "Pending",
    input: "Input",
    output: "Output",
    processingToolCall: "Processing tool call...",
    callId: "Call ID: {id}",
  },

  // ── Loader ──
  loader: {
    loading: "Loading",
    thinking: "Thinking",
  },

  // ── Units ──
  units: {
    b: "B",
    kb: "KB",
    mb: "MB",
    gb: "GB",
  },
} as const;

type DeepStringify<T> = {
  [K in keyof T]: T[K] extends string ? string : DeepStringify<T[K]>;
};

export type Translations = DeepStringify<typeof en>;
export default en;
