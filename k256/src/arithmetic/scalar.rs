//! Scalar field arithmetic.

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(any(target_pointer_width = "32", feature = "force-32-bit"))] {
        mod scalar_8x32;
        use scalar_8x32::Scalar8x32 as ScalarImpl;

        #[cfg(feature = "bits")]
        use scalar_8x32::MODULUS;
    } else if #[cfg(target_pointer_width = "64")] {
        mod scalar_4x64;
        use scalar_4x64::Scalar4x64 as ScalarImpl;
        #[cfg(feature = "bits")]
        use scalar_4x64::MODULUS;
    }
}

use crate::FieldBytes;
use core::ops::{Add, AddAssign, Mul, MulAssign, Neg, Shr, Sub, SubAssign};
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

#[cfg(feature = "bits")]
use {crate::ScalarBits, elliptic_curve::group::ff::PrimeFieldBits};

#[cfg(feature = "digest")]
use ecdsa_core::{elliptic_curve::consts::U32, hazmat::FromDigest, signature::digest::Digest};

#[cfg(feature = "zeroize")]
use elliptic_curve::zeroize::Zeroize;

#[cfg(test)]
use num_bigint::{BigUint, ToBigUint};

/// Scalars are elements in the finite field modulo n.
///
/// # Trait impls
///
/// Much of the important functionality of scalars is provided by traits from
/// the [`ff`](https://docs.rs/ff/) crate, which is re-exported as
/// `k256::elliptic_curve::ff`:
///
/// - [`Field`](https://docs.rs/ff/latest/ff/trait.Field.html) -
///   represents elements of finite fields and provides:
///   - [`Field::random`](https://docs.rs/ff/latest/ff/trait.Field.html#tymethod.random) -
///     generate a random scalar
///   - `double`, `square`, and `invert` operations
///   - Bounds for [`Add`], [`Sub`], [`Mul`], and [`Neg`] (as well as `*Assign` equivalents)
///   - Bounds for [`ConditionallySelectable`] from the `subtle` crate
/// - [`PrimeField`](https://docs.rs/ff/latest/ff/trait.PrimeField.html) -
///   represents elements of prime fields and provides:
///   - `from_repr`/`to_repr` for converting field elements from/to big integers.
///   - `char_le_bits`, `multiplicative_generator`, `root_of_unity` constants.
/// - [`PrimeFieldBits`](https://docs.rs/ff/latest/ff/trait.PrimeFieldBits.html) -
///   operations over field elements represented as bits (requires `bits` feature)
///
/// Please see the documentation for the relevant traits for more information.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
pub struct Scalar(ScalarImpl);

impl Scalar {
    #[must_use]
    pub fn double(&self) -> Self {
        self.add(self)
    }

    // TODO(tarcieri): stub! See: https://github.com/RustCrypto/elliptic-curves/issues/170
    pub fn sqrt(&self) -> CtOption<Self> {
        todo!("see RustCrypto/elliptic-curves#170");
    }
}

impl Scalar {
    /// Attempts to parse the given byte array as an SEC1-encoded scalar.
    ///
    /// Returns None if the byte array does not contain a big-endian integer in the range
    /// [0, p).
    pub fn from_repr(bytes: FieldBytes) -> Option<Self> {
        ScalarImpl::from_bytes(bytes.as_ref()).map(Self).into()
    }

    pub fn to_repr(&self) -> FieldBytes {
        self.to_bytes()
    }

    pub fn is_odd(&self) -> bool {
        self.0.is_odd().into()
    }

    pub fn multiplicative_generator() -> Self {
        7u64.into()
    }
}

#[cfg(feature = "bits")]
#[cfg_attr(docsrs, doc(cfg(feature = "bits")))]
impl PrimeFieldBits for Scalar {
    cfg_if! {
        if #[cfg(any(target_pointer_width = "32", feature = "force-32-bit"))] {
            type ReprBits = [u32; 8];
        } else if #[cfg(target_pointer_width = "64")] {
            type ReprBits = [u64; 4];
        }
    }

    fn to_le_bits(&self) -> ScalarBits {
        self.into()
    }

    fn char_le_bits() -> ScalarBits {
        MODULUS.into()
    }
}

impl From<u32> for Scalar {
    fn from(k: u32) -> Self {
        Self(ScalarImpl::from(k))
    }
}

impl From<u64> for Scalar {
    fn from(k: u64) -> Self {
        Self(ScalarImpl::from(k))
    }
}

impl Scalar {
    /// Returns the zero scalar.
    pub const fn zero() -> Self {
        Self(ScalarImpl::zero())
    }

    /// Returns the multiplicative identity.
    pub const fn one() -> Scalar {
        Self(ScalarImpl::one())
    }

    /// Checks if the scalar is zero.
    pub fn is_zero(&self) -> Choice {
        self.0.is_zero()
    }

    /// Returns the value of the scalar truncated to a 32-bit unsigned integer.
    pub fn truncate_to_u32(&self) -> u32 {
        self.0.truncate_to_u32()
    }

    /// Attempts to parse the given byte array as a scalar.
    /// Does not check the result for being in the correct range.
    pub(crate) const fn from_bytes_unchecked(bytes: &[u8; 32]) -> Self {
        Self(ScalarImpl::from_bytes_unchecked(bytes))
    }

    /// Parses the given byte array as a scalar.
    ///
    /// Subtracts the modulus when the byte array is larger than the modulus.
    pub fn from_bytes_reduced(bytes: &FieldBytes) -> Self {
        Self(ScalarImpl::from_bytes_reduced(bytes.as_ref()))
    }

    /// Returns the SEC1 encoding of this scalar.
    pub fn to_bytes(&self) -> FieldBytes {
        self.0.to_bytes()
    }

    /// Is this scalar greater than or equal to n / 2?
    pub fn is_high(&self) -> Choice {
        self.0.is_high()
    }

    /// Negates the scalar.
    pub fn negate(&self) -> Self {
        Self(self.0.negate())
    }

    /// Modulo adds two scalars
    pub fn add(&self, rhs: &Scalar) -> Scalar {
        Self(self.0.add(&(rhs.0)))
    }

    /// Modulo subtracts one scalar from the other.
    pub fn sub(&self, rhs: &Scalar) -> Scalar {
        Self(self.0.sub(&(rhs.0)))
    }

    /// Modulo multiplies two scalars.
    pub fn mul(&self, rhs: &Scalar) -> Scalar {
        Self(self.0.mul(&(rhs.0)))
    }

    /// Modulo squares the scalar.
    pub fn square(&self) -> Self {
        self.mul(&self)
    }

    /// Right shifts the scalar. Note: not constant-time in `shift`.
    pub fn rshift(&self, shift: usize) -> Scalar {
        Self(self.0.rshift(shift))
    }

    /// Raises the scalar to the power `2^k`
    fn pow2k(&self, k: usize) -> Self {
        let mut x = *self;
        for _j in 0..k {
            x = x.square();
        }
        x
    }

    /// Inverts the scalar.
    pub fn invert(&self) -> CtOption<Self> {
        // Using an addition chain from
        // https://briansmith.org/ecc-inversion-addition-chains-01#secp256k1_scalar_inversion

        let x_1 = *self;
        let x_10 = self.pow2k(1);
        let x_11 = x_10.mul(&x_1);
        let x_101 = x_10.mul(&x_11);
        let x_111 = x_10.mul(&x_101);
        let x_1001 = x_10.mul(&x_111);
        let x_1011 = x_10.mul(&x_1001);
        let x_1101 = x_10.mul(&x_1011);

        let x6 = x_1101.pow2k(2).mul(&x_1011);
        let x8 = x6.pow2k(2).mul(&x_11);
        let x14 = x8.pow2k(6).mul(&x6);
        let x28 = x14.pow2k(14).mul(&x14);
        let x56 = x28.pow2k(28).mul(&x28);

        #[rustfmt::skip]
        let res = x56
            .pow2k(56).mul(&x56)
            .pow2k(14).mul(&x14)
            .pow2k(3).mul(&x_101)
            .pow2k(4).mul(&x_111)
            .pow2k(4).mul(&x_101)
            .pow2k(5).mul(&x_1011)
            .pow2k(4).mul(&x_1011)
            .pow2k(4).mul(&x_111)
            .pow2k(5).mul(&x_111)
            .pow2k(6).mul(&x_1101)
            .pow2k(4).mul(&x_101)
            .pow2k(3).mul(&x_111)
            .pow2k(5).mul(&x_1001)
            .pow2k(6).mul(&x_101)
            .pow2k(10).mul(&x_111)
            .pow2k(4).mul(&x_111)
            .pow2k(9).mul(&x8)
            .pow2k(5).mul(&x_1001)
            .pow2k(6).mul(&x_1011)
            .pow2k(4).mul(&x_1101)
            .pow2k(5).mul(&x_11)
            .pow2k(6).mul(&x_1101)
            .pow2k(10).mul(&x_1101)
            .pow2k(4).mul(&x_1001)
            .pow2k(6).mul(&x_1)
            .pow2k(8).mul(&x6);

        CtOption::new(res, !self.is_zero())
    }

    /// Returns the scalar modulus as a `BigUint` object.
    #[cfg(test)]
    pub fn modulus_as_biguint() -> BigUint {
        Self::one().negate().to_biguint().unwrap() + 1.to_biguint().unwrap()
    }

    /// If `flag` evaluates to `true`, adds `(1 << bit)` to `self`.
    pub fn conditional_add_bit(&self, bit: usize, flag: Choice) -> Self {
        Self(self.0.conditional_add_bit(bit, flag))
    }

    /// Multiplies `self` by `b` (without modulo reduction) divide the result by `2^shift`
    /// (rounding to the nearest integer).
    /// Variable time in `shift`.
    pub fn mul_shift_var(&self, b: &Scalar, shift: usize) -> Self {
        Self(self.0.mul_shift_var(&(b.0), shift))
    }
}

#[cfg(feature = "digest")]
#[cfg_attr(docsrs, doc(cfg(feature = "digest")))]
impl FromDigest<Secp256k1> for Scalar {
    /// Convert the output of a digest algorithm into a [`Scalar`] reduced
    /// modulo n.
    fn from_digest<D>(digest: D) -> Self
    where
        D: Digest<OutputSize = U32>,
    {
        Self::from_bytes_reduced(&digest.finalize())
    }
}

impl Shr<usize> for Scalar {
    type Output = Self;

    fn shr(self, rhs: usize) -> Self::Output {
        self.rshift(rhs)
    }
}

impl Shr<usize> for &Scalar {
    type Output = Scalar;

    fn shr(self, rhs: usize) -> Self::Output {
        self.rshift(rhs)
    }
}

impl ConditionallySelectable for Scalar {
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        Self(ScalarImpl::conditional_select(&(a.0), &(b.0), choice))
    }
}

impl ConstantTimeEq for Scalar {
    fn ct_eq(&self, other: &Self) -> Choice {
        self.0.ct_eq(&(other.0))
    }
}

impl PartialEq for Scalar {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other).into()
    }
}

impl Eq for Scalar {}

impl Neg for Scalar {
    type Output = Scalar;

    fn neg(self) -> Scalar {
        self.negate()
    }
}

impl Neg for &Scalar {
    type Output = Scalar;

    fn neg(self) -> Scalar {
        self.negate()
    }
}

impl Add<Scalar> for Scalar {
    type Output = Scalar;

    fn add(self, other: Scalar) -> Scalar {
        Scalar::add(&self, &other)
    }
}

impl Add<&Scalar> for &Scalar {
    type Output = Scalar;

    fn add(self, other: &Scalar) -> Scalar {
        Scalar::add(self, other)
    }
}

impl Add<Scalar> for &Scalar {
    type Output = Scalar;

    fn add(self, other: Scalar) -> Scalar {
        Scalar::add(self, &other)
    }
}

impl Add<&Scalar> for Scalar {
    type Output = Scalar;

    fn add(self, other: &Scalar) -> Scalar {
        Scalar::add(&self, other)
    }
}

impl AddAssign<Scalar> for Scalar {
    fn add_assign(&mut self, rhs: Scalar) {
        *self = Scalar::add(self, &rhs);
    }
}

impl AddAssign<&Scalar> for Scalar {
    fn add_assign(&mut self, rhs: &Scalar) {
        *self = Scalar::add(self, &rhs);
    }
}

impl Sub<Scalar> for Scalar {
    type Output = Scalar;

    fn sub(self, other: Scalar) -> Scalar {
        Scalar::sub(&self, &other)
    }
}

impl Sub<&Scalar> for &Scalar {
    type Output = Scalar;

    fn sub(self, other: &Scalar) -> Scalar {
        Scalar::sub(self, other)
    }
}

impl Sub<&Scalar> for Scalar {
    type Output = Scalar;

    fn sub(self, other: &Scalar) -> Scalar {
        Scalar::sub(&self, other)
    }
}

impl SubAssign<Scalar> for Scalar {
    fn sub_assign(&mut self, rhs: Scalar) {
        *self = Scalar::sub(self, &rhs);
    }
}

impl SubAssign<&Scalar> for Scalar {
    fn sub_assign(&mut self, rhs: &Scalar) {
        *self = Scalar::sub(self, rhs);
    }
}

impl Mul<Scalar> for Scalar {
    type Output = Scalar;

    fn mul(self, other: Scalar) -> Scalar {
        Scalar::mul(&self, &other)
    }
}

impl Mul<&Scalar> for &Scalar {
    type Output = Scalar;

    fn mul(self, other: &Scalar) -> Scalar {
        Scalar::mul(self, other)
    }
}

impl Mul<&Scalar> for Scalar {
    type Output = Scalar;

    fn mul(self, other: &Scalar) -> Scalar {
        Scalar::mul(&self, other)
    }
}

impl MulAssign<Scalar> for Scalar {
    fn mul_assign(&mut self, rhs: Scalar) {
        *self = Scalar::mul(self, &rhs);
    }
}

impl MulAssign<&Scalar> for Scalar {
    fn mul_assign(&mut self, rhs: &Scalar) {
        *self = Scalar::mul(self, rhs);
    }
}

#[cfg(feature = "bits")]
#[cfg_attr(docsrs, doc(cfg(feature = "bits")))]
impl From<&Scalar> for ScalarBits {
    fn from(scalar: &Scalar) -> ScalarBits {
        scalar.0.into()
    }
}

impl From<Scalar> for FieldBytes {
    fn from(scalar: Scalar) -> Self {
        scalar.to_bytes()
    }
}

impl From<&Scalar> for FieldBytes {
    fn from(scalar: &Scalar) -> Self {
        scalar.to_bytes()
    }
}

#[cfg(feature = "zeroize")]
impl Zeroize for Scalar {
    fn zeroize(&mut self) {
        self.0.zeroize()
    }
}

#[cfg(test)]
mod tests {
    use super::Scalar;
    use crate::arithmetic::util::{biguint_to_bytes, bytes_to_biguint};
    use num_bigint::{BigUint, ToBigUint};
    use proptest::prelude::*;

    impl From<&BigUint> for Scalar {
        fn from(x: &BigUint) -> Self {
            debug_assert!(x < &Scalar::modulus_as_biguint());
            let bytes = biguint_to_bytes(x);
            Self::from_repr(bytes.into()).unwrap()
        }
    }

    impl From<BigUint> for Scalar {
        fn from(x: BigUint) -> Self {
            Self::from(&x)
        }
    }

    impl ToBigUint for Scalar {
        fn to_biguint(&self) -> Option<BigUint> {
            Some(bytes_to_biguint(self.to_bytes().as_ref()))
        }
    }

    #[test]
    fn is_high() {
        // 0 is not high
        let high: bool = Scalar::zero().is_high().into();
        assert!(!high);

        // 1 is not high
        let one = 1.to_biguint().unwrap();
        let high: bool = Scalar::from(&one).is_high().into();
        assert!(!high);

        let m = Scalar::modulus_as_biguint();
        let m_by_2 = &m >> 1;

        // M / 2 is not high
        let high: bool = Scalar::from(&m_by_2).is_high().into();
        assert!(!high);

        // M / 2 + 1 is high
        let high: bool = Scalar::from(&m_by_2 + &one).is_high().into();
        assert!(high);

        // MODULUS - 1 is high
        let high: bool = Scalar::from(&m - &one).is_high().into();
        assert!(high);
    }

    #[test]
    fn negate() {
        let zero_neg = -Scalar::zero();
        assert_eq!(zero_neg, Scalar::zero());

        let m = Scalar::modulus_as_biguint();
        let one = 1.to_biguint().unwrap();
        let m_minus_one = &m - &one;
        let m_by_2 = &m >> 1;

        let one_neg = -Scalar::one();
        assert_eq!(one_neg, Scalar::from(&m_minus_one));

        let frac_modulus_2_neg = -Scalar::from(&m_by_2);
        let frac_modulus_2_plus_one = Scalar::from(&m_by_2 + &one);
        assert_eq!(frac_modulus_2_neg, frac_modulus_2_plus_one);

        let modulus_minus_one_neg = -Scalar::from(&m - &one);
        assert_eq!(modulus_minus_one_neg, Scalar::one());
    }

    #[test]
    fn add_result_within_256_bits() {
        // A regression for a bug where reduction was not applied
        // when the unreduced result of addition was in the range `[modulus, 2^256)`.
        let t = 1.to_biguint().unwrap() << 255;
        let one = 1.to_biguint().unwrap();

        let a = Scalar::from(&t - &one);
        let b = Scalar::from(&t);
        let res = &a + &b;

        let m = Scalar::modulus_as_biguint();
        let res_ref = Scalar::from((&t + &t - &one) % &m);

        assert_eq!(res, res_ref);
    }


    prop_compose! {
        fn scalar()(bytes in any::<[u8; 32]>()) -> Scalar {
            let mut res = bytes_to_biguint(&bytes);
            let m = Scalar::modulus_as_biguint();
            // Modulus is 256 bit long, same as the maximum `res`,
            // so this is guaranteed to land us in the correct range.
            if res >= m {
                res -= m;
            }
            Scalar::from(&res)
        }
    }

    proptest! {
        #[test]
        fn fuzzy_roundtrip_to_bytes(a in scalar()) {
            let a_back = Scalar::from_repr(a.to_bytes()).unwrap();
            assert_eq!(a, a_back);
        }

        #[test]
        fn fuzzy_roundtrip_to_bytes_unchecked(a in scalar()) {
            let bytes = a.to_bytes();
            let a_back = Scalar::from_bytes_unchecked(bytes.as_ref());
            assert_eq!(a, a_back);
        }

        #[test]
        fn fuzzy_add(a in scalar(), b in scalar()) {
            let a_bi = a.to_biguint().unwrap();
            let b_bi = b.to_biguint().unwrap();

            let res_bi = (&a_bi + &b_bi) % &Scalar::modulus_as_biguint();
            let res_ref = Scalar::from(&res_bi);
            let res_test = a.add(&b);

            assert_eq!(res_ref, res_test);
        }

        #[test]
        fn fuzzy_sub(a in scalar(), b in scalar()) {
            let a_bi = a.to_biguint().unwrap();
            let b_bi = b.to_biguint().unwrap();

            let m = Scalar::modulus_as_biguint();
            let res_bi = (&m + &a_bi - &b_bi) % &m;
            let res_ref = Scalar::from(&res_bi);
            let res_test = a.sub(&b);

            assert_eq!(res_ref, res_test);
        }

        #[test]
        fn fuzzy_neg(a in scalar()) {
            let a_bi = a.to_biguint().unwrap();

            let m = Scalar::modulus_as_biguint();
            let res_bi = (&m - &a_bi) % &m;
            let res_ref = Scalar::from(&res_bi);
            let res_test = -a;

            assert_eq!(res_ref, res_test);
        }

        #[test]
        fn fuzzy_mul(a in scalar(), b in scalar()) {
            let a_bi = a.to_biguint().unwrap();
            let b_bi = b.to_biguint().unwrap();

            let res_bi = (&a_bi * &b_bi) % &Scalar::modulus_as_biguint();
            let res_ref = Scalar::from(&res_bi);
            let res_test = a.mul(&b);

            assert_eq!(res_ref, res_test);
        }

        #[test]
        fn fuzzy_rshift(a in scalar(), b in 0usize..512) {
            let a_bi = a.to_biguint().unwrap();

            let res_bi = &a_bi >> b;
            let res_ref = Scalar::from(&res_bi);
            let res_test = a >> b;

            assert_eq!(res_ref, res_test);
        }

        #[test]
        fn fuzzy_invert(
            a in scalar()
        ) {
            let a = if bool::from(a.is_zero()) { Scalar::one() } else { a };
            let a_bi = a.to_biguint().unwrap();
            let inv = a.invert().unwrap();
            let inv_bi = inv.to_biguint().unwrap();
            let m = Scalar::modulus_as_biguint();
            assert_eq!((&inv_bi * &a_bi) % &m, 1.to_biguint().unwrap());
        }
    }
}
