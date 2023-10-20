use std::any::Any;

use crate::core::fixed::{Fixed16, Fixed32, FixedTypeTag, IsTagInstance, VecFixedAny};
use crate::core::types::{CommonStateParametrization, VdafParameter};
use crate::core::types::{Locations, ManagerLocations};
use crate::janus_manager::interface::network::consumer::{
    get_main_locations, get_vdaf_parameter_from_task,
};

use anyhow::{anyhow, Result};

use fixed::traits::Fixed;
use janus_client::{default_http_client, Client, ClientBuilder};

use janus_messages::{Duration, TaskId};

use prio::flp::types::fixedpoint_l2::compatible_float::CompatibleFloat;
use prio::vdaf::prio3::Prio3FixedPointBoundedL2VecSum;

use super::interface::types::{
    ClientState, ClientStatePermanent, ClientStateRound, RoundConfig, RoundSettings,
};

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

/////////////////////////////////////////////////////////////////////////
// Step 1, initialization
//
// This is done at the beginning, and also if the controller notifies us
// that the hpke configs of the aggregators changed (which should not happen).
//
// We get the HPKE Configurations of both leader and helper by asking them
// directly. See 4.3.1 of ietf-ppm-dap.
//

// async fn get_crypto_config(
//     permanent: &ClientStatePermanent,
//     task_id: TaskId,
//     l: Locations,
// ) -> anyhow::Result<CryptoConfig> {
//     let c: ClientParameters = ClientParameters::new(
//         task_id,
//         l.main.external_leader,
//         l.main.external_helper,
//         Duration::from_seconds(1),
//     );
//     let f_leader = aggregator_hpke_config(&c, &Role::Leader, &task_id, &permanent.http_client);
//     let f_helper = aggregator_hpke_config(&c, &Role::Helper, &task_id, &permanent.http_client);

//     let (l, h) = try_join!(f_leader, f_helper).await?;

//     Ok(CryptoConfig {
//         leader_hpke_config: l,
//         helper_hpke_config: h,
//     })
// }

// async fn get_janus_client(
//     round_settings: RoundSettings,
//     l: Locations,
//     vdaf_parameter: VdafParameter,
// ) -> Result<Box<dyn Any>>
// {
//     match vdaf_parameter.submission_type
//     {
//         FixedTypeTag::FixedType16Bit =>
//         {
//             get_janus_client_impl::<Fixed16>(round_settings, l, vdaf_parameter).await
//         }
//         FixedTypeTag::FixedType32Bit =>
//         {
//             get_janus_client_impl::<Fixed32>(round_settings, l, vdaf_parameter).await
//         }
//         // FixedTypeTag::FixedType64Bit => get_janus_client_impl(round_settings, l, vdaf_parameter).await,
//     }
// }

async fn get_janus_client<Fx: Fixed + CompatibleFloat>(
    round_settings: RoundSettings,
    l: Locations,
    vdaf_parameter: VdafParameter,
) -> Result<Client<Prio3FixedPointBoundedL2VecSum<Fx>>>
{
    let num_aggregators = 2;
    let len = vdaf_parameter.gradient_len;

    let vdaf_client: Prio3FixedPointBoundedL2VecSum<Fx> =
        Prio3FixedPointBoundedL2VecSum::new_fixedpoint_boundedl2_vec_sum(
            num_aggregators,
            len,
            // privacy_parameter, // actually this does not matter for the client
        )?;

    let c = ClientBuilder::new(
        round_settings.task_id,
        l.main.external_leader,
        l.main.external_helper,
        Duration::from_seconds(1),
        vdaf_client,
    )
    .build()
    .await?;

    Ok(c)
}

async fn get_parametrization(task_id: TaskId, l: Locations) -> Result<CommonStateParametrization>
{
    let leader_param =
        get_vdaf_parameter_from_task(l.manager.external_leader.clone(), task_id).await?;
    let helper_param =
        get_vdaf_parameter_from_task(l.manager.external_helper.clone(), task_id).await?;

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
    pub async fn new(
        manager_locations: ManagerLocations,
        round_settings: RoundSettings,
    ) -> anyhow::Result<ClientState>
    {
        let permanent = ClientStatePermanent {
            http_client: default_http_client()?,
        };

        // we get the main locations from the tasks servers
        let main_locations = get_main_locations(manager_locations.clone()).await?;

        let locations = Locations {
            main: main_locations,
            manager: manager_locations,
        };

        // we get a new crypto config if we were asked for it
        // let c = get_crypto_config(&permanent, round_settings.task_id, locations.clone()).await?;

        // get a parametrization from locations
        let parametrization: CommonStateParametrization =
            get_parametrization(round_settings.task_id, locations.clone()).await?;

        // we create a new client
        // let c = get_janus_client(
        //     round_settings.clone(),
        //     locations,
        //     parametrization.clone().vdaf_parameter,
        // )
        // .await?;

        Ok(ClientState {
            parametrization,
            permanent,
            round: ClientStateRound {
                config: RoundConfig {
                    settings: round_settings,
                    // janus_client: c,
                },
            },
        })
    }

    /*
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
    */

    pub async fn update_to_next_round_config(
        &mut self,
        round_settings: RoundSettings,
    ) -> anyhow::Result<()>
    {
        // NOTE: We assume that the vdaf parameters don't change between tasks of the same session
        //       If they could, we would have to get the current vdaf parameters here.

        if round_settings.should_request_hpke_config
        {
            // we create a new client
            // let c = get_janus_client(
            //     round_settings.clone(),
            //     self.parametrization.location.clone(),
            //     self.parametrization.clone().vdaf_parameter,
            // )
            // .await?;

            // self.round.config.janus_client = c;
        }

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
            // VecFixedAny::VecFixed64(v) => self.get_submission_result_impl(v).await,
        }
    }

    async fn get_submission_result_impl<Fx: Fixed>(
        &self,
        measurement: &Vec<Fx>,
    ) -> anyhow::Result<()>
    where
        Fx: CompatibleFloat,
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
        // let num_aggregators = 2;
        // let len = self.parametrization.vdaf_parameter.gradient_len;
        // // let privacy_parameter = self.parametrization.vdaf_parameter.privacy_parameter.clone();
        // let vdaf_client: Prio3FixedPointBoundedL2VecSum<Fx> =
        //     Prio3FixedPointBoundedL2VecSum::new_fixedpoint_boundedl2_vec_sum(
        //         num_aggregators,
        //         len,
        //         // privacy_parameter, // actually this does not matter for the client
        //     )?;

        // let parameters = ClientParameters::new(
        //     self.round.config.settings.task_id,
        //     self.parametrization.location.main.external_leader.clone(),
        //     self.parametrization.location.main.external_helper.clone(),
        //     // .get_external_aggregator_endpoints(),
        //     self.round.config.settings.time_precision,
        // );

        // let client = Client::new(
        //     parameters,
        //     vdaf_client,
        //     RealClock::default(),
        //     &self.permanent.http_client,
        //     self.round.config.crypto.leader_hpke_config.clone(),
        //     self.round.config.crypto.helper_hpke_config.clone(),
        // );

        let client = get_janus_client(
            self.round.config.settings.clone(),
            self.parametrization.location.clone(),
            self.parametrization.clone().vdaf_parameter,
        )
        .await?;

        // let client = match self
        //     .round
        //     .config
        //     .janus_client
        //     .downcast_ref::<Client<Prio3FixedPointBoundedL2VecSum<Fx>>>()
        // {
        //     Some(a) => a,
        //     None => return Err(anyhow!("internal error: wrong janus client type!")),
        // };

        let () = client.upload(measurement).await?;

        Ok(())
    }
}
