#!/usr/bin/env bash
# Runs the checks a change must pass, on the build host (never on the panel box).
#
#   plugged-host/check.sh                 # everything below
#   plugged-host/check.sh ts rust         # only some: ts (pnpm typecheck), biome, fmt (cargo fmt --check),
#                                         # rust (cargo check), test (cargo test -p shared -p backend)
#
# The translations have their own check, plugged-host/translations.sh.
# Like build.sh it copies the working tree (uncommitted changes included) to the build host and
# yields CPU and disk to prod there.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BUILD_HOST=${BUILD_HOST:-plugged-build}
BUILD_DIR=${BUILD_DIR:-/root/panel}
CHECKS=${*:-ts biome fmt rust test}

echo "==> copying the tree to $BUILD_HOST:$BUILD_DIR"
rsync -a --delete --exclude target --exclude node_modules --exclude frontend/dist "$ROOT/" "$BUILD_HOST:$BUILD_DIR/"

ssh "$BUILD_HOST" "set -uo pipefail
  renice -n 15 -p \$\$ >/dev/null; ionice -c 3 -p \$\$
  export PATH=/opt/node/bin:\$PATH
  . \$HOME/.cargo/env
  cd '$BUILD_DIR'
  failed=''
  run() { name=\$1; shift; echo \"==> \$name\"; if ! \"\$@\"; then failed=\"\$failed \$name\"; fi; }
  for check in $CHECKS; do
    case \$check in
      ts) (cd frontend && pnpm install --frozen-lockfile >/dev/null) && run ts sh -c 'cd frontend && pnpm -s typecheck' ;;
      biome) (cd frontend && pnpm install --frozen-lockfile >/dev/null) && run biome sh -c 'cd frontend && pnpm -s biome:validate' ;;
      fmt) run fmt cargo fmt --all --check ;;
      rust) run rust env SQLX_OFFLINE=true RUSTFLAGS='-C link-arg=-fuse-ld=mold' cargo check --profile release-fast -p panel-rs ;;
      test) run test env SQLX_OFFLINE=true RUSTFLAGS='-C link-arg=-fuse-ld=mold' cargo test --profile release-fast -p shared -p backend ;;
      *) echo \"unknown check \$check\"; failed=\"\$failed \$check\" ;;
    esac
  done
  if [ -n \"\$failed\" ]; then echo \"==> failed:\$failed\"; exit 1; fi
  echo '==> all passed'"
