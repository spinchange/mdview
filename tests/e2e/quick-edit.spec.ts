import { expect, test, type Page } from "@playwright/test";
import { installTauriMock } from "./support/tauri-mock";

const FILE_CHANGED_EVENT = "mdview://file-changed";

async function replaceEditorText(page: Page, text: string) {
  await page.evaluate((nextText) => {
    const view = (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__;
    if (!view) {
      throw new Error("editor view missing");
    }

    view.dispatch({
      changes: {
        from: 0,
        to: view.state.doc.length,
        insert: nextText,
      },
    });
  }, text);
}

async function dispatchEditorUpdate(
  page: Page,
  spec: { changes?: { from: number; to?: number; insert: string }; selection?: { anchor: number; head?: number } }
) {
  await page.evaluate((nextSpec) => {
    const view = (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__;
    if (!view) {
      throw new Error("editor view missing");
    }

    view.dispatch(nextSpec);
  }, spec);
}

async function setMockMarkdown(page: Page, markdown: string) {
  await page.evaluate((nextMarkdown) => {
    (
      window as Window & {
        __MDVIEW_TEST_API__?: { setMarkdown: (value: string) => void };
      }
    ).__MDVIEW_TEST_API__?.setMarkdown(nextMarkdown);
  }, markdown);
}

async function setReadLaunchMarkdownDelays(page: Page, delays: number[]) {
  await page.evaluate((nextDelays) => {
    (
      window as Window & {
        __MDVIEW_TEST_API__?: { setReadLaunchMarkdownDelays: (value: number[]) => void };
      }
    ).__MDVIEW_TEST_API__?.setReadLaunchMarkdownDelays(nextDelays);
  }, delays);
}

async function emitFileChanged(page: Page) {
  await page.evaluate((eventName) => {
    (
      window as Window & {
        __MDVIEW_TEST_API__?: { emit: (name: string, payload?: unknown) => void };
      }
    ).__MDVIEW_TEST_API__?.emit(eventName);
  }, FILE_CHANGED_EVENT);
}

test.describe("Quick Edit", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");
  });

  test("opens quick edit with keyboard shortcut and shows the CodeMirror editor", async ({ page }) => {
    await page.keyboard.press("Control+E");

    await expect(page.locator(".cm-editor")).toBeVisible();
    const editorText = await page.evaluate(() => {
      return (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__?.state.doc.toString();
    });
    expect(editorText).toContain("# mdview");
    await expect(page.locator(".mdv-status-pill").first()).toHaveText("Saved");
  });

  test("jumps editor selection to the selected heading line", async ({ page }) => {
    await page.getByRole("button", { name: "Quick Edit" }).click();
    await page.getByRole("link", { name: "Second section" }).click();

    const selectionStart = await page.evaluate(() => {
      return (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__?.state.selection.main.from;
    });
    const editorText = await page.evaluate(() => {
      return (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__?.state.doc.toString();
    });

    expect(selectionStart).toBe(editorText.indexOf("## Second section"));
  });

  test("debounces live preview refresh while editing", async ({ page }) => {
    await page.keyboard.press("Control+E");

    await replaceEditorText(
      page,
      "# mdview\n\n## First section\nBody copy\n\n## Second section\nMore body updated\n"
    );

    await page.waitForTimeout(80);
    const earlyRenderCount = await page.evaluate(() => {
      return (window as Window & { __MDVIEW_TEST_STATE__?: { renderCallCount?: number } })
        .__MDVIEW_TEST_STATE__?.renderCallCount;
    });
    expect(earlyRenderCount).toBe(1);

    await expect(page.locator(".mdv-content")).toContainText("More body updated");
    await expect(page.locator(".mdv-status-pill").nth(1)).toHaveText("Preview live");

    const renderCount = await page.evaluate(() => {
      return (window as Window & { __MDVIEW_TEST_STATE__?: { renderCallCount?: number } })
        .__MDVIEW_TEST_STATE__?.renderCallCount;
    });
    expect(renderCount).toBe(2);
  });

  test("opens find/replace and replaces matches", async ({ page }) => {
    await page.keyboard.press("Control+E");
    await page.getByRole("button", { name: "Find / Replace" }).click();

    await expect(page.locator(".mdv-search")).toBeVisible();
    await page.locator(".mdv-search__input--find").fill("Body");
    await page.locator(".mdv-search__input--replace").fill("Updated");
    await page.getByRole("button", { name: "Replace All" }).click();

    await expect(page.locator(".mdv-status-pill").first()).toHaveText("Unsaved changes");
    await expect(page.locator(".mdv-content")).toContainText("Updated copy");
  });

  test("preserves undo history across search panel toggles", async ({ page }) => {
    await page.keyboard.press("Control+E");
    await dispatchEditorUpdate(page, {
      changes: { from: 0, insert: "Preface\n\n" },
    });

    await page.keyboard.press("Control+F");
    await expect(page.locator(".mdv-search")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.locator(".mdv-search")).toBeHidden();

    await page.keyboard.press("Control+Z");

    const editorText = await page.evaluate(() => {
      return (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__?.state.doc.toString();
    });
    expect(editorText.startsWith("Preface")).toBe(false);
    await expect(page.locator(".mdv-content")).not.toContainText("Preface");
  });

  test("preserves editor selection and focus when closing search panel", async ({ page }) => {
    await page.keyboard.press("Control+E");

    const range = await page.evaluate(() => {
      const view = (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__;
      const from = view.state.doc.toString().indexOf("First section");
      const to = from + "First section".length;
      view.dispatch({ selection: { anchor: from, head: to } });
      view.focus();
      return { from, to };
    });

    await page.keyboard.press("Control+F");
    await expect(page.locator(".mdv-search__input--find")).toBeFocused();

    await page.keyboard.press("Escape");

    const editorState = await page.evaluate(() => {
      const view = (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__;
      return {
        from: view.state.selection.main.from,
        to: view.state.selection.main.to,
        focused: view.hasFocus,
      };
    });

    expect(editorState).toEqual({
      from: range.from,
      to: range.to,
      focused: true,
    });
  });

  test("keeps preview scroll position on debounced live updates", async ({ page }) => {
    await page.keyboard.press("Control+E");

    const longMarkdown = [
      "# mdview",
      "",
      "## First section",
      ...Array.from({ length: 120 }, (_, index) => `Line ${index + 1}: body copy`),
      "",
      "## Second section",
      "More body",
    ].join("\n");
    await replaceEditorText(page, longMarkdown);
    await expect(page.locator(".mdv-content")).toContainText("Line 120: body copy");

    await page.evaluate(() => {
      window.scrollTo({ top: 900, behavior: "instant" });
    });
    await page.waitForTimeout(50);
    const beforeScroll = await page.evaluate(() => window.scrollY);
    expect(beforeScroll).toBeGreaterThan(400);

    await dispatchEditorUpdate(page, {
      changes: { from: 0, insert: "Intro line\n\n" },
    });
    await expect(page.locator(".mdv-content")).toContainText("Intro line");

    const afterScroll = await page.evaluate(() => window.scrollY);
    expect(Math.abs(afterScroll - beforeScroll)).toBeLessThan(80);
  });

  test("reloads clean editor state from external file changes without losing selection or focus", async ({ page }) => {
    await page.keyboard.press("Control+E");

    const initial = await page.evaluate(() => {
      const view = (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__;
      const from = view.state.doc.toString().indexOf("Second section");
      const to = from + "Second section".length;
      view.dispatch({ selection: { anchor: from, head: to } });
      view.focus();
      return { from, to };
    });

    await setMockMarkdown(
      page,
      "# mdview\n\n## First section\nBody copy refreshed\n\n## Second section\nMore body from disk\n"
    );
    await emitFileChanged(page);

    await expect(page.locator(".mdv-content")).toContainText("More body from disk");
    await expect(page.locator(".mdv-status-pill").first()).toHaveText("Saved");

    const reloaded = await page.evaluate(() => {
      const view = (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__;
      return {
        text: view.state.doc.toString(),
        from: view.state.selection.main.from,
        to: view.state.selection.main.to,
        focused: view.hasFocus,
      };
    });

    expect(reloaded.text).toContain("More body from disk");
    expect(reloaded.from).toBe(initial.from);
    expect(reloaded.to).toBe(initial.to);
    expect(reloaded.focused).toBe(true);
  });

  test("does not clobber unsaved edits when the file changes on disk", async ({ page }) => {
    await page.keyboard.press("Control+E");
    await dispatchEditorUpdate(page, {
      changes: { from: 0, insert: "Working copy\n\n" },
    });

    await setMockMarkdown(
      page,
      "# mdview\n\n## First section\nDisk body\n\n## Second section\nChanged externally\n"
    );
    await emitFileChanged(page);

    await expect(page.locator(".mdv-editor__message")).toContainText(
      "File changed on disk while you had unsaved edits"
    );

    const editorText = await page.evaluate(() => {
      return (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__?.state.doc.toString();
    });
    expect(editorText.startsWith("Working copy")).toBe(true);
    await expect(page.locator(".mdv-content")).not.toContainText("Changed externally");
  });

  test("keeps preview scroll stable on external clean reload", async ({ page }) => {
    await page.keyboard.press("Control+E");

    const longMarkdown = [
      "# mdview",
      "",
      "## First section",
      ...Array.from({ length: 120 }, (_, index) => `Disk line ${index + 1}: body copy`),
      "",
      "## Second section",
      "More body",
    ].join("\n");
    await setMockMarkdown(page, longMarkdown);
    await emitFileChanged(page);
    await expect(page.locator(".mdv-content")).toContainText("Disk line 120: body copy");

    await page.evaluate(() => {
      window.scrollTo({ top: 900, behavior: "instant" });
    });
    await page.waitForTimeout(50);
    const beforeScroll = await page.evaluate(() => window.scrollY);
    expect(beforeScroll).toBeGreaterThan(400);

    await setMockMarkdown(page, `${longMarkdown}\n\nTrailing disk line`);
    await emitFileChanged(page);
    await expect(page.locator(".mdv-content")).toContainText("Trailing disk line");

    const afterScroll = await page.evaluate(() => window.scrollY);
    expect(Math.abs(afterScroll - beforeScroll)).toBeLessThan(80);
  });

  test("ignores stale overlapping external reloads", async ({ page }) => {
    await page.keyboard.press("Control+E");
    await setReadLaunchMarkdownDelays(page, [120, 10]);

    await setMockMarkdown(
      page,
      "# mdview\n\n## First section\nOlder disk version\n\n## Second section\nOld body\n"
    );
    await emitFileChanged(page);

    await setMockMarkdown(
      page,
      "# mdview\n\n## First section\nNewest disk version\n\n## Second section\nNew body\n"
    );
    await emitFileChanged(page);

    await expect(page.locator(".mdv-content")).toContainText("Newest disk version");
    await expect(page.locator(".mdv-content")).toContainText("New body");
    await expect(page.locator(".mdv-content")).not.toContainText("Older disk version");
  });

  test("does not apply external reload results after the editor becomes dirty", async ({ page }) => {
    await page.keyboard.press("Control+E");
    await setReadLaunchMarkdownDelays(page, [120]);
    await setMockMarkdown(
      page,
      "# mdview\n\n## First section\nDisk version\n\n## Second section\nFrom disk\n"
    );

    await emitFileChanged(page);
    await page.waitForTimeout(20);
    await dispatchEditorUpdate(page, {
      changes: { from: 0, insert: "Local edit\n\n" },
    });

    await page.waitForTimeout(160);

    const editorText = await page.evaluate(() => {
      return (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__?.state.doc.toString();
    });
    expect(editorText.startsWith("Local edit")).toBe(true);
    await expect(page.locator(".mdv-content")).not.toContainText("From disk");
  });

  test("external reloads do not pollute undo history", async ({ page }) => {
    await page.keyboard.press("Control+E");
    await dispatchEditorUpdate(page, {
      changes: { from: 0, insert: "Local edit\n\n" },
    });
    await page.keyboard.press("Control+S");

    await setMockMarkdown(
      page,
      "# mdview\n\n## First section\nBody copy reloaded\n\n## Second section\nMore body\n"
    );
    await emitFileChanged(page);
    await expect(page.locator(".mdv-content")).toContainText("Body copy reloaded");

    await page.keyboard.press("Control+Z");

    const editorText = await page.evaluate(() => {
      return (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__?.state.doc.toString();
    });
    expect(editorText).toContain("Body copy reloaded");
    expect(editorText.startsWith("Local edit")).toBe(false);
  });

  test("saves edited markdown with Ctrl+S", async ({ page }) => {
    await page.keyboard.press("Control+E");
    await replaceEditorText(page, "# mdview\n\n## First section\nUpdated body\n");

    await expect(page.locator(".mdv-status-pill").first()).toHaveText("Unsaved changes");
    await page.keyboard.press("Control+S");

    await expect(page.locator(".mdv-status-pill").first()).toHaveText("Saved");
    const savedMarkdown = await page.evaluate(() => {
      return (window as Window & { __MDVIEW_TEST_STATE__?: { savedMarkdown?: string } })
        .__MDVIEW_TEST_STATE__?.savedMarkdown;
    });
    expect(savedMarkdown).toContain("Updated body");
  });
});
