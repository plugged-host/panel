#!/usr/bin/env bash
# Builds the panel on the build host and fetches the binary (frontend embedded).
#
#   plugged-host/build.sh            # -> target/plugged-host/panel-rs
#
# The build host is an ssh alias (BUILD_HOST, default plugged-build, a Ryzen 5 PRO 3600) with
# Rust, Node 26 and pnpm 11; never build on the panel box itself (not enough disk or memory). The
# repository is copied over with its .git, so the binary knows its commit (plugged.host release
# v{VERSION}.{commit}). Commit before building anything that gets deployed: the release names the
# commit, not the working tree.
#
# The frontend is only rebuilt when its sources (and its workspace's backend-extensions/*/frontend)
# changed since the last build, uncommitted changes included; FRONTEND=1 forces it.
set -euo pipefail

# `panel-rs version` parses the environment first; it needs these set but connects to nothing.
release_of() {
  env -i PATH=/usr/bin:/bin APP_ENCRYPTION_KEY=version-check DATABASE_URL=postgresql://version@127.0.0.1:1/version \
    "$1" version | awk '/^plugged.host /{print $2}'
}

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BUILD_HOST=${BUILD_HOST:-plugged-build}
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
  # The build host is also the prod panel box (since 2026-10-17): builds yield CPU and disk to it.
  renice -n 15 -p \$\$ >/dev/null; ionice -c 3 -p \$\$
  export PATH=/opt/node/bin:\$PATH
  . \$HOME/.cargo/env
  cd '$BUILD_DIR/frontend'
  # Its sources and the workspace's other packages (pnpm-workspace.yaml).
  source=\$(find . \$(ls -d ../backend-extensions/*/frontend 2>/dev/null) -name node_modules -prune -o -path ./dist -prune -o -type f -print0 2>/dev/null | sort -z | xargs -0 sha256sum | sha256sum | cut -c1-64)
  stamp='$BUILD_DIR.frontend-source'
  if [ '${FRONTEND:-0}' = 1 ] || [ ! -f dist/index.html ] || [ \"\$(cat \"\$stamp\" 2>/dev/null)\" != \"\$source\" ]; then
    echo '==> building the frontend'
    pnpm install --frozen-lockfile >/dev/null
    pnpm build >/dev/null
    echo \"\$source\" > \"\$stamp\"
  else
    echo '==> frontend unchanged, keeping its build'
  fi
  cd '$BUILD_DIR'
  SQLX_OFFLINE=true RUSTFLAGS='-C link-arg=-fuse-ld=mold' cargo build --profile '$PROFILE' -p panel-rs"

mkdir -p "$OUT"
rsync -a "$BUILD_HOST:$BUILD_DIR/target/$PROFILE/panel-rs" "$OUT/panel-rs.new"
mv -f "$OUT/panel-rs.new" "$OUT/panel-rs"

echo "==> built $(release_of "$OUT/panel-rs")"
