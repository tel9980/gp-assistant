#!/usr/bin/env node
// 统一检查入口：把散落在 package.json 里的 20 多个 `&&` 串联脚本收成一个命令。
//
//   node scripts/verify.mjs            # 全量（约 60~90s）
//   node scripts/verify.mjs --fast     # 跳过构建与体积预算，只跑静态检查 + 单测（约 20s）
//   node scripts/verify.mjs --only=unit,contracts
//
// 设计要点：
//  · 每步独立子进程，失败不中断，最后汇总表格 + 非零退出
//  · 只用 node，不依赖 bash/powershell —— 那 14 个 .ps1 在 Linux CI 上会直接挂，
//    这里统一「非 Windows 跳过并标注」，不当作失败
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";

const ROOT = resolve(new URL("..", import.meta.url).pathname);
const FRONTEND = join(ROOT, "desktop/frontend");
const isWindows = process.platform === "win32";
const args = process.argv.slice(2);
const fast = args.includes("--fast");
const onlyArg = args.find((a) => a.startsWith("--only="));
const only = onlyArg ? onlyArg.slice("--only=".length).split(",").map((s) => s.trim()) : null;

/** @type {{group:string, name:string, cmd:string, args:string[], cwd:string, skip?:string}[]} */
const steps = [
  // —— 版本与契约（快，纯静态） ——
  { group: "contracts", name: "版本一致性", cmd: "node", args: ["scripts/check-version-consistency.mjs"], cwd: ROOT },
  { group: "contracts", name: "配色契约", cmd: "node", args: ["scripts/check-color-contract.mjs"], cwd: ROOT },
  { group: "contracts", name: "触摸目标 ≥44px", cmd: "node", args: ["scripts/check-ui-touch-targets.mjs"], cwd: ROOT },
  { group: "contracts", name: "CSS 架构", cmd: "node", args: ["scripts/check-css-architecture.mjs"], cwd: ROOT },
  { group: "contracts", name: "未样式化 class", cmd: "node", args: ["scripts/check-unstyled-classes.mjs"], cwd: ROOT },
  { group: "contracts", name: "信息密度", cmd: "node", args: ["scripts/check-ui-density.mjs"], cwd: ROOT },
  { group: "contracts", name: "主题对齐", cmd: "node", args: ["scripts/check-theme-parity.mjs"], cwd: ROOT },

  // —— 契约自测（node:test） ——
  {
    group: "contracts",
    name: "契约自测（node --test）",
    cmd: "node",
    args: [
      "--test",
      "--test-isolation=none",
      "scripts/check-color-contract.test.mjs",
      "scripts/check-ui-touch-targets.test.mjs",
      "scripts/check-ui-density.test.mjs",
      "scripts/check-theme-parity.test.mjs",
      "scripts/check-agent-replay-css.test.mjs",
      "scripts/ui-contrast-audit.test.mjs",
      // release-check.test.mjs 会真跑 pwsh 打包门禁，非 Windows 上必挂 → 只在 Windows 上跑
      ...(process.platform === "win32" ? ["scripts/release-check.test.mjs"] : []),
    ],
    cwd: ROOT,
  },

  // —— 前端类型检查 + 单测 ——
  { group: "frontend", name: "tsc 类型检查", cmd: "npx", args: ["tsc", "-b"], cwd: FRONTEND },
  { group: "frontend", name: "单测（两段式）", cmd: "node", args: ["scripts/test-fast.mjs"], cwd: FRONTEND },

  // —— 构建与体积（慢） ——
  { group: "build", name: "vite build", cmd: "npm", args: ["run", "build:app"], cwd: FRONTEND, skip: fast ? "--fast 跳过" : undefined },
  { group: "build", name: "入口体积预算", cmd: "node", args: ["../../scripts/check-bundle-budget.mjs"], cwd: FRONTEND, skip: fast ? "--fast 跳过" : undefined },
  {
    group: "build",
    name: "导出脚本（PowerShell）",
    cmd: "npm",
    args: ["run", "test:industry-export"],
    cwd: FRONTEND,
    skip: isWindows ? undefined : "非 Windows 平台跳过（.ps1）",
  },
];

const selected = steps.filter((s) => (only ? only.includes(s.group) : true));
const rows = [];
let failed = 0;

for (const step of selected) {
  const label = `[${step.group}] ${step.name}`;
  if (step.skip) {
    rows.push({ label, status: step.skip, ms: 0 });
    console.log(`⏭  ${label} — ${step.skip}`);
    continue;
  }
  if (!existsSync(step.cwd)) {
    rows.push({ label, status: `目录不存在: ${step.cwd}`, ms: 0 });
    failed += 1;
    continue;
  }
  process.stdout.write(`▶  ${label} … `);
  const t0 = Date.now();
  const res = spawnSync(step.cmd, step.args, { cwd: step.cwd, stdio: "pipe", encoding: "utf8", timeout: 15 * 60_000 });
  const ms = Date.now() - t0;
  const ok = res.status === 0;
  rows.push({ label, status: ok ? "PASS" : `FAIL(${res.status ?? "timeout"})`, ms });
  if (ok) {
    console.log(`PASS ${(ms / 1000).toFixed(1)}s`);
  } else {
    failed += 1;
    console.log(`FAIL ${(ms / 1000).toFixed(1)}s`);
    const out = `${res.stdout || ""}${res.stderr || ""}`.trim();
    console.log(
      out
        .split("\n")
        .slice(-25)
        .map((l) => `   │ ${l}`)
        .join("\n"),
    );
  }
}

const width = Math.max(...rows.map((r) => r.label.length));
console.log("\n=== 汇总 ===");
for (const r of rows) {
  const dur = r.ms ? `${(r.ms / 1000).toFixed(1)}s`.padStart(7) : "      —";
  console.log(`${r.label.padEnd(width)}  ${r.status.padEnd(10)} ${dur}`);
}
const passed = rows.filter((r) => r.status === "PASS").length;
const skipped = rows.filter((r) => !["PASS", "FAIL"].some((s) => r.status.startsWith(s))).length;
console.log(`\n通过 ${passed} / 跳过 ${skipped} / 失败 ${failed}`);
process.exit(failed === 0 ? 0 : 1);
