use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::{Path, PathBuf}, process::Command, time::{SystemTime, UNIX_EPOCH}};

fn hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn tmp() -> PathBuf {
    let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let p = std::env::temp_dir().join(format!("titanu-pipeline-test-{}-{n}", std::process::id()));
    fs::create_dir(&p).unwrap();
    p
}
fn fixture(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let target = dir.join("my-app.apk");
    let src = b"owned fixture, not actual apk";
    fs::write(&target, src).unwrap();
    let report = json!({
        "id":"craft-evidence-001",
        "operation":"core_target_triage",
        "summary":"Detected an archive. This is synthetic test evidence.",
        "result":{"kind":"apk","next_steps":[{"tool":"core_archive_show","why":"enumerate archive"}]}
    });
    let ev = serde_json::to_vec(&report).unwrap();
    let evidence = dir.join("stdout.json");
    fs::write(&evidence, &ev).unwrap();
    let receipt = dir.join("receipt.json");
    fs::write(&receipt, serde_json::to_vec(&json!({
        "schema":"titanu.craft.inspection.v1",
        "success":true,"operation":"triage","deep_mode":true,
        "target":target.display().to_string(),
        "target_sha256":hash(src),
        "stdout_sha256":hash(&ev),
        "cli_path":"/example/cli",
        "additional_optional_metadata":"allowed"
    })).unwrap()).unwrap();
    let contract = dir.join("approved.json");
    fs::write(&contract, serde_json::to_vec(&json!({
        "schema":"titanu.behavior.v1",
        "app":"my-app",
        "approved":true,
        "source_sha256":hash(src),
        "evidence_id":"craft-evidence-001",
        "behaviors":[{"name":"play","response":"playing"},
                     {"name":"pause","response":"paused"}]
    })).unwrap()).unwrap();
    (target, receipt, contract)
}
fn exe() -> &'static str { env!("CARGO_BIN_EXE_titanu-craft-pipeline") }

#[test]
fn end_to_end_compile_execute_and_prove() {
    let dir = tmp();
    let (_, receipt, contract) = fixture(&dir);
    let proof = dir.join("proof");
    let plan = Command::new(exe()).arg("plan").arg(&receipt)
        .env("TITAN_CRAFT_PIPELINE_DIR",&proof).output().unwrap();
    assert!(plan.status.success(),"{}",String::from_utf8_lossy(&plan.stderr));
    let p = String::from_utf8_lossy(&plan.stdout);
    assert!(p.contains("\"feature_behavior_proven\": false"));
    let result = Command::new(exe()).arg("build").arg(&receipt).arg(&contract)
        .env("TITAN_CRAFT_PIPELINE_DIR",&proof).output().unwrap();
    assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
    let mut found = None;
    for entry in fs::read_dir(&proof).unwrap() {
        let child = entry.unwrap().path();
        if child.join("build-receipt.json").is_file() { found = Some(child); break; }
    }
    let build_dir = found.expect("compiled receipt");
    let rec: Value = serde_json::from_slice(&fs::read(build_dir.join("build-receipt.json")).unwrap()).unwrap();
    assert_eq!(rec["status"],"verified");
    assert_eq!(rec["test_count"],3);
    assert_eq!(rec["unauthorized_command_denied"],true);
    let artifact = build_dir.join("my-app");
    assert!(artifact.is_file());
    let run = Command::new(&artifact).arg("play").output().unwrap();
    assert_eq!(run.stdout,b"playing\n");
    assert!(run.status.success());
    let denied = Command::new(&artifact).arg("wipe").output().unwrap();
    assert!(!denied.status.success());
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn tampered_evidence_and_target_are_rejected() {
    let dir = tmp();
    let (target, receipt, _) = fixture(&dir);
    fs::write(dir.join("stdout.json"), b"{\"modified\":true}").unwrap();
    let p = Command::new(exe()).arg("plan").arg(&receipt).output().unwrap();
    assert!(!p.status.success());
    assert!(String::from_utf8_lossy(&p.stderr).contains("hash differs"));
    let (_target, receipt, _) = fixture(&dir);
    fs::write(target,b"changed").unwrap();
    let p = Command::new(exe()).arg("plan").arg(&receipt).output().unwrap();
    assert!(!p.status.success());
    assert!(String::from_utf8_lossy(&p.stderr).contains("content changed"));
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn unapproved_or_wrong_source_contract_is_rejected() {
    let dir = tmp();
    let (_, receipt, contract) = fixture(&dir);
    let mut c: Value = serde_json::from_slice(&fs::read(&contract).unwrap()).unwrap();
    c["approved"] = json!(false);
    fs::write(&contract,serde_json::to_vec(&c).unwrap()).unwrap();
    let r = Command::new(exe()).arg("build").arg(&receipt).arg(&contract)
       .env("TITAN_CRAFT_PIPELINE_DIR",dir.join("proof")).output().unwrap();
    assert!(!r.status.success());
    c["approved"] = json!(true);
    c["source_sha256"] = json!("f".repeat(64));
    fs::write(&contract,serde_json::to_vec(&c).unwrap()).unwrap();
    let r = Command::new(exe()).arg("build").arg(&receipt).arg(&contract)
       .env("TITAN_CRAFT_PIPELINE_DIR",dir.join("proof")).output().unwrap();
    assert!(!r.status.success());
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn inspection_without_deep_mode_is_rejected() {
    let dir = tmp();
    let (_, receipt, _) = fixture(&dir);
    let mut r: Value = serde_json::from_slice(&fs::read(&receipt).unwrap()).unwrap();
    r["deep_mode"] = json!(false);
    fs::write(&receipt,serde_json::to_vec(&r).unwrap()).unwrap();
    let p = Command::new(exe()).arg("plan").arg(&receipt).output().unwrap();
    assert!(!p.status.success());
    assert!(String::from_utf8_lossy(&p.stderr).contains("explicit-deep"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn bind_creates_provenance_anchored_contract() {
    let dir = tmp();
    let (_, receipt, _) = fixture(&dir);
    let proposal = dir.join("proposal.json");
    fs::write(&proposal, serde_json::to_vec(&json!({
        "app":"my-app", "approved":true, "behaviors":[
            {"name":"play","response":"playing"}]
    })).unwrap()).unwrap();
    let proof = dir.join("proof");
    let r = Command::new(exe()).arg("bind").arg(&receipt).arg(&proposal)
        .env("TITAN_CRAFT_PIPELINE_DIR",&proof).output().unwrap();
    assert!(r.status.success(),"{}",String::from_utf8_lossy(&r.stderr));
    let stdout = String::from_utf8(r.stdout).unwrap();
    let contract = PathBuf::from(stdout.strip_prefix("CONTRACT=").unwrap().trim());
    let spec: Value = serde_json::from_slice(&fs::read(&contract).unwrap()).unwrap();
    assert_eq!(spec["evidence_id"],"craft-evidence-001");
    assert_eq!(spec["source_sha256"],hash(b"owned fixture, not actual apk"));
    let b = Command::new(exe()).arg("build").arg(&receipt).arg(contract)
        .env("TITAN_CRAFT_PIPELINE_DIR",&proof).output().unwrap();
    assert!(b.status.success(),"{}",String::from_utf8_lossy(&b.stderr));
    fs::remove_dir_all(dir).unwrap();
}
