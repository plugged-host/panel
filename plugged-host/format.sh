#!/usr/bin/env bash
# Formats the tree the way upstream's CI checks it (rustfmt, biome) on the build host and copies
# the files it rewrote back. Run it before plugged-host/check.sh fmt biome.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BUILD_HOST=${BUILD_HOST:-plugged-build}
BUILD_DIR=${BUILD_DIR:-/root/panel}
LIST=/tmp/$(basename "$BUILD_DIR")-formatted

echo "==> copying the tree to $BUILD_HOST:$BUILD_DIR"
rsync -a --delete --exclude target --exclude node_modules --exclude frontend/dist "$ROOT/" "$BUILD_HOST:$BUILD_DIR/"

# The formatters only write the files they change, so whatever is newer than the marker goes back.
ssh "$BUILD_HOST" "set -euo pipefail
  renice -n 15 -p \$\$ >/dev/null; ionice -c 3 -p \$\$
  export PATH=/opt/node/bin:\$PATH
  . \$HOME/.cargo/env
  cd '$BUILD_DIR'
  marker=\$(mktemp); sleep 1
  cargo fmt --all
  (cd frontend && pnpm install --frozen-lockfile >/dev/null && pnpm -s exec biome check --write --css-parse-tailwind-directives=true >/dev/null || true)
  find . -type f -newer \"\$marker\" -not -path './target/*' -not -path '*/node_modules/*' -not -path './frontend/dist/*' -not -path './.git/*' | sed 's|^\./||' > $LIST
  rm -f \"\$marker\""

rsync -a --files-from=<(ssh "$BUILD_HOST" cat $LIST) "$BUILD_HOST:$BUILD_DIR/" "$ROOT/"
ssh "$BUILD_HOST" cat $LIST | sed 's/^/formatted: /'
