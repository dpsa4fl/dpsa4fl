use crate::core::CommonState_Parametrization;
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

pub struct ControllerState_Permanent {
    // http_client: reqwest::Client,
    janus_tasks_client: JanusTasksClient,
}

#[derive(Clone)]
pub struct ControllerState_Round {
    // config: RoundConfig,
    pub task_id: Option<TaskId>,
    pub training_session_id: Option<TrainingSessionId>,
}

pub struct ControllerState_Immut {
    pub parametrization: CommonState_Parametrization,
    pub permanent: ControllerState_Permanent,
}

pub struct ControllerState_Mut {
    pub round: ControllerState_Round,
}

////////////////////////////////////////////////////
// Implementation
impl ControllerState_Immut {
    pub fn new(p: CommonState_Parametrization) -> Self {
        // janus tasks
        let janus_tasks_client =
            JanusTasksClient::new(p.location.clone(), p.gradient_len, p.noise_parameter.clone());

        let permanent = ControllerState_Permanent { janus_tasks_client };

        // let round = ControllerState_Round {
        //     training_session_id: None,
        //     task_id: None,
        // };

        ControllerState_Immut {
            parametrization: p,
            permanent,
        }
    }
}

/////////////////////////////////////////////////////////////////////////
// api

pub fn api__new_controller_state(p: CommonState_Parametrization) -> ControllerState_Immut {
    ControllerState_Immut::new(p)
}

pub async fn api__create_session(
    istate: &ControllerState_Immut,
    mstate: &mut ControllerState_Mut,
) -> Result<u16> {
    let training_session_id = istate.permanent.janus_tasks_client.create_session().await?;

    // set our current training session id
    mstate.round.training_session_id = Some(training_session_id);

    Ok(training_session_id.into())
}

pub async fn api__start_round(
    istate: &ControllerState_Immut,
    mstate: &mut ControllerState_Mut,
) -> Result<String> {
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

pub async fn api__collect(
    istate: &ControllerState_Immut,
    mstate: &mut ControllerState_Mut,
) -> Result<Collection<Vec<f64>, TimeInterval>> {
    let task_id = mstate
        .round
        .task_id
        .ok_or(anyhow!("Cannot collect because no task_id available."))?;
    let result = istate.permanent.janus_tasks_client.collect(task_id).await?;

    Ok(result)
}
