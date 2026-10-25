const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const nodeDir = path.resolve(__dirname, "..");
// Release packaging overrides: SILLSTONE_FFI_DIR (where the cdylib was built),
// SILLSTONE_NAPI_OUT (where the addon is written) and SILLSTONE_NAPI_ARCH
// (macOS cross-compile, e.g. x86_64). Defaults keep the local dev layout.
const outDir = process.env.SILLSTONE_NAPI_OUT
  ? path.resolve(process.env.SILLSTONE_NAPI_OUT)
  : path.join(nodeDir, "build", "Release");
fs.mkdirSync(outDir, { recursive: true });

const outFile = path.join(outDir, "sillstone_napi.node");
const srcFile = path.join(nodeDir, "src", "generated.c");
const repoRoot = path.resolve(nodeDir, "..", "..");
const ffiLibDir = process.env.SILLSTONE_FFI_DIR
  ? path.resolve(process.env.SILLSTONE_FFI_DIR)
  : path.join(repoRoot, "target", "debug");
const napiArch = process.env.SILLSTONE_NAPI_ARCH ? `-arch ${process.env.SILLSTONE_NAPI_ARCH}` : "";

console.log("Building N-API addon...");

if (process.platform === "win32") {
  const nodeDef = path.join(nodeDir, "src", "node.def");
  const libNode = path.join(outDir, "libnode.a");
  
  // 1. Generate libnode.a import library if not present
  try {
    execSync(`dlltool -d "${nodeDef}" -D node.exe -l "${libNode}"`, { stdio: "inherit" });
  } catch (e) {
    console.error("dlltool failed:", e.message);
  }

  // 2. Compile generated.c with gcc linking against libnode.a and sillstone_ffi.dll
  const cmd = `gcc -shared -O3 -I"${path.join(nodeDir, "src")}" -I"${path.join(repoRoot, "include")}" "${srcFile}" "${libNode}" -L"${ffiLibDir}" -lsillstone_ffi -o "${outFile}"`;
  console.log("Executing:", cmd);
  execSync(cmd, { stdio: "inherit" });
} else if (process.platform === "darwin") {
  // @loader_path: the addon resolves libsillstone_ffi.dylib next to itself.
  const cmd = `gcc -shared -O3 -fPIC ${napiArch} -undefined dynamic_lookup -Wl,-rpath,@loader_path -I"${path.join(nodeDir, "src")}" -I"${path.join(repoRoot, "include")}" "${srcFile}" -L"${ffiLibDir}" -lsillstone_ffi -o "${outFile}"`;
  execSync(cmd, { stdio: "inherit" });
} else {
  // Linux
  const cmd = `gcc -shared -O3 -fPIC -I"${path.join(nodeDir, "src")}" -I"${path.join(repoRoot, "include")}" "${srcFile}" -L"${ffiLibDir}" -lsillstone_ffi -Wl,-rpath,'$ORIGIN' -Wl,-rpath,'$ORIGIN/../../../../target/debug' -o "${outFile}"`;
  console.log("Executing:", cmd);
  execSync(cmd, { stdio: "inherit" });
}

if (process.platform === "win32") {
  const dllSrc = path.join(ffiLibDir, "sillstone_ffi.dll");
  const dllDst = path.join(outDir, "sillstone_ffi.dll");
  if (fs.existsSync(dllSrc)) {
    fs.copyFileSync(dllSrc, dllDst);
    console.log("Copied sillstone_ffi.dll to:", dllDst);
  }
}
console.log("N-API addon built successfully at:", outFile);
