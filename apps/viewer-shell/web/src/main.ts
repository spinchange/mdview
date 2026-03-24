import {
  EditorSelection,
  EditorState,
  Transaction,
  type Extension,
} from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { markdown } from "@codemirror/lang-markdown";
import { oneDark } from "@codemirror/theme-one-dark";
import { basicSetup } from "codemirror";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { renderDocument, type RenderedDocument } from "./viewer/Viewer";

type OpenedLocalLink = {
  path: string;
  markdown: string;
};

const THEME_STYLE_ID = "mdview-theme-tokens";
const THEME_EVENT = "mdview://theme-updated";
const FILE_CHANGED_EVENT = "mdview://file-changed";
const PREVIEW_DEBOUNCE_MS = 240;

const DEMO_MARKDOWN = `# mdview

Native-feeling markdown viewer for Windows.

## Why this exists
- Fast startup
- Shared style tokens
- Explorer preview roadmap

## Source bridge
Clicking headings can map to source lines with \`data-line-start\`.
`;

type AppState = {
  launchPath: string | null;
  sourceMarkdown: string;
  renderedDocument: RenderedDocument | null;
  quickEditEnabled: boolean;
  dirty: boolean;
  saving: boolean;
  saveError: string | null;
  pendingJumpLine: number | null;
  externalReloadBlocked: boolean;
  previewPending: boolean;
  searchPanelOpen: boolean;
  searchQuery: string;
  replaceQuery: string;
  searchMatchCount: number;
  activeSearchMatch: number;
};

type AppDomRefs = {
  app: HTMLElement | null;
  toolbarMeta: HTMLElement | null;
  quickEditButton: HTMLButtonElement | null;
  saveButton: HTMLButtonElement | null;
  searchButton: HTMLButtonElement | null;
  contentGrid: HTMLElement | null;
  viewerHost: HTMLElement | null;
  editorPanel: HTMLElement | null;
  editorHint: HTMLElement | null;
  editorSurface: HTMLElement | null;
  editorMessage: HTMLElement | null;
  searchPanelHost: HTMLElement | null;
  statusPill: HTMLElement | null;
  previewPill: HTMLElement | null;
};

type SearchMatch = {
  from: number;
  to: number;
};

declare global {
  interface Window {
    __MDVIEW_EDITOR_VIEW__?: EditorView | null;
  }
}

const appState: AppState = {
  launchPath: null,
  sourceMarkdown: DEMO_MARKDOWN,
  renderedDocument: null,
  quickEditEnabled: false,
  dirty: false,
  saving: false,
  saveError: null,
  pendingJumpLine: null,
  externalReloadBlocked: false,
  previewPending: false,
  searchPanelOpen: false,
  searchQuery: "",
  replaceQuery: "",
  searchMatchCount: 0,
  activeSearchMatch: 0,
};

const domRefs: AppDomRefs = {
  app: null,
  toolbarMeta: null,
  quickEditButton: null,
  saveButton: null,
  searchButton: null,
  contentGrid: null,
  viewerHost: null,
  editorPanel: null,
  editorHint: null,
  editorSurface: null,
  editorMessage: null,
  searchPanelHost: null,
  statusPill: null,
  previewPill: null,
};

let editorView: EditorView | null = null;
let previewTimer: number | null = null;
let previewRunId = 0;
let externalReloadRunId = 0;
let suppressEditorChangeEffects = false;

const editorTheme = EditorView.theme({
  "&": {
    height: "100%",
    borderRadius: "12px",
    backgroundColor: "transparent",
    color: "var(--mdv-text, #f3f3f3)",
  },
  ".cm-scroller": {
    overflow: "auto",
    fontFamily: "\"Cascadia Code\", \"Consolas\", monospace",
    lineHeight: "1.6",
  },
  ".cm-content": {
    minHeight: "58vh",
    padding: "14px",
  },
  ".cm-focused": {
    outline: "none",
  },
  ".cm-gutters": {
    backgroundColor: "transparent",
    color: "color-mix(in srgb, var(--mdv-text, #f3f3f3) 55%, transparent)",
    border: "none",
  },
  ".cm-activeLineGutter": {
    backgroundColor: "transparent",
  },
  ".cm-activeLine": {
    backgroundColor: "color-mix(in srgb, var(--mdv-accent, #0a84ff) 9%, transparent)",
  },
  ".cm-selectionBackground": {
    backgroundColor:
      "color-mix(in srgb, var(--mdv-accent, #0a84ff) 30%, transparent) !important",
  },
});

function isTauriRuntimeAvailable(): boolean {
  const w = window as Window & {
    __TAURI_INTERNALS__?: unknown;
    __TAURI__?: {
      core?: { invoke?: unknown };
      tauri?: { invoke?: unknown };
    };
  };

  const hasInternals = !!w.__TAURI_INTERNALS__;
  const hasCoreInvoke = typeof w.__TAURI__?.core?.invoke === "function";
  const hasLegacyInvoke = typeof w.__TAURI__?.tauri?.invoke === "function";
  return hasInternals || hasCoreInvoke || hasLegacyInvoke;
}

function ensureThemeStyleHost(): HTMLStyleElement {
  const existing = document.getElementById(THEME_STYLE_ID);
  if (existing instanceof HTMLStyleElement) {
    return existing;
  }

  const style = document.createElement("style");
  style.id = THEME_STYLE_ID;
  document.head.appendChild(style);
  return style;
}

function applyThemeCss(cssText: string): void {
  ensureThemeStyleHost().textContent = cssText;
}

async function loadInitialMarkdown(): Promise<void> {
  const [launchPath, launchMarkdown] = await Promise.all([
    invoke<string | null>("get_launch_path"),
    invoke<string | null>("read_launch_markdown"),
  ]);

  appState.launchPath = launchPath;
  appState.sourceMarkdown =
    typeof launchMarkdown === "string" ? launchMarkdown : DEMO_MARKDOWN;
  appState.renderedDocument = await invoke<RenderedDocument>("render_markdown", {
    markdown: appState.sourceMarkdown,
  });
  appState.previewPending = false;
  refreshSearchState();
}

function destroyEditorView(): void {
  editorView?.destroy();
  editorView = null;
  window.__MDVIEW_EDITOR_VIEW__ = null;
}

function editorHasFocus(): boolean {
  return !!editorView?.hasFocus;
}

function renderStatusMeta(): string {
  if (!appState.launchPath) {
    return "Demo document";
  }

  if (appState.saving) {
    return "Saving...";
  }

  if (appState.dirty) {
    return "Unsaved changes";
  }

  return "Saved";
}

function renderPreviewMeta(): string {
  if (!appState.quickEditEnabled) {
    return "Viewer mode";
  }

  if (appState.previewPending) {
    return "Preview pending";
  }

  return "Preview live";
}

function syncStatusPills(): void {
  if (domRefs.statusPill) {
    domRefs.statusPill.textContent = renderStatusMeta();
  }

  if (domRefs.previewPill) {
    domRefs.previewPill.textContent = renderPreviewMeta();
  }
}

function renderViewerHost(): void {
  if (!(domRefs.viewerHost instanceof HTMLElement) || !appState.renderedDocument) {
    return;
  }

  const options = {
    quickEditEnabled: appState.quickEditEnabled,
    onJumpToLine: (lineNumber) => {
      if (appState.quickEditEnabled && editorView) {
        jumpEditorToLine(lineNumber);
        return;
      }

      appState.quickEditEnabled = true;
      appState.pendingJumpLine = lineNumber;
      renderApp();
    },
    onOpenLocalLink: (href: string) => {
      void openLocalDocumentLink(href).catch((error) => {
        const message = describeError(error);
        console.error("failed to open local link", { href, error });
        window.alert(`Failed to open local link:\n${href}\n\n${message}`);
      });
    },
  };

  renderDocument(domRefs.viewerHost, appState.renderedDocument, options);
}

async function rerenderSource(): Promise<void> {
  appState.renderedDocument = await invoke<RenderedDocument>("render_markdown", {
    markdown: appState.sourceMarkdown,
  });
}

function describeError(error: unknown): string {
  if (error instanceof Error && error.message) {
    return error.message;
  }

  if (typeof error === "string" && error.length > 0) {
    return error;
  }

  return "Unknown error";
}

async function openLocalDocumentLink(href: string): Promise<void> {
  const opened = await invoke<OpenedLocalLink>("open_local_link", { href });
  appState.launchPath = opened.path;
  appState.sourceMarkdown = opened.markdown;
  appState.renderedDocument = await invoke<RenderedDocument>("render_markdown", {
    markdown: appState.sourceMarkdown,
  });
  appState.previewPending = false;
  appState.saveError = null;
  appState.externalReloadBlocked = false;
  refreshSearchState();

  if (editorView && editorView.state.doc.toString() !== appState.sourceMarkdown) {
    replaceEditorDocument(appState.sourceMarkdown);
  }

  renderApp();
}

function schedulePreviewRefresh(): void {
  appState.previewPending = true;
  syncStatusPills();

  if (previewTimer !== null) {
    window.clearTimeout(previewTimer);
  }

  previewTimer = window.setTimeout(() => {
    previewTimer = null;
    void refreshPreviewNow();
  }, PREVIEW_DEBOUNCE_MS);
}

async function refreshPreviewNow(): Promise<void> {
  const runId = ++previewRunId;
  await rerenderSource();

  if (runId !== previewRunId) {
    return;
  }

  appState.previewPending = false;
  renderViewerHost();
  syncStatusPills();
}

function lineNumberToOffset(lineNumber: number): number {
  if (!editorView) {
    return 0;
  }

  const safeLine = Math.max(1, Math.min(lineNumber, editorView.state.doc.lines));
  return editorView.state.doc.line(safeLine).from;
}

function jumpEditorToLine(lineNumber: number): void {
  if (!editorView) {
    return;
  }

  editorView.dispatch({
    selection: EditorSelection.cursor(lineNumberToOffset(lineNumber)),
    scrollIntoView: true,
  });
  editorView.focus();
}

function replaceEditorDocument(nextMarkdown: string): void {
  if (!editorView) {
    return;
  }

  const shouldRefocus = editorHasFocus();
  const currentSelection = editorView.state.selection;
  const nextLength = nextMarkdown.length;
  const selection = EditorSelection.create(
    currentSelection.ranges.map((range) =>
      EditorSelection.range(
        Math.min(range.anchor, nextLength),
        Math.min(range.head, nextLength)
      )
    ),
    Math.min(currentSelection.mainIndex, currentSelection.ranges.length - 1)
  );

  suppressEditorChangeEffects = true;
  editorView.dispatch({
    changes: {
      from: 0,
      to: editorView.state.doc.length,
      insert: nextMarkdown,
    },
    selection,
    annotations: Transaction.addToHistory.of(false),
  });
  suppressEditorChangeEffects = false;

  if (shouldRefocus) {
    editorView.focus();
  }
}

function getSearchMatches(source: string, query: string): SearchMatch[] {
  if (!query) {
    return [];
  }

  const matches: SearchMatch[] = [];
  let cursor = 0;
  while (cursor <= source.length) {
    const index = source.indexOf(query, cursor);
    if (index < 0) {
      break;
    }

    matches.push({ from: index, to: index + query.length });
    cursor = index + Math.max(1, query.length);
  }
  return matches;
}

function refreshSearchState(): void {
  const matches = getSearchMatches(appState.sourceMarkdown, appState.searchQuery);
  appState.searchMatchCount = matches.length;

  if (matches.length === 0) {
    appState.activeSearchMatch = 0;
    return;
  }

  if (editorView) {
    const selection = editorView.state.selection.main;
    const exactMatch = matches.findIndex(
      (match) => match.from === selection.from && match.to === selection.to
    );
    if (exactMatch >= 0) {
      appState.activeSearchMatch = exactMatch + 1;
      return;
    }

    const nextMatch = matches.findIndex((match) => match.from >= selection.from);
    appState.activeSearchMatch = (nextMatch >= 0 ? nextMatch : 0) + 1;
    return;
  }

  appState.activeSearchMatch = Math.min(
    Math.max(appState.activeSearchMatch, 1),
    matches.length
  );
}

function updateSearchSummary(): void {
  const summary = document.querySelector(".mdv-search__summary");
  if (!(summary instanceof HTMLElement)) {
    return;
  }

  if (!appState.searchQuery) {
    summary.textContent = "Enter text to search";
    return;
  }

  if (appState.searchMatchCount === 0) {
    summary.textContent = "No matches";
    return;
  }

  summary.textContent = `${appState.activeSearchMatch} of ${appState.searchMatchCount} matches`;
}

function focusSearchField(kind: "find" | "replace"): void {
  const selector =
    kind === "find" ? ".mdv-search__input--find" : ".mdv-search__input--replace";
  const field = document.querySelector(selector);
  if (field instanceof HTMLInputElement) {
    field.focus();
    field.select();
  }
}

function openSearchPanel(focus: "find" | "replace" = "find"): void {
  if (!appState.quickEditEnabled) {
    return;
  }

  appState.searchPanelOpen = true;
  syncSearchPanel();
  queueMicrotask(() => {
    focusSearchField(focus);
  });
}

function closeSearchPanel(): void {
  if (!appState.searchPanelOpen) {
    return;
  }

  appState.searchPanelOpen = false;
  syncSearchPanel();
  editorView?.focus();
}

function focusSearchMatch(direction: 1 | -1): void {
  if (!editorView || !appState.searchQuery) {
    return;
  }

  const matches = getSearchMatches(appState.sourceMarkdown, appState.searchQuery);
  if (matches.length === 0) {
    refreshSearchState();
    updateSearchSummary();
    return;
  }

  const currentIndex = Math.max(0, appState.activeSearchMatch - 1);
  const nextIndex =
    direction > 0
      ? (currentIndex + 1) % matches.length
      : (currentIndex - 1 + matches.length) % matches.length;
  const match = matches[nextIndex];

  editorView.dispatch({
    selection: EditorSelection.range(match.from, match.to),
    scrollIntoView: true,
  });
  editorView.focus();
  appState.activeSearchMatch = nextIndex + 1;
  updateSearchSummary();
}

function replaceCurrentMatch(): void {
  if (!editorView || !appState.searchQuery) {
    return;
  }

  const selection = editorView.state.selection.main;
  const selectedText = editorView.state.sliceDoc(selection.from, selection.to);
  if (selectedText === appState.searchQuery) {
    editorView.dispatch({
      changes: {
        from: selection.from,
        to: selection.to,
        insert: appState.replaceQuery,
      },
      selection: EditorSelection.cursor(selection.from + appState.replaceQuery.length),
      scrollIntoView: true,
    });
    return;
  }

  focusSearchMatch(1);
}

function replaceAllMatches(): void {
  if (!editorView || !appState.searchQuery) {
    return;
  }

  const matches = getSearchMatches(appState.sourceMarkdown, appState.searchQuery);
  if (matches.length === 0) {
    return;
  }

  const selection = editorView.state.selection.main;
  const activeMatchIndex = Math.max(0, appState.activeSearchMatch - 1);
  const activeMatch = matches[activeMatchIndex] ?? matches[0];

  editorView.dispatch({
    changes: matches.map((match) => ({
      from: match.from,
      to: match.to,
      insert: appState.replaceQuery,
    })),
    selection:
      selection.from === activeMatch.from && selection.to === activeMatch.to
        ? EditorSelection.range(
            activeMatch.from,
            activeMatch.from + appState.replaceQuery.length
          )
        : editorView.state.selection,
  });
}

function createEditorExtensions(): Extension[] {
  return [
    basicSetup,
    markdown(),
    oneDark,
    editorTheme,
    EditorView.lineWrapping,
    EditorView.updateListener.of((update) => {
      if (!update.docChanged) {
        return;
      }

      appState.sourceMarkdown = update.state.doc.toString();
      if (suppressEditorChangeEffects) {
        refreshSearchState();
        updateSearchSummary();
        return;
      }

      appState.dirty = true;
      appState.saveError = null;
      appState.externalReloadBlocked = false;
      refreshSearchState();
      updateSearchSummary();
      schedulePreviewRefresh();
      syncStatusPills();
    }),
  ];
}

function mountEditor(parent: HTMLElement): void {
  if (editorView) {
    if (editorView.dom.parentElement !== parent) {
      parent.replaceChildren(editorView.dom);
    }
  } else {
    editorView = new EditorView({
      state: EditorState.create({
        doc: appState.sourceMarkdown,
        extensions: createEditorExtensions(),
      }),
      parent,
    });
  }

  window.__MDVIEW_EDITOR_VIEW__ = editorView;

  if (appState.pendingJumpLine !== null) {
    jumpEditorToLine(appState.pendingJumpLine);
    appState.pendingJumpLine = null;
  }
}

async function flushPendingPreview(): Promise<void> {
  if (previewTimer !== null) {
    window.clearTimeout(previewTimer);
    previewTimer = null;
  }

  if (appState.previewPending) {
    await refreshPreviewNow();
  }
}

async function saveCurrentMarkdown(): Promise<void> {
  if (!appState.launchPath || appState.saving) {
    return;
  }

  appState.saving = true;
  appState.saveError = null;
  syncStatusPills();

  try {
    await flushPendingPreview();
    await invoke("write_launch_markdown", { markdown: appState.sourceMarkdown });
    appState.dirty = false;
    appState.externalReloadBlocked = false;
  } catch (error) {
    appState.saveError =
      error instanceof Error ? error.message : "Failed to save markdown file.";
  } finally {
    appState.saving = false;
    syncStatusPills();
    syncEditorMessage();
  }
}

function toggleQuickEdit(nextState?: boolean): void {
  appState.quickEditEnabled = nextState ?? !appState.quickEditEnabled;
  if (!appState.quickEditEnabled) {
    appState.searchPanelOpen = false;
  }
  renderApp();
}

function renderSearchPanel(): HTMLElement {
  const panel = document.createElement("section");
  panel.className = "mdv-search";

  const fields = document.createElement("div");
  fields.className = "mdv-search__fields";

  const findInput = document.createElement("input");
  findInput.className = "mdv-search__input mdv-search__input--find";
  findInput.type = "text";
  findInput.placeholder = "Find";
  findInput.value = appState.searchQuery;
  findInput.addEventListener("input", () => {
    appState.searchQuery = findInput.value;
    refreshSearchState();
    updateSearchSummary();
  });

  const replaceInput = document.createElement("input");
  replaceInput.className = "mdv-search__input mdv-search__input--replace";
  replaceInput.type = "text";
  replaceInput.placeholder = "Replace";
  replaceInput.value = appState.replaceQuery;
  replaceInput.addEventListener("input", () => {
    appState.replaceQuery = replaceInput.value;
  });

  fields.appendChild(findInput);
  fields.appendChild(replaceInput);

  const actions = document.createElement("div");
  actions.className = "mdv-search__actions";

  const previousButton = document.createElement("button");
  previousButton.type = "button";
  previousButton.className = "mdv-button mdv-button--secondary";
  previousButton.textContent = "Previous";
  previousButton.addEventListener("click", () => {
    focusSearchMatch(-1);
  });

  const nextButton = document.createElement("button");
  nextButton.type = "button";
  nextButton.className = "mdv-button mdv-button--secondary";
  nextButton.textContent = "Next";
  nextButton.addEventListener("click", () => {
    focusSearchMatch(1);
  });

  const replaceButton = document.createElement("button");
  replaceButton.type = "button";
  replaceButton.className = "mdv-button mdv-button--secondary";
  replaceButton.textContent = "Replace";
  replaceButton.addEventListener("click", () => {
    replaceCurrentMatch();
  });

  const replaceAllButton = document.createElement("button");
  replaceAllButton.type = "button";
  replaceAllButton.className = "mdv-button mdv-button--secondary";
  replaceAllButton.textContent = "Replace All";
  replaceAllButton.addEventListener("click", () => {
    replaceAllMatches();
  });

  const closeButton = document.createElement("button");
  closeButton.type = "button";
  closeButton.className = "mdv-button mdv-button--secondary";
  closeButton.textContent = "Close";
  closeButton.addEventListener("click", () => {
    closeSearchPanel();
  });

  actions.appendChild(previousButton);
  actions.appendChild(nextButton);
  actions.appendChild(replaceButton);
  actions.appendChild(replaceAllButton);
  actions.appendChild(closeButton);

  const summary = document.createElement("p");
  summary.className = "mdv-search__summary";

  panel.appendChild(fields);
  panel.appendChild(actions);
  panel.appendChild(summary);

  refreshSearchState();
  queueMicrotask(() => {
    updateSearchSummary();
  });
  return panel;
}

function syncSearchPanel(): void {
  if (!(domRefs.searchPanelHost instanceof HTMLElement) || !appState.quickEditEnabled) {
    return;
  }

  domRefs.searchPanelHost.replaceChildren();
  if (appState.searchPanelOpen) {
    domRefs.searchPanelHost.appendChild(renderSearchPanel());
  }
}

function syncEditorMessage(): void {
  if (!(domRefs.editorMessage instanceof HTMLElement)) {
    return;
  }

  const message =
    appState.saveError ??
    (appState.externalReloadBlocked
      ? "File changed on disk while you had unsaved edits. Save to overwrite with your current changes."
      : null);

  domRefs.editorMessage.textContent = message ?? "";
  domRefs.editorMessage.hidden = !message;
}

function syncToolbar(): void {
  if (domRefs.toolbarMeta) {
    domRefs.toolbarMeta.textContent = appState.launchPath ?? "No file launched";
  }

  if (domRefs.quickEditButton) {
    domRefs.quickEditButton.className = appState.quickEditEnabled
      ? "mdv-button mdv-button--primary"
      : "mdv-button mdv-button--secondary";
    domRefs.quickEditButton.textContent = appState.quickEditEnabled
      ? "Exit Quick Edit"
      : "Quick Edit";
  }

  if (domRefs.saveButton) {
    domRefs.saveButton.disabled = !appState.launchPath || !appState.quickEditEnabled;
  }

  if (domRefs.searchButton) {
    domRefs.searchButton.disabled = !appState.quickEditEnabled;
  }
}

function renderApp(): void {
  const app = document.getElementById("app");
  if (!(app instanceof HTMLElement) || !appState.renderedDocument) {
    return;
  }

  const hadEditorFocus = editorHasFocus();
  domRefs.app = app;
  app.innerHTML = "";

  const workspace = document.createElement("section");
  workspace.className = "mdv-workspace";

  const toolbar = document.createElement("header");
  toolbar.className = "mdv-toolbar";

  const titleGroup = document.createElement("div");
  titleGroup.className = "mdv-toolbar__title-group";

  const title = document.createElement("h1");
  title.className = "mdv-toolbar__title";
  title.textContent = "mdview";

  const meta = document.createElement("p");
  meta.className = "mdv-toolbar__meta";
  meta.textContent = appState.launchPath ?? "No file launched";
  domRefs.toolbarMeta = meta;

  titleGroup.appendChild(title);
  titleGroup.appendChild(meta);

  const actions = document.createElement("div");
  actions.className = "mdv-toolbar__actions";

  const quickEditButton = document.createElement("button");
  quickEditButton.type = "button";
  quickEditButton.className = appState.quickEditEnabled
    ? "mdv-button mdv-button--primary"
    : "mdv-button mdv-button--secondary";
  quickEditButton.textContent = appState.quickEditEnabled
    ? "Exit Quick Edit"
    : "Quick Edit";
  quickEditButton.title = "Toggle Quick Edit (Ctrl+E)";
  quickEditButton.addEventListener("click", () => {
    toggleQuickEdit();
  });
  domRefs.quickEditButton = quickEditButton;

  const saveButton = document.createElement("button");
  saveButton.type = "button";
  saveButton.className = "mdv-button mdv-button--secondary";
  saveButton.textContent = "Save";
  saveButton.title = "Save current markdown (Ctrl+S)";
  saveButton.disabled = !appState.launchPath || !appState.quickEditEnabled;
  saveButton.addEventListener("click", () => {
    void saveCurrentMarkdown();
  });
  domRefs.saveButton = saveButton;

  const searchButton = document.createElement("button");
  searchButton.type = "button";
  searchButton.className = "mdv-button mdv-button--secondary";
  searchButton.textContent = "Find / Replace";
  searchButton.disabled = !appState.quickEditEnabled;
  searchButton.title = "Open find and replace (Ctrl+F / Ctrl+H)";
  searchButton.addEventListener("click", () => {
    openSearchPanel("find");
  });
  domRefs.searchButton = searchButton;

  const status = document.createElement("span");
  status.className = "mdv-status-pill";
  status.textContent = renderStatusMeta();
  domRefs.statusPill = status;

  const preview = document.createElement("span");
  preview.className = "mdv-status-pill";
  preview.textContent = renderPreviewMeta();
  domRefs.previewPill = preview;

  actions.appendChild(quickEditButton);
  actions.appendChild(saveButton);
  actions.appendChild(searchButton);
  actions.appendChild(status);
  actions.appendChild(preview);

  toolbar.appendChild(titleGroup);
  toolbar.appendChild(actions);

  const contentGrid = document.createElement("div");
  contentGrid.className = appState.quickEditEnabled
    ? "mdv-layout mdv-layout--editing"
    : "mdv-layout";
  domRefs.contentGrid = contentGrid;

  const viewerHost = document.createElement("div");
  viewerHost.className = "mdv-viewer-host";
  domRefs.viewerHost = viewerHost;
  renderViewerHost();
  contentGrid.appendChild(viewerHost);

  if (appState.quickEditEnabled) {
    const editorPanel = document.createElement("section");
    editorPanel.className = "mdv-editor";

    const editorHeader = document.createElement("div");
    editorHeader.className = "mdv-editor__header";

    const editorTitle = document.createElement("div");
    editorTitle.className = "mdv-editor__title";
    editorTitle.textContent = "Quick Edit";

    const editorHint = document.createElement("div");
    editorHint.className = "mdv-editor__hint";
    editorHint.textContent = appState.launchPath
      ? "CodeMirror editor with debounced live preview, jump-to-line, and quick find/replace."
      : "Editing demo content only. Launch a file to enable saving.";

    editorHeader.appendChild(editorTitle);
    editorHeader.appendChild(editorHint);
    domRefs.editorHint = editorHint;
    editorPanel.appendChild(editorHeader);

    const searchPanelHost = document.createElement("div");
    domRefs.searchPanelHost = searchPanelHost;
    editorPanel.appendChild(searchPanelHost);

    const surface = document.createElement("div");
    surface.className = "mdv-editor__surface";
    domRefs.editorSurface = surface;
    editorPanel.appendChild(surface);

    const message = document.createElement("p");
    message.className = "mdv-editor__message";
    domRefs.editorMessage = message;
    editorPanel.appendChild(message);

    domRefs.editorPanel = editorPanel;
    contentGrid.appendChild(editorPanel);
    mountEditor(surface);
    syncSearchPanel();
    syncEditorMessage();
    if (hadEditorFocus) {
      editorView?.focus();
    }
  } else {
    domRefs.editorPanel = null;
    domRefs.editorHint = null;
    domRefs.editorSurface = null;
    domRefs.editorMessage = null;
    domRefs.searchPanelHost = null;
  }

  workspace.appendChild(toolbar);
  workspace.appendChild(contentGrid);
  app.appendChild(workspace);
  syncToolbar();
  syncStatusPills();
}

function isEditableShortcut(event: KeyboardEvent): boolean {
  const target = event.target;
  return !(
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}

async function bootstrapThemeBridge(): Promise<void> {
  if (!isTauriRuntimeAvailable()) {
    console.info("[mdview] Running in standalone browser mode. Skipping theme sync.");
    const app = document.getElementById("app");
    if (app instanceof HTMLElement) {
      app.textContent = "mdview standalone mode";
    }
    return;
  }

  const unlistenTheme = await listen<string>(THEME_EVENT, (event) => {
    if (typeof event.payload === "string" && event.payload.length > 0) {
      applyThemeCss(event.payload);
    }
  });

  const app = document.getElementById("app");
  if (!(app instanceof HTMLElement)) {
    throw new Error("missing #app container");
  }

  const unlistenFileChanged = await listen(FILE_CHANGED_EVENT, async () => {
    const runId = ++externalReloadRunId;

    try {
      if (appState.dirty) {
        appState.externalReloadBlocked = true;
        syncEditorMessage();
        return;
      }

      const nextMarkdownResult = await invoke<string | null>("read_launch_markdown");
      if (runId !== externalReloadRunId || appState.dirty) {
        return;
      }

      const nextMarkdown =
        typeof nextMarkdownResult === "string" ? nextMarkdownResult : DEMO_MARKDOWN;
      const nextRenderedDocument = await invoke<RenderedDocument>("render_markdown", {
        markdown: nextMarkdown,
      });
      if (runId !== externalReloadRunId || appState.dirty) {
        return;
      }

      appState.sourceMarkdown = nextMarkdown;
      appState.renderedDocument = nextRenderedDocument;
      appState.previewPending = false;
      refreshSearchState();
      if (editorView && editorView.state.doc.toString() !== appState.sourceMarkdown) {
        replaceEditorDocument(appState.sourceMarkdown);
      }
      renderViewerHost();
      syncStatusPills();
      syncEditorMessage();
    } catch (error) {
      console.error("[mdview] failed to reload markdown after file change", error);
    }
  });

  try {
    const initialCss = await invoke<string>("get_initial_theme_css");
    applyThemeCss(initialCss);
    await loadInitialMarkdown();
    renderApp();
  } finally {
    await invoke("window_ready");
  }

  window.addEventListener("keydown", (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "e") {
      event.preventDefault();
      toggleQuickEdit();
      return;
    }

    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
      if (!appState.quickEditEnabled || !appState.launchPath) {
        return;
      }

      event.preventDefault();
      void saveCurrentMarkdown();
      return;
    }

    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
      if (!appState.quickEditEnabled) {
        return;
      }

      event.preventDefault();
      openSearchPanel("find");
      return;
    }

    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "h") {
      if (!appState.quickEditEnabled) {
        return;
      }

      event.preventDefault();
      openSearchPanel("replace");
      return;
    }

    if (event.key === "Escape" && appState.searchPanelOpen) {
      event.preventDefault();
      closeSearchPanel();
      return;
    }

    if (
      event.key === "Escape" &&
      appState.quickEditEnabled &&
      isEditableShortcut(event)
    ) {
      toggleQuickEdit(false);
    }
  });

  void unlistenTheme;
  void unlistenFileChanged;
}

void bootstrapThemeBridge();
