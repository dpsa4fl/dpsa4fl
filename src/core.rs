pub use janus_aggregator::dpsa4fl::core::Locations;
use prio::flp::types::fixedpoint_l2::NoiseParameterType;

////////////////////////////////////////////////////
// State

#[derive(Clone)]
pub struct CommonState_Parametrization {
    pub location: Locations,
    pub gradient_len: usize,
    pub noise_parameter: NoiseParameterType,
}
