// 从 lib/tauri.ts 拆出的模块；对外仍由 lib/tauri.ts 统一再导出。
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
export type InvokeFn = <T = unknown>(command: string, args?: Record<string, unknown>) => Promise<T>;
export type ListenFn = (event: string, handler: (event: unknown) => void) => Promise<() => void>;
export type TauriRouteHandler = (ctx: { invoke: InvokeFn; parsed: URL; path: string; payload?: unknown }) => Promise<unknown>;
export function hasTauriRuntime(): boolean {
  if (typeof window === "undefined") return false;
  return Boolean(window.__TAURI_INTERNALS__ || window.__TAURI_IPC__ || window.__TAURI__?.core?.invoke);
}

export function isTauriRuntime(): boolean {
  return hasTauriRuntime();
}

export function isMobileTauriRuntime(): boolean {
  return isTauriRuntime() && /Android|iPhone|iPad|iPod/i.test(navigator.userAgent || "");
}

export function getTauriInvoke(): InvokeFn | undefined {
  if (!hasTauriRuntime()) return undefined;
  return window.__TAURI__?.core?.invoke || (tauriInvoke as InvokeFn);
}

export function getTauriListen(): ListenFn | undefined {
  if (!hasTauriRuntime()) return undefined;
  return window.__TAURI__?.event?.listen || (tauriListen as ListenFn);
}
