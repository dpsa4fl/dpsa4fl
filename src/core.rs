pub use dpsa4fl_janus_tasks::core::Locations;
pub use dpsa4fl_janus_tasks::fixed::FixedAny;

use dpsa4fl_janus_tasks::core::VdafParameter;

pub type NoiseParameterType = FixedAny;

////////////////////////////////////////////////////
// State

#[derive(Clone)]
pub struct CommonState_Parametrization
{
    pub location: Locations,
    pub vdaf_parameter: VdafParameter,
}

