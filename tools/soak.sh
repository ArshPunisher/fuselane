#!/usr/bin/env bash
# Soak test (STEPS 9.1, TESTING.md L6): downloads files from a local test server
# with the real `fuselane` CLI, again and again, until the time is up. Every file
# is checked byte for byte by SHA-256, timed, measured for peak memory, then
# deleted. The log ends with a summary. Loopback only: needs no internet.
#
#   tools/soak.sh --duration 24h                the weekly soak
#   tools/soak.sh --duration 3m --fault mixed   a short run against a misbehaving server
#   tools/soak.sh --help
#
# Exit status: 0 when every download succeeded, 1 when any failed (or the test
# server died), 130 when stopped early (Ctrl-C) with no failures. macOS and Linux.
set -euo pipefail

# One block, so bash reads the whole file before running it: editing or updating
# this file (git pull) can't break a soak that is already running.
{

usage() {
  cat <<'EOF'
Usage: tools/soak.sh [options]

Downloads files from a local test server with the real fuselane CLI, again and
again, until the time is up. Each download lands in a fresh temporary folder, is
checked byte for byte by SHA-256, then deleted. Needs no internet.

Options:
  -d, --duration TIME   how long to keep starting downloads: 90s, 30m, 24h
                        (a plain number is seconds; default 10m)
  -s, --size LIST       file sizes, downloaded in turn (default 1M,8M,32M);
                        K, M and G mean KiB, MiB and GiB
  -f, --fault MODE      make the server misbehave (default none):
                          drops   connections close partway through an answer
                          resets  connections close before any answer
                          stalls  answers stop partway and go silent
                          busy    503 / 429 answers with Retry-After: 1
                          slow    every answer is sent at 1 MiB/s
                          lies    wrong ranges, extra bytes, no Content-Range
                          mixed   drops, resets, busy and lies in turn
      --every N         with a fault mode, misbehave on every Nth request (default 20)
  -p, --pause SECONDS   wait between downloads, to spare the disk (default 10)
  -l, --log FILE        where to write the log
                        (default <cargo target folder>/soak/soak-<date>-<time>.log)
  -n, --networks LIST   networks for `fuselane get -n` (default: this computer's loopback)
      --run-timeout S   count one download as hung after S seconds
                        (default 300, plus 1 second per MiB of the file)
      --fuselane PATH   test this fuselane binary instead of building one
  -h, --help            show this help

Builds the fuselane CLI and the test server in release mode first (the shared
cargo target folder is used: CARGO_TARGET_DIR, else ./target). Temporary files go
in $TMPDIR (else /tmp) and are removed on exit, including on Ctrl-C.
EOF
}

die() {
  printf 'soak: %s\n' "$*" >&2
  exit 1
}

bad_input() {
  printf 'soak: %s\nRun tools/soak.sh --help to see the options.\n' "$*" >&2
  exit 2
}

# ---------- parsing ----------

# "90s", "30m", "24h" or plain seconds -> seconds; fails on anything else.
parse_duration() {
  local num unit
  case $1 in '' | *[!0-9smhSMH]*) return 1 ;; esac
  num=${1%[smhSMH]}
  unit=${1#"$num"}
  case $num in '' | *[!0-9]*) return 1 ;; esac
  case $unit in
    '' | s | S) echo $((10#$num)) ;;
    m | M) echo $((10#$num * 60)) ;;
    h | H) echo $((10#$num * 3600)) ;;
    *) return 1 ;;
  esac
}

# "512K", "8M", "1G", "8MB", "8MiB" or plain bytes -> bytes; fails on anything else.
parse_size() {
  local s num unit
  s=$(printf '%s' "$1" | tr '[:lower:]' '[:upper:]')
  s=${s%IB}
  s=${s%B}
  case $s in '' | *[!0-9KMG]*) return 1 ;; esac
  num=${s%[KMG]}
  unit=${s#"$num"}
  case $num in '' | *[!0-9]*) return 1 ;; esac
  [ "${#num}" -le 12 ] || return 1
  case $unit in
    '') echo $((10#$num)) ;;
    K) echo $((10#$num * 1024)) ;;
    M) echo $((10#$num * 1048576)) ;;
    G) echo $((10#$num * 1073741824)) ;;
    *) return 1 ;;
  esac
}

# An absolute version of a path given relative to where the script was started.
absolute() {
  case $1 in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s/%s\n' "$start_dir" "$1" ;;
  esac
}

start_dir=$(pwd)
duration_arg=10m
size_arg=1M,8M,32M
fault=none
every=20
pause=10
log=""
networks=""
run_timeout=""
fuselane=""

while [ $# -gt 0 ]; do
  opt=$1
  case $opt in
    -h | --help)
      usage
      exit 0
      ;;
    -d | --duration | -s | --size | --sizes | -f | --fault | --every | -p | --pause | \
      -l | --log | -n | --networks | --run-timeout | --fuselane)
      [ $# -ge 2 ] || bad_input "$opt needs a value."
      val=$2
      shift 2
      ;;
    *) bad_input "there's no option called \"$opt\"." ;;
  esac
  case $opt in
    -d | --duration) duration_arg=$val ;;
    -s | --size | --sizes) size_arg=$val ;;
    -f | --fault) fault=$val ;;
    --every) every=$val ;;
    -p | --pause) pause=$val ;;
    -l | --log) log=$(absolute "$val") ;;
    -n | --networks) networks=$val ;;
    --run-timeout) run_timeout=$val ;;
    --fuselane) fuselane=$(absolute "$val") ;;
  esac
done

duration=$(parse_duration "$duration_arg") ||
  bad_input "\"$duration_arg\" isn't a duration. Examples: 90s, 30m, 24h."
[ "$duration" -ge 1 ] || bad_input "the duration must be at least 1 second."

case $fault in
  none | drops | resets | stalls | busy | slow | lies | mixed) ;;
  *) bad_input "there's no fault mode called \"$fault\". Pick none, drops, resets, stalls, busy, slow, lies or mixed." ;;
esac
case $every in '' | *[!0-9]*) every=x ;; esac
if [ "$every" = x ] || [ "$every" -lt 2 ] || [ "$every" -gt 1000000 ]; then
  bad_input "--every needs a whole number from 2 to 1000000."
fi

decimal='^[0-9]+([.][0-9]+)?$'
[[ $pause =~ $decimal ]] || bad_input "--pause needs a number of seconds, like 2 or 0.5."
if [ -n "$run_timeout" ]; then
  case $run_timeout in '' | *[!0-9]*) bad_input "--run-timeout needs a whole number of seconds." ;; esac
  [ "$run_timeout" -ge 1 ] || bad_input "--run-timeout must be at least 1 second."
fi

labels=()
sizes=()
old_ifs=$IFS
IFS=,
set -f
# shellcheck disable=SC2206 # splitting the comma list is the point
size_list=($size_arg)
set +f
IFS=$old_ifs
for label in ${size_list[@]+"${size_list[@]}"}; do
  bytes=$(parse_size "$label") ||
    bad_input "\"$label\" isn't a file size. Examples: 512K, 8M, 1G (or plain bytes)."
  labels+=("$label")
  sizes+=("$bytes")
done
[ "${#sizes[@]}" -ge 1 ] || bad_input "give at least one file size, like --size 8M."
largest=0
for bytes in "${sizes[@]}"; do
  if [ "$bytes" -gt "$largest" ]; then largest=$bytes; fi
done

# ---------- tools and builds ----------

cd "$(dirname "$0")/.."
repo=$(pwd)

if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | awk '{ print $1 }'; }
elif command -v shasum >/dev/null 2>&1; then
  sha256() { shasum -a 256 "$1" | awk '{ print $1 }'; }
else
  die "neither sha256sum nor shasum is installed, so downloads can't be checked. Install coreutils (Linux) or Perl's shasum."
fi
for tool in awk mktemp df ps; do
  command -v "$tool" >/dev/null 2>&1 || die "\"$tool\" is missing; it comes with every macOS and Linux install. Check your PATH."
done

# Millisecond clock: bash 5's EPOCHREALTIME, else Perl, else whole seconds.
if [ -n "${EPOCHREALTIME:-}" ]; then
  now_ms() {
    local t=${EPOCHREALTIME//[!0-9]/}
    echo $((10#$t / 1000))
  }
elif command -v perl >/dev/null 2>&1 && perl -MTime::HiRes -e 1 2>/dev/null; then
  now_ms() { perl -MTime::HiRes=time -e 'printf("%d\n", time() * 1000)'; }
else
  now_ms() { echo $(($(date +%s) * 1000)); }
fi

target_dir=${CARGO_TARGET_DIR:-$repo/target}
case $target_dir in /*) ;; *) target_dir=$repo/$target_dir ;; esac

trap 'exit 130' INT TERM HUP

command -v cargo >/dev/null 2>&1 ||
  die "cargo isn't on your PATH, so the test server can't be built. Install Rust from https://rustup.rs (or run: source ~/.cargo/env)."
if [ -z "$fuselane" ]; then
  echo "soak: building fuselane (release; the first build takes a few minutes)..."
  cargo build --release --locked -p fuselane-cli --bin fuselane ||
    die "building fuselane failed; see the errors above."
  fuselane=$target_dir/release/fuselane
fi
[ -x "$fuselane" ] || die "there's no fuselane program at $fuselane. Build it, or pass its path with --fuselane."
echo "soak: building the test server (release)..."
cargo build --release --locked -p fuselane-testkit --example soak_server ||
  die "building the test server failed; see the errors above."
server_bin=$target_dir/release/examples/soak_server
[ -x "$server_bin" ] || die "the test server wasn't found at $server_bin after building. Is CARGO_TARGET_DIR set the same way cargo sees it?"
version=$("$fuselane" --version 2>/dev/null) || die "$fuselane doesn't run; is it a fuselane binary for this computer?"

# ---------- log, work folder, cleanup ----------

if [ -z "$log" ]; then
  log=$target_dir/soak/soak-$(date +%Y%m%d-%H%M%S).log
  # Two soaks started in the same second keep separate logs.
  if [ -e "$log" ]; then log=${log%.log}-$$.log; fi
fi
mkdir -p "$(dirname "$log")" 2>/dev/null || die "couldn't create the folder for the log, $(dirname "$log"). Pick another place with --log."
: >"$log" 2>/dev/null || die "couldn't write the log at $log. Pick another place with --log."

work=""
server_pid=""
cli_pid=""
watchdog_pid=""
finishing=""
soaking=""

# Sends a signal to a process and its children (the CLI runs under /usr/bin/time).
kill_tree() {
  local kids kid
  kids=$(ps -A -o pid= -o ppid= 2>/dev/null | awk -v p="$2" '$2 == p { print $1 }') || kids=""
  for kid in $kids; do
    kill "-$1" "$kid" 2>/dev/null || true
  done
  kill "-$1" "$2" 2>/dev/null || true
}

stop_cli() {
  local i=0
  if [ -n "$cli_pid" ]; then
    kill_tree TERM "$cli_pid"
    while kill -0 "$cli_pid" 2>/dev/null && [ "$i" -lt 30 ]; do
      sleep 0.1
      i=$((i + 1))
    done
    kill_tree KILL "$cli_pid"
    wait "$cli_pid" 2>/dev/null || true
    cli_pid=""
  fi
  if [ -n "$watchdog_pid" ]; then
    kill "$watchdog_pid" 2>/dev/null || true
    wait "$watchdog_pid" 2>/dev/null || true
    watchdog_pid=""
  fi
}

# Closing our end of its input tells the server to print its counts and stop.
stop_server() {
  local i=0
  [ -n "$server_pid" ] || return 0
  exec 3>&-
  while kill -0 "$server_pid" 2>/dev/null && [ "$i" -lt 50 ]; do
    sleep 0.1
    i=$((i + 1))
  done
  kill_tree KILL "$server_pid"
  wait "$server_pid" 2>/dev/null || true
  server_pid=""
}

cleanup() {
  local status=$?
  set +e
  # A bug or a full disk stopped the script mid-soak: still end the log with a summary.
  if [ -z "$finishing" ] && [ -n "$soaking" ]; then
    finishing=1
    stop_cli
    stop_server
    summary "by an error in the soak script (exit status $status; see the message above)" ""
  fi
  stop_cli
  stop_server
  if [ -n "$work" ] && [ -d "$work" ]; then
    rm -rf "$work"
  fi
}
trap cleanup EXIT

work=$(mktemp -d "${TMPDIR:-/tmp}/fuselane-soak.XXXXXX") ||
  die "couldn't create a temporary folder in ${TMPDIR:-/tmp}. Check that it exists and has space."
work=${work%/}
mkdir "$work/home"
# Every run shares one fresh download list, never the real one.
export FUSELANE_HOME=$work/home

# Peak memory: BSD time (macOS) reports bytes, GNU time (Linux) kilobytes.
time_kind=none
if [ -x /usr/bin/time ]; then
  if /usr/bin/time -l -o "$work/time-probe" true 2>/dev/null &&
    grep -q 'maximum resident set size' "$work/time-probe"; then
    time_kind=bsd
  elif /usr/bin/time -v -o "$work/time-probe" true 2>/dev/null &&
    grep -q 'Maximum resident set size' "$work/time-probe"; then
    time_kind=gnu
  fi
fi
rm -f "$work/time-probe"

free_kb=$(df -Pk "$work" | awk 'NR == 2 { print $4 }')
need_kb=$((largest / 1024 + 512 * 1024))
if [ -n "$free_kb" ] && [ "$free_kb" -lt "$need_kb" ]; then
  die "not enough free disk space in $work: $((free_kb / 1024)) MiB free, $((need_kb / 1024)) MiB needed for the largest file plus room to spare. Free some space, use a smaller --size, or point TMPDIR at a bigger disk."
fi

nets_json=$("$fuselane" nets --all --json 2>/dev/null) || nets_json=""
if [ -z "$networks" ]; then
  networks=$(printf '%s' "$nets_json" | tr '{' '\n' |
    grep '"kind":"loopback"' | sed -n 's/.*"name":"\([^"]*\)".*/\1/p' | head -n 1) || networks=""
  if [ -z "$networks" ]; then
    case $(uname -s) in Linux) networks=lo ;; *) networks=lo0 ;; esac
  fi
elif [ -n "$nets_json" ]; then
  old_ifs=$IFS
  IFS=,
  for name in $networks; do
    case $nets_json in
      *"\"name\":\"$name\""*) ;;
      *) bad_input "there's no network called \"$name\". Run: $fuselane nets --all" ;;
    esac
  done
  IFS=$old_ifs
fi

# ---------- the test server ----------

csv=$(
  IFS=,
  echo "${sizes[*]}"
)
mkfifo "$work/server.in"
exec 3<>"$work/server.in"
(
  # Stalled answers hold sockets open; give the server room (the CLI keeps the
  # computer's normal limit, so a leak there still shows).
  limit=$(ulimit -n)
  if [ "$limit" != unlimited ] && [ "$limit" -lt 4096 ]; then
    ulimit -n 4096 2>/dev/null || true
  fi
  # Not exec: if the server is killed, the shell's note about it lands in server.err.
  status=0
  "$server_bin" --sizes "$csv" --fault "$fault" --every "$every" || status=$?
  exit "$status"
) <"$work/server.in" >"$work/server.out" 2>"$work/server.err" 3>&- &
server_pid=$!

waited=0
until grep -q '^ready$' "$work/server.out" 2>/dev/null; do
  if ! kill -0 "$server_pid" 2>/dev/null; then
    die "the test server stopped before it was ready: $(cat "$work/server.err")"
  fi
  [ "$waited" -lt 3000 ] || die "the test server wasn't ready after 10 minutes (it hashes each file first; is a size very large?)."
  sleep 0.2
  waited=$((waited + 1))
done

urls=()
shas=()
while read -r kind size sha url; do
  [ "$kind" = file ] || continue
  for i in "${!sizes[@]}"; do
    if [ "${sizes[$i]}" = "$size" ] && [ -z "${urls[$i]:-}" ]; then
      urls[i]=$url
      shas[i]=$sha
      break
    fi
  done
done <"$work/server.out"
for i in "${!sizes[@]}"; do
  [ -n "${urls[$i]:-}" ] || die "the test server didn't list a file of ${labels[$i]}."
done

# ---------- reporting ----------

results=$work/results.tsv
: >"$results"

mb() { awk -v b="$1" 'BEGIN { printf "%.1f", b / 1048576 }'; }

# Bytes in KB, MB, GB or TB (1024-based, as the app shows them).
human_bytes() {
  awk -v b="$1" 'BEGIN {
    if (b >= 1024 ^ 4) printf "%.2f TB", b / 1024 ^ 4
    else if (b >= 1024 ^ 3) printf "%.2f GB", b / 1024 ^ 3
    else if (b >= 1024 ^ 2) printf "%.1f MB", b / 1024 ^ 2
    else printf "%.0f KB", b / 1024 }'
}

human_time() {
  local s=$1
  if [ "$s" -ge 3600 ]; then
    printf '%dh %dm %ds' $((s / 3600)) $((s % 3600 / 60)) $((s % 60))
  elif [ "$s" -ge 60 ]; then
    printf '%dm %ds' $((s / 60)) $((s % 60))
  else
    printf '%ds' "$s"
  fi
}

say() { printf '%s\n' "$*" | tee -a "$log"; }

start_human=$(date '+%Y-%m-%d %H:%M:%S %z')
start_s=$(date +%s)
end_s=$((start_s + duration))
if [ "$fault" = none ]; then
  fault_text="none"
elif [ "$fault" = slow ]; then
  fault_text="slow (every answer at 1 MiB/s)"
else
  fault_text="$fault, on 1 in every $every requests"
fi
memory_text="peak memory from /usr/bin/time"
if [ "$time_kind" = none ]; then
  memory_text="peak memory not measured (/usr/bin/time is missing; on Debian or Ubuntu: sudo apt install time)"
fi

say "Fuselane soak test"
say "Started:  $start_human, for $(human_time "$duration")"
say "Program:  $version ($fuselane)"
say "System:   $(uname -sr), networks: $networks"
say "Files:    $(
  IFS=,
  echo "${labels[*]}"
) in turn; faults: $fault_text; pause ${pause}s"
say "Measured: time per download (process start to exit), $memory_text"
say ""

summary() {
  local stopped=$1 server_note=$2 end_human now_s runs ok failed bytes ms peak stats requests faults list_bytes
  end_human=$(date '+%Y-%m-%d %H:%M:%S %z')
  now_s=$(date +%s)
  read -r runs ok failed bytes ms peak <<EOF
$(awk -F '\t' '
  { runs++; if ($4 == "ok") { ok++; bytes += $5; ms += $6 } else failed++
    if ($7 != "-" && $7 + 0 > peak) peak = $7 + 0 }
  END { printf "%d %d %d %.0f %.0f %.0f\n", runs, ok, failed, bytes, ms, peak }' "$results")
EOF
  say ""
  say "== Summary =="
  say "Started:      $start_human"
  say "Ended:        $end_human ($(human_time $((now_s - start_s))) of $(human_time "$duration"))"
  if [ -n "$stopped" ]; then say "Stopped:      early, $stopped"; fi
  if [ -n "$server_note" ]; then say "Server:       $server_note"; fi
  stats=$(grep '^stats ' "$work/server.out" 2>/dev/null | tail -n 1) || stats=""
  if [ "$fault" != none ] && [ -n "$stats" ]; then
    read -r _ requests faults <<<"$stats"
    say "Faults:       $fault_text: $faults of $requests requests misbehaved"
  elif [ "$fault" != none ]; then
    say "Faults:       $fault_text (the server didn't report its counts)"
  fi
  say "Runs:         $runs ($ok ok, $failed failed)"
  say "Data:         $(human_bytes "$bytes") downloaded and checked by SHA-256"
  if [ "$ms" -gt 0 ]; then
    say "Speed:        $(awk -v b="$bytes" -v ms="$ms" 'BEGIN { printf "%.1f", b / 1048576 / (ms / 1000) }') MB/s on average over the successful downloads"
  fi
  if [ "$time_kind" = none ]; then
    say "Peak memory:  not measured"
  else
    say "Peak memory:  $(mb "$peak") MB (highest of any run)"
  fi
  list_bytes=$(cat "$FUSELANE_HOME"/jobs.db* 2>/dev/null | wc -c | tr -d ' ') || list_bytes=0
  say "History:      the shared download list is $(human_bytes "${list_bytes:-0}") after $runs downloads"
  awk -F '\t' -v order="$(
    IFS=,
    echo "${labels[*]}"
  )" '
    { n[$3]++; if ($4 == "ok") { ok[$3]++; b[$3] += $5; t[$3] += $6 }
      if ($7 != "-") { if (!($3 in first)) first[$3] = $7; last[$3] = $7; if ($7 + 0 > max[$3]) max[$3] = $7 + 0 } }
    END {
      k = split(order, names, ",")
      for (i = 1; i <= k; i++) {
        s = names[i]; if (!(s in n) || seen[s]++) continue
        line = sprintf("  %-10s  %d runs, %d ok", s, n[s], ok[s])
        if (t[s] > 0) line = line sprintf(", %.1f MB/s", b[s] / 1048576 / (t[s] / 1000))
        if (s in first) line = line sprintf(", peak memory first %.1f / last %.1f / highest %.1f MB", first[s] / 1048576, last[s] / 1048576, max[s] / 1048576)
        print line
      }
    }' "$results" | tee -a "$log"
  if [ "$failed" -gt 0 ]; then
    say "Failures (count, reason):"
    awk -F '\t' '$4 != "ok" { c[$8]++ } END { for (r in c) printf "  %5d  %s\n", c[r], r }' "$results" |
      sort -rn | tee -a "$log"
  fi
  say "Log:          $log"
  if [ "$runs" -eq 0 ]; then
    say "Result:       NO RUNS (nothing was downloaded)"
  elif [ -n "$server_note" ]; then
    say "Result:       NOT CLEAN (the test server died; $failed of $runs downloads failed before that)"
  elif [ "$failed" -gt 0 ]; then
    say "Result:       NOT CLEAN ($failed of $runs downloads failed)"
  elif [ -n "$stopped" ]; then
    say "Result:       STOPPED EARLY with no failures in $runs downloads (not a full soak)"
  else
    say "Result:       CLEAN (all $runs downloads finished and matched their SHA-256)"
  fi
  summary_failed=$failed
  summary_runs=$runs
}

# Writes the summary once and exits with the soak's status.
finish() {
  local stopped=$1 server_note=$2
  [ -z "$finishing" ] || return 0
  finishing=1
  trap '' INT TERM HUP
  stop_cli
  stop_server
  summary_failed=0
  summary_runs=0
  summary "$stopped" "$server_note"
  if [ "$summary_failed" -gt 0 ] || [ -n "$server_note" ]; then
    exit 1
  elif [ -n "$stopped" ] || [ "$summary_runs" -eq 0 ]; then
    exit 130
  fi
  exit 0
}

on_signal() {
  finish "by $1 (the download in progress was not counted)" ""
}
trap 'on_signal Ctrl-C' INT
trap 'on_signal SIGTERM' TERM
trap 'on_signal SIGHUP' HUP

# Stops the CLI if it runs too long, leaving a marker so the run counts as hung.
watchdog() {
  local pid=$1 left=$(($2 * 10)) marker=$3
  # Told to stop (the download ended): exit after the current sleep, quietly.
  trap 'exit 0' TERM
  while [ "$left" -gt 0 ]; do
    kill -0 "$pid" 2>/dev/null || exit 0
    sleep 0.1
    left=$((left - 1))
  done
  : >"$marker"
  kill_tree TERM "$pid"
  sleep 3
  kill_tree KILL "$pid"
}

# The CLI's own explanation: its first "fuselane: ..." line, else its first line.
reason_from() {
  local line
  line=$(grep -m 1 '^fuselane: ' "$1" 2>/dev/null) || line=""
  line=${line#fuselane: }
  if [ -z "$line" ]; then
    line=$(grep -m 1 -v '^[[:space:]]*$' "$1" 2>/dev/null) || line=""
  fi
  printf '%s' "${line:-no message}" | tr '\t\r\n' '   ' | cut -c 1-300
}

# One download: run, check, record, clean up.
soak_run() {
  local n=$1 i=$2 dir rc t0 t1 ms file f got sha extra rss reason limit line speed
  local label=${labels[$i]} size=${sizes[$i]}
  dir=$work/run-$n
  mkdir -p "$dir/out"
  limit=${run_timeout:-$((300 + size / 1048576))}
  set --
  case $time_kind in
    bsd) set -- /usr/bin/time -l -o "$dir/time" ;;
    gnu) set -- /usr/bin/time -v -o "$dir/time" ;;
  esac
  t0=$(now_ms)
  "$@" "$fuselane" get "${urls[$i]}" -o "$dir/out" -n "$networks" -q \
    >"$dir/stdout" 2>"$dir/stderr" 3>&- &
  cli_pid=$!
  watchdog "$cli_pid" "$limit" "$dir/hung" 3>&- &
  watchdog_pid=$!
  rc=0
  wait "$cli_pid" 2>/dev/null || rc=$?
  t1=$(now_ms)
  cli_pid=""
  kill "$watchdog_pid" 2>/dev/null || true
  wait "$watchdog_pid" 2>/dev/null || true
  watchdog_pid=""
  ms=$((t1 - t0))

  rss=-
  if [ -f "$dir/time" ]; then
    case $time_kind in
      bsd) rss=$(awk '/maximum resident set size/ { print $1; exit }' "$dir/time") ;;
      gnu) rss=$(awk -F: '/Maximum resident set size/ { gsub(/ /, "", $2); printf "%.0f\n", $2 * 1024; exit }' "$dir/time") ;;
    esac
    [ -n "$rss" ] || rss=-
  fi

  file=$dir/out/file.bin
  reason=""
  if [ -f "$dir/hung" ]; then
    reason="hung: no result after ${limit}s, so it was stopped"
  elif [ "$rc" -ne 0 ]; then
    reason="exit $rc: $(reason_from "$dir/stderr")"
    if [ -e "$file" ]; then reason="$reason (and it left a file.bin behind)"; fi
  elif [ ! -f "$file" ]; then
    reason="said it finished, but file.bin isn't in the folder"
  else
    extra=""
    for f in "$dir/out"/* "$dir/out"/.[!.]*; do
      if [ -e "$f" ] && [ "$f" != "$file" ]; then extra="$extra ${f##*/}"; fi
    done
    got=$(wc -c <"$file" | tr -d ' ')
    if [ "$got" != "$size" ]; then
      reason="wrong size: $got bytes, expected $size"
    else
      sha=$(sha256 "$file")
      if [ "$sha" != "${shas[$i]}" ]; then reason="SHA-256 mismatch: got $sha, expected ${shas[$i]}"; fi
    fi
    if [ -z "$reason" ] && [ -n "$extra" ]; then
      reason="left extra files next to the download:$extra"
    fi
  fi
  rm -rf "$dir"

  line="$(date '+%Y-%m-%dT%H:%M:%S') run $n $label"
  if [ -z "$reason" ]; then
    speed=$(awk -v b="$size" -v ms="$ms" 'BEGIN { if (ms < 1) ms = 1; printf "%.1f", b / 1048576 / (ms / 1000) }')
    line="$line ok $(mb "$size") MB in $(awk -v ms="$ms" 'BEGIN { printf "%.2f", ms / 1000 }') s ($speed MB/s)"
    printf '%s\t%s\t%s\tok\t%s\t%s\t%s\t\n' "$n" "$i" "$label" "$size" "$ms" "$rss" >>"$results"
  else
    line="$line FAILED after $(awk -v ms="$ms" 'BEGIN { printf "%.2f", ms / 1000 }') s: $reason"
    printf '%s\t%s\t%s\tfail\t0\t%s\t%s\t%s\n' "$n" "$i" "$label" "$ms" "$rss" "$reason" >>"$results"
  fi
  if [ "$rss" != - ]; then line="$line, peak memory $(mb "$rss") MB"; fi
  say "$line"
}

# ---------- the soak ----------

run=0
soaking=1
while :; do
  [ "$(date +%s)" -lt "$end_s" ] || break
  if ! kill -0 "$server_pid" 2>/dev/null; then
    say "The test server stopped unexpectedly. Its last words: $(tr -s '\n\t ' '   ' <"$work/server.err")"
    finish "" "stopped unexpectedly during the soak"
  fi
  soak_run $((run + 1)) $((run % ${#sizes[@]}))
  run=$((run + 1))
  left=$((end_s - $(date +%s)))
  if [ "$pause" != 0 ] && [ "$left" -gt 0 ]; then
    # Never past the end: the last pause is cut to the time that's left.
    sleep "$(awk -v p="$pause" -v l="$left" 'BEGIN { print (p < l ? p : l) }')" || true
  fi
done
finish "" ""
}
