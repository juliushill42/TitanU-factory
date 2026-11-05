#[cfg(unix)]
mod unix_tests {
    use serde_json::Value;
    use std::{env, fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

    fn sandbox() -> PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = env::temp_dir().join(format!("titanu-craft-test-{}-{stamp}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        dir
    }
    fn binary() -> &'static str { env!("CARGO_BIN_EXE_titanu-craft-inspector") }
    #[test]
    fn clean_room_invokes_cli_without_allow_run_and_produces_receipt() {
        let dir = sandbox();
        let cli = dir.join("craft-anything-cli");
        fs::write(&cli, "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$TEST_ARGS_FILE\"\nprintf '%s\\n' '{\"result\":\"triage fixture\"}'\n").unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o700)).unwrap();
        let input = dir.join("target;not-a-shell-command.apk");
        fs::write(&input, b"example").unwrap();
        let log = dir.join("args.txt");
        let proof = dir.join("proof");
        let result = Command::new(binary()).arg("inspect").arg(&input)
            .env("CRAFT_CLI_BIN", &cli).env("TEST_ARGS_FILE", &log)
            .env("TITAN_CRAFT_PROOF_DIR", &proof).output().unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        let args = fs::read_to_string(log).unwrap();
        assert!(args.contains("--clean-room"));
        assert!(args.contains("triage"));
        assert!(!args.contains("--allow-run"));
        assert!(!args.contains("rebuild"));
        let run = fs::read_dir(proof).unwrap().next().unwrap().unwrap().path();
        let receipt: Value = serde_json::from_slice(&fs::read(run.join("receipt.json")).unwrap()).unwrap();
        assert_eq!(receipt["success"], true);
        assert_eq!(receipt["deep_mode"], false);
        assert_eq!(receipt["execution_authorized"], false);
        assert_eq!(receipt["target_sha256"].as_str().unwrap().len(), 64);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn nonzero_cli_exit_is_not_reported_as_success() {
        let dir = sandbox();
        let cli = dir.join("craft-anything-cli");
        fs::write(&cli, "#!/bin/sh\necho '{\"error\":\"denied\"}'\nexit 1\n").unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o700)).unwrap();
        let proof = dir.join("proof");
        let result = Command::new(binary()).arg("commands").env("CRAFT_CLI_BIN", &cli)
            .env("TITAN_CRAFT_PROOF_DIR", &proof).output().unwrap();
        assert!(!result.status.success());
        let run = fs::read_dir(proof).unwrap().next().unwrap().unwrap().path();
        let receipt: Value = serde_json::from_slice(&fs::read(run.join("receipt.json")).unwrap()).unwrap();
        assert_eq!(receipt["success"], false);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn hash_pin_blocks_mismatched_executable() {
        let dir = sandbox();
        let cli = dir.join("craft-anything-cli");
        fs::write(&cli, "#!/bin/sh\necho '{}'").unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o700)).unwrap();
        let result = Command::new(binary()).arg("commands").env("CRAFT_CLI_BIN", &cli)
            .env("CRAFT_CLI_SHA256", "0000000000000000000000000000000000000000000000000000000000000000")
            .env("TITAN_CRAFT_PROOF_DIR", dir.join("proof")).output().unwrap();
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("SHA256 mismatch"));
        fs::remove_dir_all(dir).unwrap();
    }
}
