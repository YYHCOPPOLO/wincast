// CommonJS module registry for prebuilt extension bundles. Raycast's build
// keeps react, react/jsx-runtime and @raycast/api external; everything else is
// inlined. Node built-ins are supplied by node-shims.ts.

import * as React from "react";
import * as JSXRuntime from "react/jsx-runtime";
import * as RaycastAPI from "./blast/runtime/api/index.ts";
import { nodeModule } from "./node-shims.ts";

declare const __tinycastCompile: (code: string, filename: string) => (
  exports: unknown,
  require: (name: string) => unknown,
  module: { exports: unknown },
  filename: string,
  dirname: string,
) => void;

const reactModule = { ...React, default: React };
const jsxModule = { ...JSXRuntime, default: JSXRuntime };
const apiModule = { ...RaycastAPI, default: RaycastAPI };

const modules = new Map<string, unknown>([
  ["react", reactModule],
  ["react/jsx-runtime", jsxModule],
  ["react/jsx-dev-runtime", jsxModule],
  ["@raycast/api", apiModule],
]);

function requireModule(name: string): unknown {
  const known = modules.get(name);
  if (known) return known;
  const builtin = nodeModule(name);
  if (builtin !== undefined) {
    modules.set(name, builtin);
    return builtin;
  }
  if (name.startsWith(".") || name.startsWith("/") || /^[a-zA-Z]:[\\/]/.test(name)) {
    throw new Error(
      `Cannot require relative module '${name}' from a prebuilt extension bundle. ` +
        "Run `ray build` so dependencies are bundled.",
    );
  }
  throw new Error(
    `Cannot find module '${name}'. Wincast supplies react, react/jsx-runtime, @raycast/api and Node built-ins.`,
  );
}

export function evaluateCommonJS(code: string, filename: string, dirname: string): { exports: unknown } {
  const moduleObject: { exports: unknown } = { exports: {} };
  const compiled = __tinycastCompile(code, filename);
  compiled.call(
    moduleObject.exports,
    moduleObject.exports,
    requireModule,
    moduleObject,
    filename,
    dirname,
  );
  return moduleObject;
}
