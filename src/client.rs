// use crate::core::{CommonState_Parametrization, Fx, Measurement};
use crate::helpers::task_id_from_string;


use crate::core::{CommonState_Parametrization, VecFixedAny};
// use crate::helpers::task_id_from_string;

use dpsa4fl_janus_tasks::fixed::{FixedTypeTag, IsTagInstance, FixedBase};
use fixed::traits::Fixed;
// use janus_aggregator::dpsa4fl::core::Locations;
use janus_client::{ClientParameters, aggregator_hpke_config, default_http_client, Client};
use janus_core::{time::RealClock};
use janus_messages::{HpkeConfig, Role, TaskId, Duration};
use prio::field::Field128;
use prio::flp::types::fixedpoint_l2::compatible_float::CompatibleFloat;
use url::*;
use anyhow::{anyhow, Result};
use async_std::future::try_join;
use dpsa4fl_janus_tasks::core::Locations;
use dpsa4fl_janus_tasks::janus_tasks_client::get_vdaf_parameter_from_task;
// use janus_client::{aggregator_hpke_config, default_http_client, Client, ClientParameters};
// use janus_core::time::RealClock;
// use janus_messages::{Duration, HpkeConfig, Role, TaskId};
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
// Type Parametrization
//
// ToDo: remove this, and integrate into runtime parametrization.
// pub type Fx = FixedI32<U31>;
// pub type Measurement = Vec<Fx>;



////////////////////////////////////////////////////
// Settings

//
// The settings are transmitted from the controller to the clients.
// The clients generate a `RoundConfig` from them.
//
#[derive(Clone)]
pub struct RoundSettings {
    pub task_id: TaskId,
    pub time_precision: Duration,
    pub should_request_hpke_config: bool,
}

impl RoundSettings {
    pub fn new(task_id_base64: String) -> Result<Self> {
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

#[derive(Clone)]
pub struct CryptoConfig {
    leader_hpke_config: HpkeConfig,
    helper_hpke_config: HpkeConfig,
}

//
// All that is needed to know such that this client
// can submit its results.
//
#[derive(Clone)]
pub struct RoundConfig {
    pub settings: RoundSettings,
    pub crypto: CryptoConfig,
}

////////////////////////////////////////////////////
// State

pub struct ClientState_Permanent {
    http_client: reqwest::Client,
}

#[derive(Clone)]
pub struct ClientState_Round {
    pub config: RoundConfig,
}

pub struct ClientState {
    pub parametrization: CommonState_Parametrization,
    pub permanent: ClientState_Permanent,
    pub round: ClientState_Round,
}

// Possibly Uninitialized client state
pub enum ClientStatePU {
    ValidState(ClientState),
    InitState(Locations),
}

impl ClientStatePU {
    // pub fn get_parametrization(&self) -> &CommonState_Parametrization {
    //     match self {
    //         ClientStatePU::ValidState(ref state) => &state.parametrization,
    //         ClientStatePU::InitState(ref param) => &param,
    //     }
    // }
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
    permanent: &ClientState_Permanent,
    task_id: TaskId,
    l: Locations,
) -> anyhow::Result<CryptoConfig> {
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

async fn get_parametrization(
    task_id: TaskId,
    l:Locations,
) -> Result<CommonState_Parametrization>
{
    let leader_param = get_vdaf_parameter_from_task(l.external_leader_tasks.clone(), task_id).await?;
    let helper_param = get_vdaf_parameter_from_task(l.external_helper_tasks.clone(), task_id).await?;

    // make sure that the information matchs
    //
    // TODO: For the noise parameter we COULD take the minimum instead
    if leader_param == helper_param
    {
        Ok(CommonState_Parametrization {
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
impl ClientState {
    async fn new(
        locations: Locations,
        round_settings: RoundSettings,
    ) -> anyhow::Result<ClientState> {
        let permanent = ClientState_Permanent {
            http_client: default_http_client()?,
        };

        // we get a new crypto config if we were asked for it
        let c = get_crypto_config(
            &permanent,
            round_settings.task_id,
            locations.clone(),
        )
        .await?;

        // get a parametrization from locations
        let parametrization: CommonState_Parametrization = get_parametrization(round_settings.task_id, locations.clone()).await?;

        Ok(ClientState {
            parametrization,
            permanent,
            round: ClientState_Round {
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
    async fn get__next_round_config(
        &self,
        round_settings: RoundSettings,
    ) -> anyhow::Result<RoundConfig> {
        // NOTE: We assume that the vdaf parameters don't change between tasks of the same session
        //       If they could, we would have to get the current vdaf parameters here.

        if round_settings.should_request_hpke_config {
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
        } else {
            // Copy the old config from the state to the new config.
            // Just change the task_id to the new one from the round_settings.
            Ok(RoundConfig {
                settings: round_settings,
                crypto: self.round.config.crypto.clone(),
            })
        }
    }

    async fn update__to_next_round_config(
        &mut self,
        round_settings: RoundSettings,
    ) -> anyhow::Result<()> {
        self.round.config = self.get__next_round_config(round_settings).await?;
        Ok(())
    }

    ///////////////////////////////////////
    // Submission
    async fn get__submission_result(&self, measurement: &VecFixedAny) -> anyhow::Result<()>
    {
        match measurement
        {
            VecFixedAny::VecFixed16(v) => self.get__submission_result_impl(v).await,
            VecFixedAny::VecFixed32(v) => self.get__submission_result_impl(v).await,
            VecFixedAny::VecFixed64(v) => self.get__submission_result_impl(v).await,
        }
    }

    async fn get__submission_result_impl<Fx : Fixed>(&self, measurement: &Vec<Fx>) -> anyhow::Result<()>
        where
          Fx : CompatibleFloat<Field128>,
          Fx : IsTagInstance<FixedTypeTag>,
          Fx : FixedBase,
    {
        ////////////////////////
        // check length
        let actual_len = measurement.len();
        let expected_len = self.parametrization.vdaf_parameter.gradient_len;
        if actual_len != expected_len
        {
            return Err(anyhow!("Expected data to be have length {expected_len} but it was {actual_len}"));
        }


        ////////////////////////
        // check type
        let aggregator_tag = self.parametrization.vdaf_parameter.noise_parameter.get_tag();

        // assert that the compile time type `Fx` matches with the type tag for this round
        if Fx::get_tag() != aggregator_tag {
            return Err(anyhow!("Tried to submit gradient with fixed type {:?}, but the task has been registered for fixed type {:?}", Fx::get_tag(), aggregator_tag));
        }

        // cast noise_parameter
        let cast_noise_parameter = if let Some(n) = self.parametrization.vdaf_parameter.noise_parameter.clone().downcast::<Fx>() {
            n
        } else {
            return Err(anyhow!(""));
        };

        // create vdaf instance
        let num_aggregators = 2;
        let len = self.parametrization.vdaf_parameter.gradient_len;
        let vdaf_client: Prio3Aes128FixedPointBoundedL2VecSum<Fx> =
            Prio3Aes128FixedPointBoundedL2VecSum::new_aes128_fixedpoint_boundedl2_vec_sum(
                num_aggregators,
                len,
                cast_noise_parameter, // actually this does not matter for the client
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

/////////////////////////////////////////////////////////////////////////
// The api to be called from python code.
//

//
// Init
//
pub fn api__new_client_state(p: Locations) -> ClientStatePU
{
    ClientStatePU::InitState(p)
}

pub async fn api__update_client_round_settings(s: &mut ClientStatePU, round_settings: RoundSettings) -> Result<()>
{
    match s
    {
        ClientStatePU::InitState(ref parametrization) =>
        {
            let client_state = ClientState::new(parametrization.clone(), round_settings).await?;
            *s = ClientStatePU::ValidState(client_state);
        }
        ClientStatePU::ValidState(ref mut client_state) => {
            client_state
                .update__to_next_round_config(round_settings)
                .await?;
        }
    };
    Ok(())
}

//
// Submitting
//
pub async fn api__submit(s: &mut ClientStatePU, round_settings: RoundSettings, data: &VecFixedAny) -> anyhow::Result<()>
{
    api__update_client_round_settings(s, round_settings).await?;

    match s
    {
        ClientStatePU::InitState(_) =>
        {
            Err(anyhow!(""))?;
        }
        ClientStatePU::ValidState(ref mut client_state) =>
        {
            client_state.get__submission_result(data).await?;
        }
    };
    Ok(())
}


pub async fn api__submit_with<f : Fn(&CommonState_Parametrization) -> VecFixedAny>(s: &mut ClientStatePU, round_settings: RoundSettings, get_data: f) -> anyhow::Result<()>
{
    api__update_client_round_settings(s, round_settings).await?;

    match s
    {
        ClientStatePU::InitState(_) =>
        {
            Err(anyhow!(""))?;
        }
        ClientStatePU::ValidState(ref mut client_state) =>
        {
            let data = get_data(&client_state.parametrization);
            client_state.get__submission_result(&data).await?;
        }
    };
    Ok(())
}
