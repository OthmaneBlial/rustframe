#![no_main]

use libfuzzer_sys::fuzz_target;
use rustframe::{DEFAULT_MAX_IPC_REQUEST_BYTES, decode_request};

fuzz_target!(|data: &[u8]| {
    let _ = decode_request(data, DEFAULT_MAX_IPC_REQUEST_BYTES);
});
