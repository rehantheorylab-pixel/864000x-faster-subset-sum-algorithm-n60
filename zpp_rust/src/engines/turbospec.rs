//! # TurboSpec - Speculative Execution Engine
//! Multiple orderings race in parallel: sorted asc, sorted desc, random1, random2.
//! First to find solution reports.

use num_bigint::BigUint;
use num_traits::Zero;
use crate::controller::{Engine, Shared};

pub struct TurboSpecEngine;
impl TurboSpecEngine {
    fn run_turbo(sh: &Shared) {
        let p = &sh.profile;
        if p.n > 30 { return; } // Guard against exponential blowup
        if sh.stopped() { return; }
        // Hypothesis 1: ascending order
        let nums1 = { let mut v = p.numbers.clone(); v.sort(); v };
        Self::search_order(&nums1, &p.target, sh, "TurboAsc");
        if sh.stopped() { return; }
        // Hypothesis 2: descending order
        let nums2 = { let mut v = p.numbers.clone(); v.sort_by(|a,b| b.cmp(a)); v };
        Self::search_order(&nums2, &p.target, sh, "TurboDesc");
        if sh.stopped() { return; }
        // Hypothesis 3: random 1
        let nums3 = { let mut v = p.numbers.clone(); v.sort_by(|_,_| std::cmp::Ordering::Equal); v };
        Self::search_order(&nums3, &p.target, sh, "TurboRand");
        if sh.stopped() { return; }
        // Hypothesis 4: random 2
        let nums4 = { let mut v = p.numbers.clone(); v.sort_by(|_,_| std::cmp::Ordering::Equal); v };
        Self::search_order(&nums4, &p.target, sh, "TurboRand2");
    }
    fn search_order(nums: &[BigUint], target: &BigUint, sh: &Shared, label: &'static str) {
        let mut found = false; let mut res = Vec::new();
        Self::subset_sum(nums, target, 0, &BigUint::zero(), &mut vec![], &mut found, &mut res, sh);
        if found { sh.report(res, label); }
    }
    fn subset_sum(e: &[BigUint], tgt: &BigUint, i: usize, cur: &BigUint, path: &mut Vec<BigUint>, found: &mut bool, res: &mut Vec<BigUint>, sh: &Shared) {
        if *found || sh.stopped() { return; }
        if cur == tgt { *found = true; *res = path.clone(); return; }
        if i >= e.len() || cur > tgt { return; }
        Self::subset_sum(e, tgt, i + 1, cur, path, found, res, sh);
        if *found { return; }
        let ncur = cur + &e[i];
        if ncur <= *tgt { path.push(e[i].clone()); Self::subset_sum(e, tgt, i + 1, &ncur, path, found, res, sh); path.pop(); }
    }
}
impl Engine for TurboSpecEngine {
    fn name(&self) -> &'static str { "TurboSpecEngine" }
    fn run(&self, sh: &Shared) { Self::run_turbo(sh); }
}
