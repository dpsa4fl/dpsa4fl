
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

const TIME_PRECISION: u64 = 3600;

/////////////////////////////////////////////////////////////////////////
// DPSA Client
//
// A note on terminology:
// 1. parametrization is the data initially given to the client out of band.
// 2. (permanent) state is state that is needed, but should not
//     really change during the runtime of the client.
// 3. RoundSettings we get from the controller.
// 4. Using the RoundSettings, we can get the RoundConfig from the aggregators.
//    The RoundConfig allows us to submit our results to the aggregators.
// 5. RoundData we also get from the controller.

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

////////////////////////////////////////////////////
// Config

/// Stores the hpke keys of both aggregators required for submitting gradients.
#[derive(Clone)]
pub struct CryptoConfig
{
    leader_hpke_config: HpkeConfig,
    helper_hpke_config: HpkeConfig,
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
    http_client: reqwest::Client,
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

/////////////////////////////////////////////////////////////////////////
// Step 1, initialization
//
// This is done at the beginning, and also if the controller notifies us
// that the hpke configs of the aggregators changed (which should not happen).
//
// We get the HPKE Configurations of both leader and helper by asking them
// directly. See 4.3.1 of ietf-ppm-dap.
//

async fn get_crypto_config(
    permanent: &ClientStatePermanent,
    task_id: TaskId,
    l: Locations,
) -> anyhow::Result<CryptoConfig>
{
    let c: ClientParameters = ClientParameters::new(
        task_id,
        l.get_external_aggregator_endpoints(),
        Duration::from_seconds(1),
    );
    let f_leader = aggregator_hpke_config(&c, &Role::Leader, &task_id, &permanent.http_client);
    let f_helper = aggregator_hpke_config(&c, &Role::Helper, &task_id, &permanent.http_client);

    let (l, h) = try_join!(f_leader, f_helper).await?;

    Ok(CryptoConfig {
        leader_hpke_config: l,
        helper_hpke_config: h,
    })
}

async fn get_parametrization(task_id: TaskId, l: Locations) -> Result<CommonStateParametrization>
{
    let leader_param =
        get_vdaf_parameter_from_task(l.tasks.external_leader.clone(), task_id).await?;
    let helper_param =
        get_vdaf_parameter_from_task(l.tasks.external_helper.clone(), task_id).await?;

    // make sure that the information matchs
    //
    // TODO: For the noise parameter we COULD take the minimum instead
    if leader_param == helper_param
    {
        Ok(CommonStateParametrization {
            location: l,
            vdaf_parameter: leader_param,
        })
    }
    else
    {
        Err(anyhow!("The leader and helper have different vdaf params:\nleader:\n{leader_param:?}\nhelper:\n{helper_param:?}"))
    }
}

//
// Functions that take client state as parameter.
//
impl ClientState
{
    pub async fn new(task_locations: TasksLocations, round_settings: RoundSettings)
        -> anyhow::Result<ClientState>
    {
        let permanent = ClientStatePermanent {
            http_client: default_http_client()?,
        };

        // we get the main locations from the tasks servers
        let main_locations = get_main_locations(task_locations.clone()).await?;

        let locations = Locations {
            main: main_locations,
            tasks: task_locations,
        };

        // we get a new crypto config if we were asked for it
        let c = get_crypto_config(&permanent, round_settings.task_id, locations.clone()).await?;

        // get a parametrization from locations
        let parametrization: CommonStateParametrization =
            get_parametrization(round_settings.task_id, locations.clone()).await?;

        Ok(ClientState {
            parametrization,
            permanent,
            round: ClientStateRound {
                config: RoundConfig {
                    settings: round_settings,
                    crypto: c,
                },
            },
        })
    }

    //
    // Generate a round config for the next round.
    //
    // This means that we either copy most of the configuration from the previous round
    // which exists in the state, or if necessary, get a new configuration from the
    // aggregators.
    //
    async fn get_next_round_config(
        &self,
        round_settings: RoundSettings,
    ) -> anyhow::Result<RoundConfig>
    {
        // NOTE: We assume that the vdaf parameters don't change between tasks of the same session
        //       If they could, we would have to get the current vdaf parameters here.

        if round_settings.should_request_hpke_config
        {
            // we get a new crypto config if we were asked for it
            let c = get_crypto_config(
                &self.permanent,
                round_settings.task_id,
                self.parametrization.location.clone(),
            )
            .await?;
            Ok(RoundConfig {
                settings: round_settings,
                crypto: c,
            })
        }
        else
        {
            // Copy the old config from the state to the new config.
            // Just change the task_id to the new one from the round_settings.
            Ok(RoundConfig {
                settings: round_settings,
                crypto: self.round.config.crypto.clone(),
            })
        }
    }

    pub async fn update_to_next_round_config(
        &mut self,
        round_settings: RoundSettings,
    ) -> anyhow::Result<()>
    {
        self.round.config = self.get_next_round_config(round_settings).await?;
        Ok(())
    }

    ///////////////////////////////////////
    // Submission
    pub async fn get_submission_result(&self, measurement: &VecFixedAny) -> anyhow::Result<()>
    {
        match measurement
        {
            VecFixedAny::VecFixed16(v) => self.get_submission_result_impl(v).await,
            VecFixedAny::VecFixed32(v) => self.get_submission_result_impl(v).await,
            VecFixedAny::VecFixed64(v) => self.get_submission_result_impl(v).await,
        }
    }

    async fn get_submission_result_impl<Fx: Fixed>(
        &self,
        measurement: &Vec<Fx>,
    ) -> anyhow::Result<()>
    where
        Fx: CompatibleFloat<Field128>,
        Fx: IsTagInstance<FixedTypeTag>,
        // Fx: FixedBase,
    {
        ////////////////////////
        // check length
        let actual_len = measurement.len();
        let expected_len = self.parametrization.vdaf_parameter.gradient_len;
        if actual_len != expected_len
        {
            return Err(anyhow!(
                "Expected data to be have length {expected_len} but it was {actual_len}"
            ));
        }

        ////////////////////////
        // check type
        let aggregator_tag = &self.parametrization.vdaf_parameter.submission_type;

        // assert that the compile time type `Fx` matches with the type tag for this round
        if &Fx::get_tag() != aggregator_tag
        {
            return Err(anyhow!("Tried to submit gradient with fixed type {:?}, but the task has been registered for fixed type {:?}", Fx::get_tag(), aggregator_tag));
        }

        // create vdaf instance
        let num_aggregators = 2;
        let len = self.parametrization.vdaf_parameter.gradient_len;
        let privacy_parameter = self.parametrization.vdaf_parameter.privacy_parameter;
        let vdaf_client: Prio3Aes128FixedPointBoundedL2VecSum<Fx> =
            Prio3Aes128FixedPointBoundedL2VecSum::new_aes128_fixedpoint_boundedl2_vec_sum(
                num_aggregators,
                len,
                privacy_parameter, // actually this does not matter for the client
            )?;

        let parameters = ClientParameters::new(
            self.round.config.settings.task_id,
            self.parametrization
                .location
                .get_external_aggregator_endpoints(),
            self.round.config.settings.time_precision,
        );

        let client = Client::new(
            parameters,
            vdaf_client,
            RealClock::default(),
            &self.permanent.http_client,
            self.round.config.crypto.leader_hpke_config.clone(),
            self.round.config.crypto.helper_hpke_config.clone(),
        );

        let () = client.upload(measurement).await?;

        Ok(())
    }
}
