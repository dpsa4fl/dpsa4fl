
use crate::client::implementation::{ClientStatePU, RoundSettings, ClientState};
use crate::core::fixed::{VecFixedAny, IsTagInstance, FixedTypeTag};
use crate::core::helpers::task_id_from_string;

use crate::core::types::CommonStateParametrization;
use crate::core::types::{TasksLocations, Locations};
use crate::janus_manager::interface::network::consumer::{get_vdaf_parameter_from_task, get_main_locations};

// use dpsa4fl_janus_tasks::fixed::{FixedTypeTag, IsTagInstance, VecFixedAny};
use fixed::traits::Fixed;
use anyhow::{anyhow, Result};
use async_std::future::try_join;
// use dpsa4fl_janus_tasks::core::{Locations, TasksLocations};
// use dpsa4fl_janus_tasks::janus_tasks_client::{get_vdaf_parameter_from_task, get_main_locations};
use janus_client::{aggregator_hpke_config, default_http_client, Client, ClientParameters};
use janus_core::time::RealClock;
use janus_messages::{Duration, HpkeConfig, Role, TaskId};
use prio::field::Field128;
use prio::flp::types::fixedpoint_l2::compatible_float::CompatibleFloat;
use prio::vdaf::prio3::Prio3Aes128FixedPointBoundedL2VecSum;

/////////////////////////////////////////////////////////////////////////
// The api to be called from python code.
//

/// Create a new client state.
///
/// Here, `p` contains the addresses of the aggregation
/// servers and the corresponding janus-tasks instances.
/// Note that the state does not contain all information required for submitting gradients,
/// as this information can only be gotten on a round-by-round basis, once the task id
/// for a given round is known.
pub fn api_new_client_state(p: TasksLocations) -> ClientStatePU
{
    ClientStatePU::InitState(p)
}

/// Configure the client state for a given round.
///
/// If neccessary, this function will request information such as hpke keys from the aggregators.
pub async fn api_update_client_round_settings(
    s: &mut ClientStatePU,
    round_settings: RoundSettings,
) -> Result<()>
{
    match s
    {
        ClientStatePU::InitState(ref parametrization) =>
        {
            let client_state = ClientState::new(parametrization.clone(), round_settings).await?;
            *s = ClientStatePU::ValidState(client_state);
        }
        ClientStatePU::ValidState(ref mut client_state) =>
        {
            client_state
                .update_to_next_round_config(round_settings)
                .await?;
        }
    };
    Ok(())
}

/// Submit gradients to the aggregators.
///
/// Given a client state `s`, round settings describing the current round, and a function `get_data`,
/// which provides the gradient, this function calls `get_data` with the current parameters and submits
/// the resulting gradient to the aggregators.
pub async fn api_submit_with<F: FnOnce(&CommonStateParametrization) -> VecFixedAny>(
    s: &mut ClientStatePU,
    round_settings: RoundSettings,
    get_data: F,
) -> anyhow::Result<()>
{
    api_update_client_round_settings(s, round_settings).await?;

    match s
    {
        ClientStatePU::InitState(_) =>
        {
            Err(anyhow!(""))?;
        }
        ClientStatePU::ValidState(ref mut client_state) =>
        {
            let data = get_data(&client_state.parametrization);
            client_state.get_submission_result(&data).await?;
        }
    };
    Ok(())
}
