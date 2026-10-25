// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 sillstone Contributors
"use strict";
// Run from a project that has just `npm install`ed the packed tarball. The package
// must load its native addon: image hashing has no JavaScript fallback, so it
// throws when only a raw cdylib (or nothing loadable) was shipped.
const assert = require("node:assert");

const sillstone = require(require.resolve("sillstone", { paths: [process.cwd()] }));
assert.strictEqual(
  sillstone.sha256(Buffer.from("abc")).toString("hex"),
  "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
);
assert.strictEqual(typeof sillstone.phash(Buffer.alloc(64 * 64 * 3, 9), 64, 64, 2), "bigint");
console.log("ok: installed sillstone tarball loads its native addon");
