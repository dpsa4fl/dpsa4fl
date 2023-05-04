
use janus_messages::{TaskId, Duration};
use anyhow::Result;

use crate::{core::helpers::task_id_from_string, janus_manager::interface::network::consumer::TIME_PRECISION};


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
