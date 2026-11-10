// Node built-ins the extension runtime supplies itself. fs, child_process,
// crypto and zlib are served synchronously by the Go host; the rest are the
// usual browser-safe implementations.

import { Buffer } from "buffer";
import { EventEmitter } from "events";
import * as nodeAssert from "assert";
import * as nodeQuerystring from "querystring-es3";
import * as nodePunycode from "punycode";
import nodeURL from "url";
import { StringDecoder } from "string_decoder";
import * as nodeUtil from "util";
import nodeStream from "stream-browserify";
import nodePath from "path-browserify";
import processPolyfill from "process";
import { boot, host, hostCall, log } from "./bridge.ts";
import { base64ToBytes, bytesToBase64 } from "./polyfills.ts";

// ─── process ────────────────────────────────────────────────────────

let nodeInfo = boot.node ?? {};

export function refreshNodeInfo(): void {
  nodeInfo = boot.node ?? {};
}
const environment: Record<string, string> = {
  ...(nodeInfo.env ?? {}),
};
const processObject = processPolyfill as unknown as Record<string, unknown>;
processObject.env = environment;
processObject.platform = "win32";
processObject.arch = nodeInfo.arch ?? "x64";
processObject.argv = [nodeInfo.execPath ?? "wincast", ""];
processObject.execPath = nodeInfo.execPath ?? "wincast";
processObject.cwd = () => nodeInfo.cwd ?? "";
processObject.pid = 1;
processObject.version = "v22.0.0";
processObject.versions = { node: "22.0.0" };
processObject.release = { name: "node" };
processObject.nextTick = (callback: (...args: unknown[]) => void, ...args: unknown[]) => {
  Promise.resolve().then(() => callback(...args));
};
processObject.emitWarning = (warning: unknown) => log("warn", [warning]);
processObject.exit = (code = 0) => {
  throw new Error(`process.exit(${code})`);
};
const write = (level: string) => (chunk: unknown) => {
  log(level, [String(chunk).replace(/\n$/, "")]);
  return true;
};
processObject.stdout = { write: write("info"), isTTY: false, columns: 120, on: () => {} };
processObject.stderr = { write: write("error"), isTTY: false, columns: 120, on: () => {} };
const global = globalThis as Record<string, unknown>;
global.process = processObject;
global.Buffer = Buffer;
global.global = globalThis;

// ─── path (win32 default) ───────────────────────────────────────────

function normalizeWindows(input: string): string {
  let value = input.replace(/\//g, "\\");
  const drive = /^[a-zA-Z]:/.exec(value)?.[0] ?? "";
  let rest = value.slice(drive.length);
  const unc = rest.startsWith("\\\\");
  rest = rest.replace(/\\+/g, "\\");
  const absolute = rest.startsWith("\\");
  const segments: string[] = [];
  for (const segment of rest.split("\\")) {
    if (segment === "" || segment === ".") continue;
    if (segment === "..") {
      if (segments.length && segments.at(-1) !== "..") segments.pop();
      else if (!absolute) segments.push("..");
      continue;
    }
    segments.push(segment);
  }
  let prefix = drive;
  if (unc && !prefix) prefix = "";
  let output = segments.join("\\");
  if (absolute) output = "\\" + output;
  if (prefix) return output ? `${prefix}${output.startsWith("\\") ? "" : "\\"}${output}` : prefix;
  return output || (absolute ? "\\" : ".");
}

const win32 = {
  sep: "\\",
  delimiter: ";",
  normalize: normalizeWindows,
  isAbsolute(value: string): boolean {
    return /^[a-zA-Z]:[\\/]/.test(value) || value.startsWith("\\\\") || value.startsWith("\\");
  },
  join(...parts: string[]): string {
    return normalizeWindows(parts.filter((part) => part !== "").join("\\"));
  },
  resolve(...parts: string[]): string {
    let resolved = "";
    for (let index = parts.length - 1; index >= 0; index--) {
      const part = parts[index];
      if (!part) continue;
      resolved = resolved ? `${part}\\${resolved}` : part;
      if (win32.isAbsolute(part)) return normalizeWindows(resolved);
    }
    const cwd = String(processObject.cwd?.() ?? "");
    return normalizeWindows(resolved ? `${cwd}\\${resolved}` : cwd);
  },
  dirname(value: string): string {
    const normalized = value.replace(/[\\/]+$/, "");
    const index = normalized.lastIndexOf("\\");
    if (index < 0) return ".";
    if (index === 2 && /^[a-zA-Z]:/.test(normalized)) return normalized.slice(0, 3);
    return normalized.slice(0, index) || "\\";
  },
  basename(value: string, suffix?: string): string {
    const name = value.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? "";
    return suffix && name.endsWith(suffix) ? name.slice(0, -suffix.length) : name;
  },
  extname(value: string): string {
    const name = win32.basename(value);
    const index = name.lastIndexOf(".");
    return index <= 0 ? "" : name.slice(index);
  },
  relative(from: string, to: string): string {
    const left = normalizeWindows(from).toLowerCase().split("\\").filter(Boolean);
    const right = normalizeWindows(to).toLowerCase().split("\\").filter(Boolean);
    let shared = 0;
    while (shared < left.length && shared < right.length && left[shared] === right[shared]) shared++;
    return [...Array(Math.max(0, left.length - shared)).fill(".."), ...right.slice(shared)].join("\\");
  },
  parse(value: string) {
    const root = /^[a-zA-Z]:\\/.exec(value)?.[0] ?? (value.startsWith("\\") ? "\\" : "");
    const rest = value.slice(root.length);
    const dir = root + (rest.includes("\\") ? rest.slice(0, rest.lastIndexOf("\\")) : "");
    const base = rest.includes("\\") ? rest.slice(rest.lastIndexOf("\\") + 1) : rest;
    const ext = win32.extname(base);
    return { root, dir, base, ext, name: ext ? base.slice(0, -ext.length) : base };
  },
  format(value: { dir?: string; root?: string; base?: string; name?: string; ext?: string }): string {
    if (value.base) return win32.join(value.dir ?? "", value.base);
    const name = value.name ?? "";
    const ext = value.ext ?? "";
    const file = ext && !ext.startsWith(".") ? `${name}.${ext}` : `${name}${ext}`;
    return win32.join(value.dir ?? value.root ?? "", file);
  },
  toNamespacedPath(value: string): string {
    return value.startsWith("\\\\?\\") ? value : `\\\\?\\${win32.resolve(value)}`;
  },
};

const posix = nodePath as unknown as Record<string, unknown>;
export const pathModule: Record<string, unknown> = { ...win32 };
pathModule.win32 = win32;
pathModule.posix = posix;
pathModule.default = pathModule;

// ─── fs ─────────────────────────────────────────────────────────────

type StatPayload = {
  size: number;
  mode: number;
  mtimeMs: number;
  atimeMs: number;
  ctimeMs: number;
  birthtimeMs: number;
  _isFile: boolean;
  _isDirectory: boolean;
  _isSymbolicLink: boolean;
};

function expandPath(value: string): string {
  if (value === "~") return String(nodeInfo.homedir ?? value);
  if (value.startsWith("~/") || value.startsWith("~\\")) {
    return `${nodeInfo.homedir ?? ""}\\${value.slice(2)}`;
  }
  return value;
}

function syncCall(api: string, method: string, args: unknown[]): unknown {
  const raw = host.invokeSync(api, method, JSON.stringify(args));
  const result = raw ? (JSON.parse(raw) as { ok: boolean; value?: unknown; error?: string; code?: string }) : { ok: true };
  if (result.ok === false) {
    const error = new Error(result.error ?? `${api}.${method} failed`) as Error & {
      code?: string;
    };
    if (result.code) error.code = result.code;
    throw error;
  }
  return result.value;
}

function encodeData(data: unknown): string {
  if (data instanceof Uint8Array) return bytesToBase64(data);
  if (typeof data === "string") return bytesToBase64(new Uint8Array(Buffer.from(data)));
  return bytesToBase64(new Uint8Array(Buffer.from(String(data))));
}

class Stats {
  size = 0;
  mode = 0;
  mtimeMs = 0;
  atimeMs = 0;
  ctimeMs = 0;
  birthtimeMs = 0;
  private file = false;
  private directory = false;
  private symbolic = false;
  constructor(payload: StatPayload) {
    Object.assign(this, payload);
    this.file = payload._isFile;
    this.directory = payload._isDirectory;
    this.symbolic = payload._isSymbolicLink;
  }
  isFile(): boolean {
    return this.file;
  }
  isDirectory(): boolean {
    return this.directory;
  }
  isSymbolicLink(): boolean {
    return this.symbolic;
  }
  isBlockDevice(): boolean {
    return false;
  }
  isCharacterDevice(): boolean {
    return false;
  }
  isFIFO(): boolean {
    return false;
  }
  isSocket(): boolean {
    return false;
  }
  get mtime(): Date {
    return new Date(this.mtimeMs);
  }
  get atime(): Date {
    return new Date(this.atimeMs);
  }
  get ctime(): Date {
    return new Date(this.ctimeMs);
  }
  get birthtime(): Date {
    return new Date(this.birthtimeMs);
  }
}

class Dirent {
  name: string;
  parentPath: string;
  private file: boolean;
  private directory: boolean;
  private symbolic: boolean;
  constructor(entry: { name: string; parentPath: string; _isFile: boolean; _isDirectory: boolean; _isSymbolicLink: boolean }) {
    this.name = entry.name;
    this.parentPath = entry.parentPath;
    this.file = entry._isFile;
    this.directory = entry._isDirectory;
    this.symbolic = entry._isSymbolicLink;
  }
  isFile(): boolean {
    return this.file;
  }
  isDirectory(): boolean {
    return this.directory;
  }
  isSymbolicLink(): boolean {
    return this.symbolic;
  }
  isBlockDevice(): boolean {
    return false;
  }
  isCharacterDevice(): boolean {
    return false;
  }
  isFIFO(): boolean {
    return false;
  }
  isSocket(): boolean {
    return false;
  }
}

function readFileSync(file: string, options?: { encoding?: string } | string): string | Buffer {
  const base64 = String(syncCall("fs", "readFile", [expandPath(String(file))]));
  const bytes = base64ToBytes(base64);
  const encoding = typeof options === "string" ? options : options?.encoding;
  if (encoding && encoding !== "buffer") return new TextDecoder().decode(bytes);
  return Buffer.from(bytes);
}

function writeFileSync(file: string, data: unknown, options?: { encoding?: string } | string): void {
  void options;
  syncCall("fs", "writeFile", [expandPath(String(file)), encodeData(data), false]);
}

function appendFileSync(file: string, data: unknown, options?: { encoding?: string } | string): void {
  void options;
  syncCall("fs", "writeFile", [expandPath(String(file)), encodeData(data), true]);
}

function statSync(file: string, options?: { throwIfNoEntry?: boolean }): Stats {
  try {
    return new Stats(syncCall("fs", "stat", [expandPath(String(file)), false]) as StatPayload);
  } catch (error) {
    if (options?.throwIfNoEntry === false) return undefined as unknown as Stats;
    throw error;
  }
}

function lstatSync(file: string, options?: { throwIfNoEntry?: boolean }): Stats {
  try {
    return new Stats(syncCall("fs", "stat", [expandPath(String(file)), true]) as StatPayload);
  } catch (error) {
    if (options?.throwIfNoEntry === false) return undefined as unknown as Stats;
    throw error;
  }
}

function readdirSync(
  directory: string,
  options?: { withFileTypes?: boolean; encoding?: string },
): Array<string | Dirent> {
  const entries = syncCall("fs", "readdir", [expandPath(String(directory))]) as Array<{
    name: string;
    parentPath: string;
    _isFile: boolean;
    _isDirectory: boolean;
    _isSymbolicLink: boolean;
  }>;
  return options?.withFileTypes
    ? entries.map((entry) => new Dirent(entry))
    : entries.map((entry) => entry.name);
}

export const fsModule = {
  readFileSync,
  writeFileSync,
  appendFileSync,
  existsSync: (file: string) => Boolean(syncCall("fs", "exists", [expandPath(String(file))])),
  statSync,
  lstatSync,
  readdirSync,
  mkdirSync(directory: string, options?: { recursive?: boolean } | boolean) {
    return syncCall("fs", "mkdir", [expandPath(String(directory)), options === true || !!options?.recursive]);
  },
  rmSync(target: string, options?: { recursive?: boolean; force?: boolean }) {
    syncCall("fs", "remove", [expandPath(String(target)), !!options?.recursive, !!options?.force]);
  },
  rmdirSync(target: string, options?: { recursive?: boolean }) {
    syncCall("fs", "remove", [expandPath(String(target)), !!options?.recursive, false]);
  },
  unlinkSync(target: string) {
    syncCall("fs", "remove", [expandPath(String(target)), false, false]);
  },
  renameSync(from: string, to: string) {
    syncCall("fs", "rename", [expandPath(String(from)), expandPath(String(to))]);
  },
  copyFileSync(from: string, to: string) {
    syncCall("fs", "copyFile", [expandPath(String(from)), expandPath(String(to))]);
  },
  realpathSync(target: string) {
    return String(syncCall("fs", "realpath", [expandPath(String(target))]));
  },
  accessSync(target: string) {
    if (!fsModule.existsSync(target)) {
      const error = new Error(`ENOENT: no such file or directory, access '${target}'`) as Error & {
        code?: string;
      };
      error.code = "ENOENT";
      throw error;
    }
  },
  mkdtempSync(prefix: string) {
    return String(syncCall("fs", "mkdtemp", [expandPath(String(prefix))]));
  },
  chmodSync() {},
  utimesSync() {},
  constants: { F_OK: 0, R_OK: 4, W_OK: 2, X_OK: 1 },
  promises: {} as Record<string, unknown>,
};
fsModule.promises = {
  readFile: async (file: string, options?: { encoding?: string } | string) => readFileSync(file, options),
  writeFile: async (file: string, data: unknown, options?: { encoding?: string } | string) =>
    writeFileSync(file, data, options),
  appendFile: async (file: string, data: unknown, options?: { encoding?: string } | string) =>
    appendFileSync(file, data, options),
  stat: async (file: string) => statSync(file),
  lstat: async (file: string) => lstatSync(file),
  readdir: async (directory: string, options?: { withFileTypes?: boolean }) => readdirSync(directory, options),
  mkdir: async (directory: string, options?: { recursive?: boolean }) => fsModule.mkdirSync(directory, options),
  rm: async (target: string, options?: { recursive?: boolean; force?: boolean }) => fsModule.rmSync(target, options),
  unlink: async (target: string) => fsModule.unlinkSync(target),
  rename: async (from: string, to: string) => fsModule.renameSync(from, to),
  copyFile: async (from: string, to: string) => fsModule.copyFileSync(from, to),
  realpath: async (target: string) => fsModule.realpathSync(target),
  access: async (target: string) => fsModule.accessSync(target),
  mkdtemp: async (prefix: string) => fsModule.mkdtempSync(prefix),
};

// ─── child_process ──────────────────────────────────────────────────

type ProcessSpec = {
  shell: boolean;
  command: string;
  args: string[];
  cwd?: string;
  env?: Record<string, string>;
  timeout?: number;
  input?: string | null;
};

function payload(result: { stdout: string; stderr: string; status: number; signal: string | null }) {
  return {
    stdout: base64ToBytes(result.stdout),
    stderr: base64ToBytes(result.stderr),
    status: result.status,
    signal: result.signal,
  };
}

function spawnError(result: { status: number; stderr: string; stdout: string; signal: string | null }) {
  const error = new Error(
    `Command failed: ${new TextDecoder().decode(base64ToBytes(result.stderr))}`,
  ) as Error & { status?: number; stderr?: Uint8Array; stdout?: Uint8Array; signal?: string | null };
  error.status = result.status;
  error.stderr = base64ToBytes(result.stderr);
  error.stdout = base64ToBytes(result.stdout);
  error.signal = result.signal;
  return error;
}

function runSync(spec: ProcessSpec) {
  return syncCall("proc", "run", [spec]) as {
    stdout: string;
    stderr: string;
    status: number;
    signal: string | null;
  };
}

function execSync(command: string, options: Record<string, unknown> = {}) {
  const result = runSync({
    shell: true,
    command: String(command),
    args: [],
    cwd: options.cwd as string | undefined,
    env: options.env as Record<string, string> | undefined,
    timeout: options.timeout as number | undefined,
    input: options.input ? bytesToBase64(Buffer.from(options.input as string)) : null,
  });
  if (result.status !== 0) throw spawnError(result);
  const bytes = base64ToBytes(result.stdout);
  return options.encoding && options.encoding !== "buffer" ? new TextDecoder().decode(bytes) : Buffer.from(bytes);
}

function execFileSync(file: string, args: string[] = [], options: Record<string, unknown> = {}) {
  const result = runSync({
    shell: false,
    command: String(file),
    args: args.map(String),
    cwd: options.cwd as string | undefined,
    env: options.env as Record<string, string> | undefined,
    timeout: options.timeout as number | undefined,
    input: options.input ? bytesToBase64(Buffer.from(options.input as string)) : null,
  });
  if (result.status !== 0) throw spawnError(result);
  const bytes = base64ToBytes(result.stdout);
  return options.encoding && options.encoding !== "buffer" ? new TextDecoder().decode(bytes) : Buffer.from(bytes);
}

function spawnSync(file: string, args: string[] = [], options: Record<string, unknown> = {}) {
  const result = runSync({
    shell: !!options.shell,
    command: String(file),
    args: args.map(String),
    cwd: options.cwd as string | undefined,
    env: options.env as Record<string, string> | undefined,
    timeout: options.timeout as number | undefined,
    input: options.input ? bytesToBase64(Buffer.from(options.input as string)) : null,
  });
  const output = payload(result);
  return {
    ...output,
    output: [null, output.stdout, output.stderr],
    pid: 0,
    error: result.status === 0 ? undefined : spawnError(result),
  };
}

function asyncExec(command: string, options: unknown, callback?: unknown) {
  const opts = (typeof options === "function" ? {} : (options as Record<string, unknown>)) ?? {};
  const done = (typeof options === "function" ? options : callback) as
    | ((error: Error | null, stdout: string | Buffer, stderr: string | Buffer) => void)
    | undefined;
  const promise = hostCall("proc", "run", [
    {
      shell: true,
      command: String(command),
      args: [],
      cwd: opts.cwd,
      env: opts.env,
      timeout: opts.timeout,
      input: opts.input ? bytesToBase64(Buffer.from(opts.input as string)) : null,
    },
  ]).then((value) => {
    const result = value as { stdout: string; stderr: string; status: number; signal: string | null };
    const output = payload(result);
    if (result.status !== 0) throw Object.assign(spawnError(result), { stdout: output.stdout, stderr: output.stderr });
    return { stdout: output.stdout, stderr: output.stderr };
  });
  if (done) {
    promise.then(
      (value) => done(null, Buffer.from(value.stdout), Buffer.from(value.stderr)),
      (error) => done(error, Buffer.alloc(0), Buffer.from(error.stderr ?? [])),
    );
    return { pid: 0 } as unknown as EventEmitter;
  }
  return promise;
}

function spawn(file: string, args: string[] = [], options: Record<string, unknown> = {}) {
  const child = new EventEmitter() as EventEmitter & {
    stdout: EventEmitter;
    stderr: EventEmitter;
    stdin: { write: () => void; end: () => void };
    pid: number;
  };
  child.stdout = new EventEmitter();
  child.stderr = new EventEmitter();
  child.stdin = { write: () => {}, end: () => {} };
  child.pid = 0;
  hostCall("proc", "run", [
    {
      shell: !!options.shell,
      command: String(file),
      args: args.map(String),
      cwd: options.cwd,
      env: options.env,
      timeout: options.timeout,
    },
  ]).then(
    (value) => {
      const result = value as { stdout: string; stderr: string; status: number };
      const output = payload(result);
      if (output.stdout.length) child.stdout.emit("data", Buffer.from(output.stdout));
      if (output.stderr.length) child.stderr.emit("data", Buffer.from(output.stderr));
      child.stdout.emit("end");
      child.stderr.emit("end");
      child.emit("exit", result.status, null);
      child.emit("close", result.status, null);
    },
    (error) => child.emit("error", error),
  );
  return child;
}

const childProcess = {
  exec: asyncExec,
  execSync,
  execFile: (file: string, args: string[], options: unknown, callback?: unknown) =>
    asyncExec(String(file), options, callback),
  execFileSync,
  spawnSync,
  spawn,
};

// ─── crypto ─────────────────────────────────────────────────────────

function hashBytes(algorithm: string, dataBase64: string, keyBase64: string | null) {
  return String(syncCall("crypto", keyBase64 ? "hmac" : "hash", [algorithm, dataBase64, keyBase64]));
}

class Hash {
  private chunks: Uint8Array[] = [];
  private readonly algorithm: string;
  private readonly key: Uint8Array | null;
  constructor(algorithm: string, key?: Uint8Array) {
    this.algorithm = algorithm;
    this.key = key ?? null;
  }
  update(data: unknown) {
    const bytes =
      data instanceof Uint8Array
        ? data
        : ArrayBuffer.isView(data)
          ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength)
          : new TextEncoder().encode(String(data));
    this.chunks.push(bytes);
    return this;
  }
  digest(encoding?: string) {
    const total = this.chunks.reduce((sum, chunk) => sum + chunk.length, 0);
    const joined = new Uint8Array(total);
    let offset = 0;
    for (const chunk of this.chunks) {
      joined.set(chunk, offset);
      offset += chunk.length;
    }
    const base64 = hashBytes(
      this.algorithm,
      bytesToBase64(joined),
      this.key ? bytesToBase64(this.key) : null,
    );
    const bytes = base64ToBytes(base64);
    if (encoding && encoding !== "buffer") return Buffer.from(bytes).toString(encoding);
    return Buffer.from(bytes);
  }
}

export const cryptoModule = {
  createHash: (algorithm: string) => new Hash(String(algorithm).toLowerCase().replace("-", "")),
  createHmac: (algorithm: string, key: unknown) =>
    new Hash(String(algorithm).toLowerCase().replace("-", ""), key as Uint8Array),
  randomBytes(size: number, callback?: (error: Error | null, buffer: Buffer) => void) {
    const bytes = Buffer.from(base64ToBytes(String(syncCall("crypto", "random", [size | 0]))));
    if (callback) {
      Promise.resolve().then(() => callback(null, bytes));
      return undefined;
    }
    return bytes;
  },
  randomUUID: () => String(syncCall("crypto", "uuid", [])),
  randomInt(min: number, max?: number) {
    const low = max === undefined ? 0 : min;
    const high = max === undefined ? min : max;
    const bytes = base64ToBytes(String(syncCall("crypto", "random", [4])));
    const value = new DataView(bytes.buffer).getUint32(0);
    return low + (value % (high - low));
  },
  randomFillSync(target: Uint8Array) {
    const bytes = base64ToBytes(String(syncCall("crypto", "random", [target.length])));
    target.set(bytes.subarray(0, target.length));
    return target;
  },
  webcrypto: (globalThis as Record<string, unknown>).crypto,
};

// ─── zlib ───────────────────────────────────────────────────────────

function zlibSync(method: string, data: unknown) {
  const bytes =
    data instanceof Uint8Array
      ? data
      : ArrayBuffer.isView(data)
        ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength)
        : new TextEncoder().encode(String(data));
  return Buffer.from(base64ToBytes(String(syncCall("zlib", method, [bytesToBase64(bytes)]))));
}

const zlibModule = {
  gzipSync: (data: unknown) => zlibSync("gzip", data),
  gunzipSync: (data: unknown) => zlibSync("gunzip", data),
  deflateSync: (data: unknown) => zlibSync("deflate", data),
  inflateSync: (data: unknown) => zlibSync("inflate", data),
  deflateRawSync: (data: unknown) => zlibSync("deflateRaw", data),
  inflateRawSync: (data: unknown) => zlibSync("inflateRaw", data),
  gzip: async (data: unknown) => zlibSync("gzip", data),
  gunzip: async (data: unknown) => zlibSync("gunzip", data),
  deflate: async (data: unknown) => zlibSync("deflate", data),
  inflate: async (data: unknown) => zlibSync("inflate", data),
};

// ─── os ─────────────────────────────────────────────────────────────

const osModule = {
  EOL: "\r\n",
  arch: () => nodeInfo.arch ?? "x64",
  platform: () => "win32",
  release: () => nodeInfo.release ?? "Windows",
  type: () => "Windows_NT",
  homedir: () => nodeInfo.homedir ?? "",
  tmpdir: () => nodeInfo.tmpdir ?? "",
  hostname: () => nodeInfo.hostname ?? "",
  userInfo: () => ({ username: nodeInfo.username ?? "", homedir: nodeInfo.homedir ?? "", shell: nodeInfo.shell ?? "" }),
  cpus: () => Array.from({ length: nodeInfo.cpus ?? 1 }, () => ({ model: "", speed: 0, times: {} })),
  totalmem: () => nodeInfo.totalmem ?? 0,
  freemem: () => 0,
  uptime: () => 0,
  loadavg: () => [0, 0, 0],
  networkInterfaces: () => ({}),
};

// ─── unsupported built-ins ──────────────────────────────────────────

function unsupportedModule(name: string) {
  const fail = () => {
    throw new Error(`node:${name} is not supported in Wincast extensions.`);
  };
  return new Proxy(
    {},
    {
      get: () => fail,
    },
  );
}

export function nodeModule(name: string): unknown {
  const bare = name.replace(/^node:/, "");
  switch (bare) {
    case "fs":
      return fsModule;
    case "fs/promises":
      return fsModule.promises;
    case "path":
      return pathModule;
    case "path/posix":
      return posix;
    case "path/win32":
      return win32;
    case "os":
      return osModule;
    case "child_process":
      return childProcess;
    case "crypto":
      return cryptoModule;
    case "zlib":
      return zlibModule;
    case "buffer":
      return { Buffer, SlowBuffer: Buffer, INSPECT_MAX_BYTES: 50 };
    case "events":
      return { EventEmitter, default: EventEmitter };
    case "util":
      return nodeUtil;
    case "assert":
    case "assert/strict":
      return nodeAssert;
    case "querystring":
      return nodeQuerystring;
    case "punycode":
      return nodePunycode;
    case "url":
      return nodeURL;
    case "string_decoder":
      return { StringDecoder };
    case "stream":
    case "stream/promises":
      return nodeStream;
    case "timers":
    case "timers/promises":
      return { setTimeout, clearTimeout, setInterval, clearInterval };
    case "process":
      return processObject;
    case "module":
      return { builtinModules: [] };
    case "worker_threads":
    case "net":
    case "http":
    case "https":
    case "http2":
    case "tls":
    case "dgram":
      return unsupportedModule(bare);
    default:
      return undefined;
  }
}
