#!/usr/bin/env python3
"""Generates the NumPy oracle vectors for `crates/tensor/tests/numpy_oracle.rs`.

NumPy is a test-only oracle: the crate itself has no dependencies. Run this
script from anywhere and commit both it and the JSON it writes when an
operation's contract changes:

    python crates/tensor/tests/oracle/generate.py

Every vector is deterministic (no random draws), values are printed with
`repr` so they round-trip exactly, and non-finite numbers are encoded as the
strings "nan", "inf" and "-inf" because JSON has no literal for them.

`numpy` has no bfloat16, so `bf16_bits` expectations are computed here from the
`float32` bit pattern with the same round-to-nearest-even rule.
"""

import json
import math
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
OUT = HERE / "vectors.json"

NON_FINITE = {math.inf: "inf", -math.inf: "-inf"}


def enc(value):
    """Encodes one scalar for JSON, keeping non-finite values explicit."""
    if isinstance(value, (bool, np.bool_)):
        return bool(value)
    if isinstance(value, (int, np.integer)):
        return int(value)
    v = float(value)
    if math.isnan(v):
        return "nan"
    if math.isinf(v):
        return NON_FINITE[v]
    return v


def enc_list(values):
    return [enc(v) for v in np.asarray(values).ravel(order="C")]


def f16_bits(values):
    return [int(b) for b in np.asarray(values, dtype=np.float32).astype(np.float16).view(np.uint16).ravel()]


def bf16_bits(values):
    """Round-to-nearest-even narrowing of float32 to bfloat16, as raw bits."""
    f = np.asarray(values, dtype=np.float32)
    bits = f.view(np.uint32).astype(np.uint64)
    lsb = (bits >> 16) & 1
    # 0x7FFF is just below half of the discarded 16 bits; adding the lsb makes
    # an exact tie round up only when the kept mantissa is odd.
    rounded = bits + 0x7FFF + lsb
    return [int(b) for b in (rounded >> 16).astype(np.uint16).ravel()]


CASES = []


def case(name, op, shape, values, expect, dtype="f64", **extra):
    CASES.append(
        {
            "name": name,
            "op": op,
            "dtype": dtype,
            "shape": list(shape),
            "input": enc_list(values),
            "expect": [enc(e) for e in np.asarray(expect).ravel(order="C")],
            **extra,
        }
    )


def main() -> int:
    # --- elementwise math -------------------------------------------------
    pos = np.array([0.5, 1.0, 1.5, 2.0, 3.0, 0.1, 0.25, 10.0, 100.0, 1000.0,
                    1e-8, 1e8, 0.7071067811865476, 2.5, 3.25, 0.0001, 7.0,
                    1e-4, 123.456, 65536.0, 0.3333333333333333])
    unit = np.array([-1.0, -0.75, -0.5, -0.25, -0.1, 0.0, 0.1, 0.25, 0.5, 0.75,
                     1.0, 0.999999, -0.999999, 0.5, -0.5, 0.3, -0.3, 0.9, -0.9, 0.01, 0.0])
    anyv = np.array([0.0, 0.5, 1.0, -1.0, 1.5, -1.5, 2.0, -2.0, 3.141592653589793,
                     -3.141592653589793, 6.283185307179586, -6.283185307179586,
                     0.7853981633974483, -0.7853981633974483, 10.0, -10.0,
                     100.0, -100.0, 1e5, -1e5, 0.017453292519943295])
    exp_in = np.array([-700.0, -100.0, -10.0, -1.0, -0.5, -0.1, -1e-9, 0.0, 1e-9,
                       0.1, 0.5, 1.0, 2.0, 10.0, 100.0, 700.0, 709.0, -709.0,
                       -745.0, 3.0, 20.0])

    case("exp_f64", "exp", exp_in.shape, exp_in, np.exp(exp_in))
    case("ln_f64", "ln", pos.shape, pos, np.log(pos))
    case("log2_f64", "log2", pos.shape, pos, np.log2(pos))
    case("log10_f64", "log10", pos.shape, pos, np.log10(pos))
    case("sqrt_f64", "sqrt", pos.shape, pos, np.sqrt(pos))
    case("sqrt_zero", "sqrt", (3,), np.array([0.0, 4.0, 9.0]), np.sqrt([0.0, 4.0, 9.0]))
    case("sin_f64", "sin", anyv.shape, anyv, np.sin(anyv))
    case("cos_f64", "cos", anyv.shape, anyv, np.cos(anyv))
    case("tan_f64", "tan", anyv.shape, anyv, np.tan(anyv))
    case("atan_f64", "atan", anyv.shape, anyv, np.arctan(anyv))
    case("asin_f64", "asin", unit.shape, unit, np.arcsin(unit))
    case("acos_f64", "acos", unit.shape, unit, np.arccos(unit))

    bases = np.array([0.5, 1.0, 1.5, 2.0, 10.0, 0.1, 3.0, 7.0, 0.25, 100.0,
                      2.5, 0.75, 1.25, 4.0, 0.5, 9.0, 0.01, 1e3, 0.3, 6.0, 1.1])
    exps = np.array([0.0, 1.0, 2.0, 0.5, -1.0, 3.0, -0.5, 0.25, 10.0, -3.0,
                     1.5, -2.0, 2.5, 0.3333333333333333, 4.0, -0.25, 0.75, 3.0,
                     -10.0, 0.125, 20.0])
    case("pow_f64", "pow", bases.shape, bases, np.power(bases, exps), other_shape=list(bases.shape), other=enc_list(exps))

    ys = np.array([0.0, 1.0, -1.0, 2.0, -2.0, 0.5, -0.5, 10.0, -10.0, 1e-8,
                   -1e-8, 100.0, -100.0, 3.0, -3.0, 0.25, -0.25, 1e5, -1e5, 0.0, 1.0])
    xs = np.array([1.0, 1.0, 1.0, -1.0, -1.0, 2.0, 2.0, 0.0, 0.0, 1.0,
                   1.0, -100.0, -100.0, 4.0, 4.0, -0.5, -0.5, 1e5, 1e5, 0.0, -1.0])
    case("atan2_f64", "atan2", ys.shape, ys, np.arctan2(ys, xs), other_shape=list(ys.shape), other=enc_list(xs))

    # float32 keeps the same input set; the oracle is NumPy's float32 loop.
    case("sqrt_f32", "sqrt", pos.shape, pos.astype(np.float32), np.sqrt(pos.astype(np.float32)), dtype="f32")
    case("exp_f32", "exp", exp_in.shape, exp_in.astype(np.float32), np.exp(exp_in.astype(np.float32)), dtype="f32")
    case("sin_f32", "sin", anyv.shape, anyv.astype(np.float32), np.sin(anyv.astype(np.float32)), dtype="f32")

    # --- statistics -------------------------------------------------------
    stats_in = np.array([[1.0, 2.0, 3.0, 4.0],
                         [0.5, -0.5, 2.5, 7.5],
                         [10.0, 10.0, 10.0, 10.0]])
    for axis, axis_arg in ((None, None), (0, 0), (1, 1)):
        for ddof in (0, 1):
            var = np.var(stats_in, axis=axis, ddof=ddof)
            std = np.std(stats_in, axis=axis, ddof=ddof)
            shape = [] if axis is None else list(np.var(stats_in, axis=axis).shape)
            case(f"var_{axis}_{ddof}", "var", stats_in.shape, stats_in, np.atleast_1d(var),
                 args={"axis": axis_arg, "ddof": ddof}, expect_shape=shape)
            case(f"std_{axis}_{ddof}", "std", stats_in.shape, stats_in, np.atleast_1d(std),
                 args={"axis": axis_arg, "ddof": ddof}, expect_shape=shape)
    case("mean_f64", "mean", stats_in.shape, stats_in, [np.mean(stats_in)],
         args={"axis": None}, expect_shape=[])
    case("mean_axis1_f64", "mean", stats_in.shape, stats_in, np.mean(stats_in, axis=1),
         args={"axis": 1}, expect_shape=[3])
    # float16 and float32 moments are accumulated by NumPy in the wider type.
    stats_f16 = stats_in.astype(np.float16)
    case("var_f16", "var", stats_f16.shape, stats_f16, [np.var(stats_f16, dtype=np.float64)],
         dtype="f16", args={"axis": None, "ddof": 0}, expect_shape=[])
    case("mean_f16_axis0", "mean", stats_f16.shape, stats_f16,
         np.mean(stats_f16, axis=0, dtype=np.float64).astype(np.float16), dtype="f16",
         args={"axis": 0}, expect_shape=[4])

    # --- linear algebra ---------------------------------------------------
    a3 = np.array([[4.0, -2.0, 1.0], [-2.0, 4.0, -2.0], [1.0, -2.0, 4.0]])
    a4 = np.array([[2.0, 1.0, 0.0, 0.0],
                   [1.0, 3.0, 1.0, 0.0],
                   [0.0, 1.0, 4.0, 1.0],
                   [0.0, 0.0, 1.0, 5.0]])
    v3 = np.array([1.0, 2.0, 3.0])
    v4 = np.array([1.0, -1.0, 2.0, 0.5])
    m34 = np.array([[1.0, 0.0], [0.0, 1.0], [2.0, -1.0]])
    case("dot_1d_1d", "dot", v3.shape, v3, [np.dot(v3, v3)], other_shape=[3], other=enc_list(v3))
    case("dot_2d_2d", "dot", a3.shape, a3, np.dot(a3, a4[:3, :3]), other_shape=[3, 3], other=enc_list(a4[:3, :3]))
    case("dot_2d_1d", "dot", a3.shape, a3, np.dot(a3, v3), other_shape=[3], other=enc_list(v3))
    case("dot_1d_2d", "dot", v3.shape, v3, np.dot(v3, m34), other_shape=[3, 2], other=enc_list(m34))
    case("norm_vec", "norm", v3.shape, v3, [np.linalg.norm(v3)])
    case("norm_fro", "norm_fro", a3.shape, a3, [np.linalg.norm(a3)])
    case("norm_p1", "norm_p1", v3.shape, v3, [np.linalg.norm(v3, 1)])
    case("norm_pinf", "norm_pinf", v3.shape, v3, [np.linalg.norm(v3, np.inf)])
    case("inv_3", "inv", a3.shape, a3, np.linalg.inv(a3))
    case("inv_4", "inv", a4.shape, a4, np.linalg.inv(a4))
    case("det_3", "det", a3.shape, a3, [np.linalg.det(a3)])
    case("det_4", "det", a4.shape, a4, [np.linalg.det(a4)])
    case("solve_vec", "solve", a3.shape, a3, np.linalg.solve(a3, v3), other_shape=[3], other=enc_list(v3))
    case("solve_mat", "solve", a4.shape, a4, np.linalg.solve(a4, np.eye(4)), other_shape=[4, 4],
         other=enc_list(np.eye(4)))
    # NumPy has no LU, so the oracle records P*A and the factors it can check:
    # the determinant and the reconstruction of A from (L, U, perm) are checked
    # inside the Rust test.
    case("lu_reconstruct", "lu", a3.shape, a3, enc_list(a3))

    # --- shape combination ------------------------------------------------
    left = np.array([[1.0, 2.0], [3.0, 4.0]])
    right = np.array([[5.0, 6.0], [7.0, 8.0], [9.0, 10.0]])
    case("concat_axis0", "concat", left.shape, left, np.concatenate([left, right], axis=0),
         other_shape=[3, 2], other=enc_list(right), args={"axis": 0}, expect_shape=[5, 2])
    case("concat_axis1", "concat", left.shape, left, np.concatenate([left, left], axis=1),
         other_shape=[2, 2], other=enc_list(left), args={"axis": 1}, expect_shape=[2, 4])
    case("stack_axis0", "stack", left.shape, left, np.stack([left, left], axis=0),
         other_shape=[2, 2], other=enc_list(left), args={"axis": 0}, expect_shape=[2, 2, 2])
    case("stack_axis1", "stack", left.shape, left, np.stack([left, left], axis=1),
         other_shape=[2, 2], other=enc_list(left), args={"axis": 1}, expect_shape=[2, 2, 2])
    pad_in = np.array([[1.0, 2.0], [3.0, 4.0]])
    case("pad_const", "pad", pad_in.shape, pad_in, np.pad(pad_in, ((1, 0), (0, 2)), constant_values=-1.0),
         args={"pads": [[1, 0], [0, 2]], "value": -1.0}, expect_shape=[3, 4])

    # --- dtype conversion -------------------------------------------------
    cast_f = np.array([-2.7, -0.9, 0.0, 0.9, 2.7, 1234.5, -1234.5, 3.5, -3.5, 255.9, 256.1, 127.5])
    case("astype_f64_i32", "astype", cast_f.shape, cast_f, cast_f.astype(np.int32), dtype="f64", target="i32")
    case("astype_f64_i8", "astype", cast_f.shape, cast_f, np.clip(cast_f, -128, 127).astype(np.int8),
         dtype="f64", target="i8")
    case("astype_i32_f64", "astype", (6,), np.array([-3, 0, 7, 2**31 - 1, -2**31, 12345]),
         np.array([-3, 0, 7, 2**31 - 1, -2**31, 12345]).astype(np.float64), dtype="i32", target="f64")
    u8_vals = np.array([0, 1, 2, 100, 200, 255], dtype=np.uint8)
    case("astype_u8_f16", "astype", u8_vals.shape, u8_vals, f16_bits(u8_vals), dtype="u8", target="f16")
    case("astype_f64_bool", "astype", (6,), np.array([0.0, -0.0, 1.0, -1.0, 1e-9, -1e-9]),
         np.array([0.0, -0.0, 1.0, -1.0, 1e-9, -1e-9]).astype(bool), dtype="f64", target="bool")

    # --- half precision ---------------------------------------------------
    f16_in = np.array([0.0, -0.0, 1.0, -1.5, 0.1, 65504.0, 65520.0, 1e-7, 1e-8,
                       2.0**-24, 2.0**-25, 3.0 * 2.0**-25, 1.0 / 3.0, 1e4, -7e4,
                       1024.0, 0.0001220703125, 5.960464477539063e-08 * 1.5,
                       123.456, -0.333251953125])
    case("f16_bits", "f16_bits", f16_in.shape, f16_in, f16_bits(f16_in), dtype="f64")
    bf_in = np.array([0.0, -0.0, 1.0, -1.5, 0.1, 3.3895313892515355e38, 1e-40, 1.0 / 3.0,
                      123.456, -0.333251953125, 1e-8, 65536.0, 2.0**-133, 1e38, -7e38,
                      0.0001220703125, 5.0, 1000.0, 0.7, -0.7])
    case("bf16_bits", "bf16_bits", bf_in.shape, bf_in, bf16_bits(bf_in), dtype="f64")

    payload = {"numpy": np.__version__, "cases": CASES}
    OUT.write_text(json.dumps(payload, indent=1, allow_nan=False) + "\n", encoding="utf-8")
    print(f"wrote {OUT.relative_to(HERE.parent.parent.parent.parent)} with {len(CASES)} cases "
          f"({OUT.stat().st_size} bytes), numpy {np.__version__}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
