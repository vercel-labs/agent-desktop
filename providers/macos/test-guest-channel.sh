#!/bin/sh
set -eu

here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d /tmp/adgc.XXXXXX)
vm=$tmp/box/vm
build=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
peer_pid=
cleanup() {
  [ -z "$peer_pid" ] || kill "$peer_pid" 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup 0
trap 'exit 1' 1 2 3 15
mkdir -p "$vm" "$tmp/guest" "$tmp/share/builds/$build"
: > "$tmp/out"
: > "$tmp/err"

fail() {
  printf 'FAIL: %s\n%s\n' "$1" "$(cat "$tmp/out" "$tmp/err" | head -c 2000)" >&2
  exit 1
}

xcrun swiftc -Onone "$here/vmhost.swift" -o "$tmp/vmhost" 2> "$tmp/err" || fail 'the VM host did not compile'
xcrun swiftc -Onone -D GUEST_AGENT_TEST "$here/guestagent.swift" -o "$tmp/agent" 2> "$tmp/err" || fail 'the guest agent did not compile'

# A guest that answers one request with the bytes of a named scenario, however malformed.
cat > "$tmp/guest.py" <<'PY'
import socket, struct, sys
path, scenario = sys.argv[1:3]
def frame(kind, payload=b""):
    return bytes([kind]) + struct.pack(">I", len(payload)) + payload
replies = {
    "oversized": bytes([1]) + struct.pack(">I", 65537) + b"x" * 65537,
    "unknown-kind": frame(9),
    "truncated": bytes([1]) + struct.pack(">I", 10) + b"abc",
    "status-extra": frame(1, b'{"accessibility":true,"agent":1,"evil":"<x>"}\n') + frame(3, struct.pack(">I", 0)),
}
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(path)
server.listen(1)
open(path + ".ready", "w").close()
conn, _ = server.accept()
conn.recv(struct.unpack(">I", conn.recv(4, socket.MSG_WAITALL))[0], socket.MSG_WAITALL)
conn.sendall(replies[scenario])
conn.close()
PY

wait_for() {
  tries=0
  while [ ! -e "$1" ]; do
    tries=$((tries + 1))
    [ "$tries" -lt 100 ] || fail "$1 never appeared"
    sleep 0.1
  done
}

# peer <ready file> <command...>: puts a guest behind the sandbox's control socket.
peer() {
  [ -z "$peer_pid" ] || kill "$peer_pid" 2>/dev/null || true
  [ -z "$peer_pid" ] || wait "$peer_pid" 2>/dev/null || true
  rm -f "$vm/vmhost.sock" "$vm/vmhost.sock.ready"
  ready=$1
  shift
  (cd "$vm" && exec "$@" 2>> "$tmp/peer.log") &
  peer_pid=$!
  wait_for "$ready"
}

scripted_guest() {
  peer "$vm/vmhost.sock.ready" python3 "$tmp/guest.py" vmhost.sock "$1"
}

agent() {
  peer "$vm/vmhost.sock" env GUEST_TEST_SOCKET=vmhost.sock GUEST_TEST_SHARE="$tmp/share" GUEST_TEST_HOME="$tmp/guest" "$@" "$tmp/agent"
}

# call <vmhost arguments>: runs the real VM host client and keeps its exit status, stdout and stderr.
call() {
  status=0
  "$tmp/vmhost" "$@" > "$tmp/out" 2> "$tmp/err" || status=$?
}

refused() {
  [ "$status" = 125 ] && grep -q '"code":"ACTION_FAILED"' "$tmp/out" || fail "$1: exit $status without a provider failure"
  [ "$(wc -l < "$tmp/out" | tr -d ' ')" = 1 ] && [ "$(wc -c < "$tmp/out" | tr -d ' ')" -lt 1024 ] || fail "$1: guest bytes reached stdout beside the failure"
}

for scenario in oversized unknown-kind truncated; do
  scripted_guest "$scenario"
  call exec "$vm" "$build" version
  refused "a guest that answers with a $scenario frame"
done
scripted_guest status-extra
call status "$vm"
[ "$status" = 0 ] && [ "$(cat "$tmp/out")" = '{"accessibility":true,"agent":1,"ok":true}' ] || fail 'a status reply passed a field the guest added'

cat > "$tmp/share/builds/$build/agent-desktop" <<'GUEST'
#!/bin/sh
for argument do printf '[%s]\n' "$argument"; done
[ "${1:-}" != hang ] || { printf '%s\n' "$$" > "$GUEST_TEST_HOME/hang.pid"; exec sleep 60; }
exit "${2:-0}"
GUEST
: > "$tmp/share/builds/$build/agent-desktop-macos-helper"

agent
call exec "$vm" "$build" leave 7 'two words' ''
[ "$status" = 7 ] && [ "$(cat "$tmp/out")" = "$(printf '[leave]\n[7]\n[two words]\n[]')" ] || fail 'the guest agent changed the arguments or the exit status'
call exec "$vm" ../../../../etc version
refused 'a build id that is a path'
grep -q 'did not name the agent-desktop build' "$tmp/out" || fail 'a build id that is a path was not refused as a build id'

"$tmp/vmhost" exec "$vm" "$build" hang > /dev/null 2>&1 &
client_pid=$!
wait_for "$tmp/guest/hang.pid"
guest_command=$(cat "$tmp/guest/hang.pid")
kill "$client_pid"
tries=0
while kill -0 "$guest_command" 2>/dev/null; do
  tries=$((tries + 1))
  [ "$tries" -lt 50 ] || { kill "$guest_command" 2>/dev/null; fail 'the guest command kept running after the host stopped waiting'; }
  sleep 0.1
done

rm -f "$tmp/guest/hang.pid"
agent GUEST_TEST_PEER=3
call exec "$vm" "$build" hang
refused 'a caller that is not the host'
[ ! -e "$tmp/guest/hang.pid" ] || fail 'the guest agent ran a command for a caller that is not the host'

printf 'guest channel tests passed\n'
