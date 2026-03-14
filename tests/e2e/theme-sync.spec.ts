import { expect, test, type Page } from "@playwright/test";
import { installTauriMock } from "./support/tauri-mock";

const THEME_EVENT = "mdview://theme-updated";

async function emitThemeCss(page: Page, cssText: string) {
  await page.evaluate(
    ({ eventName, nextCss }) => {
      (
        window as Window & {
          __MDVIEW_TEST_API__?: { emit: (name: string, payload?: unknown) => void };
        }
      ).__MDVIEW_TEST_API__?.emit(eventName, nextCss);
    },
    { eventName: THEME_EVENT, nextCss: cssText }
  );
}

async function readThemeSnapshot(page: Page) {
  return page.evaluate(() => {
    const styleHost = document.getElementById("mdview-theme-tokens");
    const toolbar = document.querySelector(".mdv-toolbar");
    const body = getComputedStyle(document.body);
    const root = getComputedStyle(document.documentElement);

    return {
      styleText: styleHost?.textContent ?? "",
      bodyBackground: body.backgroundColor,
      bodyColor: body.color,
      toolbarBorder: toolbar instanceof HTMLElement ? getComputedStyle(toolbar).borderTopColor : "",
      accent: root.getPropertyValue("--mdv-accent").trim(),
    };
  });
}

test.describe("theme sync", () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
    await page.goto("/");
  });

  test("applies initial theme css during bootstrap", async ({ page }) => {
    const snapshot = await readThemeSnapshot(page);

    expect(snapshot.styleText).toContain("--mdv-bg: #1e1e1e");
    expect(snapshot.styleText).toContain("--mdv-accent: #4ea1ff");
    expect(snapshot.accent).toBe("#4ea1ff");
    expect(snapshot.bodyBackground).toBe("rgb(30, 30, 30)");
    expect(snapshot.bodyColor).toBe("rgb(243, 243, 243)");
  });

  test("updates app theme tokens on theme change events", async ({ page }) => {
    const nextCss =
      ":root { --mdv-bg: #f4efe6; --mdv-text: #221f1a; --mdv-surface: #fffaf2; --mdv-border: #c87b2a; --mdv-accent: #d9480f; }";

    await emitThemeCss(page, nextCss);

    await expect
      .poll(async () => {
        const snapshot = await readThemeSnapshot(page);
        return {
          styleText: snapshot.styleText,
          accent: snapshot.accent,
          bodyBackground: snapshot.bodyBackground,
          bodyColor: snapshot.bodyColor,
          toolbarBorder: snapshot.toolbarBorder,
        };
      })
      .toEqual({
        styleText: nextCss,
        accent: "#d9480f",
        bodyBackground: "rgb(244, 239, 230)",
        bodyColor: "rgb(34, 31, 26)",
        toolbarBorder: "rgb(200, 123, 42)",
      });
  });
});
