//! BCJ — Becker–Coron–Joux representation technique (multi-round).
//!
//! Reference: Becker, Coron & Joux, "Improved Generic Algorithms for
//! Hard Knapsacks", EUROCRYPT 2011.  Achieves O(2^0.291n) by using
//! {-1, 0, 1} signed subset sums so each value has many more
//! representations than HGJ's unsigned halves.
//!
//! Multi-round extension: each round uses a different modulus.
//! Early rounds (large modulus) give fine-grained bucket resolution;
//! later rounds (small modulus) keep more representations per bucket.
//!
//! FIXED verification: properly checks P - N == target instead of
//! incorrectly assuming N == 0.  Bucket filter prefers representations
//! with fewer -1 elements (then closer to zero sum as tiebreaker).

use num_bigint::BigUint;
use std::collections::HashMap;

use crate::controller::{Engine, Shared};
use crate::knapsack::{
    signed_buckets_mod_bcj, signed_mask_negative_only, signed_mask_positive_only,
    signed_mask_neg_count, SignedEntry,
};

pub struct BcjEngine;

const BCJ_MIN_N: usize = 40;
const BCJ_MAX_N: usize = 80;
const BUCK_BITS: u32 = 13;
const MAX_Q: usize = 20;

impl Engine for BcjEngine {
    fn name(&self) -> &'static str {
        "BCJ"
    }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        if p.n < BCJ_MIN_N || p.n > BCJ_MAX_N {
            return;
        }
        if !p.u128_safe() {
            return;
        }

        let target = p.target_u128() as i128;
        let nums = p.numbers_u128();
        let n = nums.len();
        let q = n / 4;
        if q == 0 || q > MAX_Q {
            return;
        }

        let qa = &nums[0..q];
        let qb = &nums[q..2 * q];
        let qc = &nums[2 * q..3 * q];
        let qd = &nums[3 * q..];

        // More rounds for harder instances
        let rounds = if n >= 66 { 6 } else if n >= 55 { 4 } else { 2 };

        for round in 0..rounds {
            if sh.stopped() {
                return;
            }
            // Vary modulus each round: 8192, 4096, 2048, 1024, 512, 256
            let shift = round * 1;
            let bb = if BUCK_BITS > shift { BUCK_BITS - shift } else { 4.max(1) };
            let modulus: u128 = 1u128 << bb;
            let mod_mask = modulus - 1;

            if sh.stopped() {
                return;
            }
            let la = signed_buckets_mod_bcj(qa, target, modulus);
            let lb = signed_buckets_mod_bcj(qb, target, modulus);
            let lc = signed_buckets_mod_bcj(qc, target, modulus);
            let ld = signed_buckets_mod_bcj(qd, target, modulus);
            if la.is_empty() || lb.is_empty() || lc.is_empty() || ld.is_empty() {
                continue;
            }
            if sh.stopped() {
                return;
            }

            let ab = merge_level1(&la, &lb, modulus, mod_mask, target);
            let cd = merge_level1(&lc, &ld, modulus, mod_mask, target);
            if ab.is_empty() || cd.is_empty() {
                continue;
            }
            if sh.stopped() {
                return;
            }

            if let Some(sol) = meet_level2(&ab, &cd, target, qa, qb, qc, qd) {
                let big: Vec<BigUint> = sol.into_iter().map(BigUint::from).collect();
                sh.report(big, "BCJ");
                return;
            }
        }
    }
}

/// Preferred bucket entry: (signed_sum, mask_a, mask_b, neg_count_a, neg_count_b)
type BucketEntry = (i128, u64, u64, u32, u32);

fn merge_level1(
    a: &HashMap<u128, SignedEntry>,
    b: &HashMap<u128, SignedEntry>,
    modulus: u128,
    mod_mask: u128,
    target: i128,
) -> HashMap<u128, BucketEntry> {
    let bound = target.saturating_abs().saturating_mul(2).saturating_add(1);
    let mut out: HashMap<u128, BucketEntry> = HashMap::new();
    for ea in a.values() {
        for eb in b.values() {
            let s = ea.sum.saturating_add(eb.sum);
            if s.unsigned_abs() > bound as u128 {
                continue;
            }
            let key = (s as i128).rem_euclid(modulus as i128) as u128 & mod_mask;
            let na = signed_mask_neg_count(ea.mask);
            let nb = signed_mask_neg_count(eb.mask);
            let total_neg = na + nb;
            out.entry(key)
                .and_modify(|cur: &mut BucketEntry| {
                    // Fewer -1 elements is better; tiebreak by smaller abs sum
                    let cur_neg = cur.3 + cur.4;
                    if total_neg < cur_neg || (total_neg == cur_neg && s.unsigned_abs() < cur.0.unsigned_abs()) {
                        *cur = (s, ea.mask, eb.mask, na, nb);
                    }
                })
                .or_insert((s, ea.mask, eb.mask, na, nb));
        }
    }
    out
}

fn meet_level2(
    ab: &HashMap<u128, BucketEntry>,
    cd: &HashMap<u128, BucketEntry>,
    target: i128,
    qa: &[u128],
    qb: &[u128],
    qc: &[u128],
    qd: &[u128],
) -> Option<Vec<u128>> {
    for &(sc, mc, md, _, _) in cd.values() {
        for &(sab, ma, mb, _, _) in ab.values() {
            if sab.saturating_add(sc) != target {
                continue;
            }
            let mut sol = signed_mask_positive_only(ma, qa);
            sol.extend(signed_mask_positive_only(mb, qb));
            sol.extend(signed_mask_positive_only(mc, qc));
            sol.extend(signed_mask_positive_only(md, qd));
            if sol.is_empty() {
                continue;
            }
            // Proper verification: P - N must equal target
            let pos_sum: u128 = sol.iter().sum();
            let neg_sum: u128 = signed_mask_negative_only(ma, qa)
                .into_iter()
                .chain(signed_mask_negative_only(mb, qb))
                .chain(signed_mask_negative_only(mc, qc))
                .chain(signed_mask_negative_only(md, qd))
                .sum();
            if pos_sum > neg_sum && pos_sum - neg_sum == target as u128 {
                return Some(sol);
            }
        }
    }
    None
}
