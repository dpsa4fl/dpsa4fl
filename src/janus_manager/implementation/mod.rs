use std::time::UNIX_EPOCH;

use crate::{
    core::types::{MainLocations, VdafParameter},
    janus_manager::interface::{
        network::consumer::TIME_PRECISION,
        types::{
            CreateTrainingSessionRequest, GetVdafParameterRequest, HpkeConfigRegistry,
            StartRoundRequest, TrainingSessionId,
        },
    },
};

use anyhow::{anyhow, Context, Error, Result};
use base64::{engine::general_purpose, Engine};
use janus_aggregator_core::datastore::{self, Datastore};
use janus_aggregator_core::task::{AggregatorTask, AggregatorTaskParameters, QueryType};
use janus_aggregator_core::SecretBytes;
use janus_core::{
    auth_tokens::{AuthenticationToken, AuthenticationTokenHash},
    hpke::HpkeKeypair,
    time::Clock,
};
use janus_messages::{Duration, HpkeConfig, Role, TaskId, Time};
use prio::codec::Decode;
use rand::random;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

//////////////////////////////////////////////////
// self:

struct TrainingSession
{
    role: Role,

    collector_hpke_config: HpkeConfig,

    // needs to be the same for both aggregators (section 4.2 of ppm-draft)
    verify_key: SecretBytes,

    // auth tokens
    collector_auth_token: AuthenticationToken,
    leader_auth_token: AuthenticationToken,

    // my hpke config & key
    hpke_config_and_key: HpkeKeypair,

    // vdaf param
    vdaf_parameter: VdafParameter,

    // my tasks, most recent one is at the end
    tasks: Vec<TaskId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskProvisionerConfig
{
    // the internal endpoint urls
    pub leader_endpoint: Url,
    pub helper_endpoint: Url,

    #[serde(flatten)]
    pub main_locations: MainLocations,
}

pub struct TaskProvisioner<C: Clock>
{
    /// Datastore used for durable storage.
    datastore: Arc<Datastore<C>>,

    /// Currently active training runs.
    training_sessions: Mutex<HashMap<TrainingSessionId, TrainingSession>>,

    /// static config
    pub config: TaskProvisionerConfig,

    /// hpke config registry
    keyring: Mutex<HpkeConfigRegistry>,
}

impl<C: Clock> TaskProvisioner<C>
{
    pub fn new(datastore: Arc<Datastore<C>>, config: TaskProvisionerConfig) -> Self
    {
        Self {
            datastore,
            training_sessions: Mutex::new(HashMap::new()),
            keyring: Mutex::new(HpkeConfigRegistry::new()),
            config,
        }
    }

    pub async fn handle_start_round(&self, request: StartRoundRequest) -> Result<(), Error>
    {
        //---------------------- decode parameters --------------------------
        // session id
        let training_session_id = request.training_session_id;

        // get training session with this id
        let mut training_sessions_lock = self.training_sessions.lock().await;
        let training_session =
            training_sessions_lock
                .get_mut(&training_session_id)
                .ok_or(anyhow!(
                    "There is no training session with id {}",
                    &training_session_id
                ))?;

        // task id
        let task_id_bytes = general_purpose::URL_SAFE_NO_PAD.decode(request.task_id_encoded)?;
        let task_id = TaskId::get_decoded(&task_id_bytes)?;

        // -------------------- create new task -----------------------------
        let deadline = UNIX_EPOCH.elapsed()?.as_secs() + 10000 * 60;

        let task_params = match training_session.role
        {
            Role::Leader => AggregatorTaskParameters::Leader {
                aggregator_auth_token: training_session.leader_auth_token.clone(), // leader auth tokens
                collector_auth_token_hash: AuthenticationTokenHash::from(
                    &training_session.collector_auth_token,
                ),
                collector_hpke_config: training_session.collector_hpke_config.clone(),
            },
            Role::Helper => AggregatorTaskParameters::Helper {
                aggregator_auth_token_hash: AuthenticationTokenHash::from(
                    &training_session.leader_auth_token,
                ), // leader auth tokens
                collector_hpke_config: training_session.collector_hpke_config.clone(),
            },
            _ => todo!(),
        };

        // choose vdafinstance
        let vdafinst = training_session.vdaf_parameter.to_vdaf_instance();

        // [TEMP] debug
        println!(
            "Got training session request with auth token for aggregator: {:?}, collector: {:?}",
            training_session.leader_auth_token.as_ref().to_vec(),
            training_session.collector_auth_token.as_ref().to_vec()
        );

        // create the task
        let task = AggregatorTask::new(
            task_id,
            self.config.helper_endpoint.clone(),
            QueryType::TimeInterval,
            vdafinst,
            training_session.verify_key.clone(),
            10,                                             // max_batch_query_count
            None, // Some(Time::from_seconds_since_epoch(deadline)), // task_expiration
            None,                                           // report_expiry_age
            2,                                              // min_batch_size
            Duration::from_seconds(TIME_PRECISION),         // time_precision
            Duration::from_seconds(1000),                   // tolerable_clock_skew,
            [training_session.hpke_config_and_key.clone()],
            task_params,
        )?;

        println!("provisioning task now with id {}", task_id);
        provision_task(&self.datastore, task).await?;

        // write the task id into the session
        training_session.tasks.push(task_id);

        Ok(())
    }

    pub async fn handle_create_session(
        &self,
        request: CreateTrainingSessionRequest,
    ) -> Result<TrainingSessionId>
    {
        // decode fields
        let CreateTrainingSessionRequest {
            training_session_id,
            role,
            verify_key_encoded,
            collector_hpke_config,
            collector_auth_token_encoded,
            leader_auth_token_encoded,
            vdaf_parameter,
        } = request;

        // prepare id
        // (take requested id if exists, else generate new one)
        let training_session_id = if let Some(id) = training_session_id
        {
            if self.training_sessions.lock().await.contains_key(&id)
            {
                return Err(anyhow!(
                    "There already exists a training session with id {id}."
                ));
            }
            id
        }
        else
        {
            let id: u16 = random();
            id.into()
        };

        let collector_auth_token_decoded = general_purpose::URL_SAFE_NO_PAD
            .decode(collector_auth_token_encoded)
            .context("invalid base64url content in \"verifyKey\"")?;
        let collector_auth_token =
            AuthenticationToken::new_bearer_token_from_bytes(collector_auth_token_decoded)?;
        // let collector_auth_token = AuthenticationToken::new_dap_auth_token_from_bytes(collector_auth_token_decoded)?;
        // DapAuthToken::try_from(collector_auth_token_decoded)?;

        let leader_auth_token = AuthenticationToken::new_bearer_token_from_bytes(
            leader_auth_token_encoded.into_bytes(),
        )?;
        // DapAuthToken::try_from(leader_auth_token_encoded.into_bytes())?;
        let verify_key = SecretBytes::new(
            general_purpose::URL_SAFE_NO_PAD
                .decode(verify_key_encoded)
                .context("invalid base64url content in \"verifyKey\"")?,
        );

        // generate new hpke config and private key
        let hpke_config_and_key = self.keyring.lock().await.get_random_keypair();

        // create session
        let training_session = TrainingSession {
            role,
            verify_key,
            collector_hpke_config,
            collector_auth_token, // AuthenticationToken::DapAuth(collector_auth_token),
            leader_auth_token,    // AuthenticationToken::DapAuth(leader_auth_token),
            hpke_config_and_key,
            vdaf_parameter,
            tasks: vec![],
        };

        // insert into list
        println!("creating training session with id {}", training_session_id);
        let mut sessions = self.training_sessions.lock().await;
        sessions.insert(training_session_id, training_session);

        // respond with id
        Ok(training_session_id)
    }

    pub async fn handle_end_session(&self, session: TrainingSessionId) -> Result<()>
    {
        let mut sessions = self.training_sessions.lock().await;
        if let Some(_) = sessions.remove(&session)
        {
            println!("Removed session with id {session}");
            Ok(())
        }
        else
        {
            println!(
                "Attempted to remove session with id {session}, but there was no such session."
            );
            Err(anyhow!(
                "Attempted to remove session with id {session}, but there was no such session."
            ))
        }
    }

    pub async fn handle_get_vdaf_parameter(
        &self,
        request: GetVdafParameterRequest,
    ) -> Result<VdafParameter, Error>
    {
        // task id
        let task_id_bytes = general_purpose::URL_SAFE_NO_PAD.decode(request.task_id_encoded)?;
        let task_id = TaskId::get_decoded(&task_id_bytes)?;

        // find training session with this task_id
        let sessions = self.training_sessions.lock().await;
        let sessions_with_id: Vec<_> = sessions
            .values()
            .filter(|v| v.tasks.contains(&task_id))
            .collect();

        let session_with_id = match sessions_with_id.len()
        {
            0 => Err(anyhow!(
                "Could not find session containing task with id {task_id}."
            )),
            1 => Ok(sessions_with_id[0]),
            _ => Err(anyhow!(
                "Multiple sessions containing taskd id {task_id} exist."
            )),
        }?;

        Ok(session_with_id.vdaf_parameter.clone())
    }
}

//////////////////////////////////////////////////
// code:

pub async fn provision_task<C: Clock>(datastore: &Datastore<C>, task: AggregatorTask)
    -> Result<()>
{
    // Write all tasks requested.
    let task = Arc::new(task);
    // info!(task_count = %tasks.len(), "Writing tasks");
    datastore.run_tx("post_task", |tx| {
        let task = Arc::clone(&task);
        Box::pin(async move {
            if let Some(existing_task) = tx.get_aggregator_task(task.id()).await? {
            // Check whether the existing task in the DB corresponds to the incoming task, ignoring
            // those fields that are randomly generated.
            if existing_task.peer_aggregator_endpoint() == task.peer_aggregator_endpoint()
                && existing_task.query_type() == task.query_type()
                && existing_task.vdaf() == task.vdaf()
                && existing_task.opaque_vdaf_verify_key() == task.opaque_vdaf_verify_key()
                && existing_task.role() == task.role()
                && existing_task.max_batch_query_count() == task.max_batch_query_count()
                && existing_task.task_expiration() == task.task_expiration()
                && existing_task.min_batch_size() == task.min_batch_size()
                && existing_task.time_precision() == task.time_precision()
                && existing_task.collector_hpke_config() == task.collector_hpke_config() {
                    return Ok(())
                }

                let err = anyhow!(
                    "task with same VDAF verify key and task ID already exists with different parameters".to_string(),
                );
                return Err(datastore::Error::User(err.into()));
            }

            tx.put_aggregator_task(&task).await
        })
    })
        .await
        .context("couldn't write tasks")

    // .run_tx(|tx| {
    //     let tasks = Arc::clone(&tasks);
    //     Box::pin(async move {
    //         for task in tasks.iter() {
    //             // We attempt to delete the task, but ignore "task not found" errors since
    //             // the task not existing is an OK outcome too.
    //             match tx.delete_task(task.id()).await {
    //                 Ok(_) | Err(datastore::Error::MutationTargetNotFound) => (),
    //                 err => err?,
    //             }

    //             tx.put_task(task).await?;
    //         }
    //         Ok(())
    //     })
    // })
    // .await
    // .context("couldn't write tasks")
}
