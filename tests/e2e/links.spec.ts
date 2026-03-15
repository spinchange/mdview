import { expect, test, type Page } from "@playwright/test";
import { installTauriMock } from "./support/tauri-mock";

const FILE_CHANGED_EVENT = "mdview://file-changed";

async function setMockMarkdown(page: Page, markdown: string) {
  await page.evaluate((nextMarkdown) => {
    (
      window as Window & {
        __MDVIEW_TEST_API__?: { setMarkdown: (value: string) => void };
      }
    ).__MDVIEW_TEST_API__?.setMarkdown(nextMarkdown);
  }, markdown);
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

test.describe("Links", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");
  });

  test("opens external https links via Tauri shell", async ({ page }) => {
    await setMockMarkdown(page, "Check out [Google](https://google.com)");
    await emitFileChanged(page);

    await page.getByRole("link", { name: "Google" }).click();

    const lastOpenedUrl = await page.evaluate(() => {
      return (window as any).__MDVIEW_TEST_STATE__.lastOpenedUrl;
    });
    expect(lastOpenedUrl).toBe("https://google.com/");
  });

  test("scrolls to internal heading links", async ({ page }) => {
    const longMarkdown = [
      "# Title",
      "",
      "[Jump to Bottom](#bottom)",
      "",
      ...Array.from({ length: 100 }, (_, i) => `Line ${i + 1}`),
      "",
      "## Bottom",
      "Reached the end."
    ].join("\n");

    await setMockMarkdown(page, longMarkdown);
    await emitFileChanged(page);

    // Initial scroll should be 0
    const initialScroll = await page.evaluate(() => window.scrollY);
    expect(initialScroll).toBe(0);

    await page.getByRole("link", { name: "Jump to Bottom" }).click();

    // Wait for smooth scroll
    await page.waitForTimeout(500);

    const finalScroll = await page.evaluate(() => window.scrollY);
    expect(finalScroll).toBeGreaterThan(500);

    // Verify it scrolled to the right element
    const bottomId = await page.locator("h2:has-text('Bottom')").getAttribute("id");
    expect(bottomId).toBe("bottom");
  });

  test("quick-edit heading click behavior still works", async ({ page }) => {
    await page.getByRole("button", { name: "Quick Edit" }).click();
    
    // Default mock markdown has "Second section"
    await page.getByRole("link", { name: "Second section" }).click();

    const selectionStart = await page.evaluate(() => {
      return (window as any).__MDVIEW_EDITOR_VIEW__?.state.selection.main.from;
    });
    const editorText = await page.evaluate(() => {
      return (window as any).__MDVIEW_EDITOR_VIEW__?.state.doc.toString();
    });

    expect(selectionStart).toBe(editorText.indexOf("## Second section"));
  });
});
