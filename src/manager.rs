// ============================================================
// MANAGER MOTE — TASK ASSIGNMENT ENGINE
// JCH-2026 IP: Julius Cameron Hill / TitanU AI LLC
// Holds all downward pipelines to Workers.
// Never accepts upward queries — only dispatches and reads
// sealed receipts from the aggregated receipt channel.
// ============================================================

use std::sync::mpsc::{Receiver, Sender};
use crate::moat1_translator::{StructuredCommand, TranslatorGate};
use crate::moat4_pipes::ExecutionReceipt;

pub struct ManagerMote {
    /// One Sender per Worker — strict 1:1 isolated downward lines
    pub worker_pipelines: Vec<Sender<StructuredCommand>>,
    /// Aggregated receipt channel — all Workers funnel results here
    pub receipt_rx: Receiver<ExecutionReceipt>,
}

impl ManagerMote {
    /// Translates raw JSON through Moat 1, dispatches to target Worker via Moat 4.
    pub fn assign_task(
        &self,
        worker_index: usize,
        raw_json: &str,
        task_counter: usize,
    ) {
        match TranslatorGate::sanitize_and_translate(raw_json, task_counter) {
            Ok(clean_cmd) => {
                match self.worker_pipelines.get(worker_index) {
                    Some(pipeline) => {
                        if pipeline.send(clean_cmd).is_err() {
                            eprintln!(
                                "[MANAGER] Pipeline to Worker {worker_index} closed — \
                                 task {task_counter} not delivered."
                            );
                        }
                    }
                    None => eprintln!(
                        "[MANAGER] No Worker at index {worker_index}. \
                         Task {task_counter} dropped."
                    ),
                }
            }
            Err(reason) => {
                eprintln!(
                    "[MANAGER] Task {task_counter} REJECTED at Translator Gate: {reason}"
                );
            }
        }
    }

    /// Consumes the ManagerMote: closes all Worker command pipelines, then
    /// blocks collecting `expected` receipts before returning.
    /// Dropping worker_pipelines signals Workers to exit their recv() loops.
    pub fn close_and_drain(self, expected: usize) {
        // Drop all command Senders — Workers see channel closed, exit execution_loop
        drop(self.worker_pipelines);

        println!("[MANAGER] Pipelines closed — draining {expected} receipts...\n");

        let mut collected = 0;
        while collected < expected {
            match self.receipt_rx.recv() {
                Ok(receipt) => {
                    let status = if receipt.success { "SUCCESS" } else { "FAILED" };
                    println!(
                        "[MANAGER] Receipt task_id={} [{status}] → {}",
                        receipt.task_id, receipt.log
                    );
                    collected += 1;
                }
                Err(_) => {
                    // All worker senders dropped before we got expected receipts
                    println!("[MANAGER] Receipt channel exhausted after {collected} receipts.");
                    break;
                }
            }
        }
    }
}
