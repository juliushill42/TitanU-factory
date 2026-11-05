//! TitanU Factory → Craft Anything evidence adapter.
//! Inspects explicitly supplied targets. Never calls rebuild/allow-run/deploy.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{env, fs::{self, File}, io::{Read, Write}, path::{Path, PathBuf}, process::{Command, Stdio}, thread, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

fn sha256(path: &Path) -> Result<String, String> {
    let mut f = File::open(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut sha = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        sha.update(&buf[..n]);
    }
    Ok(format!("{:x}", sha.finalize()))
}

fn executable(name: &str) -> Result<PathBuf, String> {
    let p = Path::new(name);
    let candidates: Vec<PathBuf> = if p.components().count() > 1 || p.is_absolute() {
        vec![p.to_path_buf()]
    } else {
        env::split_paths(&env::var_os("PATH").unwrap_or_default())
            .map(|d| d.join(name)).collect()
    };
    for candidate in candidates {
        if candidate.is_file() {
            let absolute = fs::canonicalize(&candidate).map_err(|e| e.to_string())?;
            #[cfg(unix)] {
                use std::os::unix::fs::PermissionsExt;
                if fs::metadata(&absolute).map_err(|e| e.to_string())?.permissions().mode() & 0o111 == 0 { continue; }
            }
            return Ok(absolute);
        }
    }
    Err(format!("Craft Anything CLI not found: {name}. Run bootstrap-craft.sh first."))
}

fn run_dir(root: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let ns = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    for i in 0..16 {
        let p = root.join(format!("run-{}-{}-{i}", ns, std::process::id()));
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
    Err("could not allocate unique proof directory".to_string())
}

fn timeout_seconds() -> u64 {
    env::var("TITAN_CRAFT_TIMEOUT_SECS").ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|n| (1..=1800).contains(n)).unwrap_or(120)
}

fn main() {
    if let Err(e) = execute() {
        eprintln!("TITANU_CRAFT_FAIL: {e}");
        std::process::exit(1);
    }
}

fn execute() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args[0] == "help" || args[0] == "--help" {
        println!("TitanU Craft Inspector\n  commands\n  doctor\n  inspect <local-file> [--deep]\n\nLocal file triage requires explicit --deep and authorization. Without it, inspection is refused.\nAll results and SHA256 receipts go to TITAN_CRAFT_PROOF_DIR (default ./proof/craft).\nNo generated code is executed or deployed.");
        return Ok(());
    }
    let (operation, target, deep) = match args.as_slice() {
        [op] if op == "commands" || op == "doctor" => (op.as_str(), None, false),
        [op, _file] if op == "inspect" => return Err("Craft CLI refuses local-file triage in clean-room; use inspect <file> --deep only for authorized targets".into()),
        [op, file, flag] if op == "inspect" && flag == "--deep" => ("triage", Some(file.as_str()), true),
        _ => return Err("usage: commands | doctor | inspect <local-file> [--deep]".into()),
    };
    let target = match target {
        Some(raw) => {
            let path = fs::canonicalize(raw).map_err(|e| format!("invalid target {raw}: {e}"))?;
            if !path.is_file() { return Err("inspection target must be a regular local file".into()); }
            let size = fs::metadata(&path).map_err(|e| e.to_string())?.len();
            if size > 512 * 1024 * 1024 { return Err("target exceeds 512 MiB limit".into()); }
            Some(path)
        }
        None => None,
    };
    let cli = executable(&env::var("CRAFT_CLI_BIN").unwrap_or_else(|_| "craft-anything-cli".to_owned()))?;
    let cli_hash = sha256(&cli)?;
    if let Ok(expected) = env::var("CRAFT_CLI_SHA256") {
        if expected.to_lowercase() != cli_hash { return Err("Craft CLI binary SHA256 mismatch".into()); }
    }
    let base = env::var_os("TITAN_CRAFT_PROOF_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("proof/craft"));
    let dir = run_dir(&base)?;
    let stdout_path = dir.join("stdout.json");
    let stderr_path = dir.join("stderr.txt");
    let mut command = Command::new(&cli);
    command.arg("--output-format").arg("json");
    if operation == "triage" {
        command.arg("--project").arg(dir.join("investigation.cany"));
        command.arg(if deep { "--deep" } else { "--clean-room" });
    }
    command.arg(operation);
    if let Some(ref target_path) = target { command.arg(target_path); }
    command.stdin(Stdio::null())
        .stdout(Stdio::from(File::create(&stdout_path).map_err(|e| e.to_string())?))
        .stderr(Stdio::from(File::create(&stderr_path).map_err(|e| e.to_string())?));
    let mut child = command.spawn().map_err(|e| format!("spawn Craft CLI: {e}"))?;
    let start = Instant::now();
    let (code, timed_out) = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? { break (status.code(), false); }
        if start.elapsed() > Duration::from_secs(timeout_seconds()) {
            let _ = child.kill();
            let _ = child.wait();
            break (None, true);
        }
        thread::sleep(Duration::from_millis(100));
    };
    let stdout = fs::read(&stdout_path).map_err(|e| e.to_string())?;
    let parsed = serde_json::from_slice::<Value>(&stdout).ok();
    let has_error = parsed.as_ref().and_then(|v| v.get("error")).is_some();
    let success = code == Some(0) && !timed_out && !has_error && (operation != "triage" || parsed.is_some());
    let input_hash = target.as_ref().map(|p| sha256(p)).transpose()?;
    let receipt = json!({
        "schema":"titanu.craft.inspection.v1",
        "operation":operation,
        "success":success,
        "exit_code":code,
        "timed_out":timed_out,
        "deep_mode":deep,
        "execution_authorized":false,
        "target":target.as_ref().map(|p| p.to_string_lossy().to_string()),
        "target_sha256":input_hash,
        "cli_path":cli.to_string_lossy(),
        "cli_sha256":cli_hash,
        "stdout_sha256":sha256(&stdout_path)?,
        "stderr_sha256":sha256(&stderr_path)?,
        "elapsed_ms":start.elapsed().as_millis(),
        "notes":"Evidence capture only. Does not verify generated code or sandbox the external CLI."
    });
    let receipt_path = dir.join("receipt.json");
    let mut f = File::create(&receipt_path).map_err(|e| e.to_string())?;
    f.write_all(serde_json::to_string_pretty(&receipt).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    println!("{}", serde_json::to_string_pretty(&receipt).map_err(|e| e.to_string())?);
    println!("RECEIPT={}", receipt_path.display());
    if !success { return Err(format!("Craft {operation} failed; inspect {} and {}", stdout_path.display(), stderr_path.display())); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sha256_known_vector() {
        let path = env::temp_dir().join(format!("titanu-craft-sha-test-{}", std::process::id()));
        fs::write(&path, b"abc").unwrap();
        assert_eq!(sha256(&path).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn timeout_is_bounded() { assert!((1..=1800).contains(&timeout_seconds())); }
}
