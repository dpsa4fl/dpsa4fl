pub use dpsa4fl_janus_tasks::core::Locations;
use fixed::{types::extra::U31, FixedI32};

pub use janus_aggregator::dpsa4fl::core::Locations;
use janus_client::{ClientParameters, aggregator_hpke_config, default_http_client, Client};
use janus_core::{time::RealClock};
use janus_messages::{HpkeConfig, Role, TaskId, Duration};
use prio::flp::types::fixedpoint_l2::NoiseParameterType;
use url::*;
// use anyhow::Result;
use async_std::future::try_join;
use prio::vdaf::prio3::Prio3Aes128FixedPointBoundedL2VecSum;

use fixed::types::extra::{U15, U31, U63};
use fixed::{FixedI16, FixedI32, FixedI64};


///////////////////////////////////////////////////
// Type names

pub type Fixed16 = FixedI16<U15>;
pub type Fixed32 = FixedI32<U31>;
pub type Fixed64 = FixedI64<U63>;

pub type NoiseParameterType = Fixed32;


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



////////////////////////////////////////////////////
// State

#[derive(Clone)]
pub struct CommonState_Parametrization {
    pub location: Locations,
    pub gradient_len: usize,
    pub noise_parameter: NoiseParameterType,
    pub submission_type: FixedTypeTag,
}
