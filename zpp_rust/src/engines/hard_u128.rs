//! Hard fast path — Schroeppel–Shamir with adaptive parallel partitioning.
//!
//! For n ∈ [44, 80], runs the optimal 4-way meet-in-the-middle
//! (O(2^(n/4)) space) before other engines can waste CPU on
//! exponential heuristics.  Uses u128 fast path when values fit,
//! BigUint arbitrary-precision path otherwise (no bit-size limit).

use num_bigint::BigUint;

use crate::controller::{Engine, Shared};
use crate::knapsack::{schroeppel_shamir_big, schroeppel_shamir_u128};

pub struct HardU128Engine;

const MIN_N: usize = 44;
const MAX_N: usize = 80;

impl Engine for HardU128Engine {
    fn name(&self) -> &'static str {
        "Hard-U128"
    }

    fn run(&self, sh: &Shared) {
        if sh.stopped() {
            return;
        }
        let p = &sh.profile;
        if p.n < MIN_N || p.n > MAX_N {
            return;
        }

        let n = p.n;
        let q = n / 4;
        if q == 0 || q > 20 {
            return;
        }

        if p.u128_safe() {
            self.run_u128(sh);
        } else {
            self.run_biguint(sh);
        }
    }
}

impl HardU128Engine {
    fn run_u128(&self, sh: &Shared) {
        let p = &sh.profile;
        let target = p.target_u128();
        let nums = p.numbers_u128();
        let q = p.n / 4;

        let qa = &nums[0..q];
        let qb = &nums[q..2 * q];
        let qc = &nums[2 * q..3 * q];
        let qd = &nums[3 * q..];

        if sh.stopped() {
            return;
        }

        if let Some(sol) = schroeppel_shamir_u128(qa, qb, qc, qd, target) {
            let big: Vec<BigUint> = sol.into_iter().map(BigUint::from).collect();
            sh.report(big, "Hard-U128");
        }
    }

    fn run_biguint(&self, sh: &Shared) {
        let p = &sh.profile;
        let target = &p.target;
        let nums = &p.numbers;
        let q = p.n / 4;

        let qa = &nums[0..q];
        let qb = &nums[q..2 * q];
        let qc = &nums[2 * q..3 * q];
        let qd = &nums[3 * q..];

        if sh.stopped() {
            return;
        }

        if let Some(sol) = schroeppel_shamir_big(qa, qb, qc, qd, target) {
            sh.report(sol, "Hard-U128");
        }
    }
}
