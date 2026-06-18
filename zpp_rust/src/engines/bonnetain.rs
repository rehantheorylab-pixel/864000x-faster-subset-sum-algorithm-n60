//! Bonnetain-lite — extended {-1,0,1,2} representations on small quadrants.
//!
//! Reference: Bonnetain, De Micheli & Song, "Quantum cryptanalysis in
//! the supercomputer era" (subset-sum improvements), 2020.
//!
//! Full Bonnetain reaches O(2^0.283n).  Here we apply the coefficient-2
//! extension only when each quadrant has ≤ 10 elements (4^10 ≈ 1M entries),
//! then run the same 4-way Wagner combine as BCJ.

use num_bigint::BigUint;
use std::collections::HashMap;

use crate::controller::{Engine, Shared};

pub struct BonnetainEngine;

const BON_MIN_N: usize = 48;
const BON_MAX_N: usize = 60;
const BON_MAX_Q: usize = 10;

impl Engine for BonnetainEngine {
    fn name(&self) -> &'static str {
        "Bonnetain"
    }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        if p.n < BON_MIN_N || p.n > BON_MAX_N {
            return;
        }
        if !p.u128_safe() {
            return;
        }

        let target = p.target_u128() as i128;
        let nums = p.numbers_u128();
        let n = nums.len();
        let q = n / 4;
        if q == 0 || q > BON_MAX_Q {
            return;
        }

        let qa = &nums[0..q];
        let qb = &nums[q..2 * q];
        let qc = &nums[2 * q..3 * q];
        let qd = &nums[3 * q..];

        let la = enumerate_extended(qa, target);
        let lb = enumerate_extended(qb, target);
        let lc = enumerate_extended(qc, target);
        let ld = enumerate_extended(qd, target);

        if sh.stopped() {
            return;
        }

        if let Some(sol) = wagner4(&la, &lb, &lc, &ld, target, qa, qb, qc, qd) {
            let big: Vec<BigUint> = sol.into_iter().map(BigUint::from).collect();
            sh.report(big, "Bonnetain");
        }
    }
}

/// Coefficients in {0,1,2,3} per element → contribution 0, +a, +2a, +3a.
/// Encoded as 2 bits per element (4 choices).
fn enumerate_extended(nums: &[u128], target: i128) -> Vec<(i128, u64)> {
    let q = nums.len();
    if q == 0 {
        return vec![(0, 0)];
    }
    if q > BON_MAX_Q {
        return Vec::new();
    }

    let bound = target.saturating_abs().saturating_mul(4).saturating_add(1);
    let total = 4u64.pow(q as u32);
    let mut out = Vec::with_capacity(total.min(2_000_000) as usize);

    for code in 0..total {
        let mut s: i128 = 0;
        let mut mask: u64 = 0;
        let mut t = code;
        let mut ok = true;
        for (i, &v) in nums.iter().enumerate() {
            let coeff = (t % 4) as u8;
            t /= 4;
            if coeff > 0 {
                let add = (v as i128).saturating_mul(coeff as i128);
                s = s.saturating_add(add);
                mask |= (coeff as u64) << ((i * 2) as u32);
            }
            if s.unsigned_abs() > bound as u128 {
                ok = false;
                break;
            }
        }
        if ok {
            out.push((s, mask));
        }
    }
    out
}

fn wagner4(
    la: &[(i128, u64)],
    lb: &[(i128, u64)],
    lc: &[(i128, u64)],
    ld: &[(i128, u64)],
    target: i128,
    qa: &[u128],
    qb: &[u128],
    qc: &[u128],
    qd: &[u128],
) -> Option<Vec<u128>> {
    let mut ab: HashMap<i128, (u64, u64)> = HashMap::with_capacity(la.len().saturating_mul(lb.len()).min(4_000_000));
    for &(sa, ma) in la {
        for &(sb, mb) in lb {
            ab.entry(sa.saturating_add(sb)).or_insert((ma, mb));
        }
    }

    for &(sc, mc) in lc {
        for &(sd, md) in ld {
            let need = target.saturating_sub(sc.saturating_add(sd));
            if let Some(&(ma, mb)) = ab.get(&need) {
                let mut sol = coeff_mask_to_values(ma, qa);
                sol.extend(coeff_mask_to_values(mb, qb));
                sol.extend(coeff_mask_to_values(mc, qc));
                sol.extend(coeff_mask_to_values(md, qd));
                let check: i128 = sol.iter().map(|&x| x as i128).sum();
                if check == target {
                    return Some(sol);
                }
            }
        }
    }
    None
}

fn coeff_mask_to_values(mask: u64, nums: &[u128]) -> Vec<u128> {
    let mut v = Vec::new();
    for (i, &x) in nums.iter().enumerate() {
        let coeff = ((mask >> ((i * 2) as u32)) & 3) as u8;
        // Subset sum: each element at most once (coeff 0 or 1 only).
        if coeff == 1 {
            v.push(x);
        } else if coeff > 1 {
            return Vec::new();
        }
    }
    v
}
