#!/usr/bin/env bash
# Produce deployment artifacts for the institutional invoice controls.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
output_dir="${1:-target/institutional}"

for contract in invoice-escrow secondary-market; do
  stellar contract build --locked --package "$contract" --optimize --out-dir "$output_dir"
  artifact="$output_dir/${contract//-/_}.wasm"
  size_bytes="$(wc -c < "$artifact")"
  # Leave room for the ledger-entry envelope under the 128 KiB code-entry limit.
  if (( size_bytes > 130000 )); then
    echo "Deployment artifact exceeds the 130000-byte budget: $artifact ($size_bytes bytes)" >&2
    exit 1
  fi
  # Ensure optimization preserves a readable contract specification.
  stellar contract bindings rust --wasm "$artifact" > /dev/null
done
