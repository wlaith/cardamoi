#!/bin/sh
set -eu
VERSION=${1:?usage: bump-version.sh <x.y.z>}
cd "$(dirname "$0")/.."

npm version "$VERSION" --no-git-tag-version --allow-same-version >/dev/null
node -e '
const fs = require("fs");
const [version] = process.argv.slice(1);
// Replace in place to keep each file'"'"'s formatting.
const set = (file, pattern, line) => fs.writeFileSync(file, fs.readFileSync(file, "utf8").replace(pattern, line));
set("src-tauri/tauri.conf.json", /^  "version": ".*",$/m, `  "version": "${version}",`);
set("src-tauri/Cargo.toml", /^version = ".*"$/m, `version = "${version}"`);
' "$VERSION"
# Refresh the lockfile's entry for this crate.
(cd src-tauri && cargo update --workspace --offline --quiet)

echo "cardamoi is now $VERSION"
