// ============================================================
// MOAT 3 — THE HARD-RULE CODE INSPECTOR
// JCH-2026 IP: Julius Cameron Hill / TitanU AI LLC
// Deterministic, zero-AI static analysis gate.
// Blocks blacklisted tokens before anything reaches disk.
// No probabilistic guessing — mechanical enforcement only.
// ============================================================

/// Blacklisted token categories:
///   NET  — unauthorized network binding / socket creation
///   FS   — destructive filesystem operations
///   EXEC — shell execution / process spawning
///   MEM  — raw memory/device access
///   PRIV — privilege escalation vectors
const BLACKLIST: &[(&str, &str)] = &[
    ("std::net::TcpListener",        "NET:  raw TCP listener forbidden"),
    ("std::net::UdpSocket",          "NET:  raw UDP socket forbidden"),
    ("std::net::TcpStream::connect", "NET:  outbound TCP connect forbidden"),
    ("std::fs::remove_dir_all",      "FS:   recursive directory removal forbidden"),
    ("std::fs::remove_file",         "FS:   direct file removal forbidden"),
    ("std::process::Command",        "EXEC: shell command spawning forbidden"),
    ("std::process::exit",           "EXEC: forced process exit forbidden"),
    ("libc::system",                 "EXEC: libc system() call forbidden"),
    ("/dev/mem",                     "MEM:  raw memory device access forbidden"),
    ("/dev/kmem",                    "MEM:  kernel memory access forbidden"),
    ("chmod",                        "PRIV: chmod syscall pattern forbidden"),
    ("setuid",                       "PRIV: setuid privilege escalation forbidden"),
    ("ptrace",                       "PRIV: ptrace debugging attach forbidden"),
    ("mprotect",                     "MEM:  mprotect page permission change forbidden"),
    ("include_str!(\"/etc",          "FS:   /etc file inclusion forbidden"),
];

pub struct InspectionReport {
    pub passed:     bool,
    pub violations: Vec<String>,
}

pub struct CodeInspector;

impl CodeInspector {
    /// Runs deterministic static token analysis against the full blacklist.
    /// Returns an InspectionReport — caller decides whether to block.
    pub fn inspect(code: &str) -> InspectionReport {
        let mut violations: Vec<String> = Vec::new();

        for (token, reason) in BLACKLIST {
            if code.contains(token) {
                violations.push(format!("[MOAT3] BLOCKED — {reason} (token: `{token}`)"));
            }
        }

        InspectionReport {
            passed: violations.is_empty(),
            violations,
        }
    }
}
