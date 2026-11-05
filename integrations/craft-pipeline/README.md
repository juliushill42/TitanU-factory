# TitanU Factory × Craft Anything: Evidence → Plan → Build

This is a bounded **real compile-and-execute** milestone, not a claim of app cloning.

## Real execution path

1. `bootstrap-craft.sh run /absolute/path/to/owned.apk integrations/craft-pipeline/examples/controls.json` invokes the pinned Craft Anything CLI with **explicit Deep mode** and no `--allow-run`.
2. Rust inspection adapter verifies a local regular file and captures Craft's triage evidence, stdout/stderr, binary/target SHA256, and exit status in a receipt.
3. The pipeline verifies the target content and Craft stdout against the hashes, and extracts its reported **kind**, **summary**, **recommended next tools**, and evidence ID into `facts.json`. It **does not pretend** to infer app behavior from a format triage.
4. An explicitly user-approved proposal lists safe, bounded command/response behaviors. The `bind` step copies those into an immutable-per-run approved contract and binds it to the inspected target SHA256 and Craft evidence ID.
5. The pipeline generates real independent Rust source, compiles it with `rustc`, then runs *every declared behavior* and tests unknown-command rejection and source provenance.
6. Only after all checks pass, it emits `build-receipt.json` with artifact, source, input and contract SHA256 hashes and test outcomes. Failures exit nonzero.

## Proposal format

```json
{
  "app": "titan-controls-proof",
  "approved": true,
  "behaviors": [
    {"name": "play", "response": "playing"},
    {"name": "pause", "response": "paused"},
    {"name": "stop", "response": "stopped"}
  ]
}
```

The code generator supports up to 32 independently tested command/response handlers, max 4 KiB per response. No arbitrary source supplied by the proposal is executed. The produced artifact is an actual native executable, **not an Android APK** or a reproduction of the target.

## Manual steps

```bash
# Install and test, without modifying system packages
bash bootstrap-craft.sh

# Inspect an authorized local APK or other supported file. Deep mode is required by upstream.
integrations/craft-inspector/target/release/titanu-craft-inspector inspect /absolute/path/to/owned.apk --deep

# Example output ends with RECEIPT=/path/to/proof/craft/run-.../receipt.json
integrations/craft-pipeline/target/release/titanu-craft-pipeline plan "$RECEIPT"
integrations/craft-pipeline/target/release/titanu-craft-pipeline bind "$RECEIPT" integrations/craft-pipeline/examples/controls.json

# Copy the output CONTRACT=/path/to/approved-contract.json into variable and compile
integrations/craft-pipeline/target/release/titanu-craft-pipeline build "$RECEIPT" "$CONTRACT"
```

## Proof boundary and residual risks

- CI uses **synthetic Craft evidence** while compiling and running a real Rust executable.
- A real external APK has not been inspected by CI. Hardware and Android integration are unproven.
- Craft triage gives file format metadata; it does not supply working application semantics.
- Contracts are explicitly approved; no agent invents application functionality.
- This integration does not sandbox the third-party Craft CLI or create a secure multi-tenant execution service. Run it only against trusted or authorized targets on a controlled host.
- Factory's original 5-moat worker still tests a minimal WASM module, not generated source; this feature does not address that separate flaw.
- Hashes detect unintentional mutation against the local receipt, not independently establish authenticity, signing or supply-chain attestation.

## CI

The workflow builds both Rust executables and verifies parser denial, path/hash enforcement, contract binding, actual `rustc` compilation, launched behavior checks, rejected unknown commands, and provenance reporting. Each test result is in the GitHub Actions logs.
