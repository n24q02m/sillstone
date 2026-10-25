// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 sillstone
"use strict";

// Loads the N-API addon once and owns the error type every facade throws. There is no
// pure-JS fallback: a package that cannot reach its native core must fail at require().

const path = require("node:path");
const fs = require("node:fs");

const STATUS_OK = 0;
const STATUS_INVALID = -1;
const STATUS_REJECTED = -2;

class FfiError extends Error {
  constructor(op, status) {
    const kind = { [STATUS_INVALID]: "invalid argument", [STATUS_REJECTED]: "input rejected" }[status] ?? "unknown failure";
    super(`${op} failed: ${kind} (status ${status})`);
    this.name = "FfiError";
    this.op = op;
    this.status = status;
  }
}

const platformKey = `${process.platform}-${process.arch}`;
const candidates = [
  path.join(__dirname, "prebuilds", platformKey, "sillstone_napi.node"),
  path.join(__dirname, "prebuilds", platformKey, process.platform === "win32" ? "sillstone_ffi.dll" : (process.platform === "darwin" ? "libsillstone_ffi.dylib" : "libsillstone_ffi.so")),
  path.join(__dirname, "sillstone_napi.node"),
  path.join(__dirname, "build", "Release", "sillstone_napi.node"),
  path.join(__dirname, "build", "Debug", "sillstone_napi.node"),
];

let native = null;
const loadErrors = [];
for (const c of candidates) {
  if (fs.existsSync(c)) {
    try {
      native = require(c);
      break;
    } catch (e) {
      loadErrors.push(`${c}: ${e.message}`);
    }
  }
}
if (!native) {
  throw new Error(
    `sillstone: native addon not loadable for ${platformKey}; tried:\n` +
      (loadErrors.length ? loadErrors : candidates.map((c) => `${c}: not found`)).join("\n"),
  );
}

/**
 * Calls a native function and converts the addon's status errors (an `Error` whose `code`
 * is the numeric status as a string) into {@link FfiError}. Anything else — a `TypeError`
 * from argument validation, a `RangeError` from a shape check — passes through unchanged.
 */
function call(name, ...args) {
  try {
    return native[name](...args);
  } catch (err) {
    if (err && typeof err.code === "string" && /^-?\d+$/.test(err.code)) {
      throw new FfiError(name, Number(err.code));
    }
    throw err;
  }
}

module.exports = { native, call, FfiError, STATUS_OK, STATUS_INVALID, STATUS_REJECTED };
