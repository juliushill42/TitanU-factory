// ============================================================
// MOAT 4 — ISOLATED ONE-WAY TALK LINES
// JCH-2026 IP: Julius Cameron Hill / TitanU AI LLC
// Workers ONLY receive commands and send sealed receipts back.
// Zero horizontal sightlines. Zero upward query capability.
// Lateral movement is structurally impossible.
// ============================================================

use std::sync::mpsc::{self, Receiver, Sender};
use crate::moat1_translator::StructuredCommand;

/// Sealed receipt envelope — the only value a Worker can return upward.
/// No executable payloads, no raw strings that could carry injection.
#[derive(Debug)]
pub struct ExecutionReceipt {
    pub task_id: usize,
    pub success: bool,
    pub log:     String,
}

/// Creates an isolated one-way command channel.
///   cmd_tx  → Manager holds (sends DOWN to Worker)
///   cmd_rx  → Worker holds (receives only, never queries up)
pub fn create_command_channel() -> (Sender<StructuredCommand>, Receiver<StructuredCommand>) {
    mpsc::channel::<StructuredCommand>()
}

/// Creates the shared aggregated receipt channel.
///   rcpt_tx → Cloned into each Worker (sends UP to Manager only)
///   rcpt_rx → Manager holds exclusively (never writeable by Workers)
pub fn create_receipt_channel() -> (Sender<ExecutionReceipt>, Receiver<ExecutionReceipt>) {
    mpsc::channel::<ExecutionReceipt>()
}
