#!/usr/bin/env node
// 两段式测试：
//   A 段（非隔离，单线程复用模块图）：不替换 lib/tauri 的文件，跑得快
//   B 段（隔离）：用 vi.mock 整模块替换 lib/tauri 的文件 —— 它们在同一个进程里
//      会互相抢占模块图（谁先加载谁决定），只能隔离跑
// 两段合起来覆盖与 `vitest run` 相同的用例，总耗时约为全隔离的一半。
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = new URL("..", import.meta.url).pathname;
process.chdir(ROOT);

function walk(dir) {
  const out = [];
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) out.push(...walk(p));
    else if (/\.test\.tsx?$/.test(e.name)) out.push(p);
  }
  return out;
}

const all = walk("src").sort();
const mocking = all.filter((f) => /vi\.mock\(".*lib\/tauri"/.test(readFileSync(f, "utf8")));
const shared = all.filter((f) => !mocking.includes(f));

function run(files, env) {
  execFileSync("npx", ["vitest", "run", "--config", "vitest.config.ts", ...files], {
    stdio: "inherit",
    env: { ...process.env, ...env },
  });
}

const t0 = Date.now();
console.log(`[A] 非隔离跑 ${shared.length} 个文件`);
run(shared, { VITEST_NO_ISOLATE: "1" });
const tA = Date.now();
console.log(`[B] 隔离跑 ${mocking.length} 个文件（整模块 mock lib/tauri）`);
run(mocking, {});
const t1 = Date.now();
console.log(
  `合计 ${((t1 - t0) / 1000).toFixed(1)}s （A ${((tA - t0) / 1000).toFixed(1)}s / B ${((t1 - tA) / 1000).toFixed(1)}s）`,
);
