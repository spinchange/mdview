import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "../../apps/viewer-shell/web/dist");
const port = 1420;

const contentTypes = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "application/javascript; charset=utf-8"],
  [".css", "text/css; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".ico", "image/x-icon"],
]);

createServer(async (req, res) => {
  try {
    const requestPath = req.url === "/" ? "/index.html" : req.url ?? "/index.html";
    const fsPath = path.normalize(path.join(root, requestPath));
    if (!fsPath.startsWith(root)) {
      res.writeHead(403).end("forbidden");
      return;
    }

    const body = await readFile(fsPath);
    const ext = path.extname(fsPath);
    res.writeHead(200, {
      "content-type": contentTypes.get(ext) ?? "application/octet-stream",
      "cache-control": "no-store",
    });
    res.end(body);
  } catch {
    res.writeHead(404).end("not found");
  }
}).listen(port, "127.0.0.1", () => {
  console.log(`mdview e2e server listening on http://127.0.0.1:${port}`);
});
