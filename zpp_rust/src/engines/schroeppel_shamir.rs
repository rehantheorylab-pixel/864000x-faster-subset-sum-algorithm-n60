//! Schroeppel–Shamir 4-way subset-sum.
//!
//! Reference: Schroeppel & Shamir, "A T = O(2^(n/2)), S = O(2^(n/4))
//! Algorithm for Certain NP-Complete Problems", FOCS 1979.
//!
//! Algorithmic complexity:
//!   - Time : O(2^(n/2))
//!   - Space: O(2^(n/4))
//!
//! Compared to classical Horowitz-Sahni MITM (which uses O(2^(n/2))
//! space and is therefore impossible for n > 50 on a normal PC), this
//! engine uses O(2^(n/4)) space.  At n=60 that is 32K entries instead
//! of 1 billion; at n=70 it is 1M instead of 34 billion.
//!
//! How it works (4-way split):
//!   1. Sort and split the n elements into four quarters A, B, C, D
//!   2. Generate ALL 2^|X| subset sums for each quarter (small)
//!   3. Lazily enumerate (A+B) sums in ascending order via a min-heap
//!   4. Lazily enumerate (C+D) sums in descending order via a max-heap
//!   5. Two-pointer walk: if (a+b) + (c+d) < target, advance min-heap
//!      (raise the lower side); else advance max-heap (lower the
//!      upper side); on equality, reconstruct the bitmask and stop.
//!
//! When the input fits in u128 we run the native fast path with zero
//! big-integer allocations on the inner loop — the fully unrolled
//! u128 add and compare run at ~1-2 ns each.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::collections::HashSet;
use std::sync::Arc;
use std::thread;

use num_bigint::BigUint;

use crate::controller::{Engine, Shared};

pub struct SchroeppelShamirEngine;

const SS_MIN_N: usize = 16;
const SS_MAX_N: usize = 70;

impl Engine for SchroeppelShamirEngine {
    fn name(&self) -> &'static str {
        "Schroeppel-Shamir"
    }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        if p.n < SS_MIN_N || p.n > SS_MAX_N {
            return;
        }
        if sh.stopped() {
            return;
        }

        if p.u128_safe() {
            self.run_u128(sh);
        } else {
            self.run_biguint(sh);
        }
    }
}

/// Adaptive quarter split: place largest elements in D (so C+D sums stay
/// target-tight), smallest in A, balancing quarter sizes by value magnitude.
/// Returns (qa_size, qb_size, qc_size, qd_size).
fn adaptive_split(n: usize, nums: &[u128], target: u128) -> (usize, usize, usize, usize) {
    // Default balanced split.
    let q = n / 4;
    let rem = n - 3 * q;
    let (mut qa, mut qb, mut qc, mut qd) = (q, q, q, rem.max(1));

    // Adjust if any quarter is too large.
    if qa > 28 || qb > 28 || qc > 28 || qd > 28 {
        // Tighten by moving some large elements from C into D (reduce C size,
        // because CD max-heap benefits from fewer large-value combinations).
        let excess = (qa.max(qb).max(qc).max(qd)).saturating_sub(26);
        if qc > qd + excess {
            qc -= excess;
            qd += excess;
        } else if qb > qa + excess {
            qb -= excess;
            qa += excess;
        }
    }

    // Move very large elements into D so CD-side stays target-tight.
    // If target is small relative to big elements, most large elements can
    // only appear in D (since A+B must be small enough to leave room).
    let big_threshold = target / 4;
    let big_count = nums.iter().filter(|&&v| v > big_threshold).count();
    if big_count > q + qd && qd < 28 {
        // Give D more capacity for big elements.
        let transfer = (big_count.saturating_sub(q + qd)).min(28 - qd).min(qc.saturating_sub(4));
        qc = qc.saturating_sub(transfer.min(qc.saturating_sub(4)));
        qd += transfer;
    }

    (qa, qb, qc, qd)
}

impl SchroeppelShamirEngine {
    fn run_u128(&self, sh: &Shared) {
        let p = &sh.profile;
        let n = p.n;
        let target = p.target_u128();

        // Sort ascending: large elements go into D (CD-side max-heap).
        let mut nums = p.numbers_u128();
        nums.sort_unstable();

        let (qa_sz, qb_sz, qc_sz, qd_sz) = adaptive_split(n, &nums, target);
        if qd_sz > 31 {
            return;
        }

        let qa: Vec<u128> = nums[0..qa_sz].to_vec();
        let qb: Vec<u128> = nums[qa_sz..(qa_sz + qb_sz)].to_vec();
        let qc: Vec<u128> = nums[(qa_sz + qb_sz)..(qa_sz + qb_sz + qc_sz)].to_vec();
        let qd: Vec<u128> = nums[(qa_sz + qb_sz + qc_sz)..].to_vec();

        if sh.stopped() {
            return;
        }

        let a_sums = build_sums_u128_par(&qa, target);
        if sh.stopped() {
            return;
        }
        let b_sums = build_sums_u128_par(&qb, target);
        if sh.stopped() {
            return;
        }
        let c_sums = build_sums_u128_par(&qc, target);
        if sh.stopped() {
            return;
        }
        let d_sums = build_sums_u128_par(&qd, target);
        if sh.stopped() {
            return;
        }

        if a_sums.is_empty() || b_sums.is_empty() || c_sums.is_empty() || d_sums.is_empty() {
            return;
        }

        let mut min_heap: BinaryHeap<Reverse<(u128, u32, u32)>> =
            BinaryHeap::with_capacity(b_sums.len());
        for j in 0..b_sums.len() {
            let s = a_sums[0].0.wrapping_add(b_sums[j].0);
            if s <= target {
                min_heap.push(Reverse((s, 0, j as u32)));
            }
        }

        let last_c = c_sums.len() - 1;
        let mut max_heap: BinaryHeap<(u128, u32, u32)> =
            BinaryHeap::with_capacity(d_sums.len());
        for j in 0..d_sums.len() {
            let s = c_sums[last_c].0.wrapping_add(d_sums[j].0);
            max_heap.push((s, last_c as u32, j as u32));
        }

        let mut ops: u64 = 0;
        let mut maybe_match: Option<(u32, u32, u32, u32)> = None;
        let mut seen: HashSet<u128> = HashSet::with_capacity(1024);

        while !min_heap.is_empty() && !max_heap.is_empty() {
            ops += 1;
            if (ops & 0x3FF) == 0 && sh.stopped() {
                return;
            }

            let (ab, ai, bi) = match min_heap.peek() {
                Some(&Reverse(t)) => t,
                None => break,
            };
            let (cd, ci, di) = match max_heap.peek() {
                Some(&t) => t,
                None => break,
            };

            let total = match ab.checked_add(cd) {
                Some(t) => t,
                None => {
                    max_heap.pop();
                    continue;
                }
            };

            // Dedup seen sums locally (no BigUint conversion in hot loop)
            if !seen.insert(total) {
                // Already seen — advance both sides.
                min_heap.pop();
                max_heap.pop();
                let ai_us = ai as usize;
                if ai_us + 1 < a_sums.len() {
                    let ns = a_sums[ai_us + 1].0.wrapping_add(b_sums[bi as usize].0);
                    if ns <= target {
                        min_heap.push(Reverse((ns, (ai_us + 1) as u32, bi)));
                    }
                }
                let ci_us = ci as usize;
                if ci_us > 0 {
                    let ns = c_sums[ci_us - 1].0.saturating_add(d_sums[di as usize].0);
                    max_heap.push((ns, (ci_us - 1) as u32, di));
                }
                continue;
            }

            if total == target {
                maybe_match = Some((ai, bi, ci, di));
                break;
            } else if total < target {
                min_heap.pop();
                let ai_us = ai as usize;
                if ai_us + 1 < a_sums.len() {
                    let new_sum =
                        a_sums[ai_us + 1].0.wrapping_add(b_sums[bi as usize].0);
                    if new_sum <= target {
                        min_heap.push(Reverse((new_sum, (ai_us + 1) as u32, bi)));
                    }
                }
            } else {
                max_heap.pop();
                let ci_us = ci as usize;
                if ci_us > 0 {
                    let new_sum = c_sums[ci_us - 1].0.saturating_add(d_sums[di as usize].0);
                    max_heap.push((new_sum, (ci_us - 1) as u32, di));
                }
            }
        }

        if let Some((ai, bi, ci, di)) = maybe_match {
            let a_mask = a_sums[ai as usize].1;
            let b_mask = b_sums[bi as usize].1;
            let c_mask = c_sums[ci as usize].1;
            let d_mask = d_sums[di as usize].1;

            let mut sol: Vec<BigUint> = Vec::new();
            push_selected(&qa, a_mask, &mut sol);
            push_selected(&qb, b_mask, &mut sol);
            push_selected(&qc, c_mask, &mut sol);
            push_selected(&qd, d_mask, &mut sol);
            sh.report(sol, "Schroeppel-Shamir");
        }
    }

    fn run_biguint(&self, sh: &Shared) {
        let p = &sh.profile;
        let n = p.n;
        let target = p.target.clone();

        // Sort ascending: large elements go into D (CD-side max-heap).
        let mut nums = p.numbers.clone();
        nums.sort_unstable();

        let qa_sz = n / 4;
        let qb_sz = n / 4;
        let qc_sz = n / 4;

        let qa: Vec<BigUint> = nums[0..qa_sz].to_vec();
        let qb: Vec<BigUint> = nums[qa_sz..(qa_sz + qb_sz)].to_vec();
        let qc: Vec<BigUint> = nums[(qa_sz + qb_sz)..(qa_sz + qb_sz + qc_sz)].to_vec();
        let qd: Vec<BigUint> = nums[(qa_sz + qb_sz + qc_sz)..].to_vec();

        if qd.len() > 31 {
            return;
        }

        let a_sums = build_sums_big(&qa, &target);
        if sh.stopped() {
            return;
        }
        let b_sums = build_sums_big(&qb, &target);
        if sh.stopped() {
            return;
        }
        let c_sums = build_sums_big(&qc, &target);
        if sh.stopped() {
            return;
        }
        let d_sums = build_sums_big(&qd, &target);
        if sh.stopped() {
            return;
        }

        if a_sums.is_empty() || b_sums.is_empty() || c_sums.is_empty() || d_sums.is_empty() {
            return;
        }

        let mut min_heap: BinaryHeap<Reverse<(BigUint, u32, u32)>> = BinaryHeap::new();
        for j in 0..b_sums.len() {
            let s = &a_sums[0].0 + &b_sums[j].0;
            if s <= target {
                min_heap.push(Reverse((s, 0, j as u32)));
            }
        }

        let last_c = c_sums.len() - 1;
        let mut max_heap: BinaryHeap<(BigUint, u32, u32)> = BinaryHeap::new();
        for j in 0..d_sums.len() {
            let s = &c_sums[last_c].0 + &d_sums[j].0;
            max_heap.push((s, last_c as u32, j as u32));
        }

        let mut ops: u64 = 0;
        let mut maybe_match: Option<(u32, u32, u32, u32)> = None;

        while !min_heap.is_empty() && !max_heap.is_empty() {
            ops += 1;
            if (ops & 0xFFF) == 0 && sh.stopped() {
                return;
            }

            let (ab, ai, bi) = match min_heap.peek() {
                Some(Reverse((s, ai, bi))) => (s.clone(), *ai, *bi),
                None => break,
            };
            let (cd, ci, di) = match max_heap.peek() {
                Some((s, ci, di)) => (s.clone(), *ci, *di),
                None => break,
            };

            let total = &ab + &cd;
            if total == target {
                maybe_match = Some((ai, bi, ci, di));
                break;
            } else if total < target {
                min_heap.pop();
                let ai_us = ai as usize;
                if ai_us + 1 < a_sums.len() {
                    let new_sum = &a_sums[ai_us + 1].0 + &b_sums[bi as usize].0;
                    if new_sum <= target {
                        min_heap.push(Reverse((new_sum, (ai_us + 1) as u32, bi)));
                    }
                }
            } else {
                max_heap.pop();
                let ci_us = ci as usize;
                if ci_us > 0 {
                    let new_sum = &c_sums[ci_us - 1].0 + &d_sums[di as usize].0;
                    max_heap.push((new_sum, (ci_us - 1) as u32, di));
                }
                // ci == 0: CD chain exhausted. Pop is enough.
            }
        }

        if let Some((ai, bi, ci, di)) = maybe_match {
            let a_mask = a_sums[ai as usize].1;
            let b_mask = b_sums[bi as usize].1;
            let c_mask = c_sums[ci as usize].1;
            let d_mask = d_sums[di as usize].1;

            let mut sol: Vec<BigUint> = Vec::new();
            push_selected_big(&qa, a_mask, &mut sol);
            push_selected_big(&qb, b_mask, &mut sol);
            push_selected_big(&qc, c_mask, &mut sol);
            push_selected_big(&qd, d_mask, &mut sol);
            sh.report(sol, "Schroeppel-Shamir");
        }
    }
}

/// Parallel sum generation for u128: split mask range across threads.
fn build_sums_u128_par(elems: &[u128], target: u128) -> Vec<(u128, u64)> {
    let n = elems.len();
    if n <= 16 {
        return build_sums_u128(elems, target);
    }
    let total = 1u64 << n;
    let ncpus = thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let chunk = (total / ncpus as u64).max(1);
    let mut handles = Vec::with_capacity(ncpus);

    // Prefix sums needed for Gray-code.
    let mut pref = vec![0u128; n + 1];
    for i in 0..n {
        pref[i + 1] = pref[i].wrapping_add(elems[i]);
    }

    let pref = Arc::new(pref);
    let elems = Arc::new(elems.to_vec());

    for tid in 0..ncpus {
        let start = tid as u64 * chunk;
        let end = if tid + 1 == ncpus { total } else { start + chunk };
        if start >= end { continue; }
        let elems = Arc::clone(&elems);
        let pref = Arc::clone(&pref);
        handles.push(thread::spawn(move || {
            let cap = (end - start) as usize;
            let mut sums: Vec<(u128, u64)> = Vec::with_capacity(cap);
            // Compute initial sum for `start` by iterating its bits.
            let mut s: u128 = 0;
            if start > 0 {
                let mut m = start;
                while m != 0 {
                    let k = m.trailing_zeros() as usize;
                    s = s.wrapping_add(elems[k]).wrapping_sub(pref[k]);
                    m &= m - 1;
                }
            }
            for mask in start..end {
                if mask > start {
                    let k = mask.trailing_zeros() as usize;
                    s = s.wrapping_add(elems[k]).wrapping_sub(pref[k]);
                }
                if s <= target {
                    sums.push((s, mask));
                }
            }
            sums
        }));
    }

    let mut all: Vec<(u128, u64)> = Vec::new();
    for h in handles {
        if let Ok(mut chunk) = h.join() {
            all.append(&mut chunk);
        }
    }
    all.sort_unstable_by_key(|x| x.0);
    all
}

fn build_sums_u128(elems: &[u128], target: u128) -> Vec<(u128, u64)> {
    let n = elems.len();
    let total = 1u64 << n;
    let mut sums: Vec<(u128, u64)> = Vec::with_capacity(total as usize);
    // Prefix sums: pref[i] = sum of elems[0..i]
    let mut pref = vec![0u128; n + 1];
    for i in 0..n {
        pref[i + 1] = pref[i].wrapping_add(elems[i]);
    }
    let mut s: u128 = 0;
    for mask in 0u64..total {
        if mask > 0 {
            let k = mask.trailing_zeros() as usize;
            s = s.wrapping_add(elems[k]).wrapping_sub(pref[k]);
        }
        if s <= target {
            sums.push((s, mask));
        }
    }
    sums.sort_unstable_by_key(|x| x.0);
    sums
}

fn build_sums_big(elems: &[BigUint], target: &BigUint) -> Vec<(BigUint, u64)> {
    let n = elems.len();
    let total = 1u64 << n;
    let mut sums: Vec<(BigUint, u64)> = Vec::with_capacity(total as usize);
    // Prefix sums: pref[i] = sum of elems[0..i]
    let mut pref = vec![BigUint::from(0u32); n + 1];
    for i in 0..n {
        pref[i + 1] = &pref[i] + &elems[i];
    }
    let mut s = BigUint::from(0u32);
    for mask in 0u64..total {
        if mask > 0 {
            let k = mask.trailing_zeros() as usize;
            // s = s + elems[k] - pref[k];
            // Since pref[k] <= s (all elements added before were in pref[k]),
            // no underflow possible.
            let new_s = &s + &elems[k];
            s = new_s - &pref[k];
        }
        if s <= *target {
            sums.push((s.clone(), mask));
        }
    }
    sums.sort();
    sums
}

#[inline]
fn push_selected(quarter: &[u128], mask: u64, sol: &mut Vec<BigUint>) {
    let mut m = mask;
    while m != 0 {
        let bit = m.trailing_zeros() as usize;
        sol.push(BigUint::from(quarter[bit]));
        m &= m - 1;
    }
}

#[inline]
fn push_selected_big(quarter: &[BigUint], mask: u64, sol: &mut Vec<BigUint>) {
    let mut m = mask;
    while m != 0 {
        let bit = m.trailing_zeros() as usize;
        sol.push(quarter[bit].clone());
        m &= m - 1;
    }
}
