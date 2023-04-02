pub use dpsa4fl_janus_tasks::core::Locations;
use dpsa4fl_janus_tasks::core::VdafParameter;
pub use dpsa4fl_janus_tasks::fixed::FixedAny;
use dpsa4fl_janus_tasks::fixed::FixedTypeTag;
// use fixed::{types::extra::U31, FixedI32};

// use janus_client::{ClientParameters, aggregator_hpke_config, default_http_client, Client};
// use janus_core::{time::RealClock};
// use janus_messages::{HpkeConfig, Role, TaskId, Duration};
// // use prio::flp::types::fixedpoint_l2::NoiseParameterType;
// use url::*;
// // use anyhow::Result;
// use async_std::future::try_join;
// use prio::vdaf::prio3::Prio3Aes128FixedPointBoundedL2VecSum;

// use fixed::types::extra::{U15, U31, U63};
// use fixed::{FixedI16, FixedI32, FixedI64};

// use downcast_rs::DowncastSync;
// use dyn_clone::DynClone;

/*

// To create a trait with downcasting methods, extend `Downcast` or `DowncastSync`
// and run `impl_downcast!()` on the trait.
pub trait FixedBase: DowncastSync + DynClone {}
impl_downcast!(sync FixedBase);  // `sync` => also produce `Arc` downcasts.

// implements `Clone` for FixedBase, based on the `DynClone` super trait
dyn_clone::clone_trait_object!(FixedBase);


///////////////////////////////////////////////////
// Type names

pub type Fixed16 = FixedI16<U15>;
pub type Fixed32 = FixedI32<U31>;
pub type Fixed64 = FixedI64<U63>;

*/

pub type NoiseParameterType = FixedAny;

/*


///////////////////////////////////////////////////
// Type dispatch

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedTypeTag
{
    FixedType16Bit,
    FixedType32Bit,
    FixedType64Bit,
}


pub trait IsTagInstance<Tag>
{
    fn get_tag() -> Tag;
}

// instances
impl IsTagInstance<FixedTypeTag> for Fixed16
{
    fn get_tag() -> FixedTypeTag {
        FixedTypeTag::FixedType16Bit
    }
}
impl IsTagInstance<FixedTypeTag> for Fixed32
{
    fn get_tag() -> FixedTypeTag {
        FixedTypeTag::FixedType32Bit
    }
}
impl IsTagInstance<FixedTypeTag> for Fixed64
{
    fn get_tag() -> FixedTypeTag {
        FixedTypeTag::FixedType64Bit
    }
}

// casting
impl FixedBase for Fixed16 {}
impl FixedBase for Fixed32 {}
impl FixedBase for Fixed64 {}

*/



////////////////////////////////////////////////////
// State

#[derive(Clone)]
pub struct CommonState_Parametrization {
    pub location: Locations,
    pub vdaf_parameter: VdafParameter,
    // pub submission_type: FixedTypeTag,
}
