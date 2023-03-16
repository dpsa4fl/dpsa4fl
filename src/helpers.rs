use anyhow::Result;
use base64::URL_SAFE_NO_PAD;
use janus_messages::TaskId;
use prio::codec::{Decode, Encode};

pub fn task_id_to_string(task_id: TaskId) -> String {
    base64::encode_config(&task_id.get_encoded(), URL_SAFE_NO_PAD)
}

pub fn task_id_from_string(task_id_base64: String) -> Result<TaskId> {
    let task_id_bytes = base64::decode_config(task_id_base64, base64::URL_SAFE_NO_PAD)?;
    let task_id = TaskId::get_decoded(&task_id_bytes)?;
    Ok(task_id)
}
