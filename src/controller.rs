use crate::core::CommonStateParametrization;
use anyhow::{anyhow, Result};

use dpsa4fl_janus_tasks::{core::TrainingSessionId, janus_tasks_client::JanusTasksClient};
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

pub struct ControllerStatePermanent
{
    // http_client: reqwest::Client,
    janus_tasks_client: JanusTasksClient,
}

#[derive(Clone)]
pub struct ControllerStateRound
{
    // config: RoundConfig,
    pub task_id: Option<TaskId>,
    pub training_session_id: Option<TrainingSessionId>,
}

pub struct ControllerStateImmut
{
    pub parametrization: CommonStateParametrization,
    pub permanent: ControllerStatePermanent,
}

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

        // let round = ControllerState_Round {
        //     training_session_id: None,
        //     task_id: None,
        // };

        ControllerStateImmut {
            parametrization: p,
            permanent,
        }
    }
}

/////////////////////////////////////////////////////////////////////////
// api

pub fn api_new_controller_state(p: CommonStateParametrization) -> ControllerStateImmut
{
    ControllerStateImmut::new(p)
}

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
