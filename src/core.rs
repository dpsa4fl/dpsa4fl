pub use dpsa4fl_janus_tasks::core::Locations;
use fixed::{types::extra::U31, FixedI32};

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
