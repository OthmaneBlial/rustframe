use std::{
    fs,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    str::FromStr,
};

#[cfg(target_os = "macos")]
use std::process::Command;

#[cfg(target_os = "macos")]
use cargo_packager::config::MacOsConfig;
#[cfg(target_os = "windows")]
use cargo_packager::config::WindowsConfig;
use cargo_packager::{
    Config, PackageFormat,
    config::{AppCategory, Binary, Resource},
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    AppProject, CliResult, copy_with_permissions, executable_name, packaged_fs_roots, slash_path,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactRecord {
    format: String,
    path: String,
    sha256: Option<String>,
    bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageManifest<'a> {
    schema_version: u32,
    app_id: &'a str,
    product_name: &'a str,
    version: &'a str,
    target_os: &'static str,
    target_arch: &'static str,
    signed: bool,
    signature_state: &'static str,
    tested_os_version: Option<&'a str>,
    source_commit: Option<&'a str>,
    linux_categories: &'a [String],
    linux_keywords: &'a [String],
    artifacts: &'a [ArtifactRecord],
}

pub struct PackageResult {
    pub out_dir: PathBuf,
    pub artifact_paths: Vec<PathBuf>,
    pub manifest_path: PathBuf,
    pub checksums_path: PathBuf,
    pub release_notes_path: PathBuf,
    pub signed: bool,
}

pub fn package(
    app: &AppProject,
    source_binary: &Path,
    requested_formats: &[String],
    verify: bool,
) -> CliResult<PackageResult> {
    let staging_dir = app.app_dir.join("target/rustframe/package-input");
    let out_dir = app.app_dir.join("dist/packages");
    replace_directory(&staging_dir)?;
    fs::create_dir_all(&out_dir).map_err(|error| {
        format!(
            "failed to create package output directory '{}': {error}",
            out_dir.display()
        )
    })?;

    let staged_binary = staging_dir.join(executable_name(&app.name));
    copy_with_permissions(source_binary, &staged_binary)?;

    let formats = resolve_formats(requested_formats)?;
    let mut config = Config::default();
    config.product_name = app.config.title.clone();
    config.version = native_packager_version(&app.config.packaging.version, &formats)?;
    config.binaries = vec![Binary::new(&app.name).main(true)];
    config.identifier = Some(app.config.packaging.macos.bundle_identifier.clone());
    config.formats = Some(formats);
    config.out_dir = out_dir.clone();
    config.binaries_dir = Some(staging_dir.clone());
    config.description = Some(app.config.packaging.description.clone());
    config.long_description = Some(app.config.packaging.description.clone());
    config.homepage = app.config.packaging.homepage.clone();
    config.publisher = app.config.packaging.publisher.clone();
    config.category = app
        .config
        .packaging
        .linux
        .categories
        .first()
        .and_then(|category| AppCategory::from_str(category).ok());
    config.resources = resources(app);
    if let Some(icon) = prepare_icon(app, &staging_dir)? {
        config.icons = Some(vec![slash_path(&icon)]);
    }
    let signed = configure_platform_signing(&mut config)?;

    let outputs = cargo_packager::package(&config)
        .map_err(|error| format!("native packaging failed: {error}"))?;
    let output_paths = outputs
        .iter()
        .flat_map(|output| output.paths.iter().cloned())
        .collect::<Vec<_>>();
    finalize_platform_signing(&output_paths, signed)?;
    let mut artifacts = Vec::new();
    let mut artifact_paths = Vec::new();
    for output in outputs {
        for path in output.paths {
            let (sha256, bytes) = artifact_digest(&path)?;
            artifacts.push(ArtifactRecord {
                format: output.format.short_name().to_string(),
                path: slash_path(&path),
                sha256: Some(sha256),
                bytes: Some(bytes),
            });
            artifact_paths.push(path);
        }
    }
    artifacts.sort_by(|left, right| left.path.cmp(&right.path));
    artifact_paths.sort();

    if artifacts.is_empty() {
        return Err("cargo-packager returned no native artifacts".into());
    }

    let manifest_path = out_dir.join("rustframe-package-manifest.json");
    let tested_os_version = std::env::var("RUSTFRAME_TESTED_OS_VERSION").ok();
    let source_commit = std::env::var("GITHUB_SHA").ok();
    let manifest = PackageManifest {
        schema_version: 1,
        app_id: &app.config.app_id,
        product_name: &app.config.title,
        version: &app.config.packaging.version,
        target_os: std::env::consts::OS,
        target_arch: std::env::consts::ARCH,
        signed,
        signature_state: if signed { "signed" } else { "unsigned" },
        tested_os_version: tested_os_version.as_deref(),
        source_commit: source_commit.as_deref(),
        linux_categories: &app.config.packaging.linux.categories,
        linux_keywords: &app.config.packaging.linux.keywords,
        artifacts: &artifacts,
    };
    write_text(
        &manifest_path,
        &serde_json::to_string_pretty(&manifest)
            .map_err(|error| format!("failed to serialize package manifest: {error}"))?,
    )?;

    let checksums_path = out_dir.join("SHA256SUMS");
    let checksums = artifacts
        .iter()
        .filter_map(|artifact| {
            artifact.sha256.as_ref().map(|checksum| {
                let file_name = Path::new(&artifact.path)
                    .file_name()
                    .map(|value| value.to_string_lossy())
                    .unwrap_or_default();
                format!("{checksum}  {file_name}")
            })
        })
        .collect::<Vec<_>>()
        .join("\n");
    write_text(&checksums_path, &format!("{checksums}\n"))?;

    let release_notes_path = out_dir.join("RELEASE_NOTES.md");
    write_text(
        &release_notes_path,
        &render_release_notes(app, signed, tested_os_version.as_deref()),
    )?;

    if verify {
        verify_artifacts(&artifact_paths, &manifest_path, &checksums_path)?;
    }

    Ok(PackageResult {
        out_dir,
        artifact_paths,
        manifest_path,
        checksums_path,
        release_notes_path,
        signed,
    })
}

#[cfg(target_os = "macos")]
fn configure_platform_signing(config: &mut Config) -> CliResult<bool> {
    let Some(identity) = non_empty_env("RUSTFRAME_MACOS_SIGNING_IDENTITY") else {
        return Ok(false);
    };
    let mut macos = MacOsConfig::default();
    macos.signing_identity = Some(identity);
    config.macos = Some(macos);
    Ok(true)
}

#[cfg(target_os = "windows")]
fn configure_platform_signing(config: &mut Config) -> CliResult<bool> {
    let Some(thumbprint) = non_empty_env("RUSTFRAME_WINDOWS_CERTIFICATE_THUMBPRINT") else {
        return Ok(false);
    };
    let mut windows = WindowsConfig::default();
    windows.certificate_thumbprint = Some(thumbprint);
    windows.digest_algorithm = Some("sha256".into());
    windows.timestamp_url = non_empty_env("RUSTFRAME_WINDOWS_TIMESTAMP_URL");
    windows.tsp = true;
    config.windows = Some(windows);
    Ok(true)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn configure_platform_signing(config: &mut Config) -> CliResult<bool> {
    let _ = config;
    Ok(false)
}

#[cfg(target_os = "macos")]
fn finalize_platform_signing(artifacts: &[PathBuf], signed: bool) -> CliResult<()> {
    if !signed {
        return Ok(());
    }
    let apple_id = non_empty_env("APPLE_ID");
    let apple_password = non_empty_env("APPLE_PASSWORD");
    let apple_team_id = non_empty_env("APPLE_TEAM_ID");
    let credentials = match (apple_id, apple_password, apple_team_id) {
        (Some(apple_id), Some(password), Some(team_id)) => (apple_id, password, team_id),
        (None, None, None) => return Ok(()),
        _ => {
            return Err(
                "APPLE_ID, APPLE_PASSWORD, and APPLE_TEAM_ID must be provided together".into(),
            );
        }
    };

    for artifact in artifacts.iter().filter(|path| {
        path.extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("dmg"))
    }) {
        let submission = Command::new("xcrun")
            .args(["notarytool", "submit"])
            .arg(artifact)
            .args([
                "--apple-id",
                &credentials.0,
                "--password",
                &credentials.1,
                "--team-id",
                &credentials.2,
                "--wait",
                "--output-format",
                "json",
            ])
            .output()
            .map_err(|error| format!("failed to start Apple notarization: {error}"))?;
        if !submission.status.success() {
            return Err(command_failure("Apple notarization", &submission));
        }
        let response: serde_json::Value = serde_json::from_slice(&submission.stdout)
            .map_err(|error| format!("failed to parse Apple notarization response: {error}"))?;
        if response.get("status").and_then(serde_json::Value::as_str) != Some("Accepted") {
            return Err(format!(
                "Apple notarization did not accept '{}': {}",
                artifact.display(),
                String::from_utf8_lossy(&submission.stdout).trim()
            ));
        }

        for (label, arguments) in [
            ("Apple ticket stapling", ["stapler", "staple", "-v"]),
            ("Apple ticket validation", ["stapler", "validate", "-v"]),
        ] {
            let result = Command::new("xcrun")
                .args(arguments)
                .arg(artifact)
                .output()
                .map_err(|error| format!("failed to start {label}: {error}"))?;
            if !result.status.success() {
                return Err(command_failure(label, &result));
            }
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn finalize_platform_signing(artifacts: &[PathBuf], signed: bool) -> CliResult<()> {
    let _ = (artifacts, signed);
    Ok(())
}

#[cfg(target_os = "macos")]
fn command_failure(label: &str, output: &std::process::Output) -> String {
    let detail = if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    };
    format!(
        "{label} failed with status {}: {}",
        output.status,
        String::from_utf8_lossy(detail).trim()
    )
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn render_release_notes(app: &AppProject, signed: bool, tested_os_version: Option<&str>) -> String {
    let trust = if signed {
        "This build was signed by the native packager. Release automation must still verify the downloaded signature and attach that result before publication."
    } else {
        "This local build is unsigned. It is not a trusted end-user release."
    };
    let tested_on = tested_os_version.unwrap_or("local host version not recorded");
    format!(
        "# {} {}\n\nNative RustFrame package for {} {}.\n\nSignature state: **{}**.\n\nTested on: `{tested_on}`.\n\n{trust}\n\nVerify downloaded artifacts against `SHA256SUMS` before installation.\n",
        app.config.title,
        app.config.packaging.version,
        std::env::consts::OS,
        std::env::consts::ARCH,
        if signed { "signed" } else { "unsigned" }
    )
}

fn host_formats() -> Vec<PackageFormat> {
    #[cfg(target_os = "macos")]
    {
        vec![PackageFormat::App, PackageFormat::Dmg]
    }
    #[cfg(target_os = "windows")]
    {
        vec![PackageFormat::Nsis, PackageFormat::Wix]
    }
    #[cfg(target_os = "linux")]
    {
        vec![PackageFormat::AppImage, PackageFormat::Deb]
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Vec::new()
    }
}

fn resolve_formats(requested: &[String]) -> CliResult<Vec<PackageFormat>> {
    if requested.is_empty() {
        return Ok(host_formats());
    }
    let supported = host_formats();
    let mut formats = Vec::new();
    for name in requested {
        let format = match name.as_str() {
            "app" => PackageFormat::App,
            "dmg" => PackageFormat::Dmg,
            "nsis" => PackageFormat::Nsis,
            "msi" => PackageFormat::Wix,
            "appimage" => PackageFormat::AppImage,
            "deb" => PackageFormat::Deb,
            _ => return Err(format!("unknown package format '{name}'")),
        };
        if !supported.contains(&format) {
            return Err(format!(
                "package format '{name}' is not supported on {} hosts",
                std::env::consts::OS
            ));
        }
        if !formats.contains(&format) {
            formats.push(format);
        }
    }
    Ok(formats)
}

fn native_packager_version(version: &str, formats: &[PackageFormat]) -> CliResult<String> {
    if !formats.contains(&PackageFormat::Wix) {
        return Ok(version.to_string());
    }

    let (version_without_build, build) = version
        .split_once('+')
        .map_or((version, None), |(base, build)| (base, Some(build)));
    let prerelease = version_without_build
        .split_once('-')
        .map(|(_, prerelease)| prerelease);
    if build.is_some_and(|build| build.parse::<u16>().is_ok())
        || (build.is_none()
            && prerelease.is_none_or(|prerelease| prerelease.parse::<u16>().is_ok()))
    {
        return Ok(version.to_string());
    }

    let build = msi_build_number(version, prerelease.unwrap_or_default());
    Ok(format!("{version_without_build}+{build}"))
}

fn msi_build_number(version: &str, prerelease: &str) -> u16 {
    let identifiers = prerelease.split('.').collect::<Vec<_>>();
    let sequence = identifiers
        .last()
        .and_then(|identifier| identifier.parse::<u16>().ok())
        .filter(|sequence| *sequence <= 9_999);
    if let Some(sequence) = sequence {
        let stage: u16 = match identifiers.first().copied().unwrap_or_default() {
            "alpha" | "a" => 10_000,
            "beta" | "b" => 20_000,
            "rc" => 30_000,
            _ if identifiers.len() == 1 => 0,
            _ => 40_000,
        };
        if let Some(build) = stage.checked_add(sequence) {
            if build > 0 && build < u16::MAX {
                return build;
            }
        }
    }

    let digest = Sha256::digest(version.as_bytes());
    let candidate = u16::from_be_bytes([digest[0], digest[1]]);
    candidate.max(1)
}

fn resources(app: &AppProject) -> Option<Vec<Resource>> {
    let values = packaged_fs_roots(app)
        .into_iter()
        .map(|(source, target)| Resource::Mapped {
            src: slash_path(&source),
            target,
        })
        .collect::<Vec<_>>();
    (!values.is_empty()).then_some(values)
}

fn prepare_icon(app: &AppProject, staging_dir: &Path) -> CliResult<Option<PathBuf>> {
    let configured = if cfg!(target_os = "macos") {
        app.config.packaging.macos.icon_path.as_ref()
    } else if cfg!(target_os = "windows") {
        app.config.packaging.windows.icon_path.as_ref()
    } else {
        app.config.packaging.linux.icon_path.as_ref()
    };
    let Some(source) = configured else {
        return Ok(None);
    };
    if source.extension().and_then(|value| value.to_str()) != Some("svg") {
        return Ok(Some(source.clone()));
    }

    let data = fs::read(source)
        .map_err(|error| format!("failed to read SVG icon '{}': {error}", source.display()))?;
    let options = resvg::usvg::Options {
        resources_dir: source.parent().map(Path::to_path_buf),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_data(&data, &options)
        .map_err(|error| format!("failed to parse SVG icon '{}': {error}", source.display()))?;
    let size = tree.size();
    let scale = (512.0 / size.width()).min(512.0 / size.height());
    let mut pixmap = resvg::tiny_skia::Pixmap::new(512, 512)
        .ok_or_else(|| "failed to allocate raster icon canvas".to_string())?;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let destination = staging_dir.join("rustframe-icon.png");
    pixmap
        .save_png(&destination)
        .map_err(|error| format!("failed to write rasterized icon: {error}"))?;
    Ok(Some(destination))
}

fn replace_directory(path: &Path) -> CliResult<()> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|error| {
            format!(
                "failed to replace package staging directory '{}': {error}",
                path.display()
            )
        })?;
    }
    fs::create_dir_all(path).map_err(|error| {
        format!(
            "failed to create package staging directory '{}': {error}",
            path.display()
        )
    })
}

fn file_sha256(path: &Path) -> CliResult<String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("failed to open artifact '{}': {error}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("failed to hash artifact '{}': {error}", path.display()))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn artifact_digest(path: &Path) -> CliResult<(String, u64)> {
    if path.is_file() {
        return Ok((file_sha256(path)?, file_size(path)?));
    }
    if !path.is_dir() {
        return Err(format!(
            "packaging artifact '{}' is unavailable",
            path.display()
        ));
    }

    let mut files = Vec::new();
    collect_files(path, path, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut digest = Sha256::new();
    let mut bytes = 0_u64;
    for (relative, file) in files {
        digest.update(relative.as_bytes());
        digest.update([0]);
        let contents = fs::read(&file)
            .map_err(|error| format!("failed to hash '{}': {error}", file.display()))?;
        bytes = bytes.saturating_add(contents.len() as u64);
        digest.update(&contents);
    }
    Ok((format!("{:x}", digest.finalize()), bytes))
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> CliResult<()> {
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("failed to inspect '{}': {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to inspect artifact entry: {error}"))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|error| format!("failed to inspect '{}': {error}", path.display()))?;
        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map(slash_path)
                .map_err(|error| format!("failed to resolve '{}': {error}", path.display()))?;
            files.push((relative, path));
        }
    }
    Ok(())
}

fn file_size(path: &Path) -> CliResult<u64> {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|error| format!("failed to inspect artifact '{}': {error}", path.display()))
}

fn write_text(path: &Path, contents: &str) -> CliResult<()> {
    fs::write(path, contents)
        .map_err(|error| format!("failed to write '{}': {error}", path.display()))
}

fn verify_artifacts(
    artifacts: &[PathBuf],
    manifest_path: &Path,
    checksums_path: &Path,
) -> CliResult<()> {
    for path in artifacts {
        if !path.exists() {
            return Err(format!(
                "packaging artifact '{}' is missing",
                path.display()
            ));
        }
        if path.is_file() && file_size(path)? == 0 {
            return Err(format!("packaging artifact '{}' is empty", path.display()));
        }
        if path.is_dir() && artifact_digest(path)?.1 == 0 {
            return Err(format!(
                "packaging artifact '{}' contains no files",
                path.display()
            ));
        }
    }
    if !manifest_path.is_file() || !checksums_path.is_file() {
        return Err("package verification metadata is incomplete".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use cargo_packager::PackageFormat;

    use super::{host_formats, native_packager_version, resolve_formats};

    #[test]
    fn selects_the_two_v1_native_formats_for_the_host() {
        let formats = host_formats();
        assert_eq!(formats.len(), 2);
    }

    #[test]
    fn accepts_host_compatible_format_overrides() {
        let selected = host_formats()[0];
        let requested = if selected.short_name() == "wix" {
            "msi"
        } else {
            selected.short_name()
        };
        assert_eq!(
            resolve_formats(&[requested.into()]).unwrap(),
            vec![selected]
        );
    }

    #[test]
    fn adds_a_numeric_internal_build_to_prerelease_msi_versions() {
        assert_eq!(
            native_packager_version("0.1.0-rc.1", &[PackageFormat::Wix]).unwrap(),
            "0.1.0-rc.1+30001"
        );
    }

    #[test]
    fn leaves_public_versions_unchanged_for_other_formats() {
        assert_eq!(
            native_packager_version("0.1.0-rc.1", &[PackageFormat::Nsis]).unwrap(),
            "0.1.0-rc.1"
        );
        assert_eq!(
            native_packager_version("1.2.3+42", &[PackageFormat::Wix]).unwrap(),
            "1.2.3+42"
        );
        assert_eq!(
            native_packager_version("1.2.3", &[PackageFormat::Wix]).unwrap(),
            "1.2.3"
        );
    }
}
