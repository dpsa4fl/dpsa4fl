use crate::core::types::CommonStateParametrization;
use crate::janus_manager::interface::network::consumer::JanusTasksClient;
use crate::janus_manager::interface::types::TrainingSessionId;

use janus_messages::TaskId;

/////////////////////////////////////////////////////////////////////////
// DPSA Controller
//
// What the controller does is:
//  - Once at startup: talk with `dpsa4fl-janus-tasks` and get hpke_configs
//    from both helper and leader
//
//  - Each round: talk with `dpsa4fl-janus-tasks` to setup new aggregation tasks,
//    then begin training and send the task_id to the clients. When clients return
//    their Ok, start collection job.
//

////////////////////////////////////////////////////
// State

/// State that is preserved between rounds.
pub struct ControllerStatePermanent
{
    pub janus_tasks_client: JanusTasksClient,
}

/// State that is required only for a single round.
#[derive(Clone)]
pub struct ControllerStateRound
{
    pub task_id: Option<TaskId>,
    pub training_session_id: Option<TrainingSessionId>,
}

/// State that does not change once the controller is initialized.
pub struct ControllerStateImmut
{
    pub parametrization: CommonStateParametrization,
    pub permanent: ControllerStatePermanent,
}

/// State that changes during the controller lifetime.
pub struct ControllerStateMut
{
    pub round: ControllerStateRound,
}

////////////////////////////////////////////////////
// Implementation
impl ControllerStateImmut
{
    pub fn new(p: CommonStateParametrization) -> Self
    {
        // janus tasks
        let janus_tasks_client =
            JanusTasksClient::new(p.location.clone(), p.vdaf_parameter.clone());

        let permanent = ControllerStatePermanent { janus_tasks_client };

        ControllerStateImmut {
            parametrization: p,
            permanent,
        }
    }
}
