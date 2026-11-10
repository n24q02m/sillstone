//! NumPy oracle conformance.
//!
//! `tests/oracle/generate.py` runs NumPy over fixed inputs and writes
//! `oracle/vectors.json`; this test replays those vectors through the crate and
//! compares bit patterns. NumPy is a test-only oracle — the crate itself takes
//! no dependencies.
//!
//! Budgets are `EPSILON * max(|expected|, 1)` units: a relative bound for
//! normal-sized results and a few-epsilon absolute bound near zero, which is how
//! `libm` conformance suites state accuracy. `cargo test -p sillstone-tensor
//! --test numpy_oracle -- --nocapture` prints the measured worst case per
//! operation.

mod json {
    //! A minimal JSON reader: the fixture only holds objects, arrays, numbers,
    //! strings and booleans, and the crate has no serde to lean on.

    /// One JSON value.
    #[derive(Debug, Clone)]
    pub enum Json {
        /// `null`
        Null,
        /// `true` / `false`
        Bool(bool),
        /// A number.
        Num(f64),
        /// A string.
        Str(String),
        /// An array.
        Arr(Vec<Json>),
        /// An object with its keys in file order.
        Obj(Vec<(String, Json)>),
    }

    impl Json {
        /// Looks a key up in an object.
        pub fn get(&self, key: &str) -> Option<&Json> {
            match self {
                Json::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }

        /// Iterates the array elements, or nothing for a non-array.
        pub fn items(&self) -> &[Json] {
            match self {
                Json::Arr(items) => items,
                _ => &[],
            }
        }
    }

    struct Parser<'a> {
        bytes: &'a [u8],
        pos: usize,
    }

    /// Parses a whole document.
    pub fn parse(text: &str) -> Json {
        let mut p = Parser {
            bytes: text.as_bytes(),
            pos: 0,
        };
        let value = p.value();
        p.ws();
        assert_eq!(
            p.pos,
            p.bytes.len(),
            "trailing bytes after the JSON document"
        );
        value
    }

    impl Parser<'_> {
        fn ws(&mut self) {
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
        }

        fn expect(&mut self, byte: u8) {
            assert_eq!(
                self.bytes[self.pos], byte,
                "unexpected byte at {}",
                self.pos
            );
            self.pos += 1;
        }

        fn value(&mut self) -> Json {
            self.ws();
            match self.bytes[self.pos] {
                b'{' => {
                    self.pos += 1;
                    let mut fields = Vec::new();
                    self.ws();
                    if self.bytes[self.pos] == b'}' {
                        self.pos += 1;
                        return Json::Obj(fields);
                    }
                    loop {
                        self.ws();
                        let key = self.string();
                        self.ws();
                        self.expect(b':');
                        let value = self.value();
                        fields.push((key, value));
                        self.ws();
                        if self.bytes[self.pos] == b',' {
                            self.pos += 1;
                            continue;
                        }
                        self.expect(b'}');
                        break;
                    }
                    Json::Obj(fields)
                }
                b'[' => {
                    self.pos += 1;
                    let mut items = Vec::new();
                    self.ws();
                    if self.bytes[self.pos] == b']' {
                        self.pos += 1;
                        return Json::Arr(items);
                    }
                    loop {
                        items.push(self.value());
                        self.ws();
                        if self.bytes[self.pos] == b',' {
                            self.pos += 1;
                            continue;
                        }
                        self.expect(b']');
                        break;
                    }
                    Json::Arr(items)
                }
                b'"' => Json::Str(self.string()),
                b't' => {
                    self.pos += 4;
                    Json::Bool(true)
                }
                b'f' => {
                    self.pos += 5;
                    Json::Bool(false)
                }
                b'n' => {
                    self.pos += 4;
                    Json::Null
                }
                _ => {
                    let start = self.pos;
                    while self.pos < self.bytes.len()
                        && matches!(
                            self.bytes[self.pos],
                            b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                        )
                    {
                        self.pos += 1;
                    }
                    let text =
                        core::str::from_utf8(&self.bytes[start..self.pos]).expect("ascii number");
                    Json::Num(text.parse().expect("valid number"))
                }
            }
        }

        fn string(&mut self) -> String {
            self.expect(b'"');
            let mut out = String::new();
            loop {
                let byte = self.bytes[self.pos];
                self.pos += 1;
                match byte {
                    b'"' => break,
                    b'\\' => {
                        let esc = self.bytes[self.pos];
                        self.pos += 1;
                        match esc {
                            b'"' => out.push('"'),
                            b'\\' => out.push('\\'),
                            b'/' => out.push('/'),
                            b'b' => out.push('\u{8}'),
                            b'f' => out.push('\u{c}'),
                            b'n' => out.push('\n'),
                            b'r' => out.push('\r'),
                            b't' => out.push('\t'),
                            b'u' => {
                                let hex = core::str::from_utf8(&self.bytes[self.pos..self.pos + 4])
                                    .expect("ascii hex");
                                self.pos += 4;
                                let code = u32::from_str_radix(hex, 16).expect("hex");
                                out.push(char::from_u32(code).expect("scalar value"));
                            }
                            other => panic!("unsupported escape \\{}", other as char),
                        }
                    }
                    _ => out.push(byte as char),
                }
            }
            out
        }
    }
}

use std::collections::BTreeMap;

use json::Json;
use sillstone_tensor::{Bf16, F16, NormOrd, Tensor, concat, stack};

/// Worst-case error budget per operation, in units of
/// `EPSILON * max(|expected|, 1)`. Every number here was measured with the
/// committed fixture and then given a one-step margin, so a regression trips
/// immediately; `0` means the result must be bit-exact.
const BUDGET_F64: &[(&str, u64)] = &[
    ("exp", 3),
    ("ln", 2),
    ("log2", 2),
    ("log10", 2),
    ("sqrt", 1),
    ("sin", 2),
    ("cos", 2),
    ("tan", 2),
    ("asin", 2),
    ("acos", 2),
    ("atan", 0),
    ("atan2", 0),
    ("pow", 6),
    ("var", 1),
    ("std", 2),
    ("mean", 1),
    ("dot", 1),
    ("norm", 1),
    ("norm_fro", 1),
    ("norm_p1", 1),
    ("norm_pinf", 0),
    ("inv", 2),
    ("det", 2),
    ("solve", 2),
    ("lu", 0),
    ("concat", 0),
    ("stack", 0),
    ("pad", 0),
    ("astype", 0),
    ("f16_bits", 0),
    ("bf16_bits", 0),
];

/// Same for `float32` results, where one epsilon is a much coarser step.
const BUDGET_F32: &[(&str, u32)] = &[("sqrt", 1), ("exp", 2), ("sin", 2)];

fn budget_f64(op: &str) -> u64 {
    BUDGET_F64
        .iter()
        .find(|(name, _)| *name == op)
        .map(|(_, b)| *b)
        .unwrap_or(0)
}

fn budget_f32(op: &str) -> u32 {
    BUDGET_F32
        .iter()
        .find(|(name, _)| *name == op)
        .map(|(_, b)| *b)
        .unwrap_or(2)
}

/// Error in units of `EPSILON * max(|expected|, 1)`: a relative bound for
/// normal-sized results and an absolute bound of a few epsilon near zero, which
/// is how `libm` conformance suites state accuracy.
fn error_f64(got: f64, want: f64) -> f64 {
    if got.is_nan() && want.is_nan() {
        return 0.0;
    }
    if got.is_nan() || want.is_nan() {
        return f64::INFINITY;
    }
    if got.is_infinite() || want.is_infinite() {
        return if got == want { 0.0 } else { f64::INFINITY };
    }
    (got - want).abs() / (want.abs().max(1.0) * f64::EPSILON)
}

fn error_f32(got: f32, want: f32) -> f32 {
    if got.is_nan() && want.is_nan() {
        return 0.0;
    }
    if got.is_nan() || want.is_nan() {
        return f32::INFINITY;
    }
    if got.is_infinite() || want.is_infinite() {
        return if got == want { 0.0 } else { f32::INFINITY };
    }
    (got - want).abs() / (want.abs().max(1.0) * f32::EPSILON)
}

fn ulp16(a: u16, b: u16) -> u32 {
    let mapped = |x: u16| {
        let bits = x as i16 as i32;
        if bits < 0 {
            i64::from(i32::MIN) - i64::from(bits)
        } else {
            i64::from(bits)
        }
    };
    u32::try_from((mapped(a) - mapped(b)).unsigned_abs()).unwrap_or(u32::MAX)
}

/// Reports the error in epsilon units rounded up, so it prints as an integer.
fn rounds(err: f64) -> u64 {
    if err.is_infinite() {
        u64::MAX
    } else {
        err.ceil() as u64
    }
}

fn rounds32(err: f32) -> u32 {
    if err.is_infinite() {
        u32::MAX
    } else {
        err.ceil() as u32
    }
}

/// Reads one encoded scalar; the generator writes non-finite values as strings.
fn scalar(v: &Json) -> f64 {
    match v {
        Json::Num(x) => *x,
        Json::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Json::Str(s) => match s.as_str() {
            "nan" => f64::NAN,
            "inf" => f64::INFINITY,
            "-inf" => f64::NEG_INFINITY,
            other => other.parse().expect("numeric string"),
        },
        other => panic!("not a scalar: {other:?}"),
    }
}

fn numbers(v: &Json) -> Vec<f64> {
    v.items().iter().map(scalar).collect()
}

fn usizes(v: &Json) -> Vec<usize> {
    v.items()
        .iter()
        .map(|x| {
            usize::try_from(u64::try_from(scalar(x) as i64).expect("non-negative")).expect("fits")
        })
        .collect()
}

fn required<'a>(case: &'a Json, key: &str) -> &'a Json {
    case.get(key)
        .unwrap_or_else(|| panic!("case is missing `{key}`"))
}

fn name_of(case: &Json) -> String {
    match case.get("name") {
        Some(Json::Str(s)) => s.clone(),
        _ => "<unnamed>".to_string(),
    }
}

fn op_of(case: &Json) -> String {
    match case.get("op") {
        Some(Json::Str(s)) => s.clone(),
        _ => "<no op>".to_string(),
    }
}

fn dtype_of(case: &Json) -> String {
    match case.get("dtype") {
        Some(Json::Str(s)) => s.clone(),
        _ => "f64".to_string(),
    }
}

/// Asserts the result shape against `expect_shape`, or against the input shape.
fn check_shape(name: &str, got: &[usize], expect: Option<Vec<usize>>, input: &[usize]) {
    let want = expect.unwrap_or_else(|| input.to_vec());
    assert_eq!(got, want.as_slice(), "case {name}: shape");
}

fn assert_ulp(name: &str, got: &[f64], want: &[f64], budget: u64) -> u64 {
    assert_eq!(got.len(), want.len(), "case {name}: element count");
    let mut worst = 0u64;
    for (i, (g, w)) in got.iter().zip(want.iter()).enumerate() {
        let err = error_f64(*g, *w);
        if err > budget as f64 {
            panic!("case {name}: element {i}: got {g:?}, numpy {w:?}, error {err:.3} epsilon");
        }
        worst = worst.max(rounds(err));
    }
    worst
}

fn assert_ulp32(name: &str, got: &[f32], want: &[f64], budget: u32) -> u32 {
    assert_eq!(got.len(), want.len(), "case {name}: element count");
    let mut worst = 0u32;
    for (i, (g, w)) in got.iter().zip(want.iter()).enumerate() {
        let expected = *w as f32;
        let err = error_f32(*g, expected);
        if err > budget as f32 {
            panic!("case {name}: element {i}: got {g:?}, numpy {w:?}, error {err:.3} epsilon");
        }
        worst = worst.max(rounds32(err));
    }
    worst
}

/// Runs one oracle case and returns the worst ULP distance it observed.
fn run_case(case: &Json) -> (String, u64) {
    let name = name_of(case);
    let op = op_of(case);
    let shape = usizes(required(case, "shape"));
    let input = numbers(required(case, "input"));
    let want = numbers(required(case, "expect"));
    let expect_shape = case.get("expect_shape").map(usizes);

    let ax = case
        .get("args")
        .and_then(|a| a.get("axis"))
        .map(|a| match a {
            Json::Null => None,
            other => Some(usize::try_from(scalar(other) as i64).expect("axis")),
        })
        .unwrap_or(None);
    let ddof = case
        .get("args")
        .and_then(|a| a.get("ddof"))
        .map(|d| scalar(d) as usize)
        .unwrap_or(0);

    let worst = match (op.as_str(), dtype_of(case).as_str()) {
        ("exp", "f64" | "f32") | ("ln", "f64") | ("log2", "f64") | ("log10", "f64") => {
            if dtype_of(case) == "f32" {
                let t = Tensor::<f32>::from_vec(
                    shape.clone(),
                    input.iter().map(|&v| v as f32).collect(),
                )
                .expect("input shape");
                let got = match op.as_str() {
                    "exp" => t.exp(),
                    "ln" => t.ln(),
                    "log2" => t.log2(),
                    "log10" => t.log10(),
                    _ => unreachable!(),
                };
                check_shape(&name, got.shape(), expect_shape, &shape);
                u64::from(assert_ulp32(
                    &name,
                    got.as_slice().expect("contiguous"),
                    &want,
                    budget_f32(&op),
                ))
            } else {
                let t = Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
                let got = match op.as_str() {
                    "exp" => t.exp(),
                    "ln" => t.ln(),
                    "log2" => t.log2(),
                    "log10" => t.log10(),
                    _ => unreachable!(),
                };
                check_shape(&name, got.shape(), expect_shape, &shape);
                assert_ulp(
                    &name,
                    got.as_slice().expect("contiguous"),
                    &want,
                    budget_f64(&op),
                )
            }
        }
        ("sqrt", _)
        | ("sin", _)
        | ("cos", _)
        | ("tan", _)
        | ("asin", _)
        | ("acos", _)
        | ("atan", _) => {
            if dtype_of(case) == "f32" {
                let t = Tensor::<f32>::from_vec(
                    shape.clone(),
                    input.iter().map(|&v| v as f32).collect(),
                )
                .expect("input shape");
                let got = match op.as_str() {
                    "sqrt" => t.sqrt(),
                    "sin" => t.sin(),
                    "cos" => t.cos(),
                    "tan" => t.tan(),
                    "asin" => t.asin(),
                    "acos" => t.acos(),
                    "atan" => t.atan(),
                    _ => unreachable!(),
                };
                check_shape(&name, got.shape(), expect_shape, &shape);
                u64::from(assert_ulp32(
                    &name,
                    got.as_slice().expect("contiguous"),
                    &want,
                    budget_f32(&op),
                ))
            } else {
                let t = Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
                let got = match op.as_str() {
                    "sqrt" => t.sqrt(),
                    "sin" => t.sin(),
                    "cos" => t.cos(),
                    "tan" => t.tan(),
                    "asin" => t.asin(),
                    "acos" => t.acos(),
                    "atan" => t.atan(),
                    _ => unreachable!(),
                };
                check_shape(&name, got.shape(), expect_shape, &shape);
                assert_ulp(
                    &name,
                    got.as_slice().expect("contiguous"),
                    &want,
                    budget_f64(&op),
                )
            }
        }
        ("pow", _) | ("atan2", _) => {
            let other_shape = usizes(required(case, "other_shape"));
            let other = numbers(required(case, "other"));
            let a = Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
            let b = Tensor::<f64>::from_vec(other_shape, other).expect("other shape");
            let got = if op == "pow" { a.pow(&b) } else { a.atan2(&b) }.expect("broadcastable");
            check_shape(&name, got.shape(), expect_shape, &shape);
            assert_ulp(
                &name,
                got.as_slice().expect("contiguous"),
                &want,
                budget_f64(&op),
            )
        }
        ("var", _) | ("std", _) | ("mean", _) => {
            if dtype_of(case) == "f16" {
                let t = Tensor::<F16>::from_vec(
                    shape.clone(),
                    input.iter().map(|&v| F16::from_f64(v)).collect(),
                )
                .expect("input shape");
                let got = match op.as_str() {
                    "var" => t.var(ax, ddof),
                    "std" => t.std(ax, ddof),
                    "mean" => t.mean(ax),
                    _ => unreachable!(),
                }
                .expect("moment");
                check_shape(&name, got.shape(), expect_shape, &shape);
                let got_bits: Vec<u16> = got
                    .as_slice()
                    .expect("contiguous")
                    .iter()
                    .map(|v| v.to_bits())
                    .collect();
                let want_bits: Vec<u16> =
                    want.iter().map(|&w| F16::from_f64(w).to_bits()).collect();
                assert_eq!(
                    got_bits.len(),
                    want_bits.len(),
                    "case {name}: element count"
                );
                let mut worst = 0u32;
                for (i, (g, w)) in got_bits.iter().zip(want_bits.iter()).enumerate() {
                    let d = ulp16(*g, *w);
                    if d > 1 {
                        panic!(
                            "case {name}: element {i}: got bits {g:#06x}, numpy {w:#06x}, {d} ULP"
                        );
                    }
                    worst = worst.max(d);
                }
                u64::from(worst)
            } else {
                let t = Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
                let got = match op.as_str() {
                    "var" => t.var(ax, ddof),
                    "std" => t.std(ax, ddof),
                    "mean" => t.mean(ax),
                    _ => unreachable!(),
                }
                .expect("moment");
                check_shape(&name, got.shape(), expect_shape, &shape);
                assert_ulp(
                    &name,
                    got.as_slice().expect("contiguous"),
                    &want,
                    budget_f64(&op),
                )
            }
        }
        ("dot", _) => {
            let other_shape = usizes(required(case, "other_shape"));
            let other = numbers(required(case, "other"));
            let a = Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
            let b = Tensor::<f64>::from_vec(other_shape.clone(), other).expect("other shape");
            let got = a.dot(&b).expect("dot");
            let expect = match (shape.len(), other_shape.len()) {
                (1, 1) => vec![],
                (2, 2) => vec![shape[0], other_shape[1]],
                (2, 1) => vec![shape[0]],
                (1, 2) => vec![other_shape[1]],
                _ => unreachable!(),
            };
            assert_eq!(got.shape(), expect.as_slice(), "case {name}: shape");
            assert_ulp(
                &name,
                got.as_slice().expect("contiguous"),
                &want,
                budget_f64(&op),
            )
        }
        ("norm", _) | ("norm_fro", _) | ("norm_p1", _) | ("norm_pinf", _) => {
            let t = Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
            let got = match op.as_str() {
                "norm" | "norm_fro" => t.norm(),
                "norm_p1" => t.norm_p(NormOrd::One),
                "norm_pinf" => t.norm_p(NormOrd::Infinity),
                _ => unreachable!(),
            }
            .expect("norm");
            assert_eq!(want.len(), 1, "case {name}: a norm is one value");
            assert_ulp(&name, &[got], &want, budget_f64(&op))
        }
        ("inv", _) => {
            let t = Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
            let got = t.inv().expect("inverse");
            check_shape(&name, got.shape(), expect_shape, &shape);
            assert_ulp(
                &name,
                got.as_slice().expect("contiguous"),
                &want,
                budget_f64(&op),
            )
        }
        ("det", _) => {
            let t = Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
            let got = t.det().expect("determinant");
            assert_ulp(&name, &[got], &want, budget_f64(&op))
        }
        ("solve", _) => {
            let other_shape = usizes(required(case, "other_shape"));
            let other = numbers(required(case, "other"));
            let a = Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
            let b = Tensor::<f64>::from_vec(other_shape.clone(), other).expect("other shape");
            let got = a.solve(&b).expect("solve");
            let expect = if other_shape.len() == 1 {
                vec![shape[0]]
            } else {
                other_shape
            };
            assert_eq!(got.shape(), expect.as_slice(), "case {name}: shape");
            assert_ulp(
                &name,
                got.as_slice().expect("contiguous"),
                &want,
                budget_f64(&op),
            )
        }
        ("lu", _) => {
            let n = shape[0];
            let a = Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
            let lu = a.lu().expect("factorization");
            let l = lu.l.as_slice().expect("contiguous");
            let u = lu.u.as_slice().expect("contiguous");
            // P * A, rebuilt from the row permutation the factorization reports.
            let mut pa = Vec::with_capacity(n * n);
            for i in 0..n {
                for j in 0..n {
                    pa.push(input[lu.perm[i] * n + j]);
                }
            }
            let mut worst = assert_ulp(&name, &pa, &want, 0);
            // P * A == L * U, recomputed from the factors.
            for i in 0..n {
                for j in 0..n {
                    let mut acc = 0.0f64;
                    for k in 0..n {
                        acc += l[i * n + k] * u[k * n + j];
                    }
                    let err = error_f64(acc, pa[i * n + j]);
                    assert!(
                        err <= 512.0,
                        "case {name}: L*U at [{i},{j}] is {acc}, P*A is {}, error {err:.3} epsilon",
                        pa[i * n + j]
                    );
                    worst = worst.max(rounds(err));
                }
            }
            // The factors reproduce the determinant of the input.
            let mut det_lu = 1.0f64;
            for i in 0..n {
                det_lu *= u[i * n + i];
            }
            let inversions = (0..n)
                .map(|i| ((i + 1)..n).filter(|&j| lu.perm[i] > lu.perm[j]).count())
                .sum::<usize>();
            if inversions % 2 == 1 {
                det_lu = -det_lu;
            }
            let det = a.det().expect("determinant");
            let det_err = error_f64(det_lu, det);
            assert!(
                det_err <= 512.0,
                "case {name}: det from factors {det_lu} vs det {det}, error {det_err:.3} epsilon"
            );
            worst.max(rounds(det_err))
        }
        ("concat", _) | ("stack", _) => {
            let other_shape = usizes(required(case, "other_shape"));
            let other = numbers(required(case, "other"));
            let axis = case
                .get("args")
                .and_then(|a| a.get("axis"))
                .map(|a| scalar(a) as usize)
                .expect("axis");
            let a = Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
            let b = Tensor::<f64>::from_vec(other_shape, other).expect("other shape");
            let got = if op == "concat" {
                concat(&[a, b], axis)
            } else {
                stack(&[a, b], axis)
            }
            .expect("shape combination");
            check_shape(&name, got.shape(), expect_shape, &shape);
            assert_eq!(
                got.as_slice().expect("contiguous"),
                want.as_slice(),
                "case {name}: values"
            );
            0
        }
        ("pad", _) => {
            let value = case
                .get("args")
                .and_then(|a| a.get("value"))
                .map(scalar)
                .expect("pad value");
            let pads_json = required(case, "args").get("pads").expect("pads");
            let pads: Vec<(usize, usize)> = pads_json
                .items()
                .iter()
                .map(|p| {
                    let pair = usizes(p);
                    (pair[0], pair[1])
                })
                .collect();
            let t = Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
            let got = t.pad(&pads, value).expect("pad");
            check_shape(&name, got.shape(), expect_shape, &shape);
            assert_eq!(
                got.as_slice().expect("contiguous"),
                want.as_slice(),
                "case {name}: values"
            );
            0
        }
        ("astype", _) => {
            let target = match case.get("target") {
                Some(Json::Str(s)) => s.clone(),
                _ => panic!("case {name}: astype without a target"),
            };
            match (dtype_of(case).as_str(), target.as_str()) {
                ("f64", "i32") => {
                    let t =
                        Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
                    let got: Vec<i32> = t.astype::<i32>().as_slice().expect("contiguous").to_vec();
                    let want_ints: Vec<i32> = want.iter().map(|&v| v as i32).collect();
                    assert_eq!(got, want_ints, "case {name}: values");
                }
                ("f64", "i8") => {
                    let t =
                        Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
                    let got: Vec<i8> = t.astype::<i8>().as_slice().expect("contiguous").to_vec();
                    let want_ints: Vec<i8> = want.iter().map(|&v| v as i8).collect();
                    assert_eq!(got, want_ints, "case {name}: values");
                }
                ("i32", "f64") => {
                    let ints: Vec<i32> = input.iter().map(|&v| v as i32).collect();
                    let t = Tensor::<i32>::from_vec(shape.clone(), ints).expect("input shape");
                    let got: Vec<f64> = t.astype::<f64>().as_slice().expect("contiguous").to_vec();
                    assert_eq!(got, want, "case {name}: values");
                }
                ("u8", "f16") => {
                    let ints: Vec<u8> = input.iter().map(|&v| v as u8).collect();
                    let t = Tensor::<u8>::from_vec(shape.clone(), ints).expect("input shape");
                    let got: Vec<u16> = t
                        .astype::<F16>()
                        .as_slice()
                        .expect("contiguous")
                        .iter()
                        .map(|v| v.to_bits())
                        .collect();
                    let want_bits: Vec<u16> = want.iter().map(|&v| v as u16).collect();
                    assert_eq!(got, want_bits, "case {name}: float16 bit patterns");
                }
                ("f64", "bool") => {
                    let t =
                        Tensor::<f64>::from_vec(shape.clone(), input.clone()).expect("input shape");
                    let got: Vec<bool> =
                        t.astype::<bool>().as_slice().expect("contiguous").to_vec();
                    let want_bools: Vec<bool> = want.iter().map(|&v| v != 0.0).collect();
                    assert_eq!(got, want_bools, "case {name}: values");
                }
                (from, to) => panic!("case {name}: unhandled astype {from} -> {to}"),
            }
            0
        }
        ("f16_bits", _) => {
            let got: Vec<u16> = input.iter().map(|&v| F16::from_f64(v).to_bits()).collect();
            let want_bits: Vec<u16> = want.iter().map(|&v| v as u16).collect();
            let mut worst = 0u32;
            for (i, (g, w)) in got.iter().zip(want_bits.iter()).enumerate() {
                assert_eq!(g, w, "case {name}: element {i} bits");
                worst = worst.max(ulp16(*g, *w));
            }
            u64::from(worst)
        }
        ("bf16_bits", _) => {
            let got: Vec<u16> = input.iter().map(|&v| Bf16::from_f64(v).to_bits()).collect();
            let want_bits: Vec<u16> = want.iter().map(|&v| v as u16).collect();
            assert_eq!(got, want_bits, "case {name}: bfloat16 bit patterns");
            0
        }
        (unknown, _) => panic!("case {name}: unhandled op {unknown}"),
    };
    (op, worst)
}

#[test]
fn numpy_vectors_match() {
    let doc = json::parse(include_str!("oracle/vectors.json"));
    let cases = doc.get("cases").expect("cases array").items();
    assert!(cases.len() >= 50, "the fixture lost cases");

    let mut worst_by_op: BTreeMap<String, u64> = BTreeMap::new();
    for case in cases {
        let (op, worst) = run_case(case);
        let slot = worst_by_op.entry(op).or_insert(0);
        *slot = (*slot).max(worst);
    }

    println!(
        "numpy oracle, worst observed error per operation \
         (units of EPSILON * max(|expected|, 1)):"
    );
    for (op, worst) in &worst_by_op {
        println!("  {op:<10} {worst}");
    }
    let numpy = doc.get("numpy").and_then(|v| match v {
        Json::Str(s) => Some(s.clone()),
        _ => None,
    });
    println!("oracle generated with numpy {}", numpy.unwrap_or_default());
}
