import { expect, test } from "@playwright/test";
import { installTauriMock } from "./support/tauri-mock";

const FRAME_SAMPLE_COUNT = 24;
const WHITE_THRESHOLD = 245;

type Rgb = { r: number; g: number; b: number };

function parseRgb(value: string): Rgb | null {
  const match = value.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/i);
  if (!match) {
    return null;
  }

  return {
    r: Number(match[1]),
    g: Number(match[2]),
    b: Number(match[3]),
  };
}

function isNearWhite(rgb: Rgb): boolean {
  return (
    rgb.r >= WHITE_THRESHOLD &&
    rgb.g >= WHITE_THRESHOLD &&
    rgb.b >= WHITE_THRESHOLD
  );
}

test.describe("startup no-flash guard", () => {
  test("does not render a near-white frame during initial boot", async ({ page }) => {
    await installTauriMock(page);
    await page.addInitScript(
      ({ frameSampleCount }) => {
        const frames: string[] = [];
        let count = 0;

        const capture = () => {
          const html = getComputedStyle(document.documentElement).backgroundColor;
          const body = getComputedStyle(document.body).backgroundColor;
          frames.push(`${html}|${body}`);

          count += 1;
          if (count < frameSampleCount) {
            requestAnimationFrame(capture);
          }
        };

        requestAnimationFrame(capture);
        (window as any).__mdviewBootFrames = frames;
      },
      { frameSampleCount: FRAME_SAMPLE_COUNT }
    );

    // Replace with Tauri dev URL once the frontend bootstrap is in place.
    await page.goto("/", { waitUntil: "domcontentloaded" });
    await page.waitForTimeout(450);

    const rawFrames = (await page.evaluate(
      () => (window as any).__mdviewBootFrames as string[] | undefined
    )) ?? [];

    expect(rawFrames.length).toBeGreaterThan(0);

    const nearWhiteFrames = rawFrames.filter((entry) => {
      const [htmlValue, bodyValue] = entry.split("|");
      const html = parseRgb(htmlValue);
      const body = parseRgb(bodyValue);
      return (
        (html !== null && isNearWhite(html)) ||
        (body !== null && isNearWhite(body))
      );
    });

    expect(
      nearWhiteFrames,
      `Detected near-white startup frames: ${nearWhiteFrames.join(", ")}`
    ).toHaveLength(0);
  });
});
