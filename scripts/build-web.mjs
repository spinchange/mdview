import esbuild from "esbuild";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");

await esbuild.build({
  entryPoints: [path.join(root, "apps/viewer-shell/web/src/main.ts")],
  bundle: true,
  format: "iife",
  outfile: path.join(root, "apps/viewer-shell/web/dist/app.js"),
  platform: "browser",
  target: ["chrome114"],
  logLevel: "info",
});
