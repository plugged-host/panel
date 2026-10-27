#!/usr/bin/env bash
# Builds the panel on the build host and fetches the binary (frontend embedded).
#
#   plugged-host/build.sh            # -> target/plugged-host/panel-rs
#
# The build host is an ssh alias (BUILD_HOST, default plugged-build, a Ryzen 5 PRO 3600) with
# Rust, Node 26 and pnpm 11; never build on the panel box itself (not enough disk or memory). The
# release is v{VERSION} (docs/features/release.md); commit before building anything that gets
# deployed, so what runs is in git.
#
# It stops when an offered language misses an English string (LANGUAGES, default every offered
# language: German every string, the others all but the admin-only ones; docs/features/languages.md).
# The frontend is only rebuilt when its sources (and its workspace's backend-extensions/*/frontend)
# changed since the last build, uncommitted changes included; FRONTEND=1 forces it.
set -euo pipefail

# `panel-rs version` parses the environment first; it needs these set but connects to nothing.
release_of() {
  env -i PATH=/usr/bin:/bin APP_ENCRYPTION_KEY=version-check DATABASE_URL=postgresql://version@127.0.0.1:1/version \
    "$1" version | awk '/^release /{print $2}'
}

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BUILD_HOST=${BUILD_HOST:-plugged-build}
BUILD_DIR=${BUILD_DIR:-/root/panel}
PROFILE=${PROFILE:-release-fast}
# The languages to check (plugged-host/translations.sh); empty = every offered language.
LANGUAGES=${LANGUAGES:-}
OUT="$ROOT/target/plugged-host"

if [ -n "$(git -C "$ROOT" status --porcelain --untracked-files=no)" ]; then
  echo "warning: uncommitted changes are in this build but not in git" >&2
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
  pnpm install --frozen-lockfile >/dev/null
  pnpm -s build:translations
  if ! pnpm -s coverage:translations $LANGUAGES; then
    echo 'translations incomplete: plugged-host/translations.sh --missing lists the keys' >&2
    exit 1
  fi
  # Its sources and the workspace's other packages (pnpm-workspace.yaml).
  source=\$(find . \$(ls -d ../backend-extensions/*/frontend 2>/dev/null) -name node_modules -prune -o -path ./dist -prune -o -type f -print0 2>/dev/null | sort -z | xargs -0 sha256sum | sha256sum | cut -c1-64)
  stamp='$BUILD_DIR.frontend-source'
  if [ '${FRONTEND:-0}' = 1 ] || [ ! -f dist/index.html ] || [ \"\$(cat \"\$stamp\" 2>/dev/null)\" != \"\$source\" ]; then
    echo '==> building the frontend'
    pnpm build >/dev/null
    echo \"\$source\" > \"\$stamp\"
  else
    echo '==> frontend unchanged, keeping its build'
  fi
  cd '$BUILD_DIR'
  flock /root/.panel-cargo.lock env SQLX_OFFLINE=true RUSTFLAGS='-C link-arg=-fuse-ld=mold' cargo build --profile '$PROFILE' -p panel-rs"

mkdir -p "$OUT"
rsync -a "$BUILD_HOST:$BUILD_DIR/target/$PROFILE/panel-rs" "$OUT/panel-rs.new"
mv -f "$OUT/panel-rs.new" "$OUT/panel-rs"

echo "==> built $(release_of "$OUT/panel-rs")"
