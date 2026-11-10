// The single seam between the bundled Raycast runtime and the Go host.
// Go installs `__tinycastHost` on the global before evaluating this bundle.

export type HostObject = {
  log(level: string, message: string): void;
  render(session: string, json: string): void;
  failed(session: string, message: string): void;
  navigationDepthChanged(session: string, depth: string): void;
  finished(session: string): void;
  fieldCommand(command: string, fieldID: string): void;
  invoke(callID: string, api: string, method: string, argsJSON: string): void;
  invokeSync(api: string, method: string, argsJSON: string): string;
  startTimer(id: string, milliseconds: number, repeats: boolean): void;
  clearTimer(id: string): void;
};

const hostCandidate: unknown = Reflect.get(globalThis, "__tinycastHost");
if (!hostCandidate || typeof hostCandidate !== "object") {
  throw new Error("__tinycastHost missing — the runtime was evaluated outside Wincast.");
}
// The Go bootstrap installs exactly this object before evaluating the bundle.
export const host = hostCandidate as HostObject;

export type BootConfig = {
  node?: {
    arch?: string;
    release?: string;
    hostname?: string;
    username?: string;
    shell?: string;
    homedir?: string;
    tmpdir?: string;
    cwd?: string;
    cpus?: number;
    totalmem?: number;
    env?: Record<string, string>;
    execPath?: string;
  };
};

export let boot: BootConfig = {};

export function configureBoot(json: string): void {
  try {
    boot = JSON.parse(json) as BootConfig;
  } catch {
    boot = {};
  }
  const candidate: unknown = Reflect.get(globalThis, "process");
  if (candidate && typeof candidate === "object") {
    const processObject = candidate as Record<string, unknown>;
    processObject.env = boot.node?.env ?? {};
    processObject.platform = "win32";
    processObject.arch = boot.node?.arch ?? "x64";
    processObject.cwd = () => boot.node?.cwd ?? "";
  }
}

let nextRequest = 1;
const pending = new Map<
  string,
  { resolve(value: unknown): void; reject(error: Error): void }
>();

/** Async host call, for fetch and child_process; settles through `__tinycast.settle`. */
export function hostCall(api: string, method: string, args: unknown[]): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const id = `h:${nextRequest++}`;
    pending.set(id, { resolve, reject });
    try {
      host.invoke(id, api, method, JSON.stringify(args ?? []));
    } catch (error) {
      pending.delete(id);
      reject(error instanceof Error ? error : new Error(String(error)));
    }
  });
}

/** Settles an async host call. Returns false when the id belongs to a capability request. */
export function settleCall(id: string, ok: boolean, payload: string): boolean {
  const entry = pending.get(id);
  if (!entry) return false;
  pending.delete(id);
  if (ok) {
    entry.resolve(payload === "" || payload === undefined ? undefined : JSON.parse(payload));
  } else {
    entry.reject(new Error(payload || "Host call failed"));
  }
  return true;
}

export function log(level: string, parts: unknown[]): void {
  let text: string;
  try {
    text = parts.map(formatLogArg).join(" ");
  } catch {
    text = "[unserializable log argument]";
  }
  host.log(level, text);
}

function formatLogArg(value: unknown): string {
  if (typeof value === "string") return value;
  if (value instanceof Error) return describeError(value);
  if (value === undefined) return "undefined";
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

export function describeError(error: unknown): string {
  if (!(error instanceof Error)) return String(error);
  const headline = `${error.name || "Error"}: ${error.message}`;
  const stack = String(error.stack || "");
  if (!stack) return headline;
  return stack.startsWith(headline) ? stack : `${headline}\n${stack}`;
}

// ─── Timers ─────────────────────────────────────────────────────────

type TimerEntry = { callback: () => void; repeat: number | null };
const timers = new Map<string, TimerEntry>();
let nextTimer = 1;

export function setHostTimer(callback: () => void, milliseconds: number, repeat: boolean): number {
  const id = `t:${nextTimer++}`;
  const delay = Math.max(0, Number(milliseconds) || 0);
  timers.set(id, { callback, repeat: repeat ? delay : null });
  host.startTimer(id, delay, repeat);
  return Number(id.slice(2));
}

export function clearHostTimer(id: number): void {
  const key = `t:${id}`;
  if (!timers.delete(key)) return;
  host.clearTimer(key);
}

export function fireHostTimer(key: string): void {
  const entry = timers.get(key);
  if (!entry) return;
  if (entry.repeat === null) timers.delete(key);
  try {
    entry.callback();
  } catch (error) {
    log("error", [describeError(error)]);
  }
}

export function clearAllTimers(): void {
  for (const key of timers.keys()) host.clearTimer(key);
  timers.clear();
}
