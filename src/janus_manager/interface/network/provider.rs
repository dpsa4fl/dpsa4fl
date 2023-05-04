use std::{net::SocketAddr, time::Instant};

use crate::janus_manager::{
    implementation::TaskProvisionerConfig,
    interface::types::{
        CreateTrainingSessionRequest, CreateTrainingSessionResponse, GetVdafParameterRequest,
        GetVdafParameterResponse, StartRoundRequest, StartRoundResponse, TrainingSessionId,
    },
};

use anyhow::{Context, Error, Result};

use http::{HeaderMap, StatusCode};
use janus_aggregator::{
    binary_utils::{janus_main, setup_signal_handler, BinaryOptions, CommonBinaryOptions},
    config::{BinaryConfig, CommonConfig},
    datastore::Datastore,
};
use janus_core::time::{Clock, RealClock};
use janus_messages::TaskId;
use opentelemetry::metrics::{Histogram, Unit};

use serde_json::json;

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{convert::Infallible, future::Future};

use tracing::warn;

use warp::{cors::Cors, filters::BoxedFilter, reply::Response, trace, Filter, Rejection, Reply};

use crate::janus_manager::implementation::TaskProvisioner;

//////////////////////////////////////////////////
// main:
pub async fn main() -> anyhow::Result<()>
{
    const CLIENT_USER_AGENT: &str = concat!(
        env!("CARGO_PKG_NAME"),
        "/",
        env!("CARGO_PKG_VERSION"),
        "/dpsafl-janus-tasks"
    );

    janus_main::<_, Options, Config, _, _>(RealClock::default(), |ctx| async move {
        let _meter = opentelemetry::global::meter("collect_job_driver");
        // let datastore = Arc::new(ctx.datastore);

        let shutdown_signal =
            setup_signal_handler().context("failed to register SIGTERM signal handler")?;

        let (_bound_address, server) = taskprovision_server(
            Arc::new(ctx.datastore),
            ctx.clock,
            ctx.config.task_provisioner_config,
            ctx.config.listen_address,
            HeaderMap::new(),
            // ctx.config
            //     .response_header_map()
            //     .context("failed to parse response headers")?,
            shutdown_signal,
        )
        .context("failed to create aggregator server")?;
        // info!(?bound_address, "Running aggregator");
        println!("Running taskprovision server (2023-03-16)");

        server.await;

        println!("taskprovision server stopped");
        Ok(())

        // let collect_job_driver = Arc::new(CollectJobDriver::new(
        //     reqwest::Client::builder()
        //         .user_agent(CLIENT_USER_AGENT)
        //         .build()
        //         .context("couldn't create HTTP client")?,
        //     &meter,
        // ));
        // let lease_duration =
        //     Duration::from_seconds(ctx.config.job_driver_config.worker_lease_duration_secs);
        // let shutdown_signal =
        //     setup_signal_handler().context("failed to register SIGTERM signal handler")?;

        // // Start running.
        // let job_driver = Arc::new(JobDriver::new(
        //     ctx.clock,
        //     TokioRuntime,
        //     meter,
        //     Duration::from_seconds(ctx.config.job_driver_config.min_job_discovery_delay_secs),
        //     Duration::from_seconds(ctx.config.job_driver_config.max_job_discovery_delay_secs),
        //     ctx.config.job_driver_config.max_concurrent_job_workers,
        //     Duration::from_seconds(
        //         ctx.config
        //             .job_driver_config
        //             .worker_lease_clock_skew_allowance_secs,
        //     ),
        //     collect_job_driver
        //         .make_incomplete_job_acquirer_callback(Arc::clone(&datastore), lease_duration),
        //     collect_job_driver.make_job_stepper_callback(
        //         Arc::clone(&datastore),
        //         ctx.config.job_driver_config.maximum_attempts_before_failure,
        //     ),
        // ));
        // select! {
        //     _ = job_driver.run() => {}
        //     _ = shutdown_signal => {}
        // };
        // Ok(())
    })
    .await
}

/// Construct a DAP aggregator server, listening on the provided [`SocketAddr`].
/// If the `SocketAddr`'s `port` is 0, an ephemeral port is used. Returns a
/// `SocketAddr` representing the address and port the server are listening on
/// and a future that can be `await`ed to begin serving requests.
pub fn taskprovision_server<C: Clock>(
    datastore: Arc<Datastore<C>>,
    clock: C,
    config: TaskProvisionerConfig,
    listen_address: SocketAddr,
    response_headers: HeaderMap,
    shutdown_signal: impl Future<Output = ()> + Send + 'static,
) -> Result<(SocketAddr, impl Future<Output = ()> + 'static), Error>
{
    let filter = taskprovision_filter(datastore, clock, config)?;
    let wrapped_filter = filter.with(warp::filters::reply::headers(response_headers));
    let server = warp::serve(wrapped_filter);
    Ok(server.bind_with_graceful_shutdown(listen_address, shutdown_signal))
}

pub fn taskprovision_filter<C: Clock>(
    datastore: Arc<Datastore<C>>,
    _clock: C,
    config: TaskProvisionerConfig,
) -> Result<BoxedFilter<(impl Reply,)>, Error>
{
    let meter = opentelemetry::global::meter("janus_aggregator");
    let response_time_histogram = meter
        .f64_histogram("janus_aggregator_response_time")
        .with_description("Elapsed time handling incoming requests, by endpoint & status.")
        .with_unit(Unit::new("seconds"))
        .init();

    let aggregator = Arc::new(TaskProvisioner::new(datastore, config));

    //-------------------------------------------------------
    // create new training session
    let create_session_routing = warp::path("create_session");
    let create_session_responding = warp::post()
        .and(with_cloned_value(Arc::clone(&aggregator)))
        // .and(warp::query::<HashMap<String, String>>())
        .and(warp::body::json())
        .then(
            |aggregator: Arc<TaskProvisioner<C>>,
             request: CreateTrainingSessionRequest| async move {
                let result = aggregator.handle_create_session(request).await;
                match result {
                    Ok(training_session_id) => {
                        let response = CreateTrainingSessionResponse {
                            training_session_id,
                        };
                        let response =
                            warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)
                                .into_response();
                        Ok(response)
                    }
                    Err(err) => {
                        let response = warp::reply::with_status(
                            warp::reply::json(&err.to_string()),
                            StatusCode::BAD_REQUEST,
                        )
                        .into_response();
                        Ok(response)
                    }
                }
            },
        );
    let create_session_endpoint = compose_common_wrappers(
        create_session_routing,
        create_session_responding,
        warp::cors()
            .allow_any_origin()
            .allow_method("POST")
            .max_age(CORS_PREFLIGHT_CACHE_AGE)
            .build(),
        response_time_histogram.clone(),
        "create_session",
    );

    //-------------------------------------------------------
    // end training session
    let end_session_routing = warp::path("end_session");
    let end_session_responding = warp::post()
        .and(with_cloned_value(Arc::clone(&aggregator)))
        // .and(warp::query::<HashMap<String, String>>())
        .and(warp::body::json())
        .then(
            |aggregator: Arc<TaskProvisioner<C>>, session: TrainingSessionId| async move {
                let result = aggregator.handle_end_session(session).await;
                match result
                {
                    Ok(_) =>
                    {
                        let response = ();
                        let response =
                            warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)
                                .into_response();
                        Ok(response)
                    }
                    Err(err) =>
                    {
                        let response = warp::reply::with_status(
                            warp::reply::json(&err.to_string()),
                            StatusCode::BAD_REQUEST,
                        )
                        .into_response();
                        Ok(response)
                    }
                }
            },
        );
    let end_session_endpoint = compose_common_wrappers(
        end_session_routing,
        end_session_responding,
        warp::cors()
            .allow_any_origin()
            .allow_method("POST")
            .max_age(CORS_PREFLIGHT_CACHE_AGE)
            .build(),
        response_time_histogram.clone(),
        "end_session",
    );

    //-------------------------------------------------------
    // start a training round
    let start_round_routing = warp::path("start_round");
    let start_round_responding = warp::post()
        .and(with_cloned_value(Arc::clone(&aggregator)))
        // .and(warp::query::<HashMap<String, String>>())
        .and(warp::body::json())
        .then(
            |aggregator: Arc<TaskProvisioner<C>>, request: StartRoundRequest| async move {
                let result = aggregator.handle_start_round(request).await;
                match result
                {
                    Ok(()) =>
                    {
                        let response = StartRoundResponse {};
                        let response =
                            warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)
                                .into_response();
                        Ok(response)
                    }
                    Err(err) =>
                    {
                        let response = warp::reply::with_status(
                            warp::reply::json(&err.to_string()),
                            StatusCode::BAD_REQUEST,
                        )
                        .into_response();
                        Ok(response)
                    }
                }
            },
        );
    let start_round_endpoint = compose_common_wrappers(
        start_round_routing,
        start_round_responding,
        warp::cors()
            .allow_any_origin()
            .allow_method("POST")
            .max_age(CORS_PREFLIGHT_CACHE_AGE)
            .build(),
        response_time_histogram.clone(),
        "start_round",
    );

    //-------------------------------------------------------
    // get vdaf parameter
    let get_vdaf_parameter_routing = warp::path("get_vdaf_parameter");
    let get_vdaf_parameter_responding = warp::post()
        .and(with_cloned_value(Arc::clone(&aggregator)))
        // .and(warp::query::<HashMap<String, String>>())
        .and(warp::body::json())
        .then(
            |aggregator: Arc<TaskProvisioner<C>>, request: GetVdafParameterRequest| async move {
                let result = aggregator.handle_get_vdaf_parameter(request).await;
                match result
                {
                    Ok(vdaf_parameter) =>
                    {
                        let response = GetVdafParameterResponse { vdaf_parameter };
                        let response =
                            warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)
                                .into_response();
                        Ok(response)
                    }
                    Err(err) =>
                    {
                        let response = warp::reply::with_status(
                            warp::reply::json(&err.to_string()),
                            StatusCode::BAD_REQUEST,
                        )
                        .into_response();
                        Ok(response)
                    }
                }
            },
        );
    let get_vdaf_parameter_endpoint = compose_common_wrappers(
        get_vdaf_parameter_routing,
        get_vdaf_parameter_responding,
        warp::cors()
            .allow_any_origin()
            .allow_method("POST")
            .max_age(CORS_PREFLIGHT_CACHE_AGE)
            .build(),
        response_time_histogram.clone(),
        "get_vdaf_parameter",
    );

    //-------------------------------------------------------
    // get main locations
    let get_main_locations_routing = warp::path("get_main_locations");
    let get_main_locations_responding = warp::get()
        .and(with_cloned_value(Arc::clone(&aggregator)))
        .then(|aggregator: Arc<TaskProvisioner<C>>| async move {
            let response = aggregator.config.main_locations.clone();
            let response = warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)
                .into_response();
            Ok(response)
        });
    let get_main_locations_endpoint = compose_common_wrappers(
        get_main_locations_routing,
        get_main_locations_responding,
        warp::cors()
            .allow_any_origin()
            .allow_method("GET")
            .max_age(CORS_PREFLIGHT_CACHE_AGE)
            .build(),
        response_time_histogram.clone(),
        "get_main_locations",
    );

    Ok(start_round_endpoint
        .or(create_session_endpoint)
        .or(end_session_endpoint)
        .or(get_vdaf_parameter_endpoint)
        .or(get_main_locations_endpoint)
        .boxed())
}

//////////////////////////////////////////////////
// options:

#[derive(Debug, Parser)]
#[clap(
    name = "janus-dpsa4fl-janus-tasks",
    about = "Janus task provision for dpsa4fl testing environments",
    rename_all = "kebab-case",
    version = env!("CARGO_PKG_VERSION"),
)]
struct Options
{
    #[clap(flatten)]
    common: CommonBinaryOptions,
}

impl BinaryOptions for Options
{
    fn common_options(&self) -> &CommonBinaryOptions
    {
        &self.common
    }
}

//////////////////////////////////////////////////
// config:

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Config
{
    #[serde(flatten)]
    common_config: CommonConfig,
    // #[serde(flatten)]
    // job_driver_config: JobDriverConfig,
    /// Address on which this server should listen for connections and serve its
    /// API endpoints.
    // TODO(#232): options for terminating TLS, unless that gets handled in a load balancer?
    listen_address: SocketAddr,

    #[serde(flatten)]
    task_provisioner_config: TaskProvisionerConfig,
}

impl BinaryConfig for Config
{
    fn common_config(&self) -> &CommonConfig
    {
        &self.common_config
    }

    fn common_config_mut(&mut self) -> &mut CommonConfig
    {
        &mut self.common_config
    }
}

//////////////////////////////////////////////////////
// helpers:

/// The media type for problem details formatted as a JSON document, per RFC 7807.
static PROBLEM_DETAILS_JSON_MEDIA_TYPE: &str = "application/problem+json";

/// The number of seconds we send in the Access-Control-Max-Age header. This determines for how
/// long clients will cache the results of CORS preflight requests. Of popular browsers, Mozilla
/// Firefox has the highest Max-Age cap, at 24 hours, so we use that. Our CORS preflight handlers
/// are tightly scoped to relevant endpoints, and our CORS settings are unlikely to change.
/// See: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Max-Age
const CORS_PREFLIGHT_CACHE_AGE: u32 = 24 * 60 * 60;

/// Injects a clone of the provided value into the warp filter, making it
/// available to the filter's map() or and_then() handler.
fn with_cloned_value<T>(value: T) -> impl Filter<Extract = (T,), Error = Infallible> + Clone
where
    T: Clone + Sync + Send,
{
    warp::any().map(move || value.clone())
}

/// Convenience function to perform common composition of Warp filters for a single endpoint. A
/// combined filter is returned, with a CORS handler, instrumented to measure both request
/// processing time and successes or failures for metrics, and with per-route named tracing spans.
///
/// `route_filter` should be a filter that determines whether the incoming request matches a
/// given route or not. It should inspect the ambient request, and either extract the empty tuple
/// or reject.
///
/// `response_filter` should be a filter that performs all response handling for this route, after
/// the above `route_filter` has already determined the request is applicable to this route. It
/// should only reject in response to malformed requests, not requests that may yet be served by a
/// different route. This will ensure that a single request doesn't pass through multiple wrapping
/// filters, skewing the low end of unrelated requests' timing histograms. The filter's return type
/// should be `Result<impl Reply, Error>`, and errors will be transformed into responses with
/// problem details documents as appropriate.
///
/// `cors` is a configuration object describing CORS policies for this route.
///
/// `response_time_histogram` is a `Histogram` that will be used to record request handling timings.
///
/// `name` is a unique name for this route. This will be used as a metrics label, and will be added
/// to the tracing span's values as its message.
fn compose_common_wrappers<F1, F2, T>(
    route_filter: F1,
    response_filter: F2,
    cors: Cors,
    response_time_histogram: Histogram<f64>,
    name: &'static str,
) -> BoxedFilter<(impl Reply,)>
where
    F1: Filter<Extract = (), Error = Rejection> + Send + Sync + 'static,
    F2: Filter<Extract = (Result<T, Error>,), Error = Rejection> + Clone + Send + Sync + 'static,
    T: Reply + 'static,
{
    route_filter
        .and(
            response_filter
                .with(warp::wrap_fn(error_handler(response_time_histogram, name)))
                .with(cors)
                .with(trace::named(name)),
        )
        .boxed()
}

/// Produces a closure that will transform applicable errors into a problem details JSON object
/// (see RFC 7807) and update a metrics counter tracking the error status of the result as well as
/// timing information. The returned closure is meant to be used in a warp `with` filter.
fn error_handler<F, T>(
    response_time_histogram: Histogram<f64>,
    name: &'static str,
) -> impl Fn(F) -> BoxedFilter<(Response,)>
where
    F: Filter<Extract = (Result<T, Error>,), Error = Rejection> + Clone + Send + Sync + 'static,
    T: Reply,
{
    move |filter| {
        let _response_time_histogram = response_time_histogram.clone();
        warp::any()
            .map(Instant::now)
            .and(filter)
            .map(move |_start: Instant, result: Result<T, Error>| {
                let error_code = if let Err(error) = &result
                {
                    warn!(?error, endpoint = name, "Error handling endpoint");
                    error.to_string()
                }
                else
                {
                    "".to_owned()
                };

                // response_time_histogram.record(
                //     &Context::current(),
                //     start.elapsed().as_secs_f64(),
                //     &[
                //         KeyValue::new("endpoint", name),
                //         KeyValue::new("error_code", error_code),
                //     ],
                // );

                match result
                {
                    Ok(reply) => reply.into_response(),
                    Err(_e) => build_problem_details_response(error_code, None),
                }
                //     Err(Error::InvalidConfiguration(_)) => {
                //         StatusCode::INTERNAL_SERVER_ERROR.into_response()
                //     }
                //     Err(Error::MessageDecode(_)) => StatusCode::BAD_REQUEST.into_response(),
                //     Err(Error::ReportTooLate(task_id, _, _)) => {
                //         build_problem_details_response(DapProblemType::ReportTooLate, Some(task_id))
                //     }
                //     Err(Error::UnrecognizedMessage(task_id, _)) => {
                //         build_problem_details_response(DapProblemType::UnrecognizedMessage, task_id)
                //     }
                //     Err(Error::UnrecognizedTask(task_id)) => {
                //         // TODO(#237): ensure that a helper returns HTTP 404 or 403 when this happens.
                //         build_problem_details_response(
                //             DapProblemType::UnrecognizedTask,
                //             Some(task_id),
                //         )
                //     }
                //     Err(Error::MissingTaskId) => {
                //         build_problem_details_response(DapProblemType::MissingTaskId, None)
                //     }
                //     Err(Error::UnrecognizedAggregationJob(task_id, _)) => {
                //         build_problem_details_response(
                //             DapProblemType::UnrecognizedAggregationJob,
                //             Some(task_id),
                //         )
                //     }
                //     Err(Error::DeletedCollectJob(_)) => StatusCode::NO_CONTENT.into_response(),
                //     Err(Error::UnrecognizedCollectJob(_)) => StatusCode::NOT_FOUND.into_response(),
                //     Err(Error::OutdatedHpkeConfig(task_id, _)) => build_problem_details_response(
                //         DapProblemType::OutdatedConfig,
                //         Some(task_id),
                //     ),
                //     Err(Error::ReportTooEarly(task_id, _, _)) => build_problem_details_response(
                //         DapProblemType::ReportTooEarly,
                //         Some(task_id),
                //     ),
                //     Err(Error::UnauthorizedRequest(task_id)) => build_problem_details_response(
                //         DapProblemType::UnauthorizedRequest,
                //         Some(task_id),
                //     ),
                //     Err(Error::InvalidBatchSize(task_id, _)) => build_problem_details_response(
                //         DapProblemType::InvalidBatchSize,
                //         Some(task_id),
                //     ),
                //     Err(Error::BatchInvalid(task_id, _)) => {
                //         build_problem_details_response(DapProblemType::BatchInvalid, Some(task_id))
                //     }
                //     Err(Error::BatchOverlap(task_id, _)) => {
                //         build_problem_details_response(DapProblemType::BatchOverlap, Some(task_id))
                //     }
                //     Err(Error::BatchMismatch { task_id, .. }) => {
                //         build_problem_details_response(DapProblemType::BatchMismatch, Some(task_id))
                //     }
                //     Err(Error::BatchQueriedTooManyTimes(task_id, ..)) => {
                //         build_problem_details_response(
                //             DapProblemType::BatchQueriedTooManyTimes,
                //             Some(task_id),
                //         )
                //     }
                //     Err(Error::Hpke(_)) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                //     Err(Error::Datastore(_)) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                //     Err(Error::Vdaf(_)) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                //     Err(Error::Internal(_)) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                //     Err(Error::Url(_)) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                //     Err(Error::Message(_)) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                //     Err(Error::HttpClient(_)) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                //     Err(Error::Http { .. }) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                //     Err(Error::TaskParameters(_)) => {
                //         StatusCode::INTERNAL_SERVER_ERROR.into_response()
                //     }
                // }
            })
            .boxed()
    }
}

/// Construct an error response in accordance with §3.2.
// TODO(https://github.com/ietf-wg-ppm/draft-ietf-ppm-dap/issues/209): The handling of the instance,
// title, detail, and taskid fields are subject to change.
fn build_problem_details_response(error_type: String, task_id: Option<TaskId>) -> Response
{
    // let status = error_type.http_status();
    let status = StatusCode::SEE_OTHER;

    warp::reply::with_status(
        warp::reply::with_header(
            warp::reply::json(&json!({
                // "type": error_type.type_uri(),
                // "title": error_type.description(),
                // "status": status.as_u16(),
                "detail": error_type,
                // The base URI is either "[leader]/upload", "[aggregator]/aggregate",
                // "[helper]/aggregate_share", or "[leader]/collect". Relative URLs are allowed in
                // the instance member, thus ".." will always refer to the aggregator's endpoint,
                // as required by §3.2.
                "instance": "..",
                "taskid": task_id.map(|tid| format!("{}", tid)),
            })),
            http::header::CONTENT_TYPE,
            PROBLEM_DETAILS_JSON_MEDIA_TYPE,
        ),
        status,
    )
    .into_response()
}
