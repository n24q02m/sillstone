// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 sillstone Contributors
"use strict";
// Load-time gate for a freshly built N-API addon: copy the addon and the cdylib it
// links against into an empty directory (so nothing is found through build-tree
// rpaths) and call into native code. Usage: smoke-napi.js <addon.node> <cdylib>
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const [addon, cdylib] = process.argv.slice(2);
if (!addon || !cdylib) {
  console.error("usage: smoke-napi.js <addon.node> <cdylib>");
  process.exit(2);
}
const dir = fs.mkdtempSync(path.join(os.tmpdir(), "sillstone-napi-"));
for (const f of [addon, cdylib]) fs.copyFileSync(f, path.join(dir, path.basename(f)));
const native = require(path.join(dir, path.basename(addon)));

const sha = native.sillstone_digest_sha256(Buffer.from("abc")).toString("hex");
assert.strictEqual(sha, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
const rgb = Buffer.alloc(4 * 4 * 3, 7);
assert.strictEqual(native.sillstone_image_resize(rgb, 4, 4, 2, 2, 2, 2, 12).length, 12);
assert.strictEqual(typeof native.sillstone_image_phash(Buffer.alloc(64 * 64 * 3, 9), 64, 64, 2), "bigint");
console.log(`ok: ${path.basename(addon)} loads from ${dir} and answers native calls`);
