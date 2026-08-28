use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use crate::{CliResult, command::ReleaseVerifyArgs, packaging::artifact_digest, slash_path};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageManifest {
    schema_version: u32,
    app_id: String,
    product_name: String,
    version: String,
    target_os: String,
    signed: bool,
    signature_state: String,
    #[serde(default)]
    source_commit: Option<String>,
    #[serde(default)]
    policy_hash: Option<String>,
    #[serde(default)]
    local_first_conformant: Option<bool>,
    artifacts: Vec<PackageArtifact>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageArtifact {
    format: String,
    path: String,
    sha256: Option<String>,
    bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseIndex {
    schema_version: u32,
    product: String,
    version: String,
    #[serde(default)]
    source_commit: Option<String>,
    downloads: Vec<ReleaseDownload>,
    #[serde(default)]
    verification: Vec<ReleaseEvidence>,
}

#[derive(Debug, Deserialize)]
struct ReleaseDownload {
    host: String,
    format: String,
    file: String,
    sha256: String,
    bytes: u64,
}

#[derive(Debug, Deserialize)]
struct ReleaseEvidence {
    host: String,
    format: String,
    #[serde(default)]
    sbom: Option<String>,
}

#[derive(Debug)]
enum Metadata {
    Package {
        path: PathBuf,
        manifest: PackageManifest,
    },
    Release {
        path: PathBuf,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VerificationReport {
    schema_version: u32,
    kind: &'static str,
    trusted: bool,
    artifact: VerifiedArtifact,
    identity: VerifiedIdentity,
    integrity: IntegrityVerification,
    signature: NativeVerification,
    provenance: ProvenanceVerification,
    sbom: Option<SbomVerification>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VerifiedArtifact {
    path: String,
    name: String,
    format: String,
    sha256: String,
    bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VerifiedIdentity {
    app_id: Option<String>,
    product: String,
    version: String,
    source_commit: Option<String>,
    policy_hash: Option<String>,
    local_first_conformant: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntegrityVerification {
    state: &'static str,
    algorithm: &'static str,
    checksum_manifest: String,
    metadata: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeVerification {
    state: String,
    target_os: String,
    verifier_os: &'static str,
    checks: Vec<VerificationCheck>,
}

#[derive(Debug, Serialize)]
struct VerificationCheck {
    kind: String,
    state: String,
    detail: String,
}

#[derive(Debug, Serialize)]
struct ProvenanceVerification {
    state: String,
    repository: Option<String>,
    detail: String,
}

#[derive(Debug, Serialize)]
struct SbomVerification {
    state: &'static str,
    path: String,
    sha256: String,
    bytes: u64,
    #[serde(rename = "spdxVersion")]
    spdx_version: String,
}

#[derive(Debug)]
struct MetadataSelection {
    metadata: Metadata,
    expected_sha256: String,
    expected_bytes: u64,
    target_os: String,
    format: String,
    identity: VerifiedIdentity,
    inferred_sbom: Option<PathBuf>,
}

pub fn verify(args: &ReleaseVerifyArgs) -> CliResult<()> {
    if args.allow_unsigned_local && env::var("GITHUB_ACTIONS").as_deref() == Ok("true") {
        return Err("--allow-unsigned-local is forbidden in GitHub Actions".into());
    }
    if args.manifest.is_some() && args.index.is_some() {
        return Err("use either --manifest or --index, not both".into());
    }

    let artifact = resolve_existing(&args.artifact, "release artifact")?;
    let artifact_name = file_name(&artifact)?;
    let directory = artifact
        .parent()
        .ok_or_else(|| format!("artifact '{}' has no parent directory", artifact.display()))?;
    let selection = select_metadata(args, directory, &artifact_name)?;
    let checksums_path = args
        .checksums
        .clone()
        .unwrap_or_else(|| directory.join("SHA256SUMS"));
    let checksums_path = resolve_existing(&checksums_path, "SHA256SUMS")?;
    let checksums = parse_checksums(&checksums_path)?;
    let checksum = checksums.get(&artifact_name).ok_or_else(|| {
        format!(
            "SHA256SUMS '{}' does not contain '{}'",
            checksums_path.display(),
            artifact_name
        )
    })?;

    if checksum != &selection.expected_sha256 {
        return Err(format!(
            "release metadata and SHA256SUMS disagree for '{artifact_name}'"
        ));
    }

    let (actual_sha256, actual_bytes) = artifact_digest(&artifact)?;
    if actual_sha256 != selection.expected_sha256 {
        return Err(format!(
            "checksum mismatch for '{artifact_name}': expected {}, received {actual_sha256}",
            selection.expected_sha256
        ));
    }
    if actual_bytes != selection.expected_bytes {
        return Err(format!(
            "byte count mismatch for '{artifact_name}': expected {}, received {actual_bytes}",
            selection.expected_bytes
        ));
    }

    let signature = verify_native_signature(
        &artifact,
        &selection.target_os,
        &selection.format,
        metadata_signed(&selection.metadata),
        args.allow_unsigned_local,
    )?;
    let provenance = verify_provenance(&artifact, args.repository.as_deref())?;
    if args.require_provenance && provenance.state != "verified" {
        return Err("GitHub artifact provenance was required but not verified".into());
    }

    let sbom_path = args.sbom.clone().or(selection.inferred_sbom);
    let sbom = match sbom_path {
        Some(path) => Some(verify_sbom(&path, directory)?),
        None if args.require_sbom => {
            return Err("an SPDX JSON SBOM was required but none was found".into());
        }
        None => None,
    };

    let signature_trusted = matches!(signature.state.as_str(), "verified" | "not-applicable");
    let provenance_trusted = !args.require_provenance || provenance.state == "verified";
    let report = VerificationReport {
        schema_version: 1,
        kind: "rustframe.release-verification",
        trusted: signature_trusted && provenance_trusted,
        artifact: VerifiedArtifact {
            path: slash_path(&artifact),
            name: artifact_name,
            format: selection.format,
            sha256: actual_sha256,
            bytes: actual_bytes,
        },
        identity: selection.identity,
        integrity: IntegrityVerification {
            state: "verified",
            algorithm: "SHA-256",
            checksum_manifest: slash_path(&checksums_path),
            metadata: metadata_path(&selection.metadata),
        },
        signature,
        provenance,
        sbom,
    };

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|error| format!("failed to serialize verification report: {error}"))?
        );
    } else {
        render_human(&report);
    }
    Ok(())
}

fn select_metadata(
    args: &ReleaseVerifyArgs,
    directory: &Path,
    artifact_name: &str,
) -> CliResult<MetadataSelection> {
    if let Some(path) = &args.manifest {
        return select_package(path, artifact_name, directory);
    }
    if let Some(path) = &args.index {
        return select_release(path, artifact_name, directory);
    }

    let manifest_path = directory.join("rustframe-package-manifest.json");
    if manifest_path.is_file() {
        return select_package(&manifest_path, artifact_name, directory);
    }

    let indexes = find_release_indexes(directory)?;
    match indexes.as_slice() {
        [path] => select_release(path, artifact_name, directory),
        [] => Err(format!(
            "no rustframe-package-manifest.json or *-release-index.json was found beside '{artifact_name}'"
        )),
        _ => Err("multiple release indexes were found; select one with --index".into()),
    }
}

fn select_package(
    path: &Path,
    artifact_name: &str,
    directory: &Path,
) -> CliResult<MetadataSelection> {
    let path = resolve_existing(path, "package manifest")?;
    let manifest: PackageManifest = read_json(&path, "package manifest")?;
    if manifest.schema_version != 1 {
        return Err(format!(
            "unsupported package manifest schema {}",
            manifest.schema_version
        ));
    }
    if manifest.app_id.trim().is_empty()
        || manifest.product_name.trim().is_empty()
        || manifest.version.trim().is_empty()
    {
        return Err("package manifest identity is incomplete".into());
    }
    let expected_signature_state = if manifest.signed {
        "signed"
    } else {
        "unsigned"
    };
    if manifest.signature_state != expected_signature_state {
        return Err(format!(
            "package manifest signature state '{}' is inconsistent with signed={}",
            manifest.signature_state, manifest.signed
        ));
    }
    let mut matches = manifest
        .artifacts
        .iter()
        .filter(|entry| portable_basename(&entry.path) == artifact_name);
    let artifact = matches
        .next()
        .ok_or_else(|| format!("package manifest does not describe '{artifact_name}'"))?;
    if matches.next().is_some() {
        return Err(format!(
            "package manifest contains duplicate records for '{artifact_name}'"
        ));
    }
    let artifact = PackageArtifact {
        format: artifact.format.clone(),
        path: artifact.path.clone(),
        sha256: artifact.sha256.clone(),
        bytes: artifact.bytes,
    };
    let expected_sha256 =
        validate_sha256(artifact.sha256.as_deref().ok_or_else(|| {
            format!("package manifest checksum is missing for '{artifact_name}'")
        })?)?;
    let expected_bytes = artifact
        .bytes
        .ok_or_else(|| format!("package manifest byte count is missing for '{artifact_name}'"))?;
    let inferred_sbom = infer_single_sbom(directory)?;
    let identity = VerifiedIdentity {
        app_id: Some(manifest.app_id.clone()),
        product: manifest.product_name.clone(),
        version: manifest.version.clone(),
        source_commit: manifest.source_commit.clone(),
        policy_hash: manifest.policy_hash.clone(),
        local_first_conformant: manifest.local_first_conformant,
    };
    let target_os = normalize_target_os(&manifest.target_os)?;
    let format = artifact.format.clone();
    Ok(MetadataSelection {
        metadata: Metadata::Package { path, manifest },
        expected_sha256,
        expected_bytes,
        target_os,
        format,
        identity,
        inferred_sbom,
    })
}

fn select_release(
    path: &Path,
    artifact_name: &str,
    directory: &Path,
) -> CliResult<MetadataSelection> {
    let path = resolve_existing(path, "release index")?;
    let index: ReleaseIndex = read_json(&path, "release index")?;
    if index.schema_version != 1 {
        return Err(format!(
            "unsupported release index schema {}",
            index.schema_version
        ));
    }
    if index.product.trim().is_empty() || index.version.trim().is_empty() {
        return Err("release index identity is incomplete".into());
    }
    let mut matches = index
        .downloads
        .iter()
        .filter(|entry| entry.file == artifact_name);
    let download = matches
        .next()
        .ok_or_else(|| format!("release index does not describe '{artifact_name}'"))?;
    if matches.next().is_some() {
        return Err(format!(
            "release index contains duplicate records for '{artifact_name}'"
        ));
    }
    let download = ReleaseDownload {
        host: download.host.clone(),
        format: download.format.clone(),
        file: download.file.clone(),
        sha256: download.sha256.clone(),
        bytes: download.bytes,
    };
    let expected_sha256 = validate_sha256(&download.sha256)?;
    let target_os = normalize_target_os(&download.host)?;
    let format = download.format.clone();
    let inferred_sbom = index
        .verification
        .iter()
        .find(|entry| entry.host == download.host && entry.format == download.format)
        .and_then(|entry| entry.sbom.as_deref())
        .map(|name| safe_sibling(directory, name, "SBOM"))
        .transpose()?
        .or(infer_single_sbom(directory)?);
    let identity = VerifiedIdentity {
        app_id: None,
        product: index.product.clone(),
        version: index.version.clone(),
        source_commit: index.source_commit.clone(),
        policy_hash: None,
        local_first_conformant: None,
    };
    let expected_bytes = download.bytes;
    Ok(MetadataSelection {
        metadata: Metadata::Release { path },
        expected_sha256,
        expected_bytes,
        target_os,
        format,
        identity,
        inferred_sbom,
    })
}

fn metadata_signed(metadata: &Metadata) -> Option<bool> {
    match metadata {
        Metadata::Package { manifest, .. } => Some(manifest.signed),
        Metadata::Release { .. } => None,
    }
}

fn metadata_path(metadata: &Metadata) -> String {
    match metadata {
        Metadata::Package { path, .. } | Metadata::Release { path } => slash_path(path),
    }
}

fn verify_native_signature(
    artifact: &Path,
    target_os: &str,
    format: &str,
    signed_claim: Option<bool>,
    allow_unsigned_local: bool,
) -> CliResult<NativeVerification> {
    if !matches!(target_os, "macos" | "windows") {
        return Ok(NativeVerification {
            state: "not-applicable".into(),
            target_os: target_os.into(),
            verifier_os: env::consts::OS,
            checks: vec![VerificationCheck {
                kind: "native-signature".into(),
                state: "not-applicable".into(),
                detail: format!("{target_os} packages use checksum and provenance verification"),
            }],
        });
    }

    if signed_claim == Some(false) {
        if !allow_unsigned_local {
            return Err(format!(
                "package metadata says this {target_os} artifact is unsigned; use --allow-unsigned-local only for a local test build"
            ));
        }
        return Ok(NativeVerification {
            state: "unsigned-local".into(),
            target_os: target_os.into(),
            verifier_os: env::consts::OS,
            checks: vec![VerificationCheck {
                kind: "native-signature".into(),
                state: "not-for-distribution".into(),
                detail: "unsigned local build explicitly accepted; this is not release trust"
                    .into(),
            }],
        });
    }

    let verifier_os = normalize_target_os(env::consts::OS)?;
    if verifier_os != target_os {
        return Err(format!(
            "{target_os} native trust must be verified on a {target_os} host; this host is {verifier_os}"
        ));
    }

    match target_os {
        "macos" => verify_macos_signature(artifact, format),
        "windows" => verify_windows_signature(artifact),
        _ => unreachable!(),
    }
}

#[cfg(target_os = "macos")]
fn verify_macos_signature(artifact: &Path, format: &str) -> CliResult<NativeVerification> {
    let extension = artifact
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !matches!(extension.to_ascii_lowercase().as_str(), "app" | "dmg") {
        return Err(format!(
            "the {format} download '{}' is a transport archive; extract its .app before native signature verification",
            artifact.display()
        ));
    }
    let mut checks = Vec::new();
    run_check(
        "codesign",
        Command::new("codesign")
            .args(["--verify", "--strict", "--verbose=2"])
            .arg(artifact),
        &mut checks,
    )?;
    run_check(
        "notarization-ticket",
        Command::new("xcrun")
            .args(["stapler", "validate", "-v"])
            .arg(artifact),
        &mut checks,
    )?;
    if extension.eq_ignore_ascii_case("app") {
        run_check(
            "gatekeeper",
            Command::new("spctl")
                .args(["--assess", "--type", "execute", "--verbose=2"])
                .arg(artifact),
            &mut checks,
        )?;
    }
    Ok(NativeVerification {
        state: "verified".into(),
        target_os: "macos".into(),
        verifier_os: env::consts::OS,
        checks,
    })
}

#[cfg(not(target_os = "macos"))]
fn verify_macos_signature(_artifact: &Path, _format: &str) -> CliResult<NativeVerification> {
    Err("macOS trust verification is unavailable on this host".into())
}

#[cfg(target_os = "windows")]
fn verify_windows_signature(artifact: &Path) -> CliResult<NativeVerification> {
    let escaped = artifact.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$s=Get-AuthenticodeSignature -LiteralPath '{escaped}'; [pscustomobject]@{{Status=[string]$s.Status; StatusMessage=$s.StatusMessage; Signer=if($s.SignerCertificate){{$s.SignerCertificate.Subject}}else{{$null}}; Timestamped=($null -ne $s.TimeStamperCertificate)}} | ConvertTo-Json -Compress"
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|error| format!("failed to start Authenticode verification: {error}"))?;
    if !output.status.success() {
        return Err(command_failure("Authenticode verification", &output));
    }
    let result: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("failed to parse Authenticode result: {error}"))?;
    if result.get("Status").and_then(serde_json::Value::as_str) != Some("Valid") {
        return Err(format!(
            "Authenticode status is not Valid: {}",
            result
                .get("StatusMessage")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown status")
        ));
    }
    if result
        .get("Timestamped")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Err("Authenticode signature has no observed timestamp certificate".into());
    }
    Ok(NativeVerification {
        state: "verified".into(),
        target_os: "windows".into(),
        verifier_os: env::consts::OS,
        checks: vec![VerificationCheck {
            kind: "authenticode".into(),
            state: "verified".into(),
            detail: result
                .get("Signer")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("valid signer")
                .into(),
        }],
    })
}

#[cfg(not(target_os = "windows"))]
fn verify_windows_signature(_artifact: &Path) -> CliResult<NativeVerification> {
    Err("Windows trust verification is unavailable on this host".into())
}

#[cfg(target_os = "macos")]
fn run_check(
    kind: &str,
    command: &mut Command,
    checks: &mut Vec<VerificationCheck>,
) -> CliResult<()> {
    let output = command
        .output()
        .map_err(|error| format!("failed to start {kind} verification: {error}"))?;
    if !output.status.success() {
        return Err(command_failure(kind, &output));
    }
    checks.push(VerificationCheck {
        kind: kind.into(),
        state: "verified".into(),
        detail: command_detail(&output),
    });
    Ok(())
}

fn verify_provenance(
    artifact: &Path,
    repository: Option<&str>,
) -> CliResult<ProvenanceVerification> {
    let Some(repository) = repository else {
        return Ok(ProvenanceVerification {
            state: "not-requested".into(),
            repository: None,
            detail: "pass --repository OWNER/REPO to verify a GitHub artifact attestation".into(),
        });
    };
    validate_repository(repository)?;
    let output = Command::new("gh")
        .args(["attestation", "verify"])
        .arg(artifact)
        .args(["--repo", repository])
        .output()
        .map_err(|error| format!("failed to start GitHub attestation verification: {error}"))?;
    if !output.status.success() {
        return Err(command_failure("GitHub attestation verification", &output));
    }
    Ok(ProvenanceVerification {
        state: "verified".into(),
        repository: Some(repository.into()),
        detail: command_detail(&output),
    })
}

fn verify_sbom(path: &Path, directory: &Path) -> CliResult<SbomVerification> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        directory.join(path)
    };
    let path = resolve_existing(&path, "SPDX SBOM")?;
    if path.extension().and_then(|value| value.to_str()) != Some("json") {
        return Err(format!("SPDX SBOM '{}' must be JSON", path.display()));
    }
    let value: serde_json::Value = read_json(&path, "SPDX SBOM")?;
    let spdx_version = value
        .get("spdxVersion")
        .and_then(serde_json::Value::as_str)
        .filter(|value| value.starts_with("SPDX-"))
        .ok_or_else(|| format!("SPDX SBOM '{}' has no valid spdxVersion", path.display()))?;
    let (sha256, bytes) = artifact_digest(&path)?;
    if bytes == 0 {
        return Err(format!("SPDX SBOM '{}' is empty", path.display()));
    }
    Ok(SbomVerification {
        state: "verified",
        path: slash_path(&path),
        sha256,
        bytes,
        spdx_version: spdx_version.into(),
    })
}

fn parse_checksums(path: &Path) -> CliResult<BTreeMap<String, String>> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("failed to read SHA256SUMS '{}': {error}", path.display()))?;
    let mut values = BTreeMap::new();
    for (index, line) in source.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((digest, name)) = line.split_once("  ") else {
            return Err(format!("invalid SHA256SUMS line {}: {line}", index + 1));
        };
        let digest = validate_sha256(digest)?;
        validate_safe_name(name, "checksum target")?;
        if values.insert(name.into(), digest).is_some() {
            return Err(format!("SHA256SUMS contains duplicate target '{name}'"));
        }
    }
    if values.is_empty() {
        return Err("SHA256SUMS contains no entries".into());
    }
    Ok(values)
}

fn validate_sha256(value: &str) -> CliResult<String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
    {
        return Err(format!("invalid lowercase SHA-256 digest '{value}'"));
    }
    Ok(value.into())
}

fn validate_safe_name(value: &str, label: &str) -> CliResult<()> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || Path::new(value).is_absolute()
    {
        return Err(format!("unsafe {label} '{value}'"));
    }
    Ok(())
}

fn safe_sibling(directory: &Path, name: &str, label: &str) -> CliResult<PathBuf> {
    validate_safe_name(name, label)?;
    Ok(directory.join(name))
}

fn find_release_indexes(directory: &Path) -> CliResult<Vec<PathBuf>> {
    let mut paths = fs::read_dir(directory)
        .map_err(|error| format!("failed to inspect '{}': {error}", directory.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.ends_with("-release-index.json"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn infer_single_sbom(directory: &Path) -> CliResult<Option<PathBuf>> {
    let mut paths = fs::read_dir(directory)
        .map_err(|error| format!("failed to inspect '{}': {error}", directory.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.ends_with(".spdx.json"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok((paths.len() == 1).then(|| paths.remove(0)))
}

fn normalize_target_os(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "macos" | "mac" | "darwin" => Ok("macos".into()),
        "windows" | "win" | "win32" => Ok("windows".into()),
        "linux" => Ok("linux".into()),
        other if !other.is_empty() => Ok(other.into()),
        _ => Err("release metadata target OS is missing".into()),
    }
}

fn resolve_existing(path: &Path, label: &str) -> CliResult<PathBuf> {
    if !path.exists() {
        return Err(format!("{label} '{}' is missing", path.display()));
    }
    path.canonicalize()
        .map_err(|error| format!("failed to resolve {label} '{}': {error}", path.display()))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path, label: &str) -> CliResult<T> {
    let source = fs::read(path)
        .map_err(|error| format!("failed to read {label} '{}': {error}", path.display()))?;
    serde_json::from_slice(&source)
        .map_err(|error| format!("failed to parse {label} '{}': {error}", path.display()))
}

fn file_name(path: &Path) -> CliResult<String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("artifact '{}' has no portable file name", path.display()))
}

fn portable_basename(value: &str) -> &str {
    value.rsplit(['/', '\\']).next().unwrap_or_default()
}

fn validate_repository(value: &str) -> CliResult<()> {
    let mut parts = value.split('/');
    let owner = parts.next().unwrap_or_default();
    let repository = parts.next().unwrap_or_default();
    if owner.is_empty()
        || repository.is_empty()
        || parts.next().is_some()
        || !owner
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_'))
        || !repository
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_' | b'.'))
    {
        return Err(format!(
            "invalid GitHub repository '{value}'; expected OWNER/REPO"
        ));
    }
    Ok(())
}

fn command_failure(label: &str, output: &Output) -> String {
    let detail = command_detail(output);
    format!("{label} failed with status {}: {detail}", output.status)
}

fn command_detail(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    if detail.is_empty() {
        "command completed without textual output".into()
    } else {
        detail
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(240)
            .collect()
    }
}

fn render_human(report: &VerificationReport) {
    println!("Release verification: {}", report.artifact.name);
    println!(
        "  identity   {} {}",
        report.identity.product, report.identity.version
    );
    println!("  integrity  verified (SHA-256 {})", report.artifact.sha256);
    println!(
        "  signature  {} ({})",
        report.signature.state, report.signature.target_os
    );
    println!("  provenance {}", report.provenance.state);
    println!(
        "  SBOM       {}",
        report.sbom.as_ref().map_or("not-found", |_| "verified")
    );
    println!(
        "  trust      {}",
        if report.trusted {
            "verified"
        } else {
            "NOT FOR DISTRIBUTION"
        }
    );
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{parse_checksums, validate_repository, validate_sha256};

    #[test]
    fn parses_strict_safe_checksum_entries() {
        let root = tempdir().unwrap();
        let path = root.path().join("SHA256SUMS");
        fs::write(&path, format!("{}  Research Desk.app\n", "a".repeat(64))).unwrap();
        let values = parse_checksums(&path).unwrap();
        assert_eq!(values.get("Research Desk.app"), Some(&"a".repeat(64)));
    }

    #[test]
    fn rejects_checksum_path_traversal_and_duplicates() {
        let root = tempdir().unwrap();
        let path = root.path().join("SHA256SUMS");
        fs::write(&path, format!("{}  ../escape\n", "a".repeat(64))).unwrap();
        assert!(
            parse_checksums(&path)
                .unwrap_err()
                .contains("unsafe checksum target")
        );
        fs::write(
            &path,
            format!("{0}  artifact\n{0}  artifact\n", "a".repeat(64)),
        )
        .unwrap();
        assert!(
            parse_checksums(&path)
                .unwrap_err()
                .contains("duplicate target")
        );
    }

    #[test]
    fn validates_digests_and_repository_coordinates() {
        assert!(validate_sha256(&"f".repeat(64)).is_ok());
        assert!(validate_sha256(&"F".repeat(64)).is_err());
        assert!(validate_repository("OthmaneBlial/rustframe").is_ok());
        assert!(validate_repository("owner/repo/extra").is_err());
    }
}
