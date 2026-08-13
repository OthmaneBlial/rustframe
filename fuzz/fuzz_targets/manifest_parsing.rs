#![no_main]

use libfuzzer_sys::fuzz_target;
use rustframe_cli::manifest::{parse_manifest_source, validate_relative_path};

fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data) {
        let _ = parse_manifest_source(source);
        let _ = validate_relative_path("fuzz", source);
    }
});
