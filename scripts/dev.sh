#!/usr/bin/env bash
# Dev server helper: runs `cargo leptos watch` in the background with the repo-local
# moonlight.conf and reports builds in a few lines, so nobody has to read the raw log.
#
#   scripts/dev.sh start      start watch (no-op if running) and wait until it serves
#   scripts/dev.sh restart    stop, start fresh and wait (needed after a release build, §6)
#   scripts/dev.sh stop       stop watch and the server it runs
#   scripts/dev.sh wait       after an edit: wait for watch's rebuild, print errors or "ready"
#   scripts/dev.sh status     one line: running/stopped, building, HTTP up
#   scripts/dev.sh logs [N]   last N (default 40) lines of the log, ANSI stripped
#
# Exit status: 0 ready/ok, 1 build failed or server died, 2 timeout.
# Env: DEV_URL (default http://127.0.0.1:3000), DEV_TIMEOUT seconds (default 1800).
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
# Outside target/: `cargo clean` (disk-space recovery, AGENTS.md §6) must not delete it.
LOG="${TMPDIR:-/tmp}/moonlight-dev-server.log"
URL="${DEV_URL:-http://127.0.0.1:3000}"
TIMEOUT="${DEV_TIMEOUT:-1800}"
cd "$ROOT" || exit 1

strip() { sed 's/\x1b\[[0-9;]*m//g'; }

# PIDs of `cargo leptos watch` processes whose working directory is this repo, so a watch
# of another project is never touched.
watch_pids() {
  local p
  for p in $(pgrep -f 'cargo-leptos leptos watch'); do
    [ "$(lsof -a -p "$p" -d cwd -Fn 2>/dev/null | sed -n 's/^n//p')" = "$ROOT" ] && echo "$p"
  done
}

http_up() { curl -s -o /dev/null --max-time 2 "$URL/"; }

building() {
  local w
  for w in $(watch_pids); do pgrep -P "$w" -f 'cargo build' >/dev/null && return 0; done
  return 1
}

log_size() { [ -f "$LOG" ] && wc -c <"$LOG" | tr -d ' ' || echo 0; }

# Prints rustc error blocks (up to a blank line) and server panics from new log output.
show_errors() {
  awk '
    /^error/ && !/could not compile/ { on = 1 }
    /panicked at|overflowed its stack/ { print; next }
    on { print; n++ }
    on && /^$/ { on = 0 }
    n >= 80 { print "… (more in scripts/dev.sh logs)"; exit }
  '
}

# Waits from byte offset $1 of the log until the build settles, then reports.
# $2 = seconds to give watch to notice an edit before deciding nothing changed.
wait_build() {
  local off="$1" grace="$2" start now new last
  start=$(date +%s)
  sleep "$grace"
  while :; do
    now=$(date +%s)
    if [ $((now - start)) -ge "$TIMEOUT" ]; then
      echo "timeout after ${TIMEOUT}s (building: $(building && echo yes || echo no))"
      return 2
    fi
    if [ -z "$(watch_pids)" ]; then
      echo "watch is not running"
      tail -c +"$((off + 1))" "$LOG" 2>/dev/null | strip | tail -20
      return 1
    fi
    # Settled = no cargo build child and the log quiet for 2 s (wasm-bindgen runs in-process).
    if ! building && [ $((now - $(stat -c %Y "$LOG" 2>/dev/null || stat -f %m "$LOG" 2>/dev/null || echo 0))) -ge 2 ]; then
      new=$(tail -c +"$((off + 1))" "$LOG" | strip)
      # A build that finished before `wait` was called is still reported: if nothing new,
      # judge (and show errors of) the whole log's last outcome.
      [ -z "$new" ] && new=$(strip <"$LOG" | awk '/Watch updated|Build interrupted/ { b = "" } { b = b $0 "\n" } END { printf "%s", b }')
      last=$(printf '%s\n' "$new" |
        grep -E 'Serving at|Build failed|panicked at|overflowed its stack' | tail -1)
      case "$last" in
        *"Build failed"* | *panicked* | *overflowed*)
          echo "FAILED after $((now - start))s"
          printf '%s\n' "$new" | show_errors
          return 1
          ;;
      esac
      if http_up; then
        if [ -z "$new" ]; then
          echo "ready (no rebuild) $URL"
        else
          echo "ready in $((now - start))s $URL"
          printf '%s\n' "$new" | grep -E '^warning: unused|^warning: `moonlight`' | head -5
        fi
        return 0
      fi
    fi
    sleep 1
  done
}

stop() {
  local pids
  pids=$(watch_pids)
  [ -z "$pids" ] && { echo "not running"; return 0; }
  # cargo-leptos does not always take its server with it: kill the children explicitly.
  for p in $pids; do pkill -P "$p" 2>/dev/null; kill "$p" 2>/dev/null; done
  for _ in $(seq 20); do [ -z "$(watch_pids)" ] && ! http_up && break; sleep 0.5; done
  pkill -f "$ROOT/target/debug/moonlight|^target/debug/moonlight" 2>/dev/null
  echo "stopped"
}

start() {
  if [ -n "$(watch_pids)" ]; then
    echo "already running (use restart)"
    wait_build "$(log_size)" 0
    return
  fi
  if http_up; then
    echo "something else answers on $URL; stop it first"
    return 1
  fi
  : >"$LOG"
  nohup cargo leptos watch -- --config-file moonlight.conf >"$LOG" 2>&1 </dev/null &
  echo "started (log: $LOG)"
  wait_build 0 3
}

case "${1:-}" in
  start) start ;;
  restart) stop && start ;;
  stop) stop ;;
  wait) wait_build "$(log_size)" 4 ;;
  status)
    if [ -z "$(watch_pids)" ]; then echo "stopped"; exit 0; fi
    echo "running; building: $(building && echo yes || echo no); http: $(http_up && echo up || echo down)"
    ;;
  logs) strip <"$LOG" | tail -n "${2:-40}" ;;
  *) sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 64 ;;
esac
