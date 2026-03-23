import type { Page } from "@playwright/test";

const sampleMarkdown = `# mdview

## First section
Body copy

## Second section
More body
`;

export async function installTauriMock(page: Page): Promise<void> {
  await page.addInitScript(({ markdown }) => {
    type HeadingSpan = {
      level: number;
      text: string;
      line_start: number;
      line_end: number;
      column_start: number;
      column_end: number;
    };

    const state = {
      launchPath: "C:\\Users\\user\\mdview\\tests\\fixtures\\markdown\\sample.md",
      markdown,
      savedMarkdown: markdown,
      defaultAppsOpened: false,
      lastOpenedUrl: null as string | null,
      lastOpenedLocalHref: null as string | null,
      windowReadyCalls: 0,
      renderCallCount: 0,
      readLaunchMarkdownDelays: [] as number[],
      listeners: new Map<string, Set<(payload: unknown) => void>>(),
      callbacks: new Map<number, (payload: unknown) => void>(),
      nextCallbackId: 1,
    };

    function emit(eventName: string, payload?: unknown) {
      const handlers = state.listeners.get(eventName);
      handlers?.forEach((handler) => handler(payload));
    }

    function renderMarkdown(source: string): {
      html: string;
      headings: HeadingSpan[];
      is_blank: boolean;
    } {
      if (source.trim().length === 0) {
        return { html: "", headings: [], is_blank: true };
      }

      const lines = source.split(/\r?\n/);
      const headings: HeadingSpan[] = [];
      const html: string[] = [];

      for (let index = 0; index < lines.length; index += 1) {
        let line = lines[index];
        const headingMatch = /^(#{1,6})\s+(.*)$/.exec(line);
        if (headingMatch) {
          const level = headingMatch[1].length;
          const text = headingMatch[2];
          headings.push({
            level,
            text,
            line_start: index + 1,
            line_end: index + 1,
            column_start: 1,
            column_end: line.length,
          });
          // Process links in heading text too
          const processedText = text.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2">$1</a>');
          html.push(`<h${level}>${processedText}</h${level}>`);
          continue;
        }

        if (line.trim().length > 0) {
          const processedLine = line.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2">$1</a>');
          html.push(`<p>${processedLine}</p>`);
        }
      }

      return {
        html: html.join(""),
        headings,
        is_blank: false,
      };
    }

    function invoke(command: string, args?: Record<string, unknown>) {
      switch (command) {
        case "get_launch_path":
          return Promise.resolve(state.launchPath);
        case "read_launch_markdown":
          return new Promise((resolve) => {
            const markdownAtRead = state.markdown;
            const delay = state.readLaunchMarkdownDelays.shift() ?? 0;
            window.setTimeout(() => {
              resolve(markdownAtRead);
            }, delay);
          });
        case "render_markdown":
          state.renderCallCount += 1;
          return Promise.resolve(renderMarkdown(String(args?.markdown ?? "")));
        case "get_initial_theme_css":
          return Promise.resolve(":root { --mdv-bg: #1e1e1e; --mdv-text: #f3f3f3; --mdv-surface: #252526; --mdv-border: #3c3c3c; --mdv-accent: #4ea1ff; }");
        case "window_ready":
          state.windowReadyCalls += 1;
          return Promise.resolve();
        case "open_default_apps_settings":
          state.defaultAppsOpened = true;
          return Promise.resolve();
        case "open_external_link":
          state.lastOpenedUrl = String(args?.url ?? "");
          return Promise.resolve();
        case "open_local_link":
          state.lastOpenedLocalHref = String(args?.href ?? "");
          return Promise.resolve();
        case "write_launch_markdown":
          state.markdown = String(args?.markdown ?? "");
          state.savedMarkdown = state.markdown;
          return Promise.resolve();
        case "plugin:event|listen": {
          const eventName = String(args?.event ?? "");
          const handlerId = Number(args?.handler);
          const handlers = state.listeners.get(eventName) ?? new Set();
          handlers.add((payload) => {
            const callback = state.callbacks.get(handlerId);
            callback?.({ event: eventName, id: handlerId, payload });
          });
          state.listeners.set(eventName, handlers);
          return Promise.resolve(handlerId);
        }
        case "plugin:event|unlisten":
          return Promise.resolve();
        default:
          return Promise.reject(new Error(`Unhandled invoke: ${command}`));
      }
    }

    Object.defineProperty(window, "__TAURI__", {
      value: {
        core: { invoke },
      },
      configurable: true,
    });

    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {
        invoke,
        transformCallback(callback: (payload: unknown) => void) {
          const id = state.nextCallbackId;
          state.nextCallbackId += 1;
          state.callbacks.set(id, callback);
          return id;
        },
        unregisterCallback(id: number) {
          state.callbacks.delete(id);
        },
      },
      configurable: true,
    });

    Object.defineProperty(window, "__MDVIEW_TEST_STATE__", {
      value: state,
      configurable: true,
    });

    Object.defineProperty(window, "__MDVIEW_TEST_API__", {
      value: {
        setMarkdown(nextMarkdown: string) {
          state.markdown = nextMarkdown;
        },
        setReadLaunchMarkdownDelays(delays: number[]) {
          state.readLaunchMarkdownDelays = [...delays];
        },
        emit,
      },
      configurable: true,
    });
  }, { markdown: sampleMarkdown });
}
