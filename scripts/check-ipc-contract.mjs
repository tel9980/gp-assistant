#!/usr/bin/env node
/**
 * IPC 契约检查：Rust 的 #[tauri::command] 与前端 invoke() 调用点是否对得上。
 *
 * 检查三件事：
 *  1. 前端 invoke 的命令名在 Rust 侧必须存在（否则运行时报 "command not found"）。
 *  2. 前端传的参数名必须是 Rust 形参名之一（Tauri 会因未知参数报 invalid args）。
 *  3. 报告从未被前端引用的命令（信息级，不算失败）。
 *
 * 用法：node scripts/check-ipc-contract.mjs [--strict-unused]
 */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, extname } from "node:path";

const RUST_ROOT = "desktop/src-tauri/src";
const TS_ROOTS = ["desktop/frontend/src"];
const strictUnused = process.argv.includes("--strict-unused");

/** Tauri 注入的形参，前端不需要传 */
const INJECTED = new Set(["app", "app_handle", "window", "webview", "webview_window", "state", "db", "pool"]);

function walk(dir, exts, out = []) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path, exts, out);
    else if (exts.includes(extname(path))) out.push(path);
  }
  return out;
}

/** 收集 Rust 命令：名字 + 形参名 */
function collectRustCommands() {
  const commands = new Map();
  for (const file of walk(RUST_ROOT, [".rs"])) {
    const src = readFileSync(file, "utf8");
    const re = /#\[tauri::command[^\]]*\](?:\s*#\[[^\]]*\])*\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)/g;
    let m;
    while ((m = re.exec(src))) {
      const name = m[1];
      // 从函数名后的第一个 '(' 开始括号配对取形参列表
      let i = src.indexOf("(", m.index + m[0].length - 1);
      let depth = 0;
      let start = i;
      for (; i < src.length; i += 1) {
        if (src[i] === "(") depth += 1;
        else if (src[i] === ")") {
          depth -= 1;
          if (depth === 0) break;
        }
      }
      const paramsRaw = src.slice(start + 1, i);
      const params = new Set();
      let depthB = 0;
      let cur = "";
      for (const ch of paramsRaw) {
        if ("<([{".includes(ch)) depthB += 1;
        if (">)]}".includes(ch)) depthB -= 1;
        if (ch === "," && depthB === 0) {
          addParam(params, cur);
          cur = "";
        } else cur += ch;
      }
      addParam(params, cur);
      commands.set(name, { file, params });
    }
  }
  return commands;
}

function addParam(set, raw) {
  const text = raw.replace(/#\[[^\]]*\]/g, "").trim();
  if (!text) return;
  const m = /^(?:mut\s+)?(\w+)\s*:/.exec(text);
  if (m) set.add(m[1]);
}

/** 收集前端调用：命令名字符串字面量（invoke / invokeLocalData / invokeNativeJob 等所有变体） */
function collectFrontendCalls() {
  const calls = [];
  const looksLikeCommand = (v) => /^[a-z][a-z0-9_]*$/.test(v) && v.includes("_");
  for (const file of walk(TS_ROOTS[0], [".ts", ".tsx"])) {
    if (file.includes(".test.")) continue;
    const lines = readFileSync(file, "utf8").split("\n");
    lines.forEach((text, index) => {
      if (!/invoke/i.test(text)) return;
      // 从最后一次 invoke(...) 的 '(' 之后开始，截到第一个 `{` 之前：
      // 这样既能跳过 `({ invoke }) =>` 这类解构前缀，也不会把 payload 里的 key 当命令名
      const call = [...text.matchAll(/invoke\w*\s*\(/gi)].pop();
      if (!call) return;
      const head = text.slice(call.index + call[0].length).split("{")[0];
      const literals = head.match(/["'`]([^"'`]+)["'`]/g) || [];
      for (const lit of literals) {
        const value = lit.slice(1, -1);
        if (!looksLikeCommand(value)) continue;
        calls.push({ command: value, file, line: index + 1, argsKeys: extractArgsKeys(text) });
      }
    });
  }
  return calls;
}

/** 从一行里抽 `{ key: ... }` 的顶层 key（多行调用只覆盖首行，够用） */
function extractArgsKeys(text) {
  const keys = new Set();
  const objStart = text.indexOf("{\s");
  if (objStart < 0) return keys;
  const body = text.slice(objStart + 1);
  for (const m of body.matchAll(/(?:^|[{,])\s*(\w+)\s*:/g)) keys.add(m[1]);
  return keys;
}

const rust = collectRustCommands();
const calls = collectFrontendCalls();
const used = new Set(calls.map((c) => c.command));

const unknownCommand = [];
const badArgs = [];
for (const call of calls) {
  const target = rust.get(call.command);
  if (!target) {
    unknownCommand.push(call);
    continue;
  }
  for (const key of call.argsKeys) {
    if (!target.params.has(key) && !INJECTED.has(key)) {
      badArgs.push({ ...call, key, rustParams: [...target.params] });
    }
  }
}

const unused = [...rust.keys()].filter((name) => !used.has(name)).sort();

console.log(`Rust 命令 ${rust.size} 个，前端调用点 ${calls.length} 处（引用 ${used.size} 个命令）`);

if (unknownCommand.length) {
  console.error("\n✗ 前端调用了 Rust 里不存在的命令：");
  for (const c of unknownCommand) console.error(`  ${c.command}  (${c.file}:${c.line})`);
}
if (badArgs.length) {
  console.error("\n✗ 前端传了 Rust 形参里没有的参数（Tauri 会报 invalid args）：");
  for (const c of badArgs) {
    console.error(`  ${c.command}.${c.key}  (${c.file}:${c.line})  Rust 形参: ${c.rustParams.join(", ") || "(无)"}`);
  }
}
if (unused.length) {
  console.log(`\nℹ 未被前端引用的命令 ${unused.length} 个：${unused.slice(0, 20).join(", ")}${unused.length > 20 ? " …" : ""}`);
}

const failed = unknownCommand.length > 0 || badArgs.length > 0 || (strictUnused && unused.length > 0);
if (failed) {
  console.error("\nIPC 契约检查失败");
  process.exit(1);
}
console.log("IPC 契约检查通过");
