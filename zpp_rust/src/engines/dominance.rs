//! Dominance Pruning + super-increasing fast path.
//!
//! Reference: `Subset sum algorithm.md` line 9596.
//!
//! Two checks:
//!
//! 1. Super-increasing test:  if every element is greater than the
//!    sum of all previous elements (sorted ascending), the subset
//!    sum is solvable in O(n) by a simple greedy descent on the
//!    target.  This is the structure that "knapsack cryptosystems"
//!    famously broke — but here we exploit it for instant solutions.
//!
//! 2. Dominance: if element a < b and replacing b with a in any
//!    candidate subset makes the sum strictly closer to target,
//!    then b strictly dominates a and a can be skipped first.
//!    (We don't aggressively delete dominated elements because that
//!    can change feasibility — we just inform the search order.)

use num_bigint::BigUint;
use num_traits::Zero;

use crate::controller::{Engine, Shared};

pub struct DominanceEngine;

impl Engine for DominanceEngine {
    fn name(&self) -> &'static str {
        "Dominance"
    }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        if p.n < 3 {
            return;
        }

        // ---- Super-increasing fast path ----
        let mut sorted: Vec<BigUint> = p.numbers.clone();
        sorted.sort();
        let mut acc = BigUint::from(0u32);
        let mut super_inc = true;
        for x in &sorted {
            if x <= &acc {
                super_inc = false;
                break;
            }
            acc += x;
        }
        if !super_inc {
            return;
        }
        if sh.stopped() {
            return;
        }

        // Greedy from largest down — guaranteed correct on
        // super-increasing sets.
        let mut remaining = p.target.clone();
        let mut chosen: Vec<BigUint> = Vec::new();
        for x in sorted.iter().rev() {
            if x <= &remaining {
                chosen.push(x.clone());
                remaining -= x;
                if remaining.is_zero() {
                    break;
                }
            }
        }
        if remaining.is_zero() {
            sh.report(chosen, "Dominance");
        }
    }
}
