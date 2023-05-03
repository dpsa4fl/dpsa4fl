
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

use prio::flp::types::fixedpoint_l2::PrivacyParameterType;
use reqwest::Url;
use serde::{Deserialize, Serialize};

use super::fixed::FixedTypeTag;

#[derive(Clone)]
pub struct Locations
{
    pub main: MainLocations,
    pub tasks: TasksLocations,
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
pub struct TasksLocations
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

