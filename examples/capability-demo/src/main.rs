#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{borrow::Cow, env, path::PathBuf};

use rust_embed::RustEmbed;
use rustframe::{EmbeddedAssets, Result, RustFrame, ShellCommand};

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
    let shell_command = ShellCommand::new(
        "cmd",
        vec!["/C".to_string(), format!("dir {}", frontend_dir.display())],
    )
    .current_dir(&frontend_dir)
    .timeout_ms(5_000)
    .max_output_bytes(32 * 1024);

    #[cfg(not(target_os = "windows"))]
    let shell_command = ShellCommand::new(
        "ls",
        vec!["-la".to_string(), frontend_dir.display().to_string()],
    )
    .current_dir(&frontend_dir)
    .timeout_ms(5_000)
    .max_output_bytes(32 * 1024);

    let mut builder = RustFrame::builder()
        .title("RustFrame Capability Demo")
        .size(1180.0, 760.0)
        .embedded_assets::<DemoAssets>()
        .allow_fs_root(frontend_dir)
        .allow_shell_command_configured("listFrontend", shell_command);

    if let Ok(url) = env::var("RUSTFRAME_DEV_URL") {
        builder = builder.dev_url(url);
    }

    builder.run()
}
