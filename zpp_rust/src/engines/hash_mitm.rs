//! HashMITM — Rehan's HashMap MITM (2-way + 4-way for n>=36)
//! Zero Schroeppel-Shamir techniques. Pure u128 + HashMap collision.

use num_bigint::BigUint;
use std::collections::HashMap;
use crate::fast_hash::FastHash;
use crate::controller::{Engine, Shared};

pub struct HashMitmEngine;
const HM_MIN_N: usize = 20;

impl Engine for HashMitmEngine {
    fn name(&self) -> &'static str { "HashMITM" }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        if p.n < HM_MIN_N || !p.u128_safe() { return; }
        let target = p.target_u128();
        let nums = p.numbers_u128();
        let n = nums.len();

        if n >= 36 { self.run_4way(sh, &nums, target, n); }
        else { self.run_2way(sh, &nums, target, (n/2).min(22)); }
    }
}

impl HashMitmEngine {
    fn run_2way(&self, sh: &Shared, nums: &[u128], target: u128, half: usize) {
        if half < 2 || sh.stopped() { return; }
        let left_map = build_fasthash(&nums[..half], target);
        if left_map.is_empty() || sh.stopped() { return; }

        let rn = (nums.len() - half).min(22);
        let right = &nums[half..(half + rn)];
        let total = 1u64 << rn;
        let mut pref = vec![0u128; rn + 1];
        for i in 0..rn { pref[i + 1] = pref[i].wrapping_add(right[i]); }
        let mut s: u128 = 0;
        for mask in 0u64..total {
            if mask > 0 { let k = mask.trailing_zeros() as usize; s = s.wrapping_add(right[k]).wrapping_sub(pref[k]); }
            if s > target { continue; }
            if (mask & 0xFFF) == 0 && sh.stopped() { return; }
            if let Some(lm) = left_map.get(target - s) {
                let m = lm | (mask << half as u32);
                let mut sol = Vec::new();
                let mut b = m;
                for &v in nums { if b & 1 != 0 { sol.push(BigUint::from(v)); } b >>= 1; }
                sh.report(sol, "HashMITM"); return;
            }
        }
    }

    fn run_4way(&self, sh: &Shared, nums: &[u128], target: u128, n: usize) {
        let q = n / 4;
        let a = build_sums_vec(&nums[0..q], target);
        let b = build_sums_vec(&nums[q..2*q], target);
        let c = build_sums_vec(&nums[2*q..3*q], target);
        let d = build_sums_vec(&nums[3*q..], target);
        if a.is_empty() || b.is_empty() || c.is_empty() || d.is_empty() { return; }
        if sh.stopped() { return; }

        // Hash A+B merged (left half). Then enumerate C+D (right half) and check.
        let mut left_map: HashMap<u128, (u32, u32)> = HashMap::with_capacity(a.len() * b.len() / 4);
        for (i, &(av, am)) in a.iter().enumerate() {
            if av > target { break; }
            for (j, &(bv, bm)) in b.iter().enumerate() {
                let ab = av.wrapping_add(bv);
                if ab <= target { left_map.entry(ab).or_insert((i as u32, j as u32)); }
                if sh.stopped() { return; }
            }
        }
        if left_map.is_empty() { return; }

        // Enumerate C+D and check
        for &(cv, cm) in &c {
            if cv > target { break; }
            for &(dv, dm) in &d {
                let cd = cv.wrapping_add(dv);
                if cd > target { continue; }
                if sh.stopped() { return; }
                let need = target - cd;
                if let Some(&(ai, bi)) = left_map.get(&need) {
                    let mask = a[ai as usize].1 | (b[bi as usize].1 << q as u32) | (cm << (2*q) as u32) | (dm << (3*q) as u32);
                    let mut sol = Vec::new();
                    let mut m = mask;
                    for &v in nums { if m & 1 != 0 { sol.push(BigUint::from(v)); } m >>= 1; }
                    sh.report(sol, "HashMITM"); return;
                }
            }
        }
    }
}

fn build_sums_vec(elems: &[u128], target: u128) -> Vec<(u128, u64)> {
    let n = elems.len();
    let total = 1u64 << n;
    let mut sums = Vec::with_capacity(total as usize);
    let mut pref = vec![0u128; n + 1];
    for i in 0..n { pref[i + 1] = pref[i].wrapping_add(elems[i]); }
    let mut s: u128 = 0;
    for mask in 0u64..total {
        if mask > 0 { let k = mask.trailing_zeros() as usize; s = s.wrapping_add(elems[k]).wrapping_sub(pref[k]); }
        if s <= target { sums.push((s, mask)); }
    }
    sums
}

fn build_hashmap(elems: &[u128], target: u128) -> HashMap<u128, u64> {
    let n = elems.len();
    let total = 1u64 << n;
    let mut map: HashMap<u128, u64> = HashMap::with_capacity(total as usize);
    let mut pref = vec![0u128; n + 1];
    for i in 0..n { pref[i + 1] = pref[i].wrapping_add(elems[i]); }
    let mut s: u128 = 0;
    for mask in 0u64..total {
        if mask > 0 { let k = mask.trailing_zeros() as usize; s = s.wrapping_add(elems[k]).wrapping_sub(pref[k]); }
        if s <= target { map.entry(s).or_insert(mask); }
    }
    map
}

fn build_fasthash(elems: &[u128], target: u128) -> FastHash {
    let n = elems.len();
    let total = 1u64 << n;
    let mut map = FastHash::with_capacity(total as usize);
    let mut pref = vec![0u128; n + 1];
    for i in 0..n { pref[i + 1] = pref[i].wrapping_add(elems[i]); }
    let mut s: u128 = 0;
    for mask in 0u64..total {
        if mask > 0 { let k = mask.trailing_zeros() as usize; s = s.wrapping_add(elems[k]).wrapping_sub(pref[k]); }
        if s <= target { map.insert(s, mask); }
    }
    map
}
