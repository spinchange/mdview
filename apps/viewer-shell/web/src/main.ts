import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { renderDocument, type RenderedDocument } from "./viewer/Viewer";

const THEME_STYLE_ID = "mdview-theme-tokens";
const THEME_EVENT = "mdview://theme-updated";
const FILE_CHANGED_EVENT = "mdview://file-changed";
const DEMO_MARKDOWN = `# mdview

Native-feeling markdown viewer for Windows.

## Why this exists
- Fast startup
- Shared style tokens
- Explorer preview roadmap

## Source bridge
Clicking headings can map to source lines with \`data-line-start\`.
`;

async function loadInitialMarkdown(): Promise<string> {
  const launchMarkdown = await invoke<string | null>("read_launch_markdown");
  if (typeof launchMarkdown === "string") {
    return launchMarkdown;
  }
  return DEMO_MARKDOWN;
}

async function renderCurrentDocument(app: HTMLElement): Promise<void> {
  const markdown = await loadInitialMarkdown();
  const rendered = await invoke<RenderedDocument>("render_markdown", { markdown });
  renderDocument(app, rendered);
}

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
  const host = ensureThemeStyleHost();
  host.textContent = cssText;
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

  const unlisten = await listen<string>(THEME_EVENT, (event) => {
    if (typeof event.payload === "string" && event.payload.length > 0) {
      applyThemeCss(event.payload);
    }
  });

  const app = document.getElementById("app");
  if (!(app instanceof HTMLElement)) {
    throw new Error("missing #app container");
  }

  const unlistenFileChanged = await listen(FILE_CHANGED_EVENT, async () => {
    try {
      await renderCurrentDocument(app);
    } catch (error) {
      console.error("[mdview] failed to reload markdown after file change", error);
    }
  });

  try {
    const initialCss = await invoke<string>("get_initial_theme_css");
    applyThemeCss(initialCss);

    await renderCurrentDocument(app);
  } finally {
    await invoke("window_ready");
  }

  // Keep the listener active for runtime theme changes.
  void unlisten;
  void unlistenFileChanged;
}

void bootstrapThemeBridge();
