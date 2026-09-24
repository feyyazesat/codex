#!/usr/bin/env bash

# Build the native release code-mode host with the Codex-published V8 pair.
# A plain Cargo build asks rusty_v8 for an upstream sandbox archive, which is
# not published for this target.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
codex_rs_root="$repo_root/codex-rs"

rusty_v8_pair="$({
  PYTHONPATH="$repo_root/scripts" python3 -c '
from codex_package.targets import TARGET_SPECS, default_target
from codex_package.v8 import fetch_codex_v8_artifacts

pair = fetch_codex_v8_artifacts(TARGET_SPECS[default_target()])
print(pair.archive)
print(pair.binding)
'
})" || {
  echo "failed to download and verify the Codex V8 archive and binding" >&2
  exit 1
}

rusty_v8_archive="${rusty_v8_pair%%$'\n'*}"
rusty_v8_binding="${rusty_v8_pair#*$'\n'}"

if [[ "$rusty_v8_archive" == "$rusty_v8_binding" || -z "$rusty_v8_binding" ]]; then
  echo "failed to resolve the Codex V8 archive and binding" >&2
  exit 1
fi

cd "$codex_rs_root"
RUSTY_V8_ARCHIVE="$rusty_v8_archive" \
RUSTY_V8_SRC_BINDING_PATH="$rusty_v8_binding" \
  cargo build --release --bin codex-code-mode-host
