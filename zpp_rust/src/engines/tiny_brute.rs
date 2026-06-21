use num_bigint::BigUint;
use num_traits::Zero;
use crate::controller::{Engine, Shared};

pub struct TinyBruteEngine;

impl Engine for TinyBruteEngine {
    fn name(&self) -> &'static str {
        "TinyBrute"
    }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        if sh.stopped() { return; }
        if p.n <= 12 {
            if p.u128_safe() { self.run_u128(sh); }
            else { self.run_big(sh); }
        } else if p.n <= 40 && p.u128_safe() {
            self.random_sample(sh); // Adaptive for large n
        }
    }
}

impl TinyBruteEngine {
    fn run_u128(&self, sh: &Shared) {
        let target = sh.profile.target_u128();
        let nums = sh.profile.numbers_u128();
        let n = nums.len();
        let total = 1u64 << n;
        for mask in 0u64..total {
            if (mask & 0xFF) == 0 && sh.stopped() { return; }
            let mut sum = 0u128;
            let mut m = mask;
            while m != 0 {
                let k = m.trailing_zeros() as usize;
                sum = sum.wrapping_add(nums[k]);
                if sum > target { break; }
                m &= m - 1;
            }
            if sum == target {
                let mut sol: Vec<BigUint> = Vec::new();
                let mut mm = mask;
                while mm != 0 {
                    let k = mm.trailing_zeros() as usize;
                    sol.push(BigUint::from(nums[k]));
                    mm &= mm - 1;
                }
                sh.report(sol, "TinyBrute");
                return;
            }
        }
    }

    fn run_big(&self, sh: &Shared) {
        let target = &sh.profile.target;
        let nums = &sh.profile.numbers;
        let n = nums.len();
        let total = 1u64 << n;
        for mask in 0u64..total {
            if (mask & 0xFF) == 0 && sh.stopped() { return; }
            let mut sum = BigUint::zero();
            let mut m = mask;
            while m != 0 {
                let k = m.trailing_zeros() as usize;
                sum += &nums[k];
                if sum > *target { break; }
                m &= m - 1;
            }
            if sum == *target {
                let mut sol: Vec<BigUint> = Vec::new();
                let mut mm = mask;
                while mm != 0 {
                    let k = mm.trailing_zeros() as usize;
                    sol.push(nums[k].clone());
                    mm &= mm - 1;
                }
                sh.report(sol, "TinyBrute");
                return;
            }
        }
    }

    fn random_sample(&self, sh: &Shared) {
        let nums = sh.profile.numbers_u128();
        let target = sh.profile.target_u128();
        let n = nums.len();
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
        for i in 0..500_000 {
            if sh.stopped() { return; }
            let bits = seed.wrapping_mul(i + 1) ^ seed.wrapping_shl(i as u32 % 13);
            let mut sum: u128 = 0;
            let mut m = bits;
            let mut count = 0;
            for _ in 0..n.min(64) {
                if m & 1 != 0 {
                    let idx = (m as usize) % n;
                    sum = sum.wrapping_add(nums[idx]);
                    count += 1;
                    if count > 12 { break; }
                }
                m >>= 1;
                if m == 0 { break; }
            }
            if sum == target {
                let mut sol = Vec::new();
                let mut m2 = bits;
                let mut c2 = 0;
                for _ in 0..n.min(64) {
                    if m2 & 1 != 0 {
                        let idx = (m2 as usize) % n;
                        sol.push(BigUint::from(nums[idx]));
                        c2 += 1;
                        if c2 > 12 { break; }
                    }
                    m2 >>= 1;
                    if m2 == 0 { break; }
                }
                if !sol.is_empty() { sh.report(sol, "TinyBrute"); return; }
            }
        }
    }
}
