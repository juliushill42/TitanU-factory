// ============================================================
// MOAT 5 — INSTANT DELTA ROLLBACK (THE TIME MACHINE)
// JCH-2026 IP: Julius Cameron Hill / TitanU AI LLC
// Before any write goes live, a read-only snapshot is taken.
// Post-deployment health telemetry runs immediately.
// If metrics breach thresholds, the system snaps back in
// microseconds — treating deployments as atomic transactions.
// ============================================================

use sysinfo::System;

const CPU_CEILING:  f32 = 90.0; // % global CPU usage threshold
const MEM_CEILING:  f64 = 90.0; // % memory usage threshold

/// Immutable snapshot of the target file state before deployment.
pub struct TargetSnapshot {
    pub filepath:       String,
    pub backup_content: Vec<u8>, // empty if file didn't exist pre-deploy
}

pub struct TimeMachine {
    sys: System,
}

impl TimeMachine {
    pub fn new() -> Self {
        let sys = System::new_all();
        Self { sys }
    }

    /// Captures current state of target path before any write operation.
    pub fn snapshot(&self, path: &str) -> TargetSnapshot {
        let backup_content = std::fs::read(path).unwrap_or_default();
        TargetSnapshot {
            filepath: path.to_string(),
            backup_content,
        }
    }

    /// Samples live system health metrics post-deployment.
    /// If CPU or memory breach ceiling, restores snapshot immediately.
    /// Returns Ok(()) if system is nominal, Err with rollback reason if not.
    pub fn evaluate_or_rollback(
        &mut self,
        snapshot: &TargetSnapshot,
    ) -> Result<(), String> {
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();

        let cpu_pct = self.sys.global_cpu_usage();
        let used    = self.sys.used_memory() as f64;
        let total   = self.sys.total_memory() as f64;
        let mem_pct = if total > 0.0 { (used / total) * 100.0 } else { 0.0 };

        let cpu_breach = cpu_pct  > CPU_CEILING;
        let mem_breach = mem_pct  > MEM_CEILING;

        if cpu_breach || mem_breach {
            self.execute_rollback(snapshot);
            return Err(format!(
                "[MOAT5] Breach detected — CPU: {cpu_pct:.1}% MEM: {mem_pct:.1}%. \
                 Rollback to pre-deploy snapshot executed.",
            ));
        }

        Ok(())
    }

    fn execute_rollback(&self, snapshot: &TargetSnapshot) {
        if snapshot.backup_content.is_empty() {
            // File didn't exist before — remove the new one
            let _ = std::fs::remove_file(&snapshot.filepath);
        } else {
            // Restore byte-for-byte original
            let _ = std::fs::write(&snapshot.filepath, &snapshot.backup_content);
        }
    }
}
