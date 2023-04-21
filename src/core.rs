pub use dpsa4fl_janus_tasks::core::Locations;
pub use dpsa4fl_janus_tasks::fixed::FixedAny;

use dpsa4fl_janus_tasks::core::VdafParameter;

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

