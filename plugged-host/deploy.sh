#!/usr/bin/env bash
# Installs a built panel binary on this box (and on a second box, when the panel runs on two) and
# restarts the panel, rolling back if it does not come up.
#
#   plugged-host/deploy.sh [binary]      # default target/plugged-host/panel-rs
#
# Releases live in /opt/calagopus/releases/<release>/panel-rs and /opt/calagopus/current points
# at the running one; the last KEEP releases stay for rollbacks.
#
# Two boxes: /etc/calagopus/deploy.env (not in git) names the other box as an ssh alias,
# PEER_SSH=<alias>. The release is copied there too; where the `calagopus` unit is enabled it is
# restarted after this box's came up, and `calagopus-tasks` (background tasks, one box at a time)
# is restarted wherever it runs. Migrations: with DATABASE_MIGRATE=true in /etc/calagopus/panel.env
# the panel runs them on start (one box); with false they run once here, before any restart.
set -euo pipefail

# `panel-rs version` parses the environment first; it needs these set but connects to nothing.
release_of() {
  env -i PATH=/usr/bin:/bin APP_ENCRYPTION_KEY=version-check DATABASE_URL=postgresql://version@127.0.0.1:1/version \
    "$1" version | awk '/^release /{print $2}'
}

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BINARY=${1:-$ROOT/target/plugged-host/panel-rs}
BASE=/opt/calagopus
UNIT=calagopus
TASKS=calagopus-tasks
ENV_FILE=/etc/calagopus/panel.env
KEEP=${KEEP:-3}
HEALTH_URL=${HEALTH_URL:-http://127.0.0.1:8730/api/settings}
PEER_SSH=
[ -f /etc/calagopus/deploy.env ] && . /etc/calagopus/deploy.env
peer() { ssh -o BatchMode=yes -o ConnectTimeout=10 "$PEER_SSH" "$@"; }

release=$(release_of "$BINARY")
[ -n "$release" ] || { echo "cannot read the release of $BINARY" >&2; exit 1; }

# The release is only v{VERSION}, so every deploy gets its own directory.
dir="$BASE/releases/$release-$(date -u +%Y%m%d-%H%M%S)"
install -d -m 755 "$dir"
install -m 755 "$BINARY" "$dir/panel-rs.new"
mv -f "$dir/panel-rs.new" "$dir/panel-rs"

switch_to() { # switch_to <release dir>: this box and the peer point at it
  ln -sfn "$1" "$BASE/current.new"
  mv -Tf "$BASE/current.new" "$BASE/current"
  if [ -n "$PEER_SSH" ]; then
    peer "test -x '$1/panel-rs' && ln -sfn '$1' '$BASE/current.new' && mv -Tf '$BASE/current.new' '$BASE/current'"
  fi
}

previous=$(readlink "$BASE/current" 2>/dev/null || true)
peer_running=0
if [ -n "$PEER_SSH" ]; then
  peer "install -d -m 755 '$dir'"
  rsync -a "$dir/panel-rs" "$PEER_SSH:$dir/panel-rs"
  peer "systemctl is-enabled -q $UNIT" 2>/dev/null && peer_running=1
fi
switch_to "$dir"
echo "==> $release (was ${previous##*/})$([ -n "$PEER_SSH" ] && echo ", also on $PEER_SSH")"

# Migrations once, before any instance restarts, when the panel does not run them on start.
if grep -qE '^DATABASE_MIGRATE="?false' "$ENV_FILE"; then
  echo "==> migrations"
  systemd-run --quiet --wait --pipe --collect -p User=calagopus -p WorkingDirectory=/var/lib/calagopus \
    -p EnvironmentFile="$ENV_FILE" "$dir/panel-rs" database-migrator migrate
fi

up() { # up <command prefix>: waits for the panel's health URL (60 tries)
  for _ in $(seq 1 60); do
    if $1 curl -fs -o /dev/null "$HEALTH_URL"; then return 0; fi
    sleep 2
  done
  return 1
}

rollback() {
  echo "==> $release did not come up ($1), rolling back to ${previous##*/}" >&2
  if [ -n "$previous" ]; then
    switch_to "$previous"
    systemctl restart "$UNIT"
    [ "$peer_running" = 1 ] && peer "systemctl restart $UNIT"
    systemctl try-restart "$TASKS" 2>/dev/null || true
    [ -n "$PEER_SSH" ] && peer "systemctl try-restart $TASKS" 2>/dev/null || true
  fi
  exit 1
}

systemctl restart "$UNIT"
up "" || { journalctl -u "$UNIT" -n 30 --no-pager >&2 || true; rollback "this box"; }
echo "==> up"
if [ "$peer_running" = 1 ]; then
  peer "systemctl restart $UNIT"
  up "peer" || { peer "journalctl -u $UNIT -n 30 --no-pager" >&2 || true; rollback "$PEER_SSH"; }
  echo "==> up on $PEER_SSH"
fi
systemctl try-restart "$TASKS" 2>/dev/null || true
[ -n "$PEER_SSH" ] && { peer "systemctl try-restart $TASKS" 2>/dev/null || true; }

# Keep the newest releases (and whatever is running), on both boxes.
prune="ls -1dt '$BASE'/releases/*/ | tail -n +$((KEEP + 1)) | while read -r old; do [ \"\${old%/}\" = '$dir' ] || rm -rf \"\$old\"; done"
bash -c "$prune"
[ -n "$PEER_SSH" ] && peer "$prune"
exit 0
