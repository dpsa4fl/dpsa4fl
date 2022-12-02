
use std::ffi::CString;

use janus_messages::TaskId;
use prio::codec::{Encode, Decode, CodecError};
use base64::URL_SAFE_NO_PAD;
use anyhow::Result;


pub fn task_id_to_string(task_id: TaskId) -> String
{
    base64::encode_config(&task_id.get_encoded(), URL_SAFE_NO_PAD)
}



