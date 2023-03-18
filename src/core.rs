use fixed::{types::extra::U31, FixedI32};
pub use janus_aggregator::dpsa4fl::core::Locations;

////////////////////////////////////////////////////
// Type Parametrization
//
// TODO: remove this, and integrate into runtime parametrization.
pub type Fx = FixedI32<U31>;
pub type Measurement = Vec<Fx>;

////////////////////////////////////////////////////
// State

#[derive(Clone)]
pub struct CommonState_Parametrization {
    pub location: Locations,
    pub gradient_len: usize,
    pub noise_parameter: Fx,
}
