#!/bin/sh
set -eu

here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d /tmp/adgc.XXXXXX)
vm=$tmp/box/vm
build_a=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
build_b=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
build_c=cccccccccccccccccccccccccccccccccccccccc
peer_pid=
cleanup() {
  [ -z "$peer_pid" ] || kill "$peer_pid" 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup 0
trap 'exit 1' 1 2 3 15
mkdir -p "$vm" "$tmp/guest" "$tmp/share/builds/$build_a" "$tmp/share/builds/$build_b"

fail() {
  printf 'FAIL: %s\n%s\n' "$1" "$(cat "$tmp/out" "$tmp/err" 2>/dev/null | head -c 2000)" >&2
  exit 1
}

if ! xcrun swiftc -Onone "$here/vmhost.swift" -o "$tmp/vmhost" 2> "$tmp/err" || \
  ! xcrun swiftc -Onone -D GUEST_AGENT_TEST "$here/guestagent.swift" -o "$tmp/agent" 2>> "$tmp/err"; then
  : > "$tmp/out"
  fail 'the VM host or the guest agent did not compile'
fi

# A guest that answers one request with the bytes of a named scenario, however malformed.
cat > "$tmp/guest.py" <<'PY'
import socket, struct, sys, time
path, scenario, record = sys.argv[1:4]
def frame(kind, payload=b""):
    return bytes([kind]) + struct.pack(">I", len(payload)) + payload
done = frame(3, struct.pack(">I", 0))
replies = {
    "ok": frame(1, b"out\n") + frame(2, b"err\n") + frame(3, struct.pack(">I", 7)),
    "oversized": bytes([1]) + struct.pack(">I", 65537) + b"x" * 65537,
    "unknown-kind": frame(9),
    "truncated": bytes([1]) + struct.pack(">I", 10) + b"abc",
    "bad-exit": frame(3, b"\0\0"),
    "eof": b"",
    "failure": frame(4, b"m" * 2000),
    "status-ok": frame(1, b'{"accessibility":true,"agent":1}\n') + done,
    "status-extra": frame(1, b'{"accessibility":true,"agent":1,"evil":"<x>"}\n') + done,
    "status-garbage": frame(1, b"not json") + done,
    "status-huge": frame(1, b"x" * 60000) * 2 + done,
    "status-wrong-type": frame(1, b'{"accessibility":"yes","agent":1}') + done,
}
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(path)
server.listen(1)
open(path + ".ready", "w").close()
conn, _ = server.accept()
length = struct.unpack(">I", conn.recv(4, socket.MSG_WAITALL))[0]
open(record, "wb").write(conn.recv(length, socket.MSG_WAITALL))
try:
    if scenario == "silent":
        time.sleep(30)
    elif scenario == "flood":
        while True:
            conn.sendall(frame(1, b"x" * 65536))
    else:
        conn.sendall(replies[scenario])
except OSError:
    pass
conn.close()
PY

# A host that sends the guest agent one request, however malformed, and prints the kind of the first frame back.
cat > "$tmp/host.py" <<'PY'
import socket, struct, sys
path, scenario = sys.argv[1:3]
requests = {
    "zero-length": struct.pack(">I", 0),
    "oversized": struct.pack(">I", 2 << 20),
    "bad-json": struct.pack(">I", 8) + b"not json",
    "unknown-op": struct.pack(">I", 15) + b'{"op":"reboot"}',
    "truncated": struct.pack(">I", 500) + b'{"op":',
}
conn = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
conn.settimeout(5)
conn.connect(path)
conn.sendall(requests[scenario])
if scenario == "truncated":
    conn.shutdown(socket.SHUT_WR)
try:
    first = conn.recv(1)
except OSError:
    first = b"?"
print(first[0] if first else "eof")
PY

wait_for() {
  tries=0
  while [ ! -e "$1" ]; do
    tries=$((tries + 1))
    [ "$tries" -lt 100 ] || { : > "$tmp/out"; : > "$tmp/err"; fail "$1 never appeared"; }
    sleep 0.1
  done
}

stop_peer() {
  [ -z "$peer_pid" ] || kill "$peer_pid" 2>/dev/null || true
  [ -z "$peer_pid" ] || wait "$peer_pid" 2>/dev/null || true
  peer_pid=
  rm -f "$vm/vmhost.sock" "$vm/vmhost.sock.ready"
}

# guest <scenario>: puts the scripted guest behind the sandbox's control socket.
guest() {
  stop_peer
  python3 "$tmp/guest.py" "$vm/vmhost.sock" "$1" "$tmp/request" &
  peer_pid=$!
  wait_for "$vm/vmhost.sock.ready"
}

# agent [NAME=VALUE...]: puts the real guest agent, built for tests, behind the sandbox's control socket.
agent() {
  stop_peer
  (cd "$vm" && exec env GUEST_TEST_SOCKET=vmhost.sock GUEST_TEST_SHARE="$tmp/share" GUEST_TEST_HOME="$tmp/guest" "$@" "$tmp/agent" 2>> "$tmp/agent.log") &
  peer_pid=$!
  wait_for "$vm/vmhost.sock"
}

# call <vmhost arguments>: runs the real VM host client and keeps its exit status, stdout and stderr.
call() {
  status=0
  "$tmp/vmhost" "$@" > "$tmp/out" 2> "$tmp/err" || status=$?
}

refused() {
  [ "$status" = 125 ] || fail "$1: exit $status, expected 125"
  grep -q '"code":"ACTION_FAILED"' "$tmp/out" || fail "$1: no provider failure on stdout"
  [ "$(wc -l < "$tmp/out" | tr -d ' ')" = 1 ] && [ "$(wc -c < "$tmp/out" | tr -d ' ')" -lt 1024 ] || fail "$1: guest bytes reached stdout beside the failure"
}

guest ok
call exec "$vm" "$build_a" version --flag
[ "$status" = 7 ] || fail "a well-formed reply: exit $status, expected 7"
[ "$(cat "$tmp/out")" = out ] && [ "$(cat "$tmp/err")" = err ] || fail 'a well-formed reply did not keep stdout and stderr apart'
[ "$(python3 -c 'import json,sys; r=json.load(open(sys.argv[1])); print(r["op"], r["build"], *r["args"])' "$tmp/request")" = "exec $build_a version --flag" ] || fail 'the request did not carry the operation, the build and the arguments'

for scenario in oversized unknown-kind truncated bad-exit eof; do
  guest "$scenario"
  call exec "$vm" "$build_a" version
  refused "a guest that answers with a $scenario frame"
done

guest failure
call exec "$vm" "$build_a" version
refused 'a guest that reports a failure'
[ "$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["message"]))' "$tmp/out")" = 512 ] || fail 'a failure message from the guest was not cut at 512 characters'

guest flood
"$tmp/vmhost" exec "$vm" "$build_a" version 2> "$tmp/err" | head -c 200000 > "$tmp/out" &
flood_pid=$!
tries=0
while kill -0 "$flood_pid" 2>/dev/null; do
  tries=$((tries + 1))
  [ "$tries" -lt 100 ] || { kill "$flood_pid" 2>/dev/null; : > "$tmp/out"; fail 'the client kept reading a guest that floods after its reader left'; }
  sleep 0.1
done
[ "$(wc -c < "$tmp/out" | tr -d ' ')" = 200000 ] || fail 'a flooding guest did not stream through the client'

guest status-ok
call status "$vm"
[ "$status" = 0 ] && [ "$(cat "$tmp/out")" = '{"accessibility":true,"agent":1,"ok":true}' ] || fail 'a well-formed status reply was not reported'
guest status-extra
call status "$vm"
[ "$status" = 0 ] && [ "$(cat "$tmp/out")" = '{"accessibility":true,"agent":1,"ok":true}' ] || fail 'a status reply passed a field the guest added'
for scenario in status-garbage status-huge status-wrong-type silent; do
  guest "$scenario"
  call status "$vm"
  [ "$status" = 3 ] || fail "a guest that answers status with $scenario: exit $status, expected 3"
  ! grep -q '"ok":true' "$tmp/out" || fail "a guest that answers status with $scenario was reported as answering"
done

stop_peer
call status "$vm"
[ "$status" = 3 ] || fail "status with no VM host: exit $status, expected 3"
call exec "$vm" "$build_a" version
refused 'exec with no VM host'

cat > "$tmp/share/builds/$build_a/agent-desktop" <<'GUEST'
#!/bin/sh
for argument do printf '[%s]\n' "$argument"; done
printf 'guest warning\n' >&2
case ${1:-} in
  hang) printf '%s\n' "$$" > "$GUEST_TEST_HOME/hang.pid"; exec sleep 60 ;;
  leave) exit "$2" ;;
esac
GUEST
printf '#!/bin/sh\nprintf "build b\\n"\n' > "$tmp/share/builds/$build_b/agent-desktop"
: > "$tmp/share/builds/$build_a/agent-desktop-macos-helper"
: > "$tmp/share/builds/$build_b/agent-desktop-macos-helper"

agent
call exec "$vm" "$build_a" type "two words" 'it'"'"'s $HOME `id`' '' --
[ "$status" = 0 ] || fail "exec through the guest agent exited $status"
printf '[type]\n[two words]\n[it'"'"'s $HOME `id`]\n[]\n[--]\n' > "$tmp/want"
cmp -s "$tmp/out" "$tmp/want" || fail 'the guest agent changed the arguments'
[ "$(cat "$tmp/err")" = 'guest warning' ] || fail 'the guest agent changed the guest stderr'
cmp -s "$tmp/share/builds/$build_a/agent-desktop" "$tmp/guest/.local/bin/agent-desktop" || fail 'the guest agent did not install the build the host named'
for guest_exit in 3 125 255; do
  call exec "$vm" "$build_a" leave "$guest_exit"
  [ "$status" = "$guest_exit" ] || fail "the guest agent turned exit $guest_exit into $status"
done
call exec "$vm" "$build_b" version
[ "$status" = 0 ] && [ "$(cat "$tmp/out")" = 'build b' ] || fail 'the guest agent did not switch to a newer build'
call exec "$vm" "$build_a" version
[ "$(cat "$tmp/out")" = '[version]' ] || fail 'the guest agent did not switch back to an earlier build'

for bad_build in '' short ../../../../etc "$(printf '%s' "$build_a" | cut -c1-39)/" "${build_a}a"; do
  call exec "$vm" "$bad_build" version
  refused "a build id of [$bad_build]"
  grep -q 'did not name the agent-desktop build' "$tmp/out" || fail "a build id of [$bad_build] was not refused as a build id"
done
call exec "$vm" "$build_c" version
refused 'a build the shared folder does not hold'
cmp -s "$tmp/share/builds/$build_a/agent-desktop" "$tmp/guest/.local/bin/agent-desktop" || fail 'a refused build replaced the installed one'

"$tmp/vmhost" exec "$vm" "$build_a" hang > /dev/null 2>&1 &
client_pid=$!
wait_for "$tmp/guest/hang.pid"
guest_command=$(cat "$tmp/guest/hang.pid")
kill -0 "$guest_command" 2>/dev/null || fail 'the guest command was not running before the host cancelled'
kill "$client_pid"
tries=0
while kill -0 "$guest_command" 2>/dev/null; do
  tries=$((tries + 1))
  [ "$tries" -lt 50 ] || { kill "$guest_command" 2>/dev/null; : > "$tmp/out"; : > "$tmp/err"; fail 'the guest command kept running after the host stopped waiting'; }
  sleep 0.1
done

for scenario in zero-length oversized bad-json unknown-op truncated; do
  reply=$(python3 "$tmp/host.py" "$vm/vmhost.sock" "$scenario")
  case $reply in
    4|eof) ;;
    *) : > "$tmp/out"; : > "$tmp/err"; fail "the guest agent answered a $scenario request with frame kind [$reply]" ;;
  esac
done
call status "$vm"
[ "$status" = 0 ] && grep -q '"ok":true' "$tmp/out" || fail 'the guest agent did not survive malformed requests'

rm -f "$tmp/guest/hang.pid"
agent GUEST_TEST_PEER=3
call status "$vm"
[ "$status" = 3 ] || fail "a caller that is not the host got a status reply: exit $status"
call exec "$vm" "$build_a" hang
refused 'a caller that is not the host'
[ ! -e "$tmp/guest/hang.pid" ] || fail 'the guest agent ran a command for a caller that is not the host'
agent GUEST_TEST_PEER=2
call status "$vm"
[ "$status" = 0 ] || fail "the host was refused: exit $status"

printf 'guest channel tests passed\n'
