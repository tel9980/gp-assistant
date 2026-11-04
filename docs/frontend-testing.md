# 前端测试运行方式（vitest）

## 两条命令

| 命令 | 模式 | 用例 | 耗时 |
| --- | --- | --- | --- |
| `npm run test:unit` | 全隔离（每个测试文件独立模块图） | 735 | ~34s |
| `npm run test:fast` | 两段式（91 个文件非隔离 + 16 个文件隔离） | 735 | ~18s |

两条命令覆盖完全相同的用例集合，都能在 Linux CI 上跑。

## 为什么不是「全部非隔离」

`VITEST_NO_ISOLATE=1` 时所有测试文件共享**同一个模块图**。项目里有 16 个测试文件用
`vi.mock("../../lib/tauri", ...)` **整模块替换** `src/lib/tauri.ts`，而
`src/hooks/useLlmSettingsDrafts.ts`、各面板组件都在模块顶层 `import { postJson, getJson } from "../lib/tauri"`。

于是「谁先加载谁决定」：

- `AgentPanel.test.tsx` 先加载 → `lib/tauri` 被它的替身占住（只提供 `getTauriInvoke` 等）
- 轮到 `LlmSettingsPanel.test.tsx` 时，`postJson` 已经在模块图里绑成了**真实函数**
- 断言 `expect(postJsonMock).toHaveBeenCalledWith("/api/llm/test")` → `Number of calls: 0`

实测（vitest 5.0.5，固定单线程 + 不打乱顺序后**结果完全可复现**）：非隔离全量跑稳定 67 个失败，
集中在 7 个文件（NewsRagPanel 30 / ObservePanel.interaction 13 / BacktestPanel.interaction 11 /
LlmSettingsPanel 6 / LlmSettingsPanel.drafts 5 / AgentPanel 1 / SettingsSheet 1）；
但把这 7 个摘掉后，`AgentPanel.interaction`、`lib/backup` 等又会红 —— 依赖顺序会整体平移，
说明这不是「少数文件忘了清理」，而是整模块 mock 与共享模块图的语义冲突。

## 已修复的真实缺陷（在非隔离下暴露，已提交）

1. `src/components/settings/StockResearchPanel.test.tsx`、`src/components/panels/LlmSettingsPanel.drafts.test.tsx`
   的 `afterEach` 曾被批量脚本截断成空块 → 补齐 `vi.unstubAllGlobals()` / `vi.clearAllMocks()`。
2. `src/lib/tauri.ts` 模块级读 `window.location.href` → 遇到假 window 抛
   `TypeError: Cannot read properties of undefined (reading 'href')`，已加 `!window.location?.href` 兜底。
3. `src/components/panels/AgentPanel.test.tsx`、`AgentRunDrawer.test.tsx` 曾在 `stubTestGlobals()`
   生效期间 `await import(...)`，把假 `document`/`window` 固化进模块图 → 改为导入后再 stub，
   文件末尾 `afterAll(() => vi.unstubAllGlobals())`。
4. 6 个文件的整模块 mock 补上 `async (importOriginal) => ({ ...(await importOriginal()), ... })`，
   避免抹掉 `isMobileTauriRuntime` 等其它导出。

## 测试文件写作约束（避免再踩）

- **不要**用 `vi.stubGlobal("document", {...})` / `vi.stubGlobal("window", {...})` 整体替换全局对象；
  只 stub 真正需要的那几个键，且放在 `beforeEach`，`afterEach` 里 `vi.unstubAllGlobals()`。
- **不要**在 stub 生效期间 `await import()` 组件；静态 `import` 放在文件顶部。
- 整模块 mock 必须带 `importOriginal` 展开，否则会把同模块的其它导出抹成 `undefined`。
- `vi.unmock()` 会被 hoist 到文件顶层，写在 `afterEach` 里会直接让整文件收集失败；
  同理 `vi.resetModules()` 在共享模块图里会打断其它文件已缓存的引用，不要用。

## 想再快一步？

把上述 16 个文件改成**共享 fake-tauri harness**（不替换整模块，改由 harness 提供可注入的
`getJson/postJson` 替身），就能让它们也进非隔离段，B 段消失、总耗时降到 ~6s。
工作量约 2~4 小时，风险中等（要逐个确认每个文件依赖哪些 tauri 导出的真实行为）。

## 环境变量

- `VITEST_NO_ISOLATE=1` — 打开非隔离（`vitest run` 会读 `desktop/frontend/vitest.config.ts`）。
- 非隔离段固定 `maxWorkers=1`、`singleThread`、`sequence.shuffle=false`：
  只有固定单线程，模块图加载顺序才确定，失败才可复现。**不要**给它加并发。
