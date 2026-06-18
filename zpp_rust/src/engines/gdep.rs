use num_bigint::BigUint;
use num_traits::Zero;
use crate::controller::{Engine, Shared};
use crate::engines::digit_filter::DigitFilterEngine;

pub struct GdepEngine;

impl Engine for GdepEngine {
    fn name(&self) -> &'static str { "GDEP" }

    fn run(&self, sh: &Shared) {
        let nums = &sh.profile.numbers;
        let target = &sh.profile.target;
        let n = nums.len();
        if n == 0 || target.is_zero() {
            if target.is_zero() { sh.report(vec![], "GDEP"); }
            return;
        }

        // Sort by proximity to target: elements closest to target first
        // This finds sparse solutions (few elements) much faster
        let mut ordered: Vec<BigUint> = nums.to_vec();
        ordered.sort_by(|a, b| {
            let da = if a > target { BigUint::from(u64::MAX) } else { target - a };
            let db = if b > target { BigUint::from(u64::MAX) } else { target - b };
            da.cmp(&db)
        });

        let mut suf: Vec<BigUint> = vec![BigUint::zero(); n + 1];
        for i in (0..n).rev() {
            suf[i] = &suf[i + 1] + &ordered[i];
        }
        let mut path: Vec<BigUint> = Vec::new();

        fn dfs(
            nums: &[BigUint],
            suf: &[BigUint],
            target: &BigUint,
            start: usize,
            n: usize,
            path: &mut Vec<BigUint>,
            current_sum: &BigUint,
            sh: &Shared,
        ) -> bool {
            if target.is_zero() { return true; }
            if sh.stopped() || start >= n { return false; }

            let remaining = target;
            let zero = BigUint::zero();

            if !DigitFilterEngine::last_2_digits_reachable(&nums[start..], remaining, &zero) {
                return false;
            }

            for i in start..n {
                let v = &nums[i];
                if v > remaining { continue; }
                if suf[i] < *remaining { return false; }

                if v == remaining {
                    path.push(v.clone());
                    return true;
                }

                let new_target = remaining - v;
                let new_sum = current_sum + v;
                if suf[i + 1] >= new_target {
                    path.push(v.clone());
                    if dfs(nums, suf, &new_target, i + 1, n, path, &new_sum, sh) {
                        return true;
                    }
                    path.pop();
                }
            }
            false
        }

        let zero = BigUint::zero();
        if dfs(&ordered, &suf, target, 0, n, &mut path, &zero, sh) {
            sh.report(path, "GDEP");
        }
    }
}
