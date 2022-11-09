

use janus_client::{ClientParameters, aggregator_hpke_config, default_http_client, Client};
use janus_core::{time::RealClock};
use janus_messages::{HpkeConfig, Role, TaskId, Duration};
use url::*;
// use anyhow::Result;
use async_std::future::try_join;
use prio::vdaf::prio3::Prio3Aes128FixedPointBoundedL2VecSum;

use fixed::types::extra::{U15, U31, U63};
use fixed::{FixedI16, FixedI32, FixedI64};



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
type Fx = FixedI32<U31>;
type Measurement = Vec<Fx>;


////////////////////////////////////////////////////
// Parametrization

#[derive(Clone)]
pub struct Locations
{
    leader: Url,
    helper: Url,
    controller: Url, // the server that controls the learning process
}

impl Locations
{
    fn get_aggregator_endpoints(&self) -> Vec<Url>
    {
        vec![self.leader.clone(),self.helper.clone()]
    }
}



////////////////////////////////////////////////////
// Settings

//
// The settings are transmitted from the controller to the clients.
// The clients generate a `RoundConfig` from them.
//
#[derive(Clone)]
pub struct RoundSettings
{
    task_id : TaskId,
    time_precision : Duration,
    should_request_hpke_config: bool,
}

////////////////////////////////////////////////////
// Config

#[derive(Clone)]
pub struct CryptoConfig
{
    leaderHpkeConfig: HpkeConfig,
    helperHpkeConfig: HpkeConfig,
}

//
// All that is needed to know such that this client
// can submit its results.
//
#[derive(Clone)]
pub struct RoundConfig
{
    settings: RoundSettings,
    crypto: CryptoConfig,
}

////////////////////////////////////////////////////
// State

pub struct ClientState_Parametrization
{
    location: Locations,
    gradient_len: usize,
}

pub struct ClientState_Permanent
{
    http_client: reqwest::Client,
}

#[derive(Clone)]
pub struct ClientState_Round
{
    config: RoundConfig,
}

pub struct ClientState
{
    parametrization: ClientState_Parametrization,
    permanent: ClientState_Permanent,
    round: ClientState_Round,
}

// Possibly Uninitialized client state
pub enum ClientStatePU
{
    ValidState(ClientState),
    Parametrization(ClientState_Parametrization)
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

async fn get_crypto_config(permanent: &ClientState_Permanent, task_id: TaskId, l: Locations) -> anyhow::Result<CryptoConfig>
{
    let c: ClientParameters = ClientParameters::new(
        task_id,
        l.get_aggregator_endpoints(),
        Duration::from_seconds(1),
    );
    let f_leader = aggregator_hpke_config(&c, &Role::Leader, &task_id, &permanent.http_client);
    let f_helper = aggregator_hpke_config(&c, &Role::Helper, &task_id, &permanent.http_client);

    let (l, h) = try_join!(f_leader, f_helper).await?;

    Ok(CryptoConfig {
        leaderHpkeConfig: l,
        helperHpkeConfig: h,
    })
}

//
// Functions that take client state as parameter.
//
impl ClientState
{
    async fn new(parametrization : ClientState_Parametrization, round_settings : RoundSettings) -> anyhow::Result<ClientState>
    {
        let permanent = ClientState_Permanent
        {
            http_client: default_http_client()?
        };

        // we get a new crypto config if we were asked for it
        let c = get_crypto_config(&permanent, round_settings.task_id, parametrization.location.clone()).await?;
        Ok( ClientState {
            parametrization,
            permanent,
            round: ClientState_Round
            {
                config: RoundConfig
                {
                    settings: round_settings,
                    crypto: c,
                }
            }
        })
    }

    //
    // Generate a round config for the next round.
    //
    // This means that we either copy most of the configuration from the previous round
    // which exists in the state, or if necessary, get a new configuration from the
    // aggregators.
    //
    async fn get__next_round_config(&self, round_settings : RoundSettings) -> anyhow::Result<RoundConfig>
    {
        if round_settings.should_request_hpke_config
        {
            // we get a new crypto config if we were asked for it
            let c = get_crypto_config(&self.permanent, round_settings.task_id, self.parametrization.location.clone()).await?;
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

    async fn update__to_next_round_config(&mut self, round_settings : RoundSettings) -> anyhow::Result<()>
    {
        self.round.config = self.get__next_round_config(round_settings).await?;
        Ok(())
    }

    ///////////////////////////////////////
    // Submission
    async fn get__submission_result(&self, measurement: &Measurement) -> anyhow::Result<()>
    {
        let num_aggregators = 2;
        let len = self.parametrization.gradient_len;
        let vdaf_client : Prio3Aes128FixedPointBoundedL2VecSum<Fx>
            = Prio3Aes128FixedPointBoundedL2VecSum::new_aes128_fixedpoint_boundedl2_vec_sum(
                num_aggregators,
                len
            )?;

        let parameters = ClientParameters::new(
            self.round.config.settings.task_id,
            self.parametrization.location.get_aggregator_endpoints(),
            self.round.config.settings.time_precision,
        );

        let client = Client::new(
            parameters,
            vdaf_client,
            RealClock::default(),
            &self.permanent.http_client,
            self.round.config.crypto.leaderHpkeConfig.clone(),
            self.round.config.crypto.helperHpkeConfig.clone(),
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
pub fn api_get_new_client_state(p: ClientState_Parametrization) -> ClientStatePU
{
    ClientStatePU::Parametrization(p)
}

//
// Submitting
//
pub async fn api_submit(s: ClientStatePU, round_settings: RoundSettings, data: &Measurement) -> anyhow::Result<ClientStatePU>
{
    match s
    {
        ClientStatePU::Parametrization(parametrization) =>
        {
            let client_state = ClientState::new(parametrization, round_settings).await?;
            let () = client_state.get__submission_result(data).await?;
            Ok(ClientStatePU::ValidState(client_state))
        },
        ClientStatePU::ValidState(mut client_state) =>
        {
            client_state.update__to_next_round_config(round_settings).await?;
            let () = client_state.get__submission_result(data).await?;
            Ok(ClientStatePU::ValidState(client_state))
        },
    }
}








pub fn main()
{
    println!("hello!");
}


// /////////////////////////////////////////////////////////////////////////
// // Testing

// /// Formats the sum of two numbers as string.
// #[pyfunction]
// fn sum_as_string(x: usize, y: usize) -> PyResult<String> {
//     Ok((x + y).to_string())
// }

// #[pyfunction]
// fn send_to_server(server1: String, server2: String, value: usize) -> PyResult<String> {
//     let url1 = Url::parse(&server1).unwrap();
//     let url2 = Url::parse(&server1).unwrap();
//     let urls = vec![url1, url2];

//     // let a : TaskId = 0;
//     // let a : TaskId = TaskId::new();
//     // let param = ClientParameters::new(a,urls,todo!());
//     Ok("and bye".to_owned())
// }

// /// A Python module implemented in Rust.
// #[pymodule]
// fn dpsa4fl(_py: Python, m: &PyModule) -> PyResult<()> {
//     m.add_function(wrap_pyfunction!(sum_as_string, m)?)?;
//     m.add_function(wrap_pyfunction!(send_to_server, m)?)?;
//     Ok(())
// }



