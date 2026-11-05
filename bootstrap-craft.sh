#!/usr/bin/env bash
# TitanU Factory × Craft Anything. Additive; leaves the original 5-moat engine untouched.
set -Eeuo pipefail
umask 077

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
CRATE="$ROOT/integrations/craft-inspector/Cargo.toml"
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
"$INSPECTOR" commands
# Doctor diagnoses local optional engines; a nonzero return is recorded, not disguised as success.
"$INSPECTOR" doctor || echo 'Craft doctor reported a failure; read its recorded receipt.' >&2

printf '\nINTEGRATION READY\nOriginal Factory: %s\nCraft executable hash: %s\nReceipts: %s\n' "$ROOT" "$CRAFT_CLI_SHA256" "$TITAN_CRAFT_PROOF_DIR"
printf 'Inspect your own APK: %q inspect %q --deep\n' "$INSPECTOR" '/path/to/owned.apk'
