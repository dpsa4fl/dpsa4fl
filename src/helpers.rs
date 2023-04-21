use anyhow::Result;
use janus_messages::TaskId;
use prio::codec::{Decode};

/// Decode a task id from a string in the same format as used by janus.
pub fn task_id_from_string(task_id_base64: String) -> Result<TaskId>
{
    let task_id_bytes = base64::decode_config(task_id_base64, base64::URL_SAFE_NO_PAD)?;
    let task_id = TaskId::get_decoded(&task_id_bytes)?;
    Ok(task_id)
}
