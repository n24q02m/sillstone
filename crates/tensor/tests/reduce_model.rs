//! Model-based check of axis reductions: every reduction over every axis of
//! contiguous, transposed and sliced 3-D views must equal a naive fold over
//! the elements the view exposes through `get`.

use sillstone_tensor::Tensor;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn random_tensor(rng: &mut Rng, shape: &[usize]) -> Tensor<i64> {
    let n: usize = shape.iter().product();
    // A small value range makes ties common, which exercises tie-breaking.
    let data = (0..n).map(|_| (rng.next() % 11) as i64 - 5).collect();
    Tensor::from_vec(shape.to_vec(), data).unwrap()
}

/// Naive reduction of `t` along `axis`: for each coordinate of the remaining
/// axes (C order), fold the values along `axis` in index order.
fn naive(t: &Tensor<i64>, axis: usize) -> (Vec<usize>, Vec<[i64; 4]>) {
    // [sum, max, min, argmax-first]
    let shape = t.shape().to_vec();
    let rest: Vec<usize> = (0..shape.len()).filter(|&a| a != axis).collect();
    let out_shape: Vec<usize> = rest.iter().map(|&a| shape[a]).collect();
    let count: usize = out_shape.iter().product();
    let mut out = Vec::with_capacity(count);
    for flat in 0..count {
        let mut rem = flat;
        let mut idx = vec![0usize; shape.len()];
        for (k, &a) in rest.iter().enumerate().rev() {
            idx[a] = rem % out_shape[k];
            rem /= out_shape[k];
        }
        let mut acc = [0i64, i64::MIN, i64::MAX, 0];
        for i in 0..shape[axis] {
            idx[axis] = i;
            let v = t.get(&idx).unwrap();
            acc[0] += v;
            if v > acc[1] {
                acc[1] = v;
                acc[3] = i as i64;
            }
            acc[2] = acc[2].min(v);
        }
        out.push(acc);
    }
    (out_shape, out)
}

fn check(t: &Tensor<i64>, label: &str) {
    for axis in 0..t.ndim() {
        let (out_shape, expect) = naive(t, axis);
        let sum = t.sum(Some(axis)).unwrap();
        let max = t.max(Some(axis)).unwrap();
        let min = t.min(Some(axis)).unwrap();
        let arg = t.argmax(Some(axis)).unwrap();
        for r in [&sum, &max, &min] {
            assert_eq!(r.shape(), &out_shape[..], "{label} axis {axis} shape");
        }
        assert_eq!(arg.shape(), &out_shape[..], "{label} axis {axis} arg shape");
        let flat = |x: &Tensor<i64>| x.contiguous().as_slice().unwrap().to_vec();
        assert_eq!(flat(&sum), expect.iter().map(|e| e[0]).collect::<Vec<_>>());
        assert_eq!(flat(&max), expect.iter().map(|e| e[1]).collect::<Vec<_>>());
        assert_eq!(flat(&min), expect.iter().map(|e| e[2]).collect::<Vec<_>>());
        let arg_flat = arg.contiguous().as_slice().unwrap().to_vec();
        assert_eq!(
            arg_flat,
            expect.iter().map(|e| e[3] as usize).collect::<Vec<_>>(),
            "{label} axis {axis} argmax (first maximum wins)"
        );
    }
}

#[test]
fn axis_reductions_match_a_naive_model_on_views() {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let perms: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for round in 0..4 {
        let t = random_tensor(&mut rng, &[3, 4, 5]);
        check(&t, &format!("round {round} contiguous"));
        for p in perms {
            let v = t.transpose(Some(&p)).unwrap();
            check(&v, &format!("round {round} perm {p:?}"));
        }
        let sliced = t.slice(&[(1, 3), (0, 4), (2, 5)]).unwrap();
        check(&sliced, &format!("round {round} slice"));
        let sliced_t = sliced.transpose(Some(&[2, 0, 1])).unwrap();
        check(&sliced_t, &format!("round {round} slice+transpose"));
    }
}

#[test]
fn single_element_axes_and_unit_dimensions() {
    let mut rng = Rng(77);
    for shape in [[1usize, 1, 1], [1, 4, 1], [3, 1, 2], [1, 1, 6]] {
        check(&random_tensor(&mut rng, &shape), &format!("{shape:?}"));
    }
}

#[test]
fn float_reductions_follow_view_order() {
    let t = Tensor::from_vec(vec![2, 3], vec![0.5f64, -1.5, 2.0, 4.0, 0.25, -8.0]).unwrap();
    let tr = t.transpose(None).unwrap();
    assert_eq!(tr.sum(Some(0)).unwrap().as_slice(), Some(&[1.0, -3.75][..]));
    assert_eq!(
        tr.max(Some(1)).unwrap().as_slice(),
        Some(&[4.0, 0.25, 2.0][..])
    );
    assert_eq!(
        tr.min(Some(1)).unwrap().as_slice(),
        Some(&[0.5, -1.5, -8.0][..])
    );
    assert_eq!(tr.argmax(Some(1)).unwrap().as_slice(), Some(&[1, 1, 0][..]));
}
