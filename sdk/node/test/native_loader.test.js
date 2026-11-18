// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 sillstone
"use strict";

// native.js searches a fixed candidate list once, at require() time, so each case runs
// in a fresh child process with `fs.existsSync`, `process.platform` or the `.node`
// loader replaced before the require. The asserted behaviour: the per-platform library
// name, the "not found" report when nothing is on disk, the search going on past a
// candidate that exists but fails to load, and the load error being reported for every
// candidate when none loads.

const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const NODE_DIR = path.join(__dirname, "..");
const LOADER = path.join(NODE_DIR, "native.js");

// Runs `setup` in a child, then requires native.js and reports either the type of one
// addon export or the message require() threw.
function loadWith(setup) {
  const script = `
    const fs = require("node:fs");
    const Module = require("node:module");
    ${setup}
    try {
      const { native } = require(${JSON.stringify(LOADER)});
      process.stdout.write(JSON.stringify({ loaded: typeof native.sillstone_version_major }));
    } catch (err) {
      process.stdout.write(JSON.stringify({ error: err.message }));
    }
  `;
  const child = spawnSync(process.execPath, ["-e", script], { encoding: "utf8" });
  assert.equal(child.status, 0, child.stderr);
  return JSON.parse(child.stdout);
}

test("with nothing on disk every candidate is reported as not found, named for the platform", () => {
  const libraries = { linux: "libsillstone_ffi.so", darwin: "libsillstone_ffi.dylib", win32: "sillstone_ffi.dll" };
  for (const [platform, library] of Object.entries(libraries)) {
    const result = loadWith(`
      Object.defineProperty(process, "platform", { value: ${JSON.stringify(platform)} });
      fs.existsSync = () => false;
    `);
    const key = `${platform}-${process.arch}`;
    const lines = result.error.split("\n");
    assert.equal(lines[0], `sillstone: native addon not loadable for ${key}; tried:`, platform);
    assert.deepEqual(
      lines.slice(1),
      [
        path.join(NODE_DIR, "prebuilds", key, "sillstone_napi.node"),
        path.join(NODE_DIR, "prebuilds", key, library),
        path.join(NODE_DIR, "sillstone_napi.node"),
        path.join(NODE_DIR, "build", "Release", "sillstone_napi.node"),
        path.join(NODE_DIR, "build", "Debug", "sillstone_napi.node"),
      ].map((candidate) => `${candidate}: not found`),
      platform,
    );
  }
});

test("a candidate that exists but does not load is skipped for the next one", () => {
  // The prebuilds paths claim to exist but are not on disk, so require() fails on both;
  // the built addon later in the list still loads.
  const prebuilds = path.join(NODE_DIR, "prebuilds");
  const result = loadWith(`
    const realExists = fs.existsSync;
    fs.existsSync = (p) => String(p).startsWith(${JSON.stringify(prebuilds)}) || realExists(p);
  `);
  assert.deepEqual(result, { loaded: "function" });
});

test("when every candidate fails to load, each load error is reported instead", () => {
  const result = loadWith(`
    fs.existsSync = () => true;
    Module._extensions[".node"] = () => {
      throw new Error("simulated load failure");
    };
  `);
  const key = `${process.platform}-${process.arch}`;
  assert.ok(result.error.startsWith(`sillstone: native addon not loadable for ${key}; tried:\n`), result.error);
  assert.ok(!result.error.includes(": not found"), result.error);
  // The built addon is on disk, so its line carries the loader's own error.
  const built = path.join(NODE_DIR, "build", "Release", "sillstone_napi.node");
  assert.ok(result.error.includes(`\n${built}: simulated load failure`), result.error);
  // Every candidate gets a line, in search order.
  let from = 0;
  for (const candidate of [
    path.join(NODE_DIR, "prebuilds", key, "sillstone_napi.node"),
    path.join(NODE_DIR, "sillstone_napi.node"),
    built,
    path.join(NODE_DIR, "build", "Debug", "sillstone_napi.node"),
  ]) {
    const at = result.error.indexOf(`\n${candidate}: `, from);
    assert.ok(at >= from, `${candidate} missing or out of order in:\n${result.error}`);
    from = at + 1;
  }
});
