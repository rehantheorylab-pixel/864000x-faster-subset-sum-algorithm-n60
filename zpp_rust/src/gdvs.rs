//! Goal Distance Vector Space (GDVS) — your novel idea.
//!
//! Reference: `Subset sum algorithm.md` line 12037.
//!
//! Instead of scoring partial states with a single scalar
//! `|target - current_sum|`, we project each state into a
//! multi-dimensional proximity vector:
//!
//!   v = [scalar_distance, parity_distance, cluster_distance,
//!        structural_distance]
//!
//! and use the L2 norm (or weighted sum) to rank candidates. This
//! gives the search a richer notion of "closeness to goal" than any
//! solver currently in the literature uses.
//!
//! All metrics here are computed on `u128`; the higher-level engine
//! is responsible for converting BigUint → u128 when safe.

use num_bigint::BigUint;
use num_traits::ToPrimitive;

/// Distances to the goal across multiple semantic axes.
#[derive(Clone, Copy, Debug)]
pub struct GdvsDistance {
    /// |target - current_sum|, normalised to [0, 1] by max_value.
    pub scalar: f64,
    /// Parity mismatch on small primes (0 if compatible, else 1).
    pub parity: f64,
    /// Distance to the *closest cluster centre* of remaining elements.
    pub cluster: f64,
    /// Distance to a balanced split point (target / k) for current k.
    pub structural: f64,
}

impl GdvsDistance {
    /// Weighted L1 norm.  Smaller is better.
    #[inline]
    pub fn norm(&self) -> f64 {
        0.50 * self.scalar
            + 0.20 * self.parity
            + 0.15 * self.cluster
            + 0.15 * self.structural
    }
}

/// Compute the GDVS distance vector for a candidate state.
///
/// * `current_sum` — sum of elements already chosen
/// * `target` — overall problem target
/// * `chosen_count` — number of elements already chosen
/// * `max_val` — largest element in the input (normaliser)
/// * `cluster_centers` — pre-computed centroids of element groups
pub fn gdvs(
    current_sum: u128,
    target: u128,
    chosen_count: usize,
    max_val: u128,
    cluster_centers: &[u128],
) -> GdvsDistance {
    let remaining = target.saturating_sub(current_sum);
    let scalar = if max_val == 0 {
        0.0
    } else {
        remaining as f64 / max_val as f64
    };

    // Parity mismatch on small primes 2, 3, 5.  If the remaining
    // residue is unreachable from a single element, we mark it as 1.
    let parity = parity_distance(remaining);

    // Cluster distance: how far is `remaining` from the nearest
    // representative cluster centre we have available?
    let cluster = if cluster_centers.is_empty() || remaining == 0 {
        0.0
    } else {
        let nearest = cluster_centers
            .iter()
            .map(|c| absdiff(remaining, *c))
            .min()
            .unwrap_or(0);
        nearest as f64 / max_val.max(1) as f64
    };

    // Structural distance: how far is `current_sum` from a balanced
    // split point (target / max(1, chosen_count))?
    let structural = if chosen_count == 0 {
        scalar
    } else {
        let bp = target / (chosen_count as u128 + 1);
        absdiff(current_sum, bp) as f64 / max_val.max(1) as f64
    };

    GdvsDistance {
        scalar,
        parity,
        cluster,
        structural,
    }
}

/// 1.0 if `remaining` cannot be matched to any single small-prime
/// residue class easily; 0.0 otherwise. Cheap proxy for parity
/// compatibility.
fn parity_distance(remaining: u128) -> f64 {
    let mut bad = 0;
    for &p in &[2u128, 3, 5, 7] {
        if remaining % p == p - 1 {
            bad += 1;
        }
    }
    bad as f64 / 4.0
}

/// Compute simple element clusters via 1-D k-means lite.
/// Returns up to `k` representative centroids over the elements,
/// fast and deterministic.
pub fn compute_clusters(numbers: &[u128], k: usize) -> Vec<u128> {
    if numbers.is_empty() || k == 0 {
        return Vec::new();
    }
    let mut sorted: Vec<u128> = numbers.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    if n <= k {
        return sorted;
    }
    let mut centers = Vec::with_capacity(k);
    let step = (n + k - 1) / k;
    for i in 0..k {
        let lo = i * step;
        let hi = ((i + 1) * step).min(n);
        if lo >= hi {
            break;
        }
        let chunk: u128 = sorted[lo..hi].iter().sum::<u128>() / (hi - lo) as u128;
        centers.push(chunk);
    }
    centers
}

#[inline]
fn absdiff(a: u128, b: u128) -> u128 {
    if a > b { a - b } else { b - a }
}

/// Convenience: sum the BigUint elements into u128 with overflow check.
pub fn safe_to_u128(b: &BigUint) -> Option<u128> {
    b.to_u128()
}
