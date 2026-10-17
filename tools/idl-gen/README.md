# sillstone-idl-gen

Deterministic code generator reading `abi/sillstone.idl` and producing:
- `include/sillstone.h` (C ABI header)
- `crates/ffi/src/generated.rs` (Rust FFI glue)
- `sdk/python/sillstone/_generated.py` (Python ctypes wrapper)
- `sdk/node/src/generated.c` (Node.js N-API binding)
- `sdk/go/sillstone/generated.go` (Go cgo binding)
- `sdk/java/src/main/java/io/github/sillstone/Generated.java` (Java JNI binding)

Zero dependencies, written entirely with Rust `std`.
Run with `--check` in CI to guarantee that committed wrappers remain in sync with IDL.
