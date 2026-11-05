//! Evidence-bound, bounded app compiler. No AI, no arbitrary generated source execution.
//! A Craft triage establishes provenance/format, NOT feature behavior.
//! An explicitly approved behavior contract supplies runnable semantics.
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Deserialize)]
struct Receipt {
    schema: String,
    success: bool,
    operation: String,
    deep_mode: bool,
    target: String,
    target_sha256: String,
    stdout_sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Behavior {
    name: String,
    response: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Contract {
    schema: String,
    app: String,
    approved: bool,
    source_sha256: String,
    evidence_id: String,
    behaviors: Vec<Behavior>,
}
struct Bundle {
    receipt_file: PathBuf,
    receipt_hash: String,
    receipt: Receipt,
    evidence: Value,
    evidence_id: String,
    kind: String,
    summary: String,
}

fn digest_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        let n = file.read(&mut bytes).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        hasher.update(&bytes[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
fn checked_json(path: &Path) -> Result<Value, String> {
    let size = fs::metadata(path).map_err(|e| format!("metadata {}: {e}", path.display()))?.len();
    if size > 8_388_608 { return Err(format!("JSON file exceeds 8 MiB: {}", path.display())); }
    let data = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_slice(&data).map_err(|e| format!("parse {}: {e}", path.display()))
}
fn valid_hash(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn load(receipt_file: &Path) -> Result<Bundle, String> {
    let path = fs::canonicalize(receipt_file).map_err(|e| format!("missing receipt: {e}"))?;
    let v = checked_json(&path)?;
    let r: Receipt = serde_json::from_value(v).map_err(|e| format!("invalid inspection receipt: {e}"))?;
    if r.schema != "titanu.craft.inspection.v1" || !r.success || r.operation != "triage" || !r.deep_mode {
        return Err("requires a successful explicit-deep Craft triage receipt".into());
    }
    if !valid_hash(&r.target_sha256) || !valid_hash(&r.stdout_sha256) {
        return Err("invalid SHA256 values in inspection receipt".into());
    }
    let input = fs::canonicalize(&r.target).map_err(|e| format!("inspected target is missing: {e}"))?;
    if !input.is_file() || digest_file(&input)? != r.target_sha256 {
        return Err("inspected target content changed; inspect it again".into());
    }
    let stdout = path.parent().ok_or("receipt has no parent")?.join("stdout.json");
    if digest_file(&stdout)? != r.stdout_sha256 {
        return Err("Craft output hash differs from inspection receipt".into());
    }
    let ev = checked_json(&stdout)?;
    if ev.get("operation").and_then(Value::as_str) != Some("core_target_triage") {
        return Err("Craft output is not core_target_triage evidence".into());
    }
    let evidence_id = ev.get("id").and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 256)
        .ok_or("Craft triage evidence missing id")?.to_owned();
    let kind = ev.pointer("/result/kind").and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 100)
        .ok_or("Craft triage evidence missing result.kind")?.to_owned();
    let summary = ev.get("summary").and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() < 16384)
        .ok_or("Craft triage evidence missing summary")?.to_owned();
    Ok(Bundle {
        receipt_hash: digest_file(&path)?,
        receipt_file: path,
        receipt: r,
        evidence: ev,
        evidence_id,
        kind,
        summary,
    })
}
fn slug(s: &str) -> bool {
    (1..=48).contains(&s.len())
        && s.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn command_name(s: &str) -> bool {
    (1..=32).contains(&s.len())
        && s.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
fn validate(c: &Contract, b: &Bundle) -> Result<(), String> {
    if c.schema != "titanu.behavior.v1" || !c.approved {
        return Err("behavior contract needs titanu.behavior.v1 and approved=true".into());
    }
    if c.source_sha256 != b.receipt.target_sha256 || c.evidence_id != b.evidence_id {
        return Err("behavior contract does not match inspected target SHA256 and Craft evidence id".into());
    }
    if !slug(&c.app) || !(1..=32).contains(&c.behaviors.len()) {
        return Err("app must be a lowercase slug and behaviors count must be 1..32".into());
    }
    let mut names = HashSet::new();
    for x in &c.behaviors {
        if !command_name(&x.name) || !names.insert(&x.name) || x.response.is_empty() || x.response.len() > 4096 ||
           x.response.contains('\0') || x.response.chars().any(|ch| ch.is_control() && ch != '\n') {
            return Err("bad, duplicate, or unsafe behavior name/response".into());
        }
    }
    Ok(())
}
fn output_dir() -> Result<PathBuf, String> {
    let base = env::var_os("TITAN_CRAFT_PIPELINE_DIR").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("proof/craft-pipeline"));
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let ns = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    for i in 0..24 {
        let p = base.join(format!("run-{}-{}-{i}", ns, std::process::id()));
        match fs::create_dir(&p) {
            Ok(()) => {
                #[cfg(unix)] {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
                }
                return Ok(p);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("cannot allocate proof directory".into())
}
fn save(path: &Path, value: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let mut f = File::create(path).map_err(|e| e.to_string())?;
    f.write_all(&bytes).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())
}
fn facts(b: &Bundle) -> Value {
    json!({
        "schema":"titanu.craft.facts.v1",
        "source_receipt_sha256":b.receipt_hash,
        "source_target_sha256":b.receipt.target_sha256,
        "craft_evidence_id":b.evidence_id,
        "kind":b.kind,
        "summary":b.summary,
        "recommended_tools":b.evidence.pointer("/result/next_steps").cloned().unwrap_or(Value::Array(vec![])),
        "feature_behavior_proven":false,
        "review_note":"Triage identifies format/metadata; it does not establish app behavior."
    })
}
fn generate(c: &Contract) -> String {
    let mut src = String::from("// Generated by TitanU Craft Pipeline: explicit approved behavior contract.\n");
    src.push_str("use std::{env,process};\nfn main(){\nlet a:Vec<String>=env::args().skip(1).collect();\n");
    src.push_str("if a.len()!=1 {eprintln!(\"expected one behavior command\");process::exit(2)}\n");
    src.push_str("match a[0].as_str(){\n");
    for b in &c.behaviors {
        src.push_str(&format!("{:?}=>println!(\"{{}}\",{:?}),\n", b.name, b.response));
    }
    src.push_str("_=>{eprintln!(\"unknown behavior\");process::exit(2)}\n}\n}\n");
    src
}
fn command_with_timeout(cmd: &mut Command, secs: u64) -> Result<std::process::Output, String> {
    // stdout/stderr are kept bounded by the trusted, locally generated binary.
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped())
        .stdin(Stdio::null()).spawn().map_err(|e| e.to_string())?;
    let start = Instant::now();
    loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            return child.wait_with_output().map_err(|e| e.to_string());
        }
        if start.elapsed() >= Duration::from_secs(secs) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("command exceeded {secs}s timeout"));
        }
        thread::sleep(Duration::from_millis(20));
    }
}
fn build(b: &Bundle, contract_path: &Path) -> Result<Value, String> {
    let c: Contract = serde_json::from_value(checked_json(contract_path)?)
        .map_err(|e| format!("invalid behavior contract: {e}"))?;
    validate(&c, b)?;
    let dir = output_dir()?;
    let srcdir = dir.join("src");
    fs::create_dir_all(&srcdir).map_err(|e| e.to_string())?;
    let source = srcdir.join("main.rs");
    fs::write(&source, generate(&c)).map_err(|e| e.to_string())?;
    let fact_file = dir.join("facts.json");
    save(&fact_file, &facts(b))?;
    let binary = dir.join(&c.app);
    let out = command_with_timeout(
        Command::new("rustc").arg("--edition=2021").arg("-O")
            .arg(&source).arg("-o").arg(&binary),
        180,
    )?;
    fs::write(dir.join("compile.stdout"), &out.stdout).map_err(|e| e.to_string())?;
    fs::write(dir.join("compile.stderr"), &out.stderr).map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("rustc build failed (exit {:?}), logs in {}", out.status.code(), dir.display()));
    }
    let mut checks = Vec::new();
    for op in &c.behaviors {
        let t = command_with_timeout(Command::new(&binary).arg(&op.name), 10)?;
        let expected = format!("{}\n", op.response).into_bytes();
        if !t.status.success() || t.stdout != expected || !t.stderr.is_empty() {
            return Err(format!("behavior {:?} failed; generated artifact is not verified", op.name));
        }
        checks.push(json!({"behavior":op.name,"passed":true}));
    }
    let forbidden = command_with_timeout(Command::new(&binary).arg("__unknown_behavior__"), 10)?;
    if forbidden.status.success() {
        return Err("generated artifact accepted an unrecognized command".into());
    }
    let receipt = json!({
        "schema":"titanu.craft.build.v1",
        "status":"verified",
        "claim":"approved contract behaviors compiled and exercised; no autonomous app cloning",
        "output_dir":dir.to_string_lossy(),
        "artifact_path":binary.to_string_lossy(),
        "artifact_sha256":digest_file(&binary)?,
        "generated_source_sha256":digest_file(&source)?,
        "source_inspection_receipt":b.receipt_file.to_string_lossy(),
        "source_inspection_receipt_sha256":b.receipt_hash,
        "source_target_sha256":b.receipt.target_sha256,
        "craft_evidence_id":b.evidence_id,
        "contract_sha256":digest_file(contract_path)?,
        "checked_behaviors":checks,
        "unauthorized_command_denied":true,
        "test_count":c.behaviors.len()+1,
        "note":"Feature semantics come from the approved contract, not from Craft triage."
    });
    save(&dir.join("build-receipt.json"), &receipt)?;
    Ok(receipt)
}
fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [op, receipt] if op == "plan" => {
            let b = load(Path::new(receipt))?;
            let dir = output_dir()?;
            let view = facts(&b);
            save(&dir.join("facts.json"), &view)?;
            println!("{}", serde_json::to_string_pretty(&view).map_err(|e| e.to_string())?);
            println!("FACTS={}", dir.join("facts.json").display());
            Ok(())
        }
        [op, receipt, contract] if op == "build" => {
            let b = load(Path::new(receipt))?;
            let result = build(&b, Path::new(contract))?;
            println!("{}", serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?);
            Ok(())
        }
        _ => Err("usage: titanu-craft-pipeline plan <inspection-receipt.json> | build <inspection-receipt.json> <approved-contract.json>".into()),
    }
}
fn main() {
    if let Err(err) = run() {
        eprintln!("TITANU_PIPELINE_FAIL: {err}");
        std::process::exit(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_are_strict() {
        assert!(slug("voice-vision"));
        assert!(!slug("../etc"));
        assert!(!slug("VoiceVision"));
        assert!(command_name("play_pause"));
        assert!(!command_name("x;rm -rf"));
    }
    #[test]
    fn generated_rust_contains_real_named_branches() {
        let c = Contract {
            schema:"titanu.behavior.v1".into(), app:"demo".into(), approved:true,
            source_sha256:"a".repeat(64), evidence_id:"some-id".into(),
            behaviors:vec![Behavior{name:"play".into(),response:"playing".into()}],
        };
        let s = generate(&c);
        assert!(s.contains("\"play\"=>"));
        assert!(s.contains("unknown behavior"));
        assert!(!s.contains("std::process::Command"));
    }
}
