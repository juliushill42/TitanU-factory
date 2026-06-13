// ============================================================
// MOAT 2 — THE BURNER SANDBOX
// JCH-2026 IP: Julius Cameron Hill / TitanU AI LLC
// Worker-generated WASM is executed inside a disposable
// wasmi runtime with zero OS linkage. Spins up, tests, and
// self-destructs — leaving zero persistent footprint.
// ============================================================

use std::io::Cursor;
use wasmi::{Engine, Linker, Module, Store};

pub struct BurnerSandbox;

impl BurnerSandbox {
    /// Executes raw WASM bytes inside a fully isolated wasmi runtime.
    /// The runtime has no imports — no file, network, or device access.
    /// Returns Ok(()) if the module compiles and starts cleanly.
    pub fn test_execute(wasm_bytes: &[u8]) -> Result<(), String> {
        // Validate WASM magic header before handing bytes to engine
        if wasm_bytes.len() < 8 {
            return Err("[MOAT2] WASM payload too small to be a valid module.".to_string());
        }
        if &wasm_bytes[0..4] != b"\0asm" {
            return Err("[MOAT2] Invalid WASM magic header. Module rejected.".to_string());
        }

        let engine = Engine::default();

        // Cursor<&[u8]> implements Read + Seek — required by wasmi 0.31 Module::new
        let cursor = Cursor::new(wasm_bytes);
        let module = Module::new(&engine, cursor)
            .map_err(|e| format!("[MOAT2] WASM compile failed: {e}"))?;

        let mut store: Store<()> = Store::new(&engine, ());
        let linker: Linker<()> = Linker::new(&engine);

        // Instantiate with empty linker — zero host function access
        let _instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| format!("[MOAT2] Instantiation failed: {e}"))?
            .start(&mut store)
            .map_err(|e| format!("[MOAT2] Module start/trap: {e}"))?;

        // Runtime and store drop here — complete sandbox teardown
        Ok(())
    }

    /// Returns a minimal valid WASM module (magic + version, no body).
    /// Used to confirm the sandbox runtime itself is operational.
    pub fn minimal_valid_wasm() -> Vec<u8> {
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }
}
