#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
TMP=$(mktemp -d "${TMPDIR:-/tmp}/jtv-e2e.XXXXXX")
trap 'rm -rf "$TMP"' EXIT

cd "$ROOT"
cargo build --quiet

export HOME="$TMP/home"
export XDG_CONFIG_HOME="$HOME/.config"
export TELEVISION_CONFIG="$XDG_CONFIG_HOME/television"
export JTV_TV_CABLE_DIR="$TELEVISION_CONFIG/cable"
export JTV_E2E_LOG="$TMP/executions.log"
export JTV_E2E_SECRET_TRANSCRIPT="$TMP/secret-transcript.log"
export JTV_E2E_PREVIEW_LOG="$TMP/preview.log"
export JTV_REAL_BIN="$ROOT/target/debug/jtv"
export TMPDIR="$TMP/runtime"
export PATH="$ROOT/tests/fixtures/e2e/bin:$ROOT/target/debug:$PATH"
export TERM=xterm-256color
mkdir -p "$HOME" "$TELEVISION_CONFIG" "$TMPDIR"
touch "$JTV_E2E_LOG"

jtv init >/dev/null
jtv doctor | grep -F '[OK] channel' >/dev/null

expect <<'EXPECT_SIMPLE'
set timeout 20
stty rows 40 columns 160
spawn -noecho jtv --justfile tests/fixtures/e2e/justfile
after 800
send -- "simple"
after 800
send -- "\r"
expect {
  -re {Run selected recipe} { send -- "\r" }
  timeout { puts stderr "simple: confirmation prompt not reached"; exit 1 }
}
expect eof
set result [wait]
if {[lindex $result 3] != 0} { exit 1 }
EXPECT_SIMPLE

grep -Fx 'simple' "$JTV_E2E_LOG" >/dev/null
if ! grep -F 'Write a no-argument marker.' "$JTV_E2E_PREVIEW_LOG" >/dev/null; then
    echo "live preview did not contain the selected recipe" >&2
    cat "$JTV_E2E_PREVIEW_LOG" >&2
    exit 1
fi

before_cancel=$(wc -l < "$JTV_E2E_LOG")
expect <<'EXPECT_CANCEL'
set timeout 20
spawn -noecho jtv --justfile tests/fixtures/e2e/justfile
after 800
send -- "\033"
expect eof
set result [wait]
if {[lindex $result 3] != 0} { exit 1 }
EXPECT_CANCEL
after_cancel=$(wc -l < "$JTV_E2E_LOG")
test "$before_cancel" = "$after_cancel"

expect <<'EXPECT_INTERRUPT'
set timeout 20
spawn -noecho env PS1=JTV-SHELL bash --noprofile --norc -i
expect -re {JTV-SHELL}
send -- "jtv --justfile tests/fixtures/e2e/justfile\r"
after 800
send -- "\003"
expect {
  -re {JTV-SHELL} { send -- "printf 'terminal-ready\\n'\r" }
  timeout { puts stderr "interrupt: shell prompt was not restored"; exit 1 }
}
expect {
  -re {terminal-ready} { send -- "exit\r" }
  timeout { puts stderr "interrupt: next shell command did not run"; exit 1 }
}
expect eof
EXPECT_INTERRUPT

if find "$TMPDIR" -maxdepth 1 -type f \( -name 'jtv-session-*' -o -name 'jtv-picker-*' \) | grep -q .; then
    echo "interrupt left jtv temporary state behind" >&2
    exit 1
fi

expect <<'EXPECT_LITERAL'
set timeout 20
spawn -noecho jtv --justfile tests/fixtures/e2e/justfile
after 800
send -- "capture"
after 300
send -- "\r"
expect {
  -re {value} { send -- {literal;$(printf hacked)}; send -- "\r" }
  timeout { puts stderr "capture: value prompt not reached"; exit 1 }
}
expect {
  -re {Run selected recipe} { send -- "\r" }
  timeout { puts stderr "capture: confirmation prompt not reached"; exit 1 }
}
expect eof
set result [wait]
if {[lindex $result 3] != 0} { exit 1 }
EXPECT_LITERAL

grep -Fx 'capture:literal;$(printf hacked)' "$JTV_E2E_LOG" >/dev/null

expect <<'EXPECT_CHOICE'
set timeout 20
cd tests/fixtures/e2e
spawn -noecho jtv --justfile justfile
after 800
send -- "choose"
after 300
send -- "\r"
after 800
send -- "staging"
after 300
send -- "\r"
expect {
  -re {Run selected recipe} { send -- "\r" }
  timeout { puts stderr "choice: confirmation prompt not reached"; exit 1 }
}
expect eof
set result [wait]
if {[lindex $result 3] != 0} { exit 1 }
EXPECT_CHOICE

grep -Fx 'choose:staging' "$JTV_E2E_LOG" >/dev/null

expect <<'EXPECT_FILE'
set timeout 20
cd tests/fixtures/e2e
spawn -noecho jtv --justfile justfile
after 800
send -- "pick-file"
after 300
send -- "\r"
after 800
send -- "sample.txt"
after 300
send -- "\r"
expect {
  -re {Run selected recipe} { send -- "\r" }
  timeout { puts stderr "file: confirmation prompt not reached"; exit 1 }
}
expect eof
set result [wait]
if {[lindex $result 3] != 0} { exit 1 }
EXPECT_FILE

grep -F 'file:' "$JTV_E2E_LOG" | tail -n 1 | grep -F '/tests/fixtures/e2e/sample.txt' >/dev/null

expect <<'EXPECT_SECRET'
set timeout 20
log_file -noappend $env(JTV_E2E_SECRET_TRANSCRIPT)
cd tests/fixtures/e2e
spawn -noecho jtv --justfile justfile
after 800
send -- "secret"
after 300
send -- "\r"
expect {
  -re {token} { send -- "s3cr3t-value\r" }
  timeout { puts stderr "secret: password prompt not reached"; exit 1 }
}
expect {
  -re {Run selected recipe} { send -- "\r" }
  timeout { puts stderr "secret: confirmation prompt not reached"; exit 1 }
}
expect eof
set result [wait]
if {[lindex $result 3] != 0} { exit 1 }
EXPECT_SECRET

grep -Fx 'secret:s3cr3t-value' "$JTV_E2E_LOG" >/dev/null
if grep -F 's3cr3t-value' "$JTV_E2E_SECRET_TRANSCRIPT" >/dev/null; then
    echo "secret appeared in terminal output" >&2
    exit 1
fi
grep -F '[REDACTED]' "$JTV_E2E_SECRET_TRANSCRIPT" >/dev/null

expect <<'EXPECT_QUEUE'
set timeout 20
spawn -noecho jtv --justfile tests/fixtures/e2e/justfile
after 800
send -- "queue-"
after 400
send -- "\t"
after 250
send -- "\t"
after 250
send -- "\t"
after 250
send -- "\r"
expect {
  -re {Run selected recipe} { send -- "\r" }
  timeout { puts stderr "queue: confirmation prompt not reached"; exit 1 }
}
expect eof
set result [wait]
if {[lindex $result 3] != 7} {
  puts stderr "queue: expected exit 7, got [lindex $result 3]"
  exit 1
}
EXPECT_QUEUE

tail -n 2 "$JTV_E2E_LOG" | grep -Fx 'queue-a' >/dev/null
tail -n 2 "$JTV_E2E_LOG" | grep -Fx 'queue-b-fail' >/dev/null
if grep -Fx 'queue-c-after' "$JTV_E2E_LOG" >/dev/null; then
    echo "queue-c-after ran after a failure" >&2
    exit 1
fi

test ! -e .just_history
test ! -e .just-tv-last-command
printf 'jtv real-TV E2E passed\n'
