import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
const evidenceRoot = path.join(repositoryRoot, "evidence");

function evidenceAssets(): Plugin {
  return {
    name: "projectbench-evidence-assets",
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        if (req.method !== "GET" || !req.url?.startsWith("/evidence/"))
          return next();
        const relative = decodeURIComponent(
          req.url.split("?")[0].slice("/evidence/".length),
        );
        if (
          !relative.endsWith(".json") ||
          relative.split("/").some((part) => !/^[a-zA-Z0-9._-]+$/.test(part))
        ) {
          res.statusCode = 400;
          res.end("Invalid evidence path");
          return;
        }
        const file = path.resolve(evidenceRoot, relative);
        if (
          !file.startsWith(`${evidenceRoot}${path.sep}`) ||
          !fs.existsSync(file) ||
          !fs.statSync(file).isFile()
        ) {
          res.statusCode = 404;
          res.end("Not found");
          return;
        }
        res.statusCode = 200;
        res.setHeader("Content-Type", "application/json; charset=utf-8");
        res.setHeader("Cache-Control", "no-store");
        res.end(fs.readFileSync(file, "utf8"));
      });
    },
    generateBundle() {
      if (!fs.existsSync(evidenceRoot)) return;
      const visit = (directory: string) => {
        for (const entry of fs.readdirSync(directory, {
          withFileTypes: true,
        })) {
          const file = path.join(directory, entry.name);
          if (entry.isDirectory()) visit(file);
          else if (entry.isFile() && entry.name.endsWith(".json")) {
            const fileName = path
              .relative(evidenceRoot, file)
              .split(path.sep)
              .join("/");
            this.emitFile({
              type: "asset",
              fileName: `evidence/${fileName}`,
              source: fs.readFileSync(file, "utf8"),
            });
          }
        }
      };
      visit(evidenceRoot);
    },
  };
}

export default defineConfig({ plugins: [react(), evidenceAssets()] });
