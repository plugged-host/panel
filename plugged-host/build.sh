#!/usr/bin/env bash
# Builds the panel on the build host and fetches the binary (frontend embedded).
#
#   plugged-host/build.sh            # -> target/plugged-host/panel-rs
#
# The build host is an ssh alias (BUILD_HOST, default calagopus-build) with Rust, Node 26 and
# pnpm 11; never build on the panel box itself (not enough disk or memory). The repository is
# copied over with its .git, so the binary knows its commit (plugged.host release v{VERSION}.{commit}).
# Commit before building anything that gets deployed: the release names the commit, not the
# working tree.
set -euo pipefail

# `panel-rs version` parses the environment first; it needs these set but connects to nothing.
release_of() {
  env -i PATH=/usr/bin:/bin APP_ENCRYPTION_KEY=version-check DATABASE_URL=postgresql://version@127.0.0.1:1/version \
    "$1" version | awk '/^plugged.host /{print $2}'
}

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BUILD_HOST=${BUILD_HOST:-calagopus-build}
BUILD_DIR=${BUILD_DIR:-/root/panel}
PROFILE=${PROFILE:-plugged-host}
OUT="$ROOT/target/plugged-host"

if [ -n "$(git -C "$ROOT" status --porcelain --untracked-files=no)" ]; then
  echo "warning: uncommitted changes, the release will name $(git -C "$ROOT" rev-parse --short=7 HEAD) anyway" >&2
fi

echo "==> copying the tree to $BUILD_HOST:$BUILD_DIR"
rsync -a --delete --exclude target --exclude node_modules --exclude frontend/dist "$ROOT/" "$BUILD_HOST:$BUILD_DIR/"

echo "==> building (profile $PROFILE)"
ssh "$BUILD_HOST" "set -euo pipefail
  export PATH=/opt/node/bin:\$PATH
  . \$HOME/.cargo/env
  cd '$BUILD_DIR/frontend'
  pnpm install --frozen-lockfile >/dev/null
  pnpm build >/dev/null
  cd '$BUILD_DIR'
  SQLX_OFFLINE=true RUSTFLAGS='-C link-arg=-fuse-ld=mold' cargo build --profile '$PROFILE' -p panel-rs"

mkdir -p "$OUT"
rsync -a "$BUILD_HOST:$BUILD_DIR/target/$PROFILE/panel-rs" "$OUT/panel-rs.new"
mv -f "$OUT/panel-rs.new" "$OUT/panel-rs"

echo "==> built $(release_of "$OUT/panel-rs")"
