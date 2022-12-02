
use std::ffi::CString;

use janus_messages::TaskId;
use prio::codec::{Encode, Decode, CodecError};
use base64::URL_SAFE_NO_PAD;
use anyhow::Result;


pub fn task_id_to_string(task_id: TaskId) -> String
{
    base64::encode_config(&task_id.get_encoded(), URL_SAFE_NO_PAD)
}

pub fn task_id_from_string(task_id_base64: String) -> Result<String>
{
    let task_id_bytes = base64::decode_config(task_id_base64, base64::URL_SAFE_NO_PAD)?;
    let task_id = TaskId::get_decoded(&task_id_bytes)?;
    let res = base64::encode_config(&task_id.get_encoded(), URL_SAFE_NO_PAD);
    Ok(res)
}



