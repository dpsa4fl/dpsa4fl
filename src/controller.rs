
use janus_aggregator::dpsa4fl::core::TrainingSessionId;
// use janus_client::{ClientParameters, aggregator_hpke_config, default_http_client, Client};
use janus_core::{time::RealClock};
use janus_messages::{HpkeConfig, Role, TaskId, Duration};
use janus_aggregator::dpsa4fl::janus_tasks_client::JanusTasksClient;
use url::*;
use anyhow::{anyhow, Context, Result, Error};
use async_std::future::try_join;
use prio::vdaf::prio3::Prio3Aes128FixedPointBoundedL2VecSum;

use fixed::types::extra::{U15, U31, U63};
use fixed::{FixedI16, FixedI32, FixedI64};

use crate::core::{CommonState_Parametrization, Locations};


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

pub struct ControllerState_Permanent
{
    // http_client: reqwest::Client,
    janus_tasks_client: JanusTasksClient,

}

#[derive(Clone)]
pub struct ControllerState_Round
{
    // config: RoundConfig,
    task_id: Option<TaskId>,
    training_session_id: Option<TrainingSessionId>,
}

pub struct ControllerState
{
    pub parametrization: CommonState_Parametrization,
    pub permanent: ControllerState_Permanent,
    pub round: ControllerState_Round,
}

////////////////////////////////////////////////////
// Implementation
impl ControllerState
{
    pub fn new(p: CommonState_Parametrization) -> Self
    {
        let janus_tasks_client = JanusTasksClient::new(
            p.location.external_leader.clone(),
            p.location.external_helper.clone(),
            p.location.internal_leader.clone(),
            p.location.internal_helper.clone(),
            p.gradient_len,
        );

        let permanent = ControllerState_Permanent {
            janus_tasks_client,
            training_session_id: None,
        };

        let round = ControllerState_Round {
            task_id: None,
        };

        ControllerState {
            parametrization: p,
            permanent,
            round,
        }
    }
}



/////////////////////////////////////////////////////////////////////////
// api

pub fn api__new_controller_state(p: CommonState_Parametrization) -> ControllerState
{
    ControllerState::new(p)
}


pub async fn api__create_session(s: &mut ControllerState) -> Result<u16>
{
    let training_session_id = s.permanent.janus_tasks_client.create_session().await?;

    // set our current training session id
    s.permanent.training_session_id = Some(training_session_id);

    Ok(training_session_id.into())
}

pub async fn api__start_round(s: &mut ControllerState) -> Result<String>
{
    let training_session_id = s.permanent.training_session_id.ok_or(anyhow!("Cannot start round because no session was created."))?;

    println!("Starting round for session id {training_session_id}.");
    let task_id = s.permanent.janus_tasks_client.start_round(training_session_id).await?;

    // set our current task id
    s.round.task_id = Some(task_id);

    Ok(task_id.to_string())
}


