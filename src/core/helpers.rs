use anyhow::Result;
use base64::{engine::general_purpose, Engine};
use janus_messages::TaskId;
use prio::codec::{Decode, Encode};

/// Encode a task id into a string, as implemented in janus.
pub fn task_id_to_string(task_id: TaskId) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(task_id.get_encoded())
    // base64::encode_config(&task_id.get_encoded(), URL_SAFE_NO_PAD)
    // .decode(verify_key_encoded)
}

/// Decode a task id from a string, as implemented in janus.
pub fn task_id_from_string(task_id_base64: String) -> Result<TaskId> {
    let task_id_bytes = general_purpose::URL_SAFE_NO_PAD.decode(task_id_base64)?;
    // .context("invalid base64url content in \"verifyKey\"")?,
    // let task_id_bytes = base64::decode_config(task_id_base64, base64::URL_SAFE_NO_PAD)?;
    let task_id = TaskId::get_decoded(&task_id_bytes)?;
    Ok(task_id)
}
