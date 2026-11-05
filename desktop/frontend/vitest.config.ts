import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config";

// 测试专用配置：继承 vite.config.ts（含 raf 分桶等构建/依赖优化），
// 只补 testing 相关项。这样 `isolate` 可以用环境变量显式切换。
//
// isolate: true（默认，npm test）
//   → 每个测试文件独立模块图，最稳。
// isolate: false（VITEST_NO_ISOLATE=1，npm run test:fast）
//   → 同进程复用模块，实测 34s → ~13s，但要求测试文件自身不泄漏全局状态。
const fast = process.env.VITEST_NO_ISOLATE === "1";

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      isolate: !fast,
      pool: "threads",
      // 非隔离模式下所有测试文件共享同一个模块图（同一个 worker 线程）。
      // 若文件分布在多个线程，模块图的“最先加载者”取决于调度顺序，
      // 失败会变成偶发；固定单线程后结果可复现（实测 34s → ~13s）。
      // isolate:false 时所有测试文件共享同一个全局对象与模块图：
      //  · 固定单 worker + 不打乱顺序 → 执行顺序确定，失败可复现
      //  · unstubGlobals → 每个用例前自动还原 vi.stubGlobal，漏写 afterEach 也不再泄漏
      ...(fast
        ? {
            maxWorkers: 1,
            minWorkers: 1,
            fileParallelism: false,
            poolOptions: { threads: { singleThread: true, isolate: false } },
            unstubGlobals: true,
            unstubEnvs: true,
            sequence: { shuffle: false, hooks: "stack" },
            // 少数面板用例依赖微任务顺序（stdout 背压时会抖），给 1 次重试
            retry: 1,
          }
        : {}),
      testTimeout: 20_000,
      // 覆盖率只统计 src/lib（IPC 包装、存储、契约、解析这类风险最高的一层）。
      // 阈值取 2026-11-05 基线（statements 76.4 / branches 65.7 / functions 74.7 / lines 80.1）
      // 留 2~3 个点余量，防住回退但不因重构误报。
      coverage: {
        provider: "v8",
        include: ["src/lib/**/*.{ts,tsx}"],
        reporter: ["text-summary", "lcov"],
        thresholds: { statements: 74, branches: 62, functions: 72, lines: 78 },
      },
    },
  }),
);
