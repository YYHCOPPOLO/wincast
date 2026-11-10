// Installed before every other bundled module. A handful of dependencies read
// these globals while their own module body runs, so the full polyfills would
// be too late.

import { TextDecoder, TextEncoder } from "./text-codecs.ts";

const globals = globalThis as Record<string, unknown>;

if (typeof globals.global === "undefined") globals.global = globalThis;
if (typeof globals.TextEncoder === "undefined") globals.TextEncoder = TextEncoder;
if (typeof globals.TextDecoder === "undefined") globals.TextDecoder = TextDecoder;
if (typeof globals.process === "undefined") {
  globals.process = {
    env: {},
    platform: "win32",
    arch: "x64",
    cwd: () => "",
    nextTick: (callback: (...args: unknown[]) => void, ...args: unknown[]) =>
      Promise.resolve().then(() => callback(...args)),
    versions: { node: "22.0.0" },
    pid: 1,
    version: "v22.0.0",
  };
}
