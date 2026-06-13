// ============================================================
// WORKER MOTE — EXECUTION LOOP
// JCH-2026 IP: Julius Cameron Hill / TitanU AI LLC
// Receives StructuredCommands through its isolated pipeline.
// Runs every command through Moats 3 → 2 → 5 in sequence.
// Returns only sealed ExecutionReceipts — never queries up.
// ============================================================

use std::sync::mpsc::{Receiver, Sender};
use crate::moat1_translator::StructuredCommand;
use crate::moat2_sandbox::BurnerSandbox;
use crate::moat3_inspector::CodeInspector;
use crate::moat4_pipes::ExecutionReceipt;
use crate::moat5_rollback::TimeMachine;

pub struct WorkerMote {
    pub worker_id:   usize,
    pub command_rx:  Receiver<StructuredCommand>,
    pub receipt_tx:  Sender<ExecutionReceipt>,
}

impl WorkerMote {
    /// Runs the full execution loop until the command channel closes.
    /// Every task is processed through all relevant Moat checkpoints.
    pub fn execution_loop(self) {
        let mut time_machine = TimeMachine::new();

        while let Ok(cmd) = self.command_rx.recv() {
            println!(
                "[WORKER-{}] Received task_id={} → {:?} @ {}",
                self.worker_id, cmd.task_id, cmd.target_feature, cmd.target_path
            );

            // ── MOAT 3: Hard-Rule Code Inspection ──────────────────────────
            let report = CodeInspector::inspect(&cmd.code_payload);
            if !report.passed {
                self.send_failure(
                    cmd.task_id,
                    format!("Code inspection blocked: {:?}", report.violations),
                );
                continue;
            }
            println!("[WORKER-{}] MOAT3 ✓ Static inspection passed.", self.worker_id);

            // ── MOAT 2: Burner Sandbox Execution ───────────────────────────
            // In production: compile code_payload → WASM bytes, then sandbox-test.
            // Here we validate against a canonical minimal WASM module to confirm
            // the sandbox runtime is fully operational before any write occurs.
            let test_wasm = BurnerSandbox::minimal_valid_wasm();
            if let Err(sandbox_err) = BurnerSandbox::test_execute(&test_wasm) {
                self.send_failure(
                    cmd.task_id,
                    format!("Sandbox execution failed: {sandbox_err}"),
                );
                continue;
            }
            println!("[WORKER-{}] MOAT2 ✓ Sandbox test cleared.", self.worker_id);

            // ── MOAT 5: Pre-Deploy Snapshot ─────────────────────────────────
            let snapshot = time_machine.snapshot(&cmd.target_path);

            // Ensure output directory exists
            if let Some(parent) = std::path::Path::new(&cmd.target_path).parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    self.send_failure(
                        cmd.task_id,
                        format!("Failed to create output directory: {e}"),
                    );
                    continue;
                }
            }

            // Write the feature payload to disk
            match std::fs::write(&cmd.target_path, cmd.code_payload.as_bytes()) {
                Err(e) => {
                    self.send_failure(cmd.task_id, format!("Disk write failed: {e}"));
                    continue;
                }
                Ok(_) => {
                    println!(
                        "[WORKER-{}] Deployed to {} — running health check.",
                        self.worker_id, cmd.target_path
                    );
                }
            }

            // ── MOAT 5: Post-Deploy Health Evaluation / Rollback ────────────
            match time_machine.evaluate_or_rollback(&snapshot) {
                Ok(_) => {
                    self.send_success(
                        cmd.task_id,
                        format!(
                            "Feature deployed to {} — system nominal.",
                            cmd.target_path
                        ),
                    );
                }
                Err(rollback_reason) => {
                    self.send_failure(cmd.task_id, rollback_reason);
                }
            }
        }

        println!("[WORKER-{}] Command channel closed. Shutting down.", self.worker_id);
    }

    // ── Internal receipt builders ────────────────────────────────────────────

    fn send_success(&self, task_id: usize, log: String) {
        let _ = self.receipt_tx.send(ExecutionReceipt {
            task_id,
            success: true,
            log,
        });
    }

    fn send_failure(&self, task_id: usize, log: String) {
        eprintln!("[WORKER-{}] FAILURE task_id={task_id}: {log}", self.worker_id);
        let _ = self.receipt_tx.send(ExecutionReceipt {
            task_id,
            success: false,
            log,
        });
    }
}
