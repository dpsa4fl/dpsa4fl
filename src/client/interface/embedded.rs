use crate::core::fixed::VecFixedAny;
use crate::core::types::CommonStateParametrization;
use crate::core::types::TasksLocations;

use super::types::ClientState;
use super::types::ClientStatePU;
use super::types::RoundSettings;

use anyhow::{anyhow, Result};

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
