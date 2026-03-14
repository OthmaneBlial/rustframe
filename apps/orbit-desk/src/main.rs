#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{borrow::Cow, collections::BTreeMap, path::PathBuf};

use rust_embed::RustEmbed;
use rustframe::{EmbeddedAssets, Result, RuntimeError, RustFrame};
use serde::Deserialize;

const MANIFEST_TEXT: &str = include_str!("../rustframe.toml");

#[derive(RustEmbed)]
#[folder = "frontend/"]
struct AppAssets;

impl EmbeddedAssets for AppAssets {
    fn get(path: &str) -> Option<Cow<'static, [u8]>> {
        AppAssets::get(path).map(|asset| asset.data)
    }
}

#[derive(Debug, Deserialize)]
struct AppManifest {
    app: AppSection,
    #[serde(default)]
    dev: DevSection,
    #[serde(default)]
    permissions: PermissionsSection,
}

#[derive(Debug, Deserialize)]
struct AppSection {
    title: String,
    width: f64,
    height: f64,
}

#[derive(Debug, Default, Deserialize)]
struct DevSection {
    url: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct PermissionsSection {
    #[serde(default)]
    fs: FsPermissions,
    #[serde(default)]
    shell: ShellPermissions,
}

#[derive(Debug, Default, Deserialize)]
struct FsPermissions {
    #[serde(default)]
    roots: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ShellPermissions {
    #[serde(default)]
    commands: BTreeMap<String, ShellCommandConfig>,
}

#[derive(Debug, Deserialize)]
struct ShellCommandConfig {
    program: String,
    #[serde(default)]
    args: Vec<String>,
}

fn main() -> Result<()> {
    let manifest: AppManifest = toml::from_str(MANIFEST_TEXT).map_err(|error| {
        RuntimeError::InvalidConfiguration(format!("invalid rustframe.toml: {error}"))
    })?;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let mut builder = RustFrame::builder()
        .title(&manifest.app.title)
        .size(manifest.app.width, manifest.app.height)
        .embedded_assets::<AppAssets>();

    if let Some(url) = manifest.dev.url {
        builder = builder.dev_url(url);
    }

    for root in manifest.permissions.fs.roots {
        builder = builder.allow_fs_root(manifest_dir.join(root));
    }

    for (name, command) in manifest.permissions.shell.commands {
        builder = builder.allow_shell_command(name, command.program, command.args);
    }

    builder.run()
}
