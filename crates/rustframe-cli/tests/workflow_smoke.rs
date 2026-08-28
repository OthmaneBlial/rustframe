use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::tempdir;

fn cli_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rustframe"))
}

fn run(current_dir: &Path, args: &[&str]) -> Output {
    let output = Command::new(cli_binary())
        .current_dir(current_dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rustframe {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn creates_and_validates_a_standalone_typescript_project() {
    let temp = tempdir().unwrap();
    run(
        temp.path(),
        &["new", "field-notes", "--template", "vanilla-ts"],
    );
    let project = temp.path().join("field-notes");

    assert!(project.join("rustframe.json").is_file());
    assert!(project.join("package.json").is_file());
    assert!(project.join("src/main.ts").is_file());
    assert!(project.join("src/rustframe.generated.ts").is_file());
    assert!(project.join("data/migrations").is_dir());
    assert!(!project.join("native").exists());

    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(project.join("rustframe.json")).unwrap()).unwrap();
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(manifest["app"]["id"], "field-notes");
    assert_eq!(manifest["frontend"]["distDir"], "dist");
    assert!(
        !manifest["security"]["permissions"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    run(&project, &["validate"]);
    let inspection = run(&project, &["inspect", "--json"]);
    let inspection: Value = serde_json::from_slice(&inspection.stdout).unwrap();
    assert_eq!(inspection["appId"], "field-notes");
}

#[test]
fn supports_plain_javascript_without_generated_typescript() {
    let temp = tempdir().unwrap();
    run(
        temp.path(),
        &["new", "plain-tool", "--template", "vanilla-js"],
    );
    let project = temp.path().join("plain-tool");
    assert!(project.join("src/main.js").is_file());
    assert!(project.join("src/rustframe.generated.js").is_file());
    assert!(!project.join("src/rustframe.generated.ts").exists());
    run(&project, &["validate"]);
}

#[test]
fn eject_uses_the_registry_runtime_and_no_repository_path() {
    let temp = tempdir().unwrap();
    run(temp.path(), &["new", "portable-tool"]);
    let project = temp.path().join("portable-tool");
    run(&project, &["eject"]);

    let cargo = fs::read_to_string(project.join("native/Cargo.toml")).unwrap();
    assert!(cargo.contains("package = \"rustframe-runtime\""));
    assert!(cargo.contains(&format!("version = \"={}\"", env!("CARGO_PKG_VERSION"))));
    assert!(!cargo.contains("rustframe = { package = \"rustframe-runtime\", path ="));
    assert!(!cargo.contains("crates/rustframe"));
}

#[test]
fn migrates_pre_v1_manifests_without_rewriting_application_logic() {
    let temp = tempdir().unwrap();
    let project = temp.path().join("legacy-tool");
    fs::create_dir_all(project.join("data")).unwrap();
    fs::create_dir_all(project.join("src/features")).unwrap();
    fs::write(project.join("index.html"), "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'self'\"><script src=\"app.js\"></script>").unwrap();
    fs::write(project.join("app.js"), "window.RustFrame.db.info();").unwrap();
    fs::write(
        project.join("src/features/files.ts"),
        "window.RustFrame.fs.listGrants();",
    )
    .unwrap();
    fs::write(project.join("data/schema.json"), r#"{"version":1,"tables":[{"name":"items","columns":[{"name":"title","type":"text","required":true}]}]}"#).unwrap();
    fs::write(project.join("rustframe.json"), r#"{"appId":"legacy-tool","window":{"title":"Legacy Tool","width":900,"height":700},"security":{"model":"local-first"},"filesystem":{"roots":[]},"shell":{"commands":[]},"packaging":{"version":"0.1.0"}}"#).unwrap();

    let output = run(&project, &["migrate"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("manual bridge review required: db, fs"));
    assert!(project.join("rustframe.pre-v1.json").is_file());
    assert_eq!(
        fs::read_to_string(project.join("app.js")).unwrap(),
        "window.RustFrame.db.info();"
    );
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(project.join("rustframe.json")).unwrap()).unwrap();
    assert_eq!(manifest["schemaVersion"], 1);
    assert!(project.join("package.json").is_file());
}

#[test]
fn reports_local_ownership_and_enforces_a_reviewed_capability_baseline() {
    let temp = tempdir().unwrap();
    run(temp.path(), &["new", "owned-desk"]);
    let project = temp.path().join("owned-desk");
    fs::create_dir_all(project.join("dist")).unwrap();
    fs::copy(project.join("index.html"), project.join("dist/index.html")).unwrap();

    let inspection = run(&project, &["inspect", "--local-first", "--json"]);
    let inspection: Value = serde_json::from_slice(&inspection.stdout).unwrap();
    assert_eq!(inspection["kind"], "rustframe.local-first-conformance");
    assert_eq!(inspection["conformant"], true);
    assert_eq!(inspection["network"]["productionServerRequired"], false);

    let policy = run(&project, &["capabilities", "explain", "--json"]);
    let policy: Value = serde_json::from_slice(&policy.stdout).unwrap();
    assert_eq!(policy["kind"], "rustframe.capability-policy");
    assert!(
        policy["policyHash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );

    run(&project, &["capabilities", "check", "--write-baseline"]);
    run(
        &project,
        &["capabilities", "check", "--deny-expansion", "--json"],
    );
}

#[test]
fn verifies_a_release_without_requiring_a_rustframe_project() {
    let temp = tempdir().unwrap();
    let artifact_name = "Owned Desk.AppImage";
    let artifact = temp.path().join(artifact_name);
    let contents = b"verified release fixture\n";
    fs::write(&artifact, contents).unwrap();
    let digest = format!("{:x}", Sha256::digest(contents));
    fs::write(
        temp.path().join("SHA256SUMS"),
        format!("{digest}  {artifact_name}\n"),
    )
    .unwrap();
    fs::write(
        temp.path().join("rustframe-package-manifest.json"),
        format!(
            "{{\"schemaVersion\":1,\"appId\":\"owned-desk\",\"productName\":\"Owned Desk\",\"version\":\"1.0.0\",\"targetOs\":\"linux\",\"signed\":false,\"signatureState\":\"unsigned\",\"artifacts\":[{{\"format\":\"appimage\",\"path\":\"{artifact_name}\",\"sha256\":\"{digest}\",\"bytes\":{}}}]}}\n",
            contents.len()
        ),
    )
    .unwrap();

    let output = run(temp.path(), &["release", "verify", artifact_name, "--json"]);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["kind"], "rustframe.release-verification");
    assert_eq!(report["trusted"], true);
    assert_eq!(report["integrity"]["state"], "verified");
    assert_eq!(report["signature"]["state"], "not-applicable");
}

#[test]
fn verifies_an_assembled_release_index_and_spdx_sbom() {
    let temp = tempdir().unwrap();
    let artifact_name = "Owned-Desk.AppImage";
    let artifact = temp.path().join(artifact_name);
    let contents = b"assembled release fixture\n";
    fs::write(&artifact, contents).unwrap();
    let digest = format!("{:x}", Sha256::digest(contents));
    let sbom_name = "owned-desk.spdx.json";
    fs::write(
        temp.path().join(sbom_name),
        "{\"spdxVersion\":\"SPDX-2.3\",\"name\":\"Owned Desk\"}\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("SHA256SUMS"),
        format!("{digest}  {artifact_name}\n"),
    )
    .unwrap();
    fs::write(
        temp.path().join("owned-desk-release-index.json"),
        format!(
            "{{\"schemaVersion\":1,\"product\":\"Owned Desk\",\"version\":\"1.0.0\",\"sourceCommit\":\"{}\",\"downloads\":[{{\"host\":\"Linux\",\"format\":\"appimage\",\"file\":\"{artifact_name}\",\"sha256\":\"{digest}\",\"bytes\":{}}}],\"verification\":[{{\"host\":\"Linux\",\"format\":\"appimage\",\"sbom\":\"{sbom_name}\"}}]}}\n",
            "a".repeat(40),
            contents.len()
        ),
    )
    .unwrap();

    let output = run(
        temp.path(),
        &[
            "release",
            "verify",
            artifact_name,
            "--require-sbom",
            "--json",
        ],
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["trusted"], true);
    assert_eq!(report["identity"]["sourceCommit"], "a".repeat(40));
    assert_eq!(report["sbom"]["state"], "verified");
    assert_eq!(report["sbom"]["spdxVersion"], "SPDX-2.3");
}
