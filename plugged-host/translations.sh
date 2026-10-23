#!/usr/bin/env bash
# Builds frontend/public/translations/en.json from frontend/src/translations.ts on the build host,
# copies it back, and checks that the offered languages have every English string, the panel's
# and the mails' (`pnpm coverage:translations`). Fails below 100%; build.sh runs it before every build.
#
#   plugged-host/translations.sh            # LANGUAGES (default: every language that must be complete)
#   plugged-host/translations.sh --missing  # also lists the missing keys
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BUILD_HOST=${BUILD_HOST:-plugged-build}
BUILD_DIR=${BUILD_DIR:-/root/panel}
# English is the source; German is complete since v3 Phase 1, the others join in Phase 6.
LANGUAGES=${LANGUAGES:-de}

rsync -a --delete --exclude node_modules --exclude dist "$ROOT/frontend/" "$BUILD_HOST:$BUILD_DIR/frontend/"
rsync -a --delete "$ROOT/shared/mails/" "$BUILD_HOST:$BUILD_DIR/shared/mails/"
status=0
ssh "$BUILD_HOST" "set -euo pipefail
  renice -n 15 -p \$\$ >/dev/null; ionice -c 3 -p \$\$
  export PATH=/opt/node/bin:\$PATH
  cd '$BUILD_DIR/frontend'
  pnpm install --frozen-lockfile >/dev/null
  pnpm -s build:translations
  pnpm -s coverage:translations $LANGUAGES ${1:-}" || status=$?
rsync -a "$BUILD_HOST:$BUILD_DIR/frontend/public/translations/en.json" "$ROOT/frontend/public/translations/en.json"
exit $status
