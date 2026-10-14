#!/usr/bin/env bash
# Installs a built panel binary on this box and restarts the panel, rolling back if it does
# not come up.
#
#   plugged-host/deploy.sh [binary]      # default target/plugged-host/panel-rs
#
# Releases live in /opt/calagopus/releases/<release>/panel-rs and /opt/calagopus/current points
# at the running one; the last KEEP releases stay for rollbacks. Migrations run on start
# (DATABASE_MIGRATE=true in /etc/calagopus/panel.env).
set -euo pipefail

# `panel-rs version` parses the environment first; it needs these set but connects to nothing.
release_of() {
  env -i PATH=/usr/bin:/bin APP_ENCRYPTION_KEY=version-check DATABASE_URL=postgresql://version@127.0.0.1:1/version \
    "$1" version | awk '/^plugged.host /{print $2}'
}

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BINARY=${1:-$ROOT/target/plugged-host/panel-rs}
BASE=/opt/calagopus
UNIT=calagopus
KEEP=${KEEP:-3}
HEALTH_URL=${HEALTH_URL:-http://127.0.0.1:8730/api/settings}

release=$(release_of "$BINARY")
[ -n "$release" ] || { echo "cannot read the release of $BINARY" >&2; exit 1; }

dir="$BASE/releases/$release"
install -d -m 755 "$dir"
install -m 755 "$BINARY" "$dir/panel-rs.new"
mv -f "$dir/panel-rs.new" "$dir/panel-rs"

previous=$(readlink "$BASE/current" 2>/dev/null || true)
ln -sfn "$dir" "$BASE/current.new"
mv -Tf "$BASE/current.new" "$BASE/current"

echo "==> $release (was ${previous##*/})"
systemctl restart "$UNIT"

for _ in $(seq 1 60); do
  if curl -fsS -o /dev/null "$HEALTH_URL"; then
    echo "==> up"

    # Keep the newest releases (and whatever is running).
    ls -1dt "$BASE"/releases/*/ | tail -n +$((KEEP + 1)) | while read -r old; do
      [ "${old%/}" = "$dir" ] || rm -rf "$old"
    done

    exit 0
  fi
  sleep 2
done

echo "==> $release did not come up, rolling back to ${previous##*/}" >&2
journalctl -u "$UNIT" -n 30 --no-pager >&2 || true

if [ -n "$previous" ]; then
  ln -sfn "$previous" "$BASE/current.new"
  mv -Tf "$BASE/current.new" "$BASE/current"
  systemctl restart "$UNIT"
fi

exit 1
