// Wincast's Raycast-extension entry point. The API implementation, React
// reconciler and protocol types come from Blast (MIT); this file is the host
// adapter: it evaluates a prebuilt CommonJS bundle, forwards scene trees to Go,
// and turns Go host results back into capability results.

import "./prelude.ts";
import { startRuntime, type RuntimeController } from "./blast/runtime/core.ts";
import { environment } from "./blast/runtime/api/environment.ts";
import type {
  InitEnvironment,
  InitMessage,
  JsonValue,
  RuntimeMessage,
  SceneNode,
} from "./blast/protocol/index.ts";
import {
  clearAllTimers,
  configureBoot,
  describeError,
  fireHostTimer,
  host,
  log,
  settleCall,
} from "./bridge.ts";
import { evaluateCommonJS } from "./modules.ts";
import { installTimers, installWebGlobals } from "./polyfills.ts";
import { refreshNodeInfo } from "./node-shims.ts";

type LaunchContext = {
  environment?: Partial<InitEnvironment>;
  preferences?: Record<string, JsonValue>;
  caches?: Record<string, Record<string, string>>;
  launchProps?: { arguments?: Record<string, string>; fallbackText?: string };
};

type SceneMessage = { type: "scene"; revision: number; root: SceneNode | null };

let session = "";
let listener: ((value: unknown) => void) | undefined;
let controller: RuntimeController | undefined;
let commandCode = "";
let commandFile = "";
let commandDir = "";
let lastRoot: SceneNode | null = null;

function requireListener(): (value: unknown) => void {
  if (!listener) throw new Error("The extension runtime is not listening for host messages.");
  return listener;
}

function send(message: RuntimeMessage): void {
  switch (message.type) {
    case "scene":
      lastRoot = (message as SceneMessage).root;
      host.render(session, JSON.stringify(adaptTree(lastRoot)));
      return;
    case "capability.request":
      try {
        const separator = message.name.indexOf(".");
        const api = separator < 0 ? message.name : message.name.slice(0, separator);
        const method = separator < 0 ? "" : message.name.slice(separator + 1);
        host.invoke(message.id, api, method, JSON.stringify(message.params));
      } catch (error) {
        listener?.({
          type: "capability.result",
          id: message.id,
          ok: false,
          error: { code: "HOST_ERROR", message: describeError(error) },
        });
      }
      return;
    case "ready":
      return;
    case "log":
      host.log(message.level, message.message);
      return;
    case "error":
      if (message.fatal) host.failed(session, message.message);
      else host.log("error", message.message);
      return;
    default:
      return;
  }
}

function loadCommand(command: { entry: string; mode: string }): unknown {
  const moduleObject = evaluateCommonJS(commandCode, command.entry || commandFile, commandDir);
  const exports = moduleObject.exports as { default?: unknown } | undefined;
  if (exports && typeof exports === "object" && "default" in exports) return exports;
  return { default: exports };
}

function start(rawSession: string, code: string, filename: string, dirname: string, mode: string, contextJSON: string): string {
  stop(rawSession);
  session = rawSession;
  commandCode = code;
  commandFile = filename;
  commandDir = dirname;
  const context = parseContext(contextJSON);
  const extensionName = String(context.environment?.extensionName ?? "");
  const commandName = String(context.environment?.commandName ?? "");
  const launchArguments = context.launchProps?.arguments ?? {};

  controller = startRuntime({
    send,
    onMessage(next) {
      listener = next;
      return () => {
        if (listener === next) listener = undefined;
      };
    },
    exit(code) {
      if (code === 0) host.finished(session);
    },
    log(level, args) {
      log(level, args);
    },
    loadCommand,
  });

  const environmentMessage: InitEnvironment = {
    extensionName,
    commandName,
    ownerOrAuthorName: String(context.environment?.ownerOrAuthorName ?? extensionName),
    assetsPath: String(context.environment?.assetsPath ?? ""),
    supportPath: String(context.environment?.supportPath ?? ""),
    isDevelopment: Boolean(context.environment?.isDevelopment),
    appearance: context.environment?.appearance === "light" ? "light" : "dark",
    textSize: context.environment?.textSize === "large" ? "large" : "medium",
  };
  const init: InitMessage = {
    type: "init",
    protocol: 1,
    environment: environmentMessage,
    preferences: context.preferences ?? {},
    command: {
      extension: extensionName,
      name: commandName,
      entry: filename,
      mode: mode === "view" ? "view" : mode === "menu-bar" ? "menu-bar" : "no-view",
      arguments: launchArguments,
      ...(context.launchProps?.fallbackText === undefined
        ? {}
        : { fallbackText: context.launchProps.fallbackText }),
    },
  };
  requireListener()(init);
  // The runtime pretends to be the @raycast/api level it implements, not Blast's own.
  environment.raycastVersion = "2.0.3";
  return "ok";
}

function stop(_session?: string): string {
  controller?.();
  controller = undefined;
  listener = undefined;
  clearAllTimers();
  lastRoot = null;
  session = "";
  return "ok";
}

function dispatch(_session: string, handler: string, argsJSON: string): string {
  const separator = handler.indexOf(":");
  if (separator <= 0) return "0";
  const node = handler.slice(0, separator);
  const name = handler.slice(separator + 1);
  let args: unknown[];
  try {
    args = JSON.parse(argsJSON || "[]") as unknown[];
  } catch {
    args = [];
  }
  try {
    requireListener()({ type: "event", node, handler: name, args });
    return "1";
  } catch (error) {
    host.log("error", describeError(error));
    return "0";
  }
}

function popNavigation(_session: string): string {
  const navigation = lastRoot ? findNode(lastRoot, "Navigation") : null;
  const pop = navigation?.props?.pop;
  if (!navigation || !pop || typeof pop !== "object" || !("$handler" in pop)) return "0";
  try {
    requireListener()({
      type: "event",
      node: navigation.id,
      handler: String((pop as { $handler: unknown }).$handler),
      args: [],
    });
    return "1";
  } catch (error) {
    host.log("error", describeError(error));
    return "0";
  }
}

function runToastAction(_token: string): string {
  return "0";
}

function settle(id: string, ok: boolean, payload: string): void {
  if (settleCall(id, ok, payload)) return;
  const current = listener;
  if (!current) return;
  if (ok) {
    let value: JsonValue = null;
    if (payload !== "") {
      try {
        value = JSON.parse(payload) as JsonValue;
      } catch {
        value = payload;
      }
    }
    current({ type: "capability.result", id, ok: true, value });
  } else {
    current({
      type: "capability.result",
      id,
      ok: false,
      error: { code: "HOST_ERROR", message: payload || "Host call failed" },
    });
  }
}

function parseContext(raw: string): LaunchContext {
  if (!raw) return {};
  try {
    return JSON.parse(raw) as LaunchContext;
  } catch {
    return {};
  }
}

function findNode(node: SceneNode, type: string): SceneNode | null {
  if (node.type === type) return node;
  for (const child of node.children) {
    const found = findNode(child, type);
    if (found) return found;
  }
  return null;
}

function adaptTree(root: SceneNode | null): { children: unknown[] } {
  if (!root) return { children: [] };
  const roots = root.type === "#root" ? root.children : [root];
  const children: unknown[] = [];
  for (const node of roots) {
    if (node.type === "Navigation") {
      const navigation = adaptNode(node);
      navigation.children = [];
      navigation.type = "__navigation";
      children.push(navigation);
      for (const child of node.children) children.push(adaptNode(child));
      continue;
    }
    children.push(adaptNode(node));
  }
  return { children };
}

type AdaptedNode = {
  id: number;
  type: string;
  props: Record<string, unknown>;
  children: AdaptedNode[];
  text?: string;
};

function adaptNode(node: SceneNode): AdaptedNode {
  if (node.type === "#text") {
    return {
      id: 0,
      type: "#text",
      props: {},
      children: [],
      text: String(node.props.text ?? ""),
    };
  }
  const id = Number(node.id) || 0;
  const props: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(node.props)) {
    if (key === "name") continue;
    props[key] = adaptValue(value, id);
  }
  const children: AdaptedNode[] = [];
  for (const child of node.children) {
    if (child.type === "Slot") {
      const name = String(child.props.name ?? "");
      const folded = child.children.map(adaptNode);
      props[name] = folded.length <= 1 ? (folded[0] ?? null) : folded;
      continue;
    }
    children.push(adaptNode(child));
  }
  let type = node.type;
  if (type === "Navigation.Entry") type = "__screen";
  return { id, type, props, children };
}

function adaptValue(value: unknown, nodeID: number): unknown {
  if (Array.isArray(value)) return value.map((item) => adaptValue(item, nodeID));
  if (!value || typeof value !== "object") return value;
  const object = value as Record<string, unknown>;
  if (typeof object.$handler === "string") return { $fn: `${nodeID}:${object.$handler}` };
  const output: Record<string, unknown> = {};
  for (const [key, item] of Object.entries(object)) output[key] = adaptValue(item, nodeID);
  return output;
}

declare global {
  // eslint-disable-next-line no-var
  var __tinycast: {
    boot(configJSON: string): string;
    start(session: string, code: string, filename: string, dirname: string, mode: string, contextJSON: string): string;
    dispatch(session: string, handler: string, argsJSON: string): string;
    popNavigation(session: string): string;
    runToastAction(token: string): string;
    settle(id: string, ok: boolean, payload: string): void;
    fireTimer(id: string): void;
    stop(session: string): string;
  };
}

installTimers();
installWebGlobals();

globalThis.__tinycast = {
  boot(configJSON) {
    configureBoot(configJSON);
    refreshNodeInfo();
    return "ok";
  },
  start,
  stop,
  dispatch,
  popNavigation,
  runToastAction,
  settle,
  fireTimer: fireHostTimer,
};
