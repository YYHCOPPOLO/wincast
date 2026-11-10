// Web primitives QuickJS does not ship. The extension runtime only promises
// what the Raycast API uses: fetch, URL, base64, text codecs, abort and timers.

import { hostCall, host, setHostTimer, clearHostTimer } from "./bridge.ts";
import { TextDecoder, TextEncoder } from "./text-codecs.ts";
import { URL, URLSearchParams } from "whatwg-url";

export { TextDecoder, TextEncoder };

// ─── Base64 ─────────────────────────────────────────────────────────

const BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

export function bytesToBase64(bytes: Uint8Array): string {
  let output = "";
  for (let index = 0; index < bytes.length; index += 3) {
    const a = bytes[index] ?? 0;
    const b = bytes[index + 1];
    const c = bytes[index + 2];
    const triple = (a << 16) | ((b ?? 0) << 8) | (c ?? 0);
    output += BASE64[(triple >> 18) & 63];
    output += BASE64[(triple >> 12) & 63];
    output += b === undefined ? "=" : BASE64[(triple >> 6) & 63];
    output += c === undefined ? "=" : BASE64[triple & 63];
  }
  return output;
}

export function base64ToBytes(text: string): Uint8Array {
  const clean = text.replace(/[^A-Za-z0-9+/=]/g, "");
  const output: number[] = [];
  for (let index = 0; index < clean.length; index += 4) {
    const a = BASE64.indexOf(clean[index]);
    const b = BASE64.indexOf(clean[index + 1]);
    const c = clean[index + 2] === "=" ? -1 : BASE64.indexOf(clean[index + 2]);
    const d = clean[index + 3] === "=" ? -1 : BASE64.indexOf(clean[index + 3]);
    const triple = (a << 18) | (b << 12) | ((c < 0 ? 0 : c) << 6) | (d < 0 ? 0 : d);
    output.push((triple >> 16) & 255);
    if (c >= 0) output.push((triple >> 8) & 255);
    if (d >= 0) output.push(triple & 255);
  }
  return Uint8Array.from(output);
}

export function btoa(text: string): string {
  const bytes = new Uint8Array(text.length);
  for (let index = 0; index < text.length; index++) bytes[index] = text.charCodeAt(index) & 255;
  return bytesToBase64(bytes);
}

export function atob(text: string): string {
  let output = "";
  for (const byte of base64ToBytes(text)) output += String.fromCharCode(byte);
  return output;
}

// ─── DOMException ───────────────────────────────────────────────────

export class DOMException extends Error {
  constructor(message = "", name = "Error") {
    super(message);
    this.name = name;
  }
}

// ─── Abort ──────────────────────────────────────────────────────────

type AbortListener = (event: { type: string; target: AbortSignal }) => void;

export class AbortSignal {
  aborted = false;
  reason: unknown = undefined;
  private listeners = new Set<AbortListener>();
  addEventListener(type: string, listener: AbortListener): void {
    if (type === "abort") this.listeners.add(listener);
  }
  removeEventListener(type: string, listener: AbortListener): void {
    if (type === "abort") this.listeners.delete(listener);
  }
  throwIfAborted(): void {
    if (this.aborted) throw this.reason;
  }
  static abort(reason?: unknown): AbortSignal {
    const signal = new AbortSignal();
    signal.abort(reason);
    return signal;
  }
  static timeout(milliseconds: number): AbortSignal {
    const signal = new AbortSignal();
    setHostTimer(() => signal.abort(new DOMException("The operation timed out.", "TimeoutError")), milliseconds, false);
    return signal;
  }
  static any(signals: AbortSignal[]): AbortSignal {
    const result = new AbortSignal();
    for (const signal of signals) {
      if (signal.aborted) {
        result.abort(signal.reason);
        break;
      }
      signal.addEventListener("abort", () => result.abort(signal.reason));
    }
    return result;
  }
  abort(reason?: unknown): void {
    if (this.aborted) return;
    this.aborted = true;
    this.reason = reason ?? new DOMException("The operation was aborted.", "AbortError");
    for (const listener of [...this.listeners]) {
      try {
        listener({ type: "abort", target: this });
      } catch {
        // A listener must not break the abort path.
      }
    }
    this.listeners.clear();
  }
}

export class AbortController {
  readonly signal = new AbortSignal();
  abort(reason?: unknown): void {
    this.signal.abort(reason);
  }
}

// ─── Timers ─────────────────────────────────────────────────────────

export function installTimers(): void {
  const global = globalThis as Record<string, unknown>;
  global.setTimeout = (callback: (...args: unknown[]) => void, milliseconds = 0, ...args: unknown[]) =>
    setHostTimer(() => callback(...args), Number(milliseconds), false);
  global.clearTimeout = (id: number) => clearHostTimer(Number(id));
  global.setInterval = (callback: (...args: unknown[]) => void, milliseconds = 0, ...args: unknown[]) =>
    setHostTimer(() => callback(...args), Number(milliseconds), true);
  global.clearInterval = (id: number) => clearHostTimer(Number(id));
  if (typeof global.queueMicrotask !== "function") {
    global.queueMicrotask = (callback: () => void) => {
      Promise.resolve().then(callback);
    };
  }
  const performanceObject = (global.performance ?? {}) as Record<string, unknown>;
  if (typeof performanceObject.now !== "function") {
    performanceObject.now = () => Date.now();
    performanceObject.timeOrigin = Date.now();
    global.performance = performanceObject;
  }
  global.btoa = btoa;
  global.atob = atob;
  global.TextEncoder = TextEncoder;
  global.TextDecoder = TextDecoder;
  global.URL = URL;
  global.URLSearchParams = URLSearchParams;
  global.DOMException = DOMException;
  global.AbortController = AbortController;
  global.AbortSignal = AbortSignal;
  const cryptoObject = (global.crypto ?? {}) as Record<string, unknown>;
  cryptoObject.randomUUID = () => {
    const bytes = base64ToBytes(
      String(host.invokeSync("crypto", "random", JSON.stringify([16]))),
    );
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
  };
  cryptoObject.getRandomValues = (array: Uint8Array) => {
    const bytes = base64ToBytes(
      String(host.invokeSync("crypto", "random", JSON.stringify([array.length]))),
    );
    array.set(bytes);
    return array;
  };
  global.crypto = cryptoObject;
}

// ─── structuredClone ────────────────────────────────────────────────

export function structuredClone<T>(value: T): T {
  const seen = new Map<object, unknown>();
  function clone(input: unknown): unknown {
    if (input === null || typeof input !== "object") return input;
    if (seen.has(input)) return seen.get(input);
    if (input instanceof Date) return new Date(input.getTime());
    if (input instanceof ArrayBuffer) return input.slice(0);
    if (ArrayBuffer.isView(input)) {
      return new Uint8Array(input.buffer.slice(input.byteOffset, input.byteOffset + input.byteLength));
    }
    if (Array.isArray(input)) {
      const output: unknown[] = [];
      seen.set(input, output);
      for (const item of input) output.push(clone(item));
      return output;
    }
    if (input instanceof Map) {
      const output = new Map();
      seen.set(input, output);
      for (const [key, item] of input) output.set(clone(key), clone(item));
      return output;
    }
    if (input instanceof Set) {
      const output = new Set();
      seen.set(input, output);
      for (const item of input) output.add(clone(item));
      return output;
    }
    const output: Record<string, unknown> = {};
    seen.set(input, output);
    for (const [key, item] of Object.entries(input)) output[key] = clone(item);
    return output;
  }
  return clone(value) as T;
}

// ─── Headers / Request / Response / fetch ───────────────────────────

export class Headers {
  private values = new Map<string, string>();
  constructor(init?: Headers | Record<string, string> | [string, string][]) {
    if (!init) return;
    if (init instanceof Headers) {
      init.forEach((value, key) => this.set(key, value));
    } else if (Array.isArray(init)) {
      for (const [key, value] of init) this.append(key, value);
    } else {
      for (const [key, value] of Object.entries(init)) this.set(key, String(value));
    }
  }
  append(key: string, value: string): void {
    const name = key.toLowerCase();
    const previous = this.values.get(name);
    this.values.set(name, previous === undefined ? String(value) : `${previous}, ${value}`);
  }
  set(key: string, value: string): void {
    this.values.set(key.toLowerCase(), String(value));
  }
  get(key: string): string | null {
    return this.values.get(key.toLowerCase()) ?? null;
  }
  has(key: string): boolean {
    return this.values.has(key.toLowerCase());
  }
  delete(key: string): void {
    this.values.delete(key.toLowerCase());
  }
  forEach(callback: (value: string, key: string) => void): void {
    for (const [key, value] of this.values) callback(value, key);
  }
  entries(): [string, string][] {
    return [...this.values.entries()];
  }
  toJSON(): Record<string, string> {
    return Object.fromEntries(this.values);
  }
}

export class Response {
  readonly ok: boolean;
  readonly status: number;
  readonly statusText: string;
  readonly headers: Headers;
  readonly url: string;
  readonly bodyUsed = false;
  private body: Uint8Array;
  constructor(
    bytes: Uint8Array,
    init: { status?: number; statusText?: string; headers?: Record<string, string>; url?: string },
  ) {
    this.body = bytes;
    this.status = init.status ?? 200;
    this.statusText = init.statusText ?? "";
    this.headers = new Headers(init.headers ?? {});
    this.url = init.url ?? "";
    this.ok = this.status >= 200 && this.status < 300;
  }
  private fail(): never {
    throw new TypeError("Body is unusable");
  }
  async arrayBuffer(): Promise<ArrayBuffer> {
    if (this.bodyUsed) this.fail();
    return this.body.slice().buffer;
  }
  async bytes(): Promise<Uint8Array> {
    if (this.bodyUsed) this.fail();
    return this.body.slice();
  }
  async text(): Promise<string> {
    if (this.bodyUsed) this.fail();
    return new TextDecoder().decode(this.body);
  }
  async json(): Promise<unknown> {
    return JSON.parse(await this.text());
  }
  async blob(): Promise<never> {
    throw new Error("Response.blob() is not supported in Wincast extensions.");
  }
}

export class Request {
  readonly url: string;
  readonly method: string;
  readonly headers: Headers;
  readonly body: unknown;
  readonly signal?: AbortSignal;
  constructor(input: string | Request, init: Record<string, unknown> = {}) {
    if (input instanceof Request) {
      this.url = input.url;
      this.method = String(init.method ?? input.method);
      this.headers = new Headers((init.headers as Record<string, string>) ?? input.headers);
      this.body = init.body !== undefined ? init.body : input.body;
      this.signal = (init.signal as AbortSignal) ?? input.signal;
      return;
    }
    this.url = String(input);
    this.method = String(init.method ?? "GET").toUpperCase();
    this.headers = new Headers(init.headers as Record<string, string>);
    this.body = init.body;
    this.signal = init.signal as AbortSignal;
  }
}

function encodeBody(body: unknown): string | null {
  if (body === undefined || body === null) return null;
  if (typeof body === "string") return bytesToBase64(new TextEncoder().encode(body));
  if (body instanceof Uint8Array) return bytesToBase64(body);
  if (ArrayBuffer.isView(body)) {
    return bytesToBase64(new Uint8Array(body.buffer, body.byteOffset, body.byteLength));
  }
  if (body instanceof ArrayBuffer) return bytesToBase64(new Uint8Array(body));
  return bytesToBase64(new TextEncoder().encode(String(body)));
}

function abortError(): DOMException {
  return new DOMException("The operation was aborted.", "AbortError");
}

export async function fetch(input: string | Request, init: Record<string, unknown> = {}): Promise<Response> {
  const request = input instanceof Request ? input : new Request(input, init);
  const signal = (init.signal as AbortSignal) ?? request.signal;
  if (signal?.aborted) throw abortError();
  const promise = hostCall("fetch", "request", [
    {
      url: request.url,
      method: request.method,
      headers: request.headers.toJSON(),
      bodyBase64: encodeBody(request.body),
    },
  ]);
  const raw = (await promise) as {
    status?: number;
    statusText?: string;
    headers?: Record<string, string>;
    url?: string;
    bodyBase64?: string;
  };
  if (signal?.aborted) throw abortError();
  return new Response(base64ToBytes(raw.bodyBase64 ?? ""), {
    status: raw.status,
    statusText: raw.statusText,
    headers: raw.headers,
    url: raw.url,
  });
}

export function installWebGlobals(): void {
  const global = globalThis as Record<string, unknown>;
  global.Headers = Headers;
  global.Request = Request;
  global.Response = Response;
  global.fetch = fetch;
  global.structuredClone = structuredClone;
}
