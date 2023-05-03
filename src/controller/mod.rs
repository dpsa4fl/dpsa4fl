use crate::core::types::CommonStateParametrization;
use crate::janus_manager::interface::network::consumer::JanusTasksClient;
use crate::janus_manager::interface::types::TrainingSessionId;
use anyhow::{anyhow, Result};

// use dpsa4fl_janus_tasks::{core::TrainingSessionId, janus_tasks_client::JanusTasksClient};
use janus_collector::Collection;
use janus_messages::query_type::TimeInterval;
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
    janus_tasks_client: JanusTasksClient,
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
    fn new(p: CommonStateParametrization) -> Self
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

/////////////////////////////////////////////////////////////////////////
// api

/// Create a new immutable controller state from a given set of parameters.
pub fn api_new_controller_state(p: CommonStateParametrization) -> ControllerStateImmut
{
    ControllerStateImmut::new(p)
}

/// Create a new training session.
///
/// Calls both janus-tasks instances (i.e., on both aggregators), and
/// requests the creation of a new session. The session id is returned.
pub async fn api_create_session(
    istate: &ControllerStateImmut,
    mstate: &mut ControllerStateMut,
) -> Result<u16>
{
    let training_session_id = istate.permanent.janus_tasks_client.create_session().await?;

    // set our current training session id
    mstate.round.training_session_id = Some(training_session_id);

    Ok(training_session_id.into())
}

/// Ends a training session.
///
/// Ends the current training session on both aggregators. If no session is active, fail.
pub async fn api_end_session(
    istate: &ControllerStateImmut,
    mstate: &mut ControllerStateMut,
) -> Result<()>
{
    if let Some(training_session_id) = mstate.round.training_session_id
    {
        istate
            .permanent
            .janus_tasks_client
            .end_session(training_session_id)
            .await?;

        // reset the current training session id
        mstate.round.training_session_id = None;

        Ok(())
    }
    else
    {
        Err(anyhow!("Tried to end a session, but none was started."))
    }
}

/// Start a new training round.
///
/// This requires an active training session. Returns the task id of the
/// tasks belonging to this training round.
pub async fn api_start_round(
    istate: &ControllerStateImmut,
    mstate: &mut ControllerStateMut,
) -> Result<String>
{
    let training_session_id = mstate.round.training_session_id.ok_or(anyhow!(
        "Cannot start round because no session was created."
    ))?;

    println!("Starting round for session id {training_session_id}.");
    let task_id = istate
        .permanent
        .janus_tasks_client
        .start_round(training_session_id)
        .await?;

    // set our current task id
    mstate.round.task_id = Some(task_id);

    Ok(task_id.to_string())
}

/// Collect aggregated gradients.
///
/// This calls the leader aggregator and requests the aggregated
/// gradient vector, associated to the currently active training round.
pub async fn api_collect(
    istate: &ControllerStateImmut,
    mstate: &mut ControllerStateMut,
) -> Result<Collection<Vec<f64>, TimeInterval>>
{
    let task_id = mstate
        .round
        .task_id
        .ok_or(anyhow!("Cannot collect because no task_id available."))?;
    let result = istate.permanent.janus_tasks_client.collect(task_id).await?;

    Ok(result)
}
