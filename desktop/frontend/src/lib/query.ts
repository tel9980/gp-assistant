// 统一查询层。
//
// 目的只有三件事，不做别的：
//   1. **合并并发重复调用**：多个面板同时挂载时，同一条 IPC 只发一次；
//   2. **可选的短期缓存**：只读状态（诊断、能力清单）给一个 TTL，避免来回切页狂刷；
//   3. **错误归一化**：失败统一抛 `AppError`（带 code/hint/details），调用方不必再各写一套判断。
//
// 刻意不做：不做全局状态容器、不做自动重试、不做请求编排。写操作（set/enable/delete）
// 一律直接用 `lib/tauri` 的命令函数，不进这层缓存 —— 缓存写操作是 bug 制造机。
//
// 走的是 `lib/tauri` 门面里的 `getTauriInvoke`，所以 `vi.mock("../../lib/tauri")`
// 的测试照样能拦到，浏览器无 Tauri 时也照旧拿到 null 并抛错。

import { toAppError } from "./appError";
import { getTauriInvoke } from "./tauri";

export interface QueryOptions {
  /** 缓存有效期（毫秒）。0 或省略 = 只用并发合并，不缓存结果。 */
  ttlMs?: number;
  /** 跳过缓存读，强制重新取一次（仍会写回缓存）。 */
  force?: boolean;
  /** 调用前已取消则直接拒绝，不发 IPC。 */
  signal?: AbortSignal | null;
}

interface CacheEntry {
  expiresAt: number;
  value: unknown;
}

const cache = new Map<string, CacheEntry>();
const inFlight = new Map<string, Promise<unknown>>();

function stableStringify(value: unknown): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value) ?? "null";
  if (Array.isArray(value)) return `[${value.map(stableStringify).join(",")}]`;
  const record = value as Record<string, unknown>;
  const keys = Object.keys(record).sort();
  return `{${keys.map((key) => `${JSON.stringify(key)}:${stableStringify(record[key])}`).join(",")}}`;
}

/** 缓存键：命令名 + 稳定序列化的参数（参数键顺序不影响命中）。 */
export function queryKey(command: string, args?: unknown): string {
  return args === undefined ? command : `${command}?${stableStringify(args)}`;
}

function abortError(): Error {
  const error = new Error("请求已取消");
  error.name = "AbortError";
  return error;
}

/** 读一次查询，命中缓存/合并进行中的请求时不再发 IPC。 */
export async function queryTauri<T>(
  command: string,
  args?: Record<string, unknown>,
  options: QueryOptions = {},
): Promise<T> {
  if (options.signal?.aborted) throw abortError();

  const key = queryKey(command, args);
  const ttl = options.ttlMs ?? 0;

  if (!options.force && ttl > 0) {
    const hit = cache.get(key);
    if (hit) {
      if (hit.expiresAt > Date.now()) return hit.value as T;
      cache.delete(key);
    }
  }

  const pending = inFlight.get(key);
  if (pending) return pending as Promise<T>;

  const invoke = getTauriInvoke();
  if (!invoke) throw toAppError(new Error(`Tauri 运行时不可用，无法执行 ${command}`));

  const task = (async () => {
    try {
      const value = await invoke<T>(command, args);
      if (ttl > 0) cache.set(key, { expiresAt: Date.now() + ttl, value });
      return value;
    } catch (error) {
      throw toAppError(error);
    } finally {
      inFlight.delete(key);
    }
  })();

  inFlight.set(key, task);
  return task;
}

/**
 * 写操作：不走缓存、不做并发合并，但同样要求运行时可用并归一化错误。
 * 语义是「每次调用都真的执行一次」，所以调用方不要用它做可重放的重试。
 */
export async function mutateTauri<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const invoke = getTauriInvoke();
  if (!invoke) throw toAppError(new Error(`Tauri 运行时不可用，无法执行 ${command}`));
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toAppError(error);
  }
}

/**
 * 失效缓存。传命令名或前缀；不传则清空。
 * 写操作完成后调用，避免读到旧状态。
 */
export function invalidateQuery(match?: string): number {
  if (!match) {
    const count = cache.size;
    cache.clear();
    return count;
  }
  let count = 0;
  for (const key of cache.keys()) {
    if (key === match || key.startsWith(`${match}?`)) {
      cache.delete(key);
      count += 1;
    }
  }
  return count;
}

export function queryCacheSize(): number {
  return cache.size;
}

/** 仅测试用：清空缓存与进行中的表。 */
export function resetQueryCacheForTests(): void {
  cache.clear();
  inFlight.clear();
}
