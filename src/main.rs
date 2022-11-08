

use pyo3::prelude::*;
use janus_client::{ClientParameters, aggregator_hpke_config};
use janus_core::message::*;
use janus_messages::HpkeConfig;
use url::*;
use anyhow::Result;


/////////////////////////////////////////////////////////////////////////
// DPSA Client

struct Locations
{
    leader: URL,
    helper: URL,
    controller: URL, // the server that controls the learning process
}

struct CryptoConfiguration
{
    leaderHpkeConfig: HpkeConfig,
    helperHpkeConfig: HpkeConfig,
}

struct DPSAClientState
{
    http_client: reqwest::Client
}


/////////////////////////////////////////////////////////////////////////
// Step 1, initialization
//
// This is done only once, before the learning process. We get the
// HPKE Configurations of both leader and helper by asking them
// directly. See 4.3.1 of ietf-ppm-dap.
//

fn get_crypto_config(s: &DPSAClientState, l: Locations) -> anyhow::Result<CryptoConfiguration>
{
    aggregator_hpke_config(a, aggregator_role, task_id, s.http_client)
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



