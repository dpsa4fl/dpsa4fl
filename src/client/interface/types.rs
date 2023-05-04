use anyhow::Result;
use janus_messages::{Duration, HpkeConfig, TaskId};

use crate::{
    core::{
        helpers::task_id_from_string,
        types::{CommonStateParametrization, TasksLocations},
    },
    janus_manager::interface::network::consumer::TIME_PRECISION,
};

////////////////////////////////////////////////////
// Config

/// Stores the hpke keys of both aggregators required for submitting gradients.
#[derive(Clone)]
pub struct CryptoConfig
{
    pub leader_hpke_config: HpkeConfig,
    pub helper_hpke_config: HpkeConfig,
}

/// Full configuration for a round, consisting of settings and crypto config.
#[derive(Clone)]
pub struct RoundConfig
{
    pub settings: RoundSettings,
    pub crypto: CryptoConfig,
}

////////////////////////////////////////////////////
// State

/// State which persists from round to round.
pub struct ClientStatePermanent
{
    pub http_client: reqwest::Client,
}

/// State relevant for a single round.
#[derive(Clone)]
pub struct ClientStateRound
{
    pub config: RoundConfig,
}

/// All client state.
pub struct ClientState
{
    pub parametrization: CommonStateParametrization,
    pub permanent: ClientStatePermanent,
    pub round: ClientStateRound,
}

/// Client state which is possibly uninitialized until now.
pub enum ClientStatePU
{
    ValidState(ClientState),
    InitState(TasksLocations),
}

impl ClientStatePU
{
    /// Return the initialized client state if available, otherwise fail.
    pub fn get_valid_state(&self) -> Option<&ClientState>
    {
        match self
        {
            ClientStatePU::ValidState(s) => Some(s),
            ClientStatePU::InitState(_) => None,
        }
    }
}

////////////////////////////////////////////////////
// Settings

/// Settings for a single round.
///
/// The `task_id` identifies the task
/// on both aggregators where the gradient is going to be submitted.
#[derive(Clone)]
pub struct RoundSettings
{
    pub task_id: TaskId,
    pub time_precision: Duration,
    pub should_request_hpke_config: bool,
}

impl RoundSettings
{
    /// Create a default round settings from a task id.
    pub fn new(task_id_base64: String) -> Result<Self>
    {
        let res = RoundSettings {
            task_id: task_id_from_string(task_id_base64)?,
            time_precision: Duration::from_seconds(TIME_PRECISION),
            should_request_hpke_config: false,
        };
        Ok(res)
    }
}
