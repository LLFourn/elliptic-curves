//! Affine points

use super::{FieldElement, ProjectivePoint, CURVE_EQUATION_B};
use crate::{FieldBytes, Scalar};
use core::ops::{Mul, Neg};
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

#[cfg(feature = "zeroize")]
use elliptic_curve::zeroize::Zeroize;

/// A point on the secp256k1 curve in affine coordinates.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
#[allow(missing_docs)]
pub struct AffinePoint {
    pub x: FieldElement,
    pub y: FieldElement,
    pub infinity: Choice,
}

impl AffinePoint {
    /// Returns the identity of the group: the point at infinity.
    pub fn identity() -> Self {
        Self {
            x: FieldElement::zero(),
            y: FieldElement::zero(),
            infinity: Choice::from(1),
        }
    }

    // /// Returns the base point of SECP256k1.
    // fn generator() -> Self {
    //     // SECP256k1 basepoint in affine coordinates:
    //     // x = 79be667e f9dcbbac 55a06295 ce870b07 029bfcdb 2dce28d9 59f2815b 16f81798
    //     // y = 483ada77 26a3c465 5da4fbfc 0e1108a8 fd17b448 a6855419 9c47d08f fb10d4b8
    //     AffinePoint {
    //         x: FieldElement::from_bytes(&arr![u8;
    //             0x79, 0xbe, 0x66, 0x7e, 0xf9, 0xdc, 0xbb, 0xac, 0x55, 0xa0, 0x62, 0x95, 0xce, 0x87,
    //             0x0b, 0x07, 0x02, 0x9b, 0xfc, 0xdb, 0x2d, 0xce, 0x28, 0xd9, 0x59, 0xf2, 0x81, 0x5b,
    //             0x16, 0xf8, 0x17, 0x98
    //         ])
    //         .unwrap(),
    //         y: FieldElement::from_bytes(&arr![u8;
    //             0x48, 0x3a, 0xda, 0x77, 0x26, 0xa3, 0xc4, 0x65, 0x5d, 0xa4, 0xfb, 0xfc, 0x0e, 0x11,
    //             0x08, 0xa8, 0xfd, 0x17, 0xb4, 0x48, 0xa6, 0x85, 0x54, 0x19, 0x9c, 0x47, 0xd0, 0x8f,
    //             0xfb, 0x10, 0xd4, 0xb8
    //         ])
    //         .unwrap(),
    //         infinity: Choice::from(0),
    //     }
    // }

    /// Is this point the identity point?
    pub fn is_identity(&self) -> Choice {
        self.infinity
    }

    /// Convert to curve representation.
    pub fn to_curve(&self) -> ProjectivePoint {
        ProjectivePoint::from(*self)
    }
}

impl ConditionallySelectable for AffinePoint {
    fn conditional_select(a: &AffinePoint, b: &AffinePoint, choice: Choice) -> AffinePoint {
        AffinePoint {
            x: FieldElement::conditional_select(&a.x, &b.x, choice),
            y: FieldElement::conditional_select(&a.y, &b.y, choice),
            infinity: Choice::conditional_select(&a.infinity, &b.infinity, choice),
        }
    }
}

impl ConstantTimeEq for AffinePoint {
    fn ct_eq(&self, other: &AffinePoint) -> Choice {
        (self.x.negate(1) + &other.x).normalizes_to_zero()
            & (self.y.negate(1) + &other.y).normalizes_to_zero()
            & self.infinity.ct_eq(&other.infinity)
    }
}

impl Default for AffinePoint {
    fn default() -> Self {
        Self::identity()
    }
}

impl PartialEq for AffinePoint {
    fn eq(&self, other: &AffinePoint) -> bool {
        self.ct_eq(other).into()
    }
}

impl Eq for AffinePoint {}

impl AffinePoint {
    /// decompress point from x coordinate and oddness of y
    pub fn decompress(x_bytes: &FieldBytes, y_is_odd: Choice) -> CtOption<Self> {
        FieldElement::from_bytes(x_bytes).and_then(|x| {
            let alpha = (x * &x * &x) + &CURVE_EQUATION_B;
            let beta = alpha.sqrt();

            beta.map(|beta| {
                let y = FieldElement::conditional_select(
                    &beta.negate(1),
                    &beta,
                    beta.normalize().is_odd().ct_eq(&y_is_odd),
                );

                Self {
                    x,
                    y: y.normalize(),
                    infinity: Choice::from(0),
                }
            })
        })
    }
}

impl Mul<Scalar> for AffinePoint {
    type Output = ProjectivePoint;

    fn mul(self, scalar: Scalar) -> ProjectivePoint {
        ProjectivePoint::from(self) * scalar
    }
}

impl Mul<&Scalar> for AffinePoint {
    type Output = ProjectivePoint;

    fn mul(self, scalar: &Scalar) -> ProjectivePoint {
        ProjectivePoint::from(self) * scalar
    }
}

impl Neg for AffinePoint {
    type Output = AffinePoint;

    fn neg(self) -> Self::Output {
        AffinePoint {
            x: self.x,
            y: self.y.negate(1).normalize_weak(),
            infinity: self.infinity,
        }
    }
}

#[cfg(feature = "zeroize")]
impl Zeroize for AffinePoint {
    fn zeroize(&mut self) {
        self.x.zeroize();
        self.y.zeroize();
    }
}
