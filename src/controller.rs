
// use janus_client::{ClientParameters, aggregator_hpke_config, default_http_client, Client};
use janus_core::{time::RealClock};
use janus_messages::{HpkeConfig, Role, TaskId, Duration};
use janus_aggregator::dpsa4fl::janus_tasks_client::JanusTasksClient;
use url::*;
use anyhow::Result;
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
    janus_tasks_clients: Vec<JanusTasksClient>,

}

#[derive(Clone)]
pub struct ControllerState_Round
{
    // config: RoundConfig,
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
        let leader_client = JanusTasksClient::new(p.location.leader.clone());
        let helper_client = JanusTasksClient::new(p.location.helper.clone());

        let permanent = ControllerState_Permanent {
            janus_tasks_clients: vec![leader_client, helper_client]
        };

        let round = ControllerState_Round {};

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


pub async fn api__create_session(s: &ControllerState, id: u16) -> Result<()>
{
    for client in &s.permanent.janus_tasks_clients
    {
        client.create_session(id.into()).await?
    }
    Ok(())
}




