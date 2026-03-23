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

  test("opens local file urls via the local-link bridge", async ({ page }) => {
    await setMockMarkdown(
      page,
      "Open [Local note](file:///C:/Users/user/Documents/other-note.md)"
    );
    await emitFileChanged(page);

    await page.getByRole("link", { name: "Local note" }).click();

    const lastOpenedLocalHref = await page.evaluate(() => {
      return (window as any).__MDVIEW_TEST_STATE__.lastOpenedLocalHref;
    });
    expect(lastOpenedLocalHref).toBe("file:///C:/Users/user/Documents/other-note.md");
  });

  test("opens relative local links via the local-link bridge", async ({ page }) => {
    await setMockMarkdown(page, "Open [Sibling](./other-note.md)");
    await emitFileChanged(page);

    await page.getByRole("link", { name: "Sibling" }).click();

    const state = await page.evaluate(() => {
      return (window as any).__MDVIEW_TEST_STATE__;
    });
    expect(state.lastOpenedLocalHref).toBe("./other-note.md");
    expect(state.lastOpenedUrl).toBeNull();
  });

  test("opens mailto links via Tauri shell", async ({ page }) => {
    await setMockMarkdown(page, "Contact [Support](mailto:support@example.com)");
    await emitFileChanged(page);

    await page.getByRole("link", { name: "Support" }).click();

    const lastOpenedUrl = await page.evaluate(() => {
      return (window as any).__MDVIEW_TEST_STATE__.lastOpenedUrl;
    });
    expect(lastOpenedUrl).toBe("mailto:support@example.com");

    // Scroll should stay at 0
    const finalScroll = await page.evaluate(() => window.scrollY);
    expect(finalScroll).toBe(0);
  });

  test("quick-edit mode: links within headings still work as links", async ({ page }) => {
    const markdown = [
      "# [Top Section](#bottom)",
      "",
      ...Array.from({ length: 100 }, (_, i) => `Line ${i + 1}`),
      "",
      "## Bottom",
      "End"
    ].join("\n");

    await setMockMarkdown(page, markdown);
    await emitFileChanged(page);

    await page.getByRole("button", { name: "Quick Edit" }).click();

    // Click the link inside the H1
    await page.locator("h1").getByRole("link", { name: "Top Section" }).click();

    // Wait for smooth scroll
    await page.waitForTimeout(500);

    // Verify it scrolled
    const finalScroll = await page.evaluate(() => window.scrollY);
    expect(finalScroll).toBeGreaterThan(500);

    // Verify editor selection did NOT change to the heading line (it should still be at 0 or wherever it was)
    const selectionStart = await page.evaluate(() => {
      return (window as any).__MDVIEW_EDITOR_VIEW__?.state.selection.main.from;
    });
    expect(selectionStart).toBe(0);
  });

  test("TOC links scroll within the document and do not trigger external links", async ({ page }) => {
    const longMarkdown = [
      "# Title",
      "",
      ...Array.from({ length: 100 }, (_, i) => `Line ${i + 1}`),
      "",
      "## Bottom Section",
      "Reached the end."
    ].join("\n");

    await setMockMarkdown(page, longMarkdown);
    await emitFileChanged(page);

    // Initial scroll should be 0
    const initialScroll = await page.evaluate(() => window.scrollY);
    expect(initialScroll).toBe(0);

    // Click the TOC link
    await page.locator(".mdv-toc__link:has-text('Bottom Section')").click();

    // Verify it scrolled
    const finalScroll = await page.evaluate(() => window.scrollY);
    expect(finalScroll).toBeGreaterThan(500);

    // Verify it did NOT trigger an external link
    const lastOpenedUrl = await page.evaluate(() => {
      return (window as any).__MDVIEW_TEST_STATE__.lastOpenedUrl;
    });
    expect(lastOpenedUrl).toBeNull();
  });

  test("internal links within headings are routed correctly", async ({ page }) => {
    // A heading that contains a link to another section
    const markdown = [
      "# [Top Section](#bottom)",
      "",
      ...Array.from({ length: 100 }, (_, i) => `Line ${i + 1}`),
      "",
      "## Bottom",
      "End"
    ].join("\n");

    await setMockMarkdown(page, markdown);
    await emitFileChanged(page);

    // Click the link inside the H1
    await page.locator("h1").getByRole("link", { name: "Top Section" }).click();

    // Wait for smooth scroll
    await page.waitForTimeout(500);

    // Verify it scrolled
    const finalScroll = await page.evaluate(() => window.scrollY);
    expect(finalScroll).toBeGreaterThan(500);

    // Verify it did NOT trigger an external link
    const lastOpenedUrl = await page.evaluate(() => {
      return (window as any).__MDVIEW_TEST_STATE__.lastOpenedUrl;
    });
    expect(lastOpenedUrl).toBeNull();
  });

  test("external links within headings are routed correctly", async ({ page }) => {
    await setMockMarkdown(page, "# [External Heading](https://example.com)");
    await emitFileChanged(page);

    await page.locator("h1").getByRole("link", { name: "External Heading" }).click();

    const lastOpenedUrl = await page.evaluate(() => {
      return (window as any).__MDVIEW_TEST_STATE__.lastOpenedUrl;
    });
    expect(lastOpenedUrl).toBe("https://example.com/");
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
