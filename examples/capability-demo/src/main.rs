#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{borrow::Cow, env, path::PathBuf};

use rust_embed::RustEmbed;
use rustframe::{EmbeddedAssets, Result, RustFrame};

#[derive(RustEmbed)]
#[folder = "frontend/"]
struct DemoAssets;

impl EmbeddedAssets for DemoAssets {
    fn get(path: &str) -> Option<Cow<'static, [u8]>> {
        DemoAssets::get(path).map(|asset| asset.data)
    }
}

fn main() -> Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let frontend_dir = manifest_dir.join("frontend");

    #[cfg(target_os = "windows")]
    let shell_command = (
        "listFrontend",
        "cmd",
        vec!["/C".to_string(), format!("dir {}", frontend_dir.display())],
    );

    #[cfg(not(target_os = "windows"))]
    let shell_command = (
        "listFrontend",
        "ls",
        vec!["-la".to_string(), frontend_dir.display().to_string()],
    );

    let mut builder = RustFrame::builder()
        .title("RustFrame Capability Demo")
        .size(1180.0, 760.0)
        .embedded_assets::<DemoAssets>()
        .allow_fs_root(frontend_dir)
        .allow_shell_command(shell_command.0, shell_command.1, shell_command.2);

    if let Ok(url) = env::var("RUSTFRAME_DEV_URL") {
        builder = builder.dev_url(url);
    }

    builder.run()
}
