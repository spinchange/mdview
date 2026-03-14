import { expect, test, type Page } from "@playwright/test";
import { installTauriMock } from "./support/tauri-mock";

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

    await expect(page.locator(".mdv-status-pill").nth(1)).toHaveText("Preview pending");
    await expect(page.locator(".mdv-content")).toContainText("More body updated");

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
