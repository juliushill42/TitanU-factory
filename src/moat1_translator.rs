// ============================================================
// MOAT 1 — THE TRANSLATOR GATE
// JCH-2026 IP: Julius Cameron Hill / TitanU AI LLC
// Raw natural language is structurally translated into a
// strict, non-executable schema before the Manager sees it.
// Prompt injection is mechanically neutralized at this layer.
// ============================================================

use serde::{Deserialize, Serialize};

const MAX_PAYLOAD_BYTES: usize = 65_536;
const MAX_PATH_LEN:      usize = 512;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeatureType {
    WriteFile,
    DeployPatch,
    UpdateRoute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredCommand {
    pub task_id:        usize,
    pub target_feature: FeatureType,
    pub code_payload:   String,
    pub target_path:    String,
}

pub struct TranslatorGate;

impl TranslatorGate {
    /// Accepts raw JSON input and enforces structural validation.
    /// Returns a clean StructuredCommand or a static error string.
    pub fn sanitize_and_translate(
        raw_input: &str,
        assigned_id: usize,
    ) -> Result<StructuredCommand, &'static str> {
        // Strip leading/trailing whitespace — no tolerance for hidden control chars
        let trimmed = raw_input.trim();
        if trimmed.is_empty() {
            return Err("[MOAT1] Empty input rejected.");
        }

        // Parse strictly into schema — serde rejects any extra or missing fields
        let parsed: StructuredCommand = serde_json::from_str(trimmed)
            .map_err(|_| "[MOAT1] Schema validation failed. Injection/malformation blocked.")?;

        // Path traversal / sensitive directory checks
        let dangerous_paths = ["../", ".\\", "/etc", "/proc", "/sys", "/dev", "C:\\Windows"];
        for dp in &dangerous_paths {
            if parsed.target_path.contains(dp) {
                return Err("[MOAT1] Directory traversal attempt detected. Blocked.");
            }
        }

        // Absolute path must resolve within ./output/ namespace
        if !parsed.target_path.starts_with("./output/") {
            return Err("[MOAT1] Target path outside allowed ./output/ namespace. Blocked.");
        }

        // Payload size guard
        if parsed.code_payload.len() > MAX_PAYLOAD_BYTES {
            return Err("[MOAT1] Payload exceeds 64KB ceiling. Blocked.");
        }

        // Path length guard
        if parsed.target_path.len() > MAX_PATH_LEN {
            return Err("[MOAT1] Target path exceeds length limit. Blocked.");
        }

        Ok(StructuredCommand {
            task_id: assigned_id,
            ..parsed
        })
    }
}
