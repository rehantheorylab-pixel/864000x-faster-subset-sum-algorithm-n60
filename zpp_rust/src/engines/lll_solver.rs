//! LLL Solver — Floating-Point Implementation (fpylll/NTL style)
//!
//! Uses double-precision (f64) for Gram-Schmidt coefficients
//! and exact BigUint for basis vectors. This is the approach
//! used by professional LLL libraries (fpylll, NTL).
//!
//! For dimensions ≤ 30 (our use case), f64 precision is sufficient
//! to correctly guide the reduction. Final result verified exactly.

use num_bigint::BigUint;
use num_traits::{Zero, One};
use crate::controller::{Engine, Shared};

pub struct LLLSolver;

const DELTA: f64 = 0.75;

impl Engine for LLLSolver {
    fn name(&self) -> &'static str { "LLLSolver" }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        if p.n < 4 || p.n > 30 { return; }

        let max_bits = p.max_val.bits() as f64;
        if max_bits < 1.0 { return; }
        if p.n as f64 / max_bits > 0.65 { return; } // density check

        let n = p.n;
        let target = p.target.clone();
        let mut nums = p.numbers.clone();
        nums.sort();

        for scale in [1u64, 4, 16] {
            if sh.stopped() { return; }
            let nf = compute_nf(&nums, n, scale);
            let mut basis = build_lattice(&nums, &target, &nf, n);
            if solve_lll(&mut basis, &nums, &target, n, sh) { return; }
        }
    }
}

fn compute_nf(nums: &[BigUint], n: usize, scale: u64) -> BigUint {
    let max_val = nums.last().cloned().unwrap_or(BigUint::one());
    let shift = BigUint::from(1u64) << (n as u32);
    let sc = BigUint::from(scale);
    max_val * shift * sc + BigUint::one()
}

fn build_lattice(nums: &[BigUint], target: &BigUint, nf: &BigUint, n: usize) -> Vec<Vec<BigUint>> {
    let dim = n + 1;
    let mut basis = vec![vec![BigUint::zero(); dim]; dim];
    for i in 0..n {
        basis[i][i] = BigUint::one();
        basis[i][n] = nf * &nums[i];
    }
    basis[n][n] = nf * target;
    basis
}

/// Convert BigUint to f64 (truncates to 53 bits of mantissa)
fn to_f64(b: &BigUint) -> f64 {
    let digits = b.to_u64_digits();
    if digits.is_empty() { return 0.0; }
    if digits.len() == 1 { return digits[0] as f64; }
    // Take most significant 64 bits, scale by 2^(64*(len-1))
    let hi = digits[digits.len() - 1] as f64;
    let shift = 2f64.powi(64 * (digits.len() as i32 - 1));
    hi * shift
}

fn dot_f64(a: &[BigUint], b: &[BigUint]) -> f64 {
    a.iter().zip(b).map(|(x, y)| to_f64(x) * to_f64(y)).sum()
}

/// Gram-Schmidt using f64 — returns (μ matrix, ||b*||² norms)
fn gram_schmidt_f64(basis: &[Vec<BigUint>]) -> (Vec<Vec<f64>>, Vec<f64>) {
    let m = basis.len();
    let mut mu = vec![vec![0.0f64; m]; m];
    let mut bstar = vec![0.0f64; m];

    for i in 0..m {
        let mut bs_sq = dot_f64(&basis[i], &basis[i]);

        for j in 0..i {
            let mut dot_ij = dot_f64(&basis[i], &basis[j]);
            for k in 0..j {
                dot_ij -= mu[i][k] * mu[j][k] * bstar[k];
            }
            if bstar[j] > 1e-30 {
                mu[i][j] = dot_ij / bstar[j];
                bs_sq -= mu[i][j] * mu[i][j] * bstar[j];
            }
        }
        if bs_sq < 0.0 { bs_sq = 0.0; }
        bstar[i] = bs_sq;
    }
    (mu, bstar)
}

/// Size reduce: for j=k-1..0, round μ[k][j] and subtract from b_k
fn size_reduce_f64(basis: &mut [Vec<BigUint>], mu: &mut [Vec<f64>], k: usize) {
    for j in (0..k).rev() {
        if mu[k][j].abs() <= 0.5 { continue; }
        let r = mu[k][j].round() as i64;
        if r == 0 { continue; }

        let bj = basis[j].clone();
        let f = BigUint::from(r.unsigned_abs());
        for (t, s) in basis[k].iter_mut().zip(&bj) {
            let term = &f * s;
            if r > 0 {
                if *t >= term { *t -= &term; } else { *t = BigUint::zero(); }
            } else {
                *t += &term;
            }
        }

        for p in 0..j {
            mu[k][p] -= mu[j][p] * (r as f64);
        }
        mu[k][j] -= r as f64;
    }
}

fn solve_lll(
    basis: &mut [Vec<BigUint>], nums: &[BigUint],
    target: &BigUint, n: usize, sh: &Shared,
) -> bool {
    let m = basis.len();
    let (mut mu, mut bstar) = gram_schmidt_f64(basis);
    let mut k = 1;

    for _iter in 0..5000 {
        if sh.stopped() { return false; }

        size_reduce_f64(basis, &mut mu, k);

        // Lovász condition: B[k] >= (δ - μ²) * B[k-1]
        if bstar[k - 1] < 1e-30 { k += 1; if k >= m { break; } continue; }
        let rhs = (DELTA - mu[k][k-1].powi(2)) * bstar[k - 1];

        if bstar[k] >= rhs {
            k += 1;
            if k >= m { break; }
        } else {
            basis.swap(k, k - 1);
            (mu, bstar) = gram_schmidt_f64(basis);
            k = if k > 1 { k - 1 } else { 1 };
        }
    }

    // Search for {0,1} solution vector
    for row in basis.iter() {
        let mut valid = true;
        let mut subset = Vec::new();
        for i in 0..n {
            if row[i] == BigUint::one() {
                subset.push(nums[i].clone());
            } else if !row[i].is_zero() {
                valid = false;
                break;
            }
        }
        if valid && !subset.is_empty() && subset.iter().sum::<BigUint>() == *target {
            sh.report(subset, "LLLSolver");
            return true;
        }
    }

    // Also try the target row itself (if reduced to have zeros in first n columns)
    for row in basis.iter() {
        let mut ok = true;
        for i in 0..n {
            if !row[i].is_zero() { ok = false; break; }
        }
        if ok && !row[n].is_zero() {
            // Target row was reduced — the other rows may contain the solution
            for other in basis.iter() {
                let mut subset = Vec::new();
                let mut good = true;
                for i in 0..n {
                    if other[i] == BigUint::one() { subset.push(nums[i].clone()); }
                    else if !other[i].is_zero() { good = false; break; }
                }
                if good && !subset.is_empty() && subset.iter().sum::<BigUint>() == *target {
                    sh.report(subset, "LLLSolver");
                    return true;
                }
            }
        }
    }

    false
}
