use std::fmt::Debug;

use fixed::types::extra::{U15, U31, U63};
use fixed::{traits::Fixed, FixedI16, FixedI32, FixedI64};

use num_traits::NumCast;
use serde::{Deserialize, Serialize};

use anyhow::anyhow;
use anyhow::Result;

///////////////////////////////////////////////////
// Type names

pub type Fixed16 = FixedI16<U15>;
pub type Fixed32 = FixedI32<U31>;
pub type Fixed64 = FixedI64<U63>;

//////////////////////////////////////////////////
// Existentially quantified fixed type

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FixedAny {
    Fixed16(Fixed16),
    Fixed32(Fixed32),
    Fixed64(Fixed64),
}

pub enum VecFixedAny {
    VecFixed16(Vec<Fixed16>),
    VecFixed32(Vec<Fixed32>),
    VecFixed64(Vec<Fixed64>),
}

///////////////////////////////////////////////////
// Type tags

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FixedTypeTag {
    FixedType16Bit,
    FixedType32Bit,
    FixedType64Bit,
}

pub trait IsTagInstance<Tag> {
    fn get_tag() -> Tag;
}

// instances
impl IsTagInstance<FixedTypeTag> for Fixed16 {
    fn get_tag() -> FixedTypeTag {
        FixedTypeTag::FixedType16Bit
    }
}
impl IsTagInstance<FixedTypeTag> for Fixed32 {
    fn get_tag() -> FixedTypeTag {
        FixedTypeTag::FixedType32Bit
    }
}
impl IsTagInstance<FixedTypeTag> for Fixed64 {
    fn get_tag() -> FixedTypeTag {
        FixedTypeTag::FixedType64Bit
    }
}

//////////////////////////////////////////////////
// converting float to fixed

pub fn float_to_fixed_floor<Fl, Fx>(x: Fl) -> Result<Fx>
where
    Fl: num_traits::Float + Debug,
    Fx: Fixed,
    Fx::Bits: num_traits::NumCast,
{
    float_to_fixed_with(x, Fl::floor)
}

pub fn float_to_fixed_ceil<Fl, Fx>(x: Fl) -> Result<Fx>
where
    Fl: num_traits::Float + Debug,
    Fx: Fixed,
    Fx::Bits: num_traits::NumCast,
{
    float_to_fixed_with(x, Fl::ceil)
}

fn float_to_fixed_with<Fl, Fx, Fun>(x: Fl, f: Fun) -> Result<Fx>
where
    Fl: num_traits::Float + Debug,
    Fx: Fixed,
    Fx::Bits: num_traits::NumCast + Debug,
    Fun: FnOnce(Fl) -> Fl,
{
    // the number of bits of our fixed type representation
    let n = Fx::Signed::FRAC_NBITS + Fx::Signed::INT_NBITS;

    // a fixed number is ostensibly an integer i in the range [-2^(n-1)..2^(n-1)]
    // We do a manual conversion:
    // - the float `x` should be in the range [-1..1]
    // - we expand to the range [-2^(n-1)..2^(n-1)]
    let x = x * Fl::from(2u64.pow(n - 1)).ok_or(anyhow!(
        "Floating point cannot represent 2^(n-1). (This should not happen when using f32 or f64)"
    ))?;

    // - we apply the postprocessing function
    //   this could be floor or ceil, in order to remove the fractional part
    let x = f(x);

    // - convert to integer
    let x = <Fx::Bits as NumCast>::from(x).ok_or(anyhow!("Could not convert {x:?} to integer."))?;

    // - turn bitwise rep into fixed
    let bits: Fx = Fixed::from_bits(x);

    Ok(bits)
}

#[cfg(test)]
#[allow(clippy::excessive_precision)]
mod tests {
    use super::*;
    use fixed_macro::fixed;

    #[test]
    fn float_to_fixed_floor_test() {
        // left: 2^(-15)
        // right: 2^(-15)
        assert_eq!(
            float_to_fixed_floor::<f32, Fixed16>(0.000030517578125).unwrap(),
            fixed!(0.000030517578125: I1F15)
        );

        // left: 2^(-16)
        // right: 0
        assert_eq!(
            float_to_fixed_floor::<f32, Fixed16>(0.0000152587890625).unwrap(),
            fixed!(0.0: I1F15)
        );

        // left: 2^(-15) + 2^(-16) + 2^(-17) + 2^(-18)
        // right: 2^(-15)
        assert_eq!(
            float_to_fixed_floor::<f32, Fixed16>(0.000057220458984375).unwrap(),
            fixed!(0.000030517578125: I1F15)
        );

        // left: 2^(-31)
        // right: 2^(-31)
        assert_eq!(
            float_to_fixed_floor::<f32, Fixed32>(0.0000000004656612873077392578125).unwrap(),
            fixed!(0.0000000004656612873077392578125: I1F31)
        );

        assert_eq!(
            float_to_fixed_floor::<f32, Fixed32>(0.5).unwrap(),
            fixed!(0.5: I1F31)
        );
    }

    #[test]
    fn float_to_fixed_ceil_test() {
        // left: 2^(-15)
        // right: 2^(-15)
        assert_eq!(
            float_to_fixed_ceil::<f32, Fixed16>(0.000030517578125).unwrap(),
            fixed!(0.000030517578125: I1F15)
        );

        // left: 2^(-16)
        // right: 2^(-15)
        assert_eq!(
            float_to_fixed_ceil::<f32, Fixed16>(0.0000152587890625).unwrap(),
            fixed!(0.000030517578125: I1F15)
        );

        // left: 2^(-15) + 2^(-20)
        // right: 2^(-14)
        assert_eq!(
            float_to_fixed_ceil::<f32, Fixed16>(0.00003147125244140625).unwrap(),
            fixed!(0.00006103515625: I1F15)
        );
    }
}
