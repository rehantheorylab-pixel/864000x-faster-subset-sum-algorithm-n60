//! Fraction type for exact rational arithmetic
use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::{Zero, One};

#[derive(Clone, Debug)]
pub struct Fraction {
    pub num: BigUint,
    pub den: BigUint,
}

impl Fraction {
    pub fn zero() -> Self {
        Fraction { num: BigUint::zero(), den: BigUint::one() }
    }

    pub fn from_biguint(n: BigUint) -> Self {
        Fraction { num: n, den: BigUint::one() }
    }

    pub fn from_u64(n: u64) -> Self {
        Fraction { num: BigUint::from(n), den: BigUint::one() }
    }

    pub fn from_i64(n: i64) -> Self {
        Fraction { num: BigUint::from(n.unsigned_abs()), den: BigUint::one() }
    }

    pub fn from_f64(val: f64) -> Self {
        if val <= 0.0 { return Fraction::zero(); }
        let den = 1_000_000_000u64;
        let num = (val * den as f64) as u64;
        Fraction { num: BigUint::from(num), den: BigUint::from(den) }
    }

    pub fn new(num: BigUint, den: BigUint) -> Self {
        if den.is_zero() { return Fraction { num: BigUint::zero(), den: BigUint::one() }; }
        let g = num.gcd(&den);
        Fraction { num: num / &g, den: den / &g }
    }

    pub fn is_zero(&self) -> bool { self.num.is_zero() }

    pub fn abs_gt(&self, val: f64) -> bool {
        if self.den.is_zero() { return false; }
        let nbytes = self.num.to_bytes_le();
        let dbytes = self.den.to_bytes_le();
        let mut nval: u64 = 0;
        let mut dval: u64 = 1;
        for (i, &b) in nbytes.iter().enumerate().take(8) { nval |= (b as u64) << (i*8); }
        for (i, &b) in dbytes.iter().enumerate().take(8) { dval |= (b as u64) << (i*8); }
        if dval == 0 { dval = 1; }
        (nval as f64) / (dval as f64) > val
    }

    pub fn round_i64(&self) -> i64 {
        if self.den.is_zero() { return 0; }
        let nb = self.num.to_bytes_le();
        let db = self.den.to_bytes_le();
        let mut nv: u64 = 0;
        let mut dv: u64 = 1;
        for (i, &b) in nb.iter().enumerate().take(8) { nv |= (b as u64) << (i*8); }
        for (i, &b) in db.iter().enumerate().take(8) { dv |= (b as u64) << (i*8); }
        if dv == 0 { return 0; }
        (nv as f64 / dv as f64).round() as i64
    }

    pub fn mul(a: &Fraction, b: &Fraction) -> Fraction {
        Fraction::new(&a.num * &b.num, &a.den * &b.den)
    }

    pub fn div(a: &Fraction, b: &Fraction) -> Fraction {
        Fraction::new(&a.num * &b.den, &a.den * &b.num)
    }

    pub fn sub(a: &Fraction, b: &Fraction) -> Fraction {
        let num1 = &a.num * &b.den;
        let num2 = &b.num * &a.den;
        let den = &a.den * &b.den;
        if num1 >= num2 {
            Fraction::new(num1 - num2, den)
        } else {
            // Negative result — return zero (for our use case, norms are non-negative)
            Fraction::zero()
        }
    }

    pub fn ge(a: &Fraction, b: &Fraction) -> bool {
        let lhs = &a.num * &b.den;
        let rhs = &b.num * &a.den;
        lhs >= rhs
    }

    pub fn mul_scalar(a: &Fraction, scalar: i64) -> Fraction {
        let s = BigUint::from(scalar.unsigned_abs());
        Fraction::new(&a.num * &s, a.den.clone())
    }
}
