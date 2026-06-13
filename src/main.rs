// ============================================================
// TITAN FACTORY — 5-MOAT AUTONOMOUS BUILD ENGINE
// JCH-2026 IP: Julius Cameron Hill / TitanU AI LLC
//
// Flow:
//   create_receipt_channel() → shared aggregated receipt bus
//   create_command_channel() × N → isolated 1:1 cmd lines per Worker
//   ManagerMote dispatches (post Moat 1) down isolated pipelines.
//   Workers run Moat 3 → Moat 2 → Moat 5 on every task.
//   close_and_drain() drops cmd Senders → Workers exit → receipts drained.
// ============================================================

mod moat1_translator;
mod moat2_sandbox;
mod moat3_inspector;
mod moat4_pipes;
mod moat5_rollback;
mod manager;
mod worker;

use std::thread;

use manager::ManagerMote;
use moat4_pipes::{create_command_channel, create_receipt_channel};
use worker::WorkerMote;

const NUM_WORKERS: usize = 2;

fn main() {
    println!("╔══════════════════════════════════════════════════╗");
    println!("║  TITAN FACTORY — 5-Moat Build Engine ONLINE      ║");
    println!("║  JCH-2026 IP · Julius Cameron Hill · TitanU AI   ║");
    println!("╚══════════════════════════════════════════════════╝\n");

    // Shared aggregated receipt channel — all Workers write here, Manager reads
    let (shared_rcpt_tx, shared_rcpt_rx) = create_receipt_channel();

    let mut worker_cmd_txs = Vec::new();

    // Spawn N isolated Workers, each in its own thread
    for id in 0..NUM_WORKERS {
        // Per-Worker isolated command channel (Moat 4)
        let (cmd_tx, cmd_rx) = create_command_channel();
        worker_cmd_txs.push(cmd_tx);

        // Clone shared receipt Sender — Worker can only write, never read
        let rcpt_tx_clone = shared_rcpt_tx.clone();

        let mote = WorkerMote {
            worker_id:  id,
            command_rx: cmd_rx,
            receipt_tx: rcpt_tx_clone,
        };

        thread::spawn(move || mote.execution_loop());
        println!("[MAIN] Worker {id} isolated thread spawned.");
    }

    // Drop original shared_rcpt_tx — only Worker clones remain alive.
    // When all Workers exit and drop their clone, receipt_rx.recv() returns Err.
    drop(shared_rcpt_tx);

    let manager = ManagerMote {
        worker_pipelines: worker_cmd_txs,
        receipt_rx:       shared_rcpt_rx,
    };

    println!();

    // Ensure output directory exists before Workers attempt writes
    std::fs::create_dir_all("./output")
        .expect("[MAIN] Failed to create output directory.");

    // ── Task Batch ──────────────────────────────────────────────────────────

    // Task 1: VALID — passes all gates → SUCCESS
    let task_1 = r#"{
        "task_id": 1,
        "target_feature": "WriteFile",
        "code_payload": "fn feature_one() { println!(\"Feature One deployed.\"); }",
        "target_path": "./output/feature_one.rs"
    }"#;

    // Task 2: VALID — passes all gates → SUCCESS
    let task_2 = r#"{
        "task_id": 2,
        "target_feature": "UpdateRoute",
        "code_payload": "fn update_route() { println!(\"Route patch applied.\"); }",
        "target_path": "./output/route_patch.rs"
    }"#;

    // Task 3: BLOCKED at Moat 1 — path escapes ./output/ namespace
    let task_3 = r#"{
        "task_id": 3,
        "target_feature": "WriteFile",
        "code_payload": "malicious payload here",
        "target_path": "../../../etc/passwd"
    }"#;

    // Task 4: PASSES Moat 1, BLOCKED at Moat 3 — blacklisted token
    let task_4 = r#"{
        "task_id": 4,
        "target_feature": "DeployPatch",
        "code_payload": "use std::process::Command; fn evil() { Command::new(\"rm\").spawn().unwrap(); }",
        "target_path": "./output/evil_patch.rs"
    }"#;

    println!("[MAIN] → Task 1 to Worker 0 (valid WriteFile)");
    manager.assign_task(0, task_1, 1);

    println!("[MAIN] → Task 2 to Worker 1 (valid UpdateRoute)");
    manager.assign_task(1, task_2, 2);

    println!("[MAIN] → Task 3 to Worker 0 (traversal attack — Moat 1 blocks)");
    manager.assign_task(0, task_3, 3);

    println!("[MAIN] → Task 4 to Worker 0 (injection attack — Moat 3 blocks)");
    manager.assign_task(0, task_4, 4);

    println!();

    // Receipts expected: Task 1 ✓, Task 2 ✓, Task 4 ✓ (FAILURE)
    // Task 3 blocked at Moat 1 — never reaches a Worker — no receipt
    manager.close_and_drain(3);

    println!("\n[MAIN] Factory run complete. All moats held.");
}
