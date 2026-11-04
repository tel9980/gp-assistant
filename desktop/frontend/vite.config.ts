import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";

// 打包进前端的两个大 JSON（各 3.5MB / 1MB）不参与 JS 打包，走 public/ 直接拷贝。
// 为了能长缓存（cache: "force-cache"），文件名带上内容哈希：
//   mobile-financial-snapshot.<hash>.json
// 内容一改哈希就变，WebView 缓存自动失效；内容不变则二次冷启不再读盘解析。
const SNAPSHOT_FILES = ["mobile-financial-snapshot.json", "mobile-industry-snapshot.json"];
const PUBLIC_DIR = resolve(import.meta.dirname, "public");

function snapshotVersions(): Record<string, string> {
  const out: Record<string, string> = {};
  for (const name of SNAPSHOT_FILES) {
    try {
      out[name] = createHash("sha256").update(readFileSync(resolve(PUBLIC_DIR, name))).digest("hex").slice(0, 10);
    } catch {
      out[name] = "";
    }
  }
  return out;
}

const VERSIONS = snapshotVersions();
const HASHED = Object.fromEntries(
  SNAPSHOT_FILES.map((name) => {
    const [stem, ext] = [name.slice(0, name.lastIndexOf(".")), ".json"];
    return [name, VERSIONS[name] ? `${stem}.${VERSIONS[name]}${ext}` : name];
  }),
);

/** 把 public/ 里的快照再复制一份带哈希的副本到产物目录（public/ 原文件仍保留，兼容旧引用）。 */
function hashedSnapshots(): Plugin {
  return {
    name: "hashed-snapshots",
    apply: "build",
    writeBundle(options) {
      const outDir = resolve(String(options.dir || "../mobile-dist"));
      mkdirSync(outDir, { recursive: true });
      for (const name of SNAPSHOT_FILES) {
        if (HASHED[name] === name) continue;
        copyFileSync(resolve(PUBLIC_DIR, name), resolve(outDir, HASHED[name]));
      }
    },
  };
}

export default defineConfig({
  plugins: [react(), hashedSnapshots()],
  define: {
    __SNAPSHOT_FINANCIAL__: JSON.stringify(VERSIONS["mobile-financial-snapshot.json"]),
    __SNAPSHOT_INDUSTRY__: JSON.stringify(VERSIONS["mobile-industry-snapshot.json"]),
  },
  build: {
    outDir: "../mobile-dist",
    emptyOutDir: true,
    manifest: true,
    sourcemap: false,
    cssCodeSplit: true,
    assetsInlineLimit: 2048,
    reportCompressedSize: true,
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            {
              name: "react",
              test: /node_modules[\\/](?:react|react-dom|scheduler)[\\/]/,
              priority: 20,
            },
            {
              name: "icons",
              test: /node_modules[\\/]lucide-react[\\/]/,
              priority: 15,
            },
          ],
        },
      },
    },
  },
  server: {
    proxy: {
      "/api": "http://127.0.0.1:8000",
      "/health": "http://127.0.0.1:8000",
    },
  },
});
