#!/bin/sh
set -eu

here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d /tmp/adgc.XXXXXX)
vm=$tmp/box/vm
build=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
peer_pid=
agent_pid=
cleanup() {
  kill $peer_pid $agent_pid 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup 0
trap 'exit 1' 1 2 3 15
mkdir -p "$vm" "$tmp/guest" "$tmp/other" "$tmp/share/builds/$build"
: > "$tmp/out"
: > "$tmp/err"

fail() {
  printf 'FAIL: %s\n%s\n' "$1" "$(cat "$tmp/out" "$tmp/err" | head -c 2000)" >&2
  exit 1
}

xcrun swiftc -Onone "$here/vmhost.swift" -o "$tmp/vmhost" 2> "$tmp/err" || fail 'the VM host did not compile'
xcrun swiftc -Onone -D GUEST_AGENT_TEST "$here/guestagent.swift" -o "$tmp/agent" 2> "$tmp/err" || fail 'the guest agent did not compile'

# A process on the agent's port. "impostor" replays a proof the real agent gave for another nonce; every other
# scenario relays the real agent's proof for this nonce and then answers one request itself.
cat > "$tmp/guest.py" <<'PY'
import socket, struct, sys
path, scenario, agent, record = sys.argv[1:5]
def frame(kind, payload=b""):
    return bytes([kind]) + struct.pack(">I", len(payload)) + payload
replies = {
    "oversized": bytes([1]) + struct.pack(">I", 65537) + b"x" * 65537,
    "unknown-kind": frame(9),
    "truncated": bytes([1]) + struct.pack(">I", 10) + b"abc",
    "complete": frame(1, b"ran\n") + frame(3, struct.pack(">I", 0)),
    "status-extra": frame(1, b'{"accessibility":true,"agent":1,"evil":"<x>"}\n') + frame(3, struct.pack(">I", 0)),
}
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(path)
server.listen(1)
open(path + ".ready", "w").close()
conn, _ = server.accept()
nonce = conn.recv(32, socket.MSG_WAITALL)
real = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
real.connect(agent)
real.sendall(bytes(32) if scenario == "impostor" else nonce)
conn.sendall(real.recv(96, socket.MSG_WAITALL))
if scenario == "impostor":
    conn.settimeout(1)
    try:
        open(record, "wb").write(conn.recv(4096))
    except OSError:
        open(record, "wb").close()
else:
    header = conn.recv(4, socket.MSG_WAITALL)
    if len(header) == 4:
        conn.recv(struct.unpack(">I", header)[0], socket.MSG_WAITALL)
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

stop() {
  [ -n "$1" ] || return 0
  kill "$1" 2>/dev/null || true
  wait "$1" 2>/dev/null || true
}

# agent <socket> <guest directory> [NAME=VALUE...]: starts the real guest agent, built for tests, on a socket file.
agent() {
  stop "$agent_pid"
  rm -f "$vm/$1"
  agent_socket=$1
  agent_guest=$2
  shift 2
  (cd "$vm" && exec env GUEST_TEST_SOCKET="$agent_socket" GUEST_TEST_SHARE="$tmp/share" GUEST_TEST_HOME="$agent_guest" "$@" "$tmp/agent" 2>> "$tmp/agent.log") &
  agent_pid=$!
  wait_for "$vm/$agent_socket"
}

# scripted_guest <scenario>: puts the scripted process behind the sandbox's control socket.
scripted_guest() {
  stop "$peer_pid"
  rm -f "$vm/vmhost.sock" "$vm/vmhost.sock.ready" "$tmp/received"
  (cd "$vm" && exec python3 "$tmp/guest.py" vmhost.sock "$1" agent.sock "$tmp/received" 2> /dev/null) &
  peer_pid=$!
  wait_for "$vm/vmhost.sock.ready"
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

agent agent.sock "$tmp/guest"
scripted_guest status-extra
call status "$vm"
key=$(sed -n 's/.*"key":"\([0-9a-f]\{64\}\)".*/\1/p' "$tmp/out")
[ "$status" = 0 ] && [ "$(cat "$tmp/out")" = "{\"accessibility\":true,\"agent\":1,\"key\":\"$key\",\"ok\":true}" ] || fail 'a status reply passed a field the guest added'

scripted_guest complete
call exec "$vm" "$build" version
refused 'exec before the guest agent key is recorded'
printf '%s\n' "$key" > "$vm/agent.pub"

for scenario in oversized unknown-kind truncated; do
  scripted_guest "$scenario"
  call exec "$vm" "$build" version
  refused "a guest that answers with a $scenario frame"
done

scripted_guest impostor
call exec "$vm" "$build" type 'a secret'
refused 'a process that cannot prove the guest agent key'
wait "$peer_pid" 2>/dev/null || true
[ -f "$tmp/received" ] && [ ! -s "$tmp/received" ] || fail 'the request reached a process that could not prove the guest agent key'

cat > "$tmp/share/builds/$build/agent-desktop" <<'GUEST'
#!/bin/sh
for argument do printf '[%s]\n' "$argument"; done
[ "${1:-}" != hang ] || { printf '%s\n' "$$" > "$GUEST_TEST_HOME/hang.pid"; exec sleep 60; }
exit "${2:-0}"
GUEST
: > "$tmp/share/builds/$build/agent-desktop-macos-helper"
stop "$peer_pid"
peer_pid=

agent vmhost.sock "$tmp/guest"
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
agent vmhost.sock "$tmp/guest" GUEST_TEST_PEER=3
call exec "$vm" "$build" hang
refused 'a caller that is not the host'
[ ! -e "$tmp/guest/hang.pid" ] || fail 'the guest agent ran a command for a caller that is not the host'

agent vmhost.sock "$tmp/other"
call status "$vm"
[ "$status" = 4 ] || fail "an agent with another account's key: status exit $status, expected 4"
call exec "$vm" "$build" hang
refused "an agent with another account's key"
[ ! -e "$tmp/other/hang.pid" ] || fail "a command ran under an agent with another account's key"

printf 'guest channel tests passed\n'
