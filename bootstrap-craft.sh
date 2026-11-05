#!/usr/bin/env bash
# TitanU Factory × Craft Anything. Additive; leaves the original 5-moat engine untouched.
set -Eeuo pipefail
umask 077

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
CRATE="$ROOT/integrations/craft-inspector/Cargo.toml"
PIPE_CRATE="$ROOT/integrations/craft-pipeline/Cargo.toml"
PIN='869eb8548ae3cf9162181659c3ab47441b9c300d'

[[ -f "$ROOT/Cargo.toml" && -f "$CRATE" ]] || { echo 'Run from the TitanU-factory repository with the Craft integration files.' >&2; exit 2; }
for bin in git cargo rustc sha256sum; do
  command -v "$bin" >/dev/null || { echo "Required executable missing: $bin" >&2; exit 2; }
done

# Upstream currently requires Rust >=1.97. Do not upgrade the host or overwrite toolchains.
RUST_CMD=(cargo)
if [[ -z "${CRAFT_CLI_BIN:-}" ]] && ! command -v craft-anything-cli >/dev/null 2>&1; then
  if command -v rustup >/dev/null 2>&1; then
    rustup toolchain install 1.97.0 --profile minimal --no-self-update
    RUST_CMD=(cargo +1.97.0)
  fi
  SRC="$ROOT/.craft-source"
  if [[ ! -e "$SRC/.git" ]]; then
    mkdir -p "$SRC"
    git -C "$SRC" init -q
    git -C "$SRC" remote add origin https://github.com/storytold/craft-anything.git
  fi
  git -C "$SRC" fetch --depth 1 origin "$PIN"
  git -C "$SRC" checkout --detach -q FETCH_HEAD
  [[ "$(git -C "$SRC" rev-parse HEAD)" == "$PIN" ]] || { echo 'Upstream commit does not match pin' >&2; exit 1; }
  echo "Building pinned Craft CLI: $PIN"
  "${RUST_CMD[@]}" install --locked --path "$SRC/apps/craft-anything-cli" --root "$ROOT/.craft-tools" --force
  export CRAFT_CLI_BIN="$ROOT/.craft-tools/bin/craft-anything-cli"
fi

if [[ -z "${CRAFT_CLI_BIN:-}" ]]; then
  CRAFT_CLI_BIN="$(command -v craft-anything-cli)"
  export CRAFT_CLI_BIN
fi
[[ -f "$CRAFT_CLI_BIN" ]] || { echo "Craft CLI missing: $CRAFT_CLI_BIN" >&2; exit 2; }
export CRAFT_CLI_SHA256="$(sha256sum "$CRAFT_CLI_BIN" | awk '{print $1}')"
export TITAN_CRAFT_PROOF_DIR="${TITAN_CRAFT_PROOF_DIR:-$ROOT/proof/craft}"

cargo test --manifest-path "$CRATE"
cargo build --release --manifest-path "$CRATE"
INSPECTOR="$ROOT/integrations/craft-inspector/target/release/titanu-craft-inspector"
cargo test --manifest-path "$PIPE_CRATE"
cargo build --release --manifest-path "$PIPE_CRATE"
PIPELINE="$ROOT/integrations/craft-pipeline/target/release/titanu-craft-pipeline"

"$INSPECTOR" commands
# Doctor diagnoses local optional engines; a nonzero return is recorded, not disguised as success.
"$INSPECTOR" doctor || echo 'Craft doctor reported a failure; read its recorded receipt.' >&2

printf '\nINTEGRATION READY\nOriginal Factory: %s\nCraft executable hash: %s\nReceipts: %s\n' "$ROOT" "$CRAFT_CLI_SHA256" "$TITAN_CRAFT_PROOF_DIR"
printf 'Inspect your own APK: %q inspect %q --deep\n' "$INSPECTOR" '/path/to/owned.apk'

case "${1:-}" in
  "")
    echo 'To inspect and build a bounded Rust app from your approved behavior proposal:'
    echo '  bash bootstrap-craft.sh run /absolute/path/to/owned.apk /absolute/path/to/approved-proposal.json'
    ;;
  run)
    [[ $# -eq 3 && -f "$2" && -f "$3" ]] || { echo 'usage: bootstrap-craft.sh run /path/to/owned.apk /path/to/approved-proposal.json' >&2; exit 2; }
    # Explicit opt-in to Deep mode; never permit autonomous rebuild or execution of copied code.
    INSPECT_OUTPUT="$("$INSPECTOR" inspect "$2" --deep)"
    printf '%s\n' "$INSPECT_OUTPUT"
    INSPECT_RECEIPT="$(printf '%s\n' "$INSPECT_OUTPUT" | sed -n 's/^RECEIPT=//p' | tail -n 1)"
    [[ -n "$INSPECT_RECEIPT" && -f "$INSPECT_RECEIPT" ]] || { echo 'Craft inspection did not produce a verifiable receipt' >&2; exit 1; }
    "$PIPELINE" plan "$INSPECT_RECEIPT"
    CONTRACT_OUTPUT="$("$PIPELINE" bind "$INSPECT_RECEIPT" "$3")"
    printf '%s\n' "$CONTRACT_OUTPUT"
    CONTRACT="$(printf '%s\n' "$CONTRACT_OUTPUT" | sed -n 's/^CONTRACT=//p' | tail -n 1)"
    [[ -n "$CONTRACT" && -f "$CONTRACT" ]] || { echo 'Evidence-bound approved contract not produced' >&2; exit 1; }
    "$PIPELINE" build "$INSPECT_RECEIPT" "$CONTRACT"
    ;;
  plan)
    [[ $# -eq 2 ]] || { echo 'usage: bootstrap-craft.sh plan /path/to/inspection/receipt.json' >&2; exit 2; }
    "$PIPELINE" plan "$2"
    ;;
  build)
    [[ $# -eq 3 ]] || { echo 'usage: bootstrap-craft.sh build /path/to/receipt.json /path/to/contract.json' >&2; exit 2; }
    "$PIPELINE" build "$2" "$3"
    ;;
  *)
    echo 'usage: bootstrap-craft.sh [run <authorized-file> <approved-proposal.json> | plan <receipt> | build <receipt> <contract>]' >&2
    exit 2
    ;;
esac
