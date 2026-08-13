#![no_main]

use std::{path::Path, sync::LazyLock};

use libfuzzer_sys::fuzz_target;
use rustframe::FsCapability;

static FILESYSTEM: LazyLock<FsCapability> = LazyLock::new(|| {
    FsCapability::new([std::env::temp_dir()]).expect("temporary directory must be resolvable")
});

fuzz_target!(|data: &[u8]| {
    if let Ok(requested) = std::str::from_utf8(data) {
        let _ = FILESYSTEM.resolve(Path::new(requested));
    }
});
