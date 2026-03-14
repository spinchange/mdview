import { expect, test, type Page } from "@playwright/test";
import { installTauriMock } from "./support/tauri-mock";

async function getEditorSnapshot(page: Page) {
  return page.evaluate(() => {
    const view = (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__;
    if (!view) {
      throw new Error("editor view missing");
    }

    return {
      text: view.state.doc.toString(),
      selectionFrom: view.state.selection.main.from,
      selectionTo: view.state.selection.main.to,
      focused: view.hasFocus,
      domIdentity: view.dom.dataset.testid ?? "",
    };
  });
}

test.describe("jump to line", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");
  });

  test("jumps to the clicked heading from the rendered document in quick edit", async ({ page }) => {
    await page.getByRole("button", { name: "Quick Edit" }).click();
    await page.getByRole("heading", { name: "Second section" }).click();

    const snapshot = await getEditorSnapshot(page);
    expect(snapshot.selectionFrom).toBe(snapshot.text.indexOf("## Second section"));
    expect(snapshot.selectionTo).toBe(snapshot.selectionFrom);
    expect(snapshot.focused).toBe(true);
  });

  test("jumps from the toc link without recreating the live editor instance", async ({ page }) => {
    await page.getByRole("button", { name: "Quick Edit" }).click();

    const beforeJump = await page.evaluate(() => {
      const view = (window as Window & { __MDVIEW_EDITOR_VIEW__?: any }).__MDVIEW_EDITOR_VIEW__;
      if (!view) {
        throw new Error("editor view missing");
      }

      view.dom.dataset.testid = "editor-view";
      const from = view.state.doc.toString().indexOf("# mdview");
      view.dispatch({ selection: { anchor: from } });
      return {
        domIdentity: view.dom.dataset.testid,
        selectionFrom: view.state.selection.main.from,
      };
    });

    await page.getByRole("link", { name: "Second section" }).click();

    const afterJump = await getEditorSnapshot(page);
    expect(afterJump.domIdentity).toBe(beforeJump.domIdentity);
    expect(afterJump.selectionFrom).toBe(afterJump.text.indexOf("## Second section"));
    expect(afterJump.selectionTo).toBe(afterJump.selectionFrom);
    expect(afterJump.focused).toBe(true);
  });

  test("keeps the editor usable after a viewer heading jump", async ({ page }) => {
    await page.getByRole("button", { name: "Quick Edit" }).click();
    await page.getByRole("heading", { name: "Second section" }).click();

    await page.keyboard.type("\nPost jump edit");

    const snapshot = await getEditorSnapshot(page);
    expect(snapshot.text).toContain("Post jump edit");
    expect(snapshot.text).toContain("## Second section");
    await expect(page.locator(".mdv-status-pill").first()).toHaveText("Unsaved changes");
    await expect(page.locator(".mdv-content")).toContainText("Post jump edit");
  });
});
