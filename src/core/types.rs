////////////////////////////////////////////////////
// State

/// The parameters for a training session, to be known
/// by both controller and clients.
#[derive(Clone)]
pub struct CommonStateParametrization
{
    pub location: Locations,
    pub vdaf_parameter: VdafParameter,
}

/////////////////////////////
// Locations

use janus_core::{
    dp::NoDifferentialPrivacy,
    task::{Prio3FixedPointBoundedL2VecSumBitSize, VdafInstance},
};
use prio::dp::{
    distributions::ZCdpDiscreteGaussian, DifferentialPrivacyStrategy, Rational, ZCdpBudget,
};
use reqwest::Url;
use serde::{Deserialize, Serialize};

use super::fixed::FixedTypeTag;

pub type PrivacyParameterType = ZCdpBudget;

#[derive(Clone)]
pub struct Locations
{
    pub main: MainLocations,
    pub manager: ManagerLocations,
}

impl Locations
{
    pub fn get_external_aggregator_endpoints(&self) -> Vec<Url>
    {
        vec![
            self.main.external_leader.clone(),
            self.main.external_helper.clone(),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagerLocations
{
    pub external_leader: Url,
    pub external_helper: Url,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainLocations
{
    pub external_leader: Url,
    pub external_helper: Url,
}

/////////////////////////////
// VDAF Parametrization

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VdafParameter
{
    pub gradient_len: usize,

    pub privacy_parameter: PrivacyParameterType,

    pub submission_type: FixedTypeTag,
}

impl VdafParameter
{
    pub fn to_vdaf_instance(&self) -> VdafInstance
    {
        let bitsize = match self.submission_type
        {
            FixedTypeTag::FixedType16Bit => Prio3FixedPointBoundedL2VecSumBitSize::BitSize16,
            FixedTypeTag::FixedType32Bit => Prio3FixedPointBoundedL2VecSumBitSize::BitSize32,
            FixedTypeTag::FixedType64Bit => Prio3FixedPointBoundedL2VecSumBitSize::BitSize64,
        };

        VdafInstance::Prio3FixedPointBoundedL2VecSum {
            length: self.gradient_len,
            bitsize,
            dp_strategy: janus_core::task::vdaf_instance_strategies::Prio3FixedPointBoundedL2VecSum::ZCdpDiscreteGaussian(ZCdpDiscreteGaussian::from_budget(self.privacy_parameter.clone()))
        }
    }
}
