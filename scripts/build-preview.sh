#!/usr/bin/env bash
# Build on Linux ARM64 (for example, GitHub's ubuntu-24.04-arm runner).
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ $(uname -m) != aarch64 || $(uname -s) != Linux ]]; then
  echo 'Preview bundles must be built on Linux ARM64.' >&2
  exit 1
fi
cargo build --locked --release -p boltrunner-server
./scripts/build-web.sh --release
# Avoid including an earlier bundle in a manually repeated build.
rm -f dist/preview.tar.gz
package_dir=$(mktemp -d "${TMPDIR:-/tmp}/boltrunner-preview.XXXXXX")
trap 'rm -rf -- "$package_dir"' EXIT
server="${CARGO_TARGET_DIR:-target}/release/boltrunner-server"
cp "$server" "$package_dir/"
"$server" --print-build > "$package_dir/simulation-build.txt"
cp -R dist "$package_dir/dist"
# Editable source archives are available in the repository and full web build;
# preview uploads contain the playable package only.
tar -C "$package_dir" \
  --exclude='dist/pkg/*/maps-source.zip' \
  --exclude='dist/pkg/*/starter-pistol-source.zip' \
  -czf dist/preview.tar.gz boltrunner-server simulation-build.txt dist
bundle_bytes=$(stat -c %s dist/preview.tar.gz)
if (( bundle_bytes > 256 * 1024 * 1024 )); then
  echo "Preview bundle exceeds the 256 MiB upload limit ($bundle_bytes bytes)." >&2
  exit 1
fi
echo "Preview bundle: dist/preview.tar.gz ($bundle_bytes bytes)"
