#!/bin/sh
set -eu

here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/ad macos provider.XXXXXX")
cleanup() {
  [ -z "${vm_pid:-}" ] || kill "$vm_pid" 2>/dev/null || true
  chmod -R u+rwX "$tmp" 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup 0
trap 'exit 1' 1 2 3 15
mkdir -p "$tmp/bin" "$tmp/provider" "$tmp/sandboxes" "$tmp/guest/.local/bin"

AD_PROVIDER_DIR=$tmp/provider
AD_BINARY=$tmp/agent-desktop
AD_CALLER_CWD=$tmp
VMHOST=$tmp/bin/vmhost
FAKE_LOG=$tmp/calls
FAKE_GUEST_HOME=$tmp/guest
export AD_PROVIDER_DIR AD_BINARY AD_CALLER_CWD VMHOST FAKE_LOG FAKE_GUEST_HOME

printf '#!/bin/sh\n' > "$AD_PROVIDER_DIR/guest-setup.sh"
printf 'guest agent source\n' > "$AD_PROVIDER_DIR/guestagent.swift"
mkdir -p "$tmp/bin/agent-desktop-guest.app/Contents/MacOS"
printf 'guest agent\n' > "$tmp/bin/agent-desktop-guest.app/Contents/MacOS/agent-desktop-guest"
printf 'binary\n' > "$AD_BINARY"
printf 'helper\n' > "$tmp/agent-desktop-macos-helper"
chmod 0755 "$tmp/agent-desktop-macos-helper"

fake() {
  cat > "$tmp/bin/$1"
  chmod 0755 "$tmp/bin/$1"
}
fake uname <<'FAKE'
#!/bin/sh
printf 'arm64\n'
FAKE
fake sw_vers <<'FAKE'
#!/bin/sh
printf '14.0\n'
FAKE
fake sysctl <<'FAKE'
#!/bin/sh
printf '%s\n' "${FAKE_HV_SUPPORT:-1}"
FAKE
fake codesign <<'FAKE'
#!/bin/sh
printf 'com.apple.security.virtualization\n'
FAKE
fake pgrep <<'FAKE'
#!/bin/sh
[ "${FAKE_JOB_ALIVE:-0}" = 1 ]
FAKE
fake ps <<'FAKE'
#!/bin/sh
while [ "$#" -gt 1 ]; do shift; done
kill -0 "$1" 2>/dev/null && printf '/opt/build/vmhost run %s\n' "${FAKE_VM_DIR:-$AD_SANDBOX_DIR/vm}"
FAKE
fake vmhost <<'FAKE'
#!/bin/sh
printf 'vmhost %s\n' "$*" >> "$FAKE_LOG"
case $1 in
  install) [ "${FAKE_INSTALL:-ok}" = ok ] || { printf 'the disk is full\n'; exit 1; } ;;
  clone)
    [ "${FAKE_CLONE:-ok}" = ok ] || { printf 'clonefile is not supported here\n'; exit 1; }
    mkdir -p "$3"
    cp "$2/Disk.img" "$3/Disk.img" 2>/dev/null || :
    ;;
  status)
    case ${FAKE_GUEST:-granted} in
      granted) printf '{"accessibility":true,"agent":1,"ok":true}\n' ;;
      denied) printf '{"accessibility":false,"agent":1,"ok":true}\n' ;;
      silent)
        printf '{"message":"The guest agent is not answering.","ok":false,"suggestion":"Run it again."}\n'
        exit 3
        ;;
    esac
    ;;
  exec)
    shift 3
    case ${FAKE_GUEST:-granted} in
      silent)
        printf '{"code":"ACTION_FAILED","message":"The guest agent is not answering.","ok":false,"suggestion":"Run agent-desktop sandbox status demo."}\n'
        exit 125
        ;;
    esac
    HOME=$FAKE_GUEST_HOME "$FAKE_GUEST_HOME/.local/bin/agent-desktop" "$@"
    exit $?
    ;;
  run)
    [ "${FAKE_BOOT:-ok}" = ok ] || exit 1
    printf '%s\n' "$$" > "$2/vmhost.pid"
    : > "$2/vmhost.started"
    exec sleep 30
    ;;
esac
FAKE
fake curl <<'FAKE'
#!/bin/sh
printf 'curl %s\n' "$*" >> "$FAKE_LOG"
while [ "$#" -gt 0 ]; do
  if [ "$1" = -o ]; then out=$2; fi
  shift
done
case $out in
  *.catalog) printf '%s' "$FAKE_CATALOG" > "$out" ;;
  *) printf '%s' "$FAKE_IMAGE" > "$out" ;;
esac
FAKE
cat > "$tmp/guest/.local/bin/agent-desktop" <<'FAKE'
#!/bin/sh
for argument do printf '[%s]\n' "$argument"; done
[ -z "${FAKE_GUEST_NOISE:-}" ] || head -c "$FAKE_GUEST_NOISE" /dev/zero | tr '\0' 'x' >&2
printf 'guest warning\n' >&2
exit "${FAKE_GUEST_EXIT:-0}"
FAKE
chmod 0755 "$tmp/guest/.local/bin/agent-desktop"
PATH=$tmp/bin:$PATH
export PATH

fail() {
  printf 'FAIL: %s\n%s\n' "$1" "$(cat "$tmp/out" "$tmp/err" 2>/dev/null)" >&2
  exit 1
}

# provider <sandbox> [NAME=VALUE...] <verb> [args]: runs one verb and keeps its exit status, stdout and stderr.
provider() {
  AD_SANDBOX_DIR=$tmp/sandboxes/$1
  export AD_SANDBOX_DIR
  mkdir -p "$AD_SANDBOX_DIR"
  shift
  inserted=
  for arg do
    shift
    if [ -z "$inserted" ]; then
      case $arg in *=*) ;; *) set -- "$@" sh "$here/provider.sh"; inserted=1 ;; esac
    fi
    set -- "$@" "$arg"
  done
  status=0
  env "$@" > "$tmp/out" 2> "$tmp/err" || status=$?
}

field() {
  sed -n '$p' "$tmp/out" | sed -n "s/.*\"$1\":\"\([^\"]*\)\".*/\1/p"
}

# expect <label> <exit status> <field> <value>
expect() {
  [ "$status" = "$2" ] || fail "$1: exit $status, expected $2"
  [ "$(field "$3")" = "$4" ] || fail "$1: $3 is [$(field "$3")], expected [$4]"
}

# put <sandbox> <phase>: lays down the files that put a sandbox in one phase of sandbox_phase.
put() {
  dir=$tmp/sandboxes/$1
  [ -z "${vm_pid:-}" ] || kill "$vm_pid" 2>/dev/null || true
  vm_pid=
  chmod -R u+rwX "$dir" 2>/dev/null || true
  rm -rf "$dir"
  mkdir -p "$dir/vm/share"
  FAKE_JOB_ALIVE=0
  FAKE_VM_DIR=$dir/vm
  for file in Disk.img AuxiliaryStorage HardwareModel MachineIdentifier config.json \
    share/guest-setup.sh share/agent-desktop install.done; do
    : > "$dir/vm/$file"
  done
  : > "$dir/setup_done"
  case $2 in
    installing) rm "$dir/vm/install.done" "$dir/vm/Disk.img"; printf '1\n' > "$dir/vm/install.pid"; FAKE_JOB_ALIVE=1 ;;
    failed) rm "$dir/vm/install.done" "$dir/vm/Disk.img"; printf '1\n' > "$dir/vm/install.pid" ;;
    starting|running)
      sleep 30 &
      vm_pid=$!
      printf '%s\n' "$vm_pid" > "$dir/vm/vmhost.pid"
      [ "$2" = starting ] || : > "$dir/vm/vmhost.started"
      ;;
  esac
  export FAKE_JOB_ALIVE FAKE_VM_DIR
}

provider none FAKE_HV_SUPPORT=0 check
expect 'a Mac that cannot run VMs' 0 state unavailable
provider none check
expect 'a built and signed VM host' 0 state ready
mv "$tmp/bin/agent-desktop-guest.app" "$tmp/agent-desktop-guest.away"
fake xcrun <<'FAKE'
#!/bin/sh
exit 0
FAKE
provider none check
expect 'a VM host without the guest agent' 0 state missing
mv "$tmp/agent-desktop-guest.away" "$tmp/bin/agent-desktop-guest.app"

# Every verb in every phase. Columns: phase, then the expected "exit:field=value" for each verb.
while read -r phase status_want start_want stop_want rm_want exec_want; do
  for pair in "status:$status_want" "start:$start_want" "stop:$stop_want" "exec:$exec_want" "rm:$rm_want"; do
    verb=${pair%%:*}
    want=${pair#*:}
    put demo "$phase"
    if [ "$verb" = exec ]; then provider demo exec demo -- version; else provider demo "$verb" demo; fi
    want_field=${want#*:}
    expect "$verb while $phase" "${want%%:*}" "${want_field%%=*}" "${want_field#*=}"
  done
done <<'TABLE'
installing 0:state=working 0:state=working 1:code=ACTION_FAILED 1:code=ACTION_FAILED 125:code=ACTION_FAILED
failed     0:state=broken  0:state=broken  0:state=broken       0:state=stopped      125:code=ACTION_FAILED
stopped    0:state=stopped 0:state=running 0:state=stopped      0:state=stopped      125:code=ACTION_FAILED
starting   0:state=working 0:state=working 0:state=stopped      1:code=INVALID_ARGS  125:code=ACTION_FAILED
running    0:state=running 0:state=running 0:state=stopped      1:code=INVALID_ARGS  0:state=
TABLE

put demo stopped
provider demo start demo
vm_pid=$(cat "$tmp/sandboxes/demo/vm/vmhost.pid")
expect 'start boots a stopped sandbox' 0 state running
grep -q "vmhost run $tmp/sandboxes/demo/vm\$" "$FAKE_LOG" || fail 'a set-up sandbox did not start headless'

put demo stopped
: > "$tmp/sandboxes/demo/id_ed25519"
: > "$tmp/sandboxes/demo/known_hosts"
printf 'guest\n' > "$tmp/sandboxes/demo/guest_user"
: > "$tmp/sandboxes/demo/vm/host_key.pub"
provider demo start demo
vm_pid=$(cat "$tmp/sandboxes/demo/vm/vmhost.pid")
expect 'start of a sandbox that was set up over SSH' 0 state needs_setup
grep -q "vmhost run $tmp/sandboxes/demo/vm --window" "$FAKE_LOG" || fail 'a sandbox that was set up over SSH did not reopen its setup window'
[ ! -e "$tmp/sandboxes/demo/setup_done" ] || fail 'a sandbox that was set up over SSH kept its setup record'
[ -f "$tmp/sandboxes/demo/vm/share/agent-desktop-guest.app/Contents/MacOS/agent-desktop-guest" ] || fail 'start did not put the guest agent in the shared folder'
build_id=$(ls "$tmp/sandboxes/demo/vm/share/builds")
[ "${#build_id}" -eq 40 ] || fail "the shared builds are [$build_id], expected one 40-character id"
cmp -s "$AD_BINARY" "$tmp/sandboxes/demo/vm/share/builds/$build_id/agent-desktop" || fail 'start did not share the host build under its id'
rm "$tmp/sandboxes/demo/vm/share/agent-desktop"
provider demo stop demo
provider demo start demo
vm_pid=$(cat "$tmp/sandboxes/demo/vm/vmhost.pid")
[ -f "$tmp/sandboxes/demo/vm/share/agent-desktop" ] && [ -f "$tmp/sandboxes/demo/vm/share/id_ed25519.pub" ] || fail 'start left out a file that agent-desktop 1.3.0 needs before it calls a sandbox complete'
printf '%s' "$build_id" | grep -q '[^0-9a-f]' && fail "the build id [$build_id] is not lowercase hex"
for leftover in id_ed25519 known_hosts guest_user; do
  [ ! -e "$tmp/sandboxes/demo/$leftover" ] || fail "start kept the leftover $leftover"
done
[ ! -e "$tmp/sandboxes/demo/vm/host_key.pub" ] || fail 'start kept the leftover host key file'

put demo stopped
provider demo AD_SET_WINDOW=on 'AD_SET_TITLE=Review build' start demo
vm_pid=$(cat "$tmp/sandboxes/demo/vm/vmhost.pid")
expect 'start a set-up sandbox in a window' 0 state running
grep -q "vmhost run $tmp/sandboxes/demo/vm --window Review build\$" "$FAKE_LOG" || fail 'the window and its title did not reach the VM host'
put demo running
provider demo AD_SET_WINDOW=on start demo
expect 'asking for a window on a headless running sandbox' 1 code INVALID_ARGS
provider demo AD_SET_WINDOWS=on start demo
expect 'start with an unknown setting' 1 code INVALID_ARGS
put demo stopped
provider demo FAKE_BOOT=fails start demo
expect 'a VM host that does not boot' 1 code ACTION_FAILED
put demo stopped
rm "$tmp/sandboxes/demo/setup_done"
provider demo start demo
vm_pid=$(cat "$tmp/sandboxes/demo/vm/vmhost.pid")
expect 'a sandbox that is not set up starts in a window' 0 state needs_setup
grep -q "vmhost run $tmp/sandboxes/demo/vm --window" "$FAKE_LOG" || fail 'setup did not open a window'

put demo running
rm "$tmp/sandboxes/demo/setup_done"
provider demo status demo
expect 'status records a finished setup' 0 state running
[ -f "$tmp/sandboxes/demo/setup_done" ] || fail 'status did not record the finished setup'
provider demo FAKE_GUEST=denied status demo
expect 'accessibility not granted' 0 state needs_setup
[ "$(field next)" = 'agent-desktop sandbox stop demo' ] && [ ! -f "$tmp/sandboxes/demo/setup_done" ] || fail 'a headless sandbox that lost its setup was not sent back to a window'
put demo running
provider demo FAKE_GUEST=silent status demo
expect 'a guest that does not answer yet' 0 state working
put demo running
touch -t 202001010000 "$tmp/sandboxes/demo/vm/vmhost.started"
provider demo FAKE_GUEST=silent status demo
expect 'a guest agent silent for two minutes' 0 state needs_setup
[ -f "$tmp/sandboxes/demo/setup_done" ] || fail 'a guest that was only slow to answer lost its setup record'
provider demo status demo
expect 'a slow guest that answers after two minutes' 0 state running

put demo running
: > "$FAKE_LOG"
provider demo exec demo -- type "two words" 'it'"'"'s $HOME `id`' '' --
[ "$status" = 0 ] || fail "exec exited $status"
printf '[type]\n[two words]\n[it'"'"'s $HOME `id`]\n[]\n[--]\n' > "$tmp/want"
cmp -s "$tmp/out" "$tmp/want" || fail 'exec changed the guest arguments'
[ "$(cat "$tmp/err")" = 'guest warning' ] || fail 'exec changed the guest stderr'
build_id=$(ls "$tmp/sandboxes/demo/vm/share/builds")
grep -q "^vmhost exec $tmp/sandboxes/demo/vm $build_id type " "$FAKE_LOG" || fail 'exec did not tell the guest which build to run'
cp "$AD_BINARY" "$tmp/host-build"
cp "$AD_BINARY" "$tmp/sandboxes/demo/vm/share/builds/$build_id/first-read"
printf 'changed\n' >> "$AD_BINARY"
: > "$FAKE_LOG"
provider demo exec demo -- version
[ "$status" = 0 ] || fail "exec after a host rebuild exited $status"
new_build_id=$(sed -n "s|^vmhost exec $tmp/sandboxes/demo/vm \([0-9a-f]*\) version\$|\1|p" "$FAKE_LOG")
[ "${#new_build_id}" -eq 40 ] && [ "$new_build_id" != "$build_id" ] || fail 'exec kept the old build id after the host build changed'
cmp -s "$AD_BINARY" "$tmp/sandboxes/demo/vm/share/builds/$new_build_id/agent-desktop" || fail 'exec did not share the rebuilt agent-desktop under a new id'
cmp -s "$tmp/host-build" "$tmp/sandboxes/demo/vm/share/builds/$build_id/agent-desktop" && [ -f "$tmp/sandboxes/demo/vm/share/builds/$build_id/first-read" ] || fail 'exec changed a build the running guest may already have read'
cp "$tmp/host-build" "$AD_BINARY"
for guest_exit in 3 125 255; do
  provider demo FAKE_GUEST_EXIT=$guest_exit exec demo -- version
  [ "$status" = "$guest_exit" ] || fail "exec turned guest exit $guest_exit into $status"
done
provider demo FAKE_GUEST_NOISE=300000 FAKE_GUEST_EXIT=7 exec demo -- version
[ "$status" = 7 ] && [ "$(cat "$tmp/out")" = '[version]' ] || fail 'a noisy guest lost its output or exit status'
provider demo FAKE_GUEST=silent exec demo -- version
expect 'exec with a guest agent that does not answer' 125 code ACTION_FAILED

# create: every bad combination of settings is refused before anything is touched.
refuse() {
  label=$1
  shift
  rm -rf "$tmp/sandboxes/new"
  : > "$FAKE_LOG"
  AD_SANDBOX_DIR=$tmp/sandboxes/new
  export AD_SANDBOX_DIR
  mkdir -p "$AD_SANDBOX_DIR"
  status=0
  env "$@" sh "$here/provider.sh" create new > "$tmp/out" 2> "$tmp/err" || status=$?
  expect "create with $label" 1 code INVALID_ARGS
  [ -z "$(ls -A "$AD_SANDBOX_DIR")" ] || fail "create with $label left files behind"
  [ ! -s "$FAKE_LOG" ] || fail "create with $label ran a tool: $(cat "$FAKE_LOG")"
}
refuse 'no source' AD_UNUSED=1
refuse 'from and ipsw' AD_SET_FROM=base AD_SET_IPSW=latest
refuse 'from and user' AD_SET_FROM=base AD_SET_USER=alice
refuse 'an unknown key' AD_SET_ISPW=latest
refuse 'an empty value' AD_SET_IPSW=
refuse 'a missing image' AD_SET_IPSW=/nowhere/image.ipsw
refuse 'the retired user setting' AD_SET_IPSW=latest AD_SET_USER=alice

wait_for_install() {
  tries=0
  while [ ! -f "$tmp/sandboxes/$1/vm/install.done" ] && [ "$tries" -lt 50 ]; do
    grep -q . "$tmp/sandboxes/$1/vm/install.log" 2>/dev/null && return 0
    sleep 0.1
    tries=$((tries + 1))
  done
}

printf 'image\n' > "$tmp/my image.ipsw"
rm -rf "$tmp/sandboxes/new"
provider new AD_SET_IPSW='my image.ipsw' create new
expect 'create from a supplied image' 0 state working
wait_for_install new
grep -q "vmhost install $tmp/sandboxes/new/vm $tmp/my image.ipsw" "$FAKE_LOG" || fail 'a relative image path was not resolved against the caller directory'
[ -f "$tmp/sandboxes/new/vm/install.done" ] || fail "the install job did not finish: $(cat "$tmp/sandboxes/new/vm/install.log")"

FAKE_IMAGE='newest image this Mac can run'
image_sha=$(printf '%s' "$FAKE_IMAGE" | shasum -a 1 | awk '{ print $1 }')
entry() {
  printf '<key>Restore</key><dict><key>FirmwareSHA1</key><string>%s</string><key>FirmwareURL</key><string>https://example.test/%s.ipsw</string><key>ProductVersion</key><string>%s</string></dict>' "$1" "$2" "$2"
}
FAKE_CATALOG="$(entry "$image_sha" 14.6)$(entry 0000000000000000000000000000000000000000 26.1)"
export FAKE_IMAGE FAKE_CATALOG
rm -rf "$tmp/sandboxes/new"
: > "$FAKE_LOG"
provider new AD_SET_IPSW=latest create new
expect 'create from the latest image' 0 state working
wait_for_install new
grep -q 'https://example.test/14.6.ipsw' "$FAKE_LOG" || fail 'the download did not pick the newest image the host can run'
grep -q "vmhost install $tmp/sandboxes/new/vm $tmp/sandboxes/new/restore.ipsw" "$FAKE_LOG" || fail "the verified image was not installed: $(cat "$tmp/sandboxes/new/vm/install.log")"
[ ! -e "$tmp/sandboxes/new/restore.ipsw" ] || fail 'the downloaded image was kept after the install'

FAKE_CATALOG=$(entry 1111111111111111111111111111111111111111 14.6)
rm -rf "$tmp/sandboxes/new"
: > "$FAKE_LOG"
provider new AD_SET_IPSW=latest create new
wait_for_install new
grep -q 'did not match its published SHA-1' "$tmp/sandboxes/new/vm/install.log" || fail 'a corrupt download was not reported'
grep -q 'vmhost install' "$FAKE_LOG" && fail 'a corrupt download was installed'
provider new status new
expect 'status after a failed install' 0 state broken

FAKE_CATALOG=$(entry 2222222222222222222222222222222222222222 26.1)
rm -rf "$tmp/sandboxes/new"
: > "$FAKE_LOG"
provider new AD_SET_IPSW=latest create new
expect 'a catalog image newer than this Mac' 1 code ACTION_FAILED
field message | grep -q 'no image this Mac running macOS 14.0 can install' || fail 'the refusal did not name the host version'
[ ! -f "$tmp/sandboxes/new/vm/install.pid" ] || fail 'a doomed install job was started'

put base running
rm -rf "$tmp/sandboxes/new"
provider new AD_SET_FROM=base create new
expect 'cloning a running sandbox' 1 code ACTION_FAILED
provider new AD_SET_FROM=absent create new
expect 'cloning a sandbox that does not exist' 1 code SANDBOX_NOT_FOUND
put base stopped
rm -rf "$tmp/sandboxes/new"
provider new FAKE_CLONE=fails AD_SET_FROM=base create new
expect 'a clone the VM host refuses' 1 code ACTION_FAILED
field message | grep -q 'clonefile is not supported here' || fail 'the clone failure hid its reason'
rm -rf "$tmp/sandboxes/new"
provider new AD_SET_FROM=base create new
expect 'cloning a stopped, set-up sandbox' 0 state stopped
[ -f "$tmp/sandboxes/new/setup_done" ] || fail 'the clone has no setup record'
rm -rf "$tmp/sandboxes/old"
: > "$tmp/sandboxes/base/id_ed25519"
provider old AD_SET_FROM=base create old
[ "$status" = 0 ] && [ ! -e "$tmp/sandboxes/old/setup_done" ] || fail 'a clone of a sandbox still set up over SSH was recorded as set up'
[ -n "$(ls "$tmp/sandboxes/new/vm/share/builds" 2>/dev/null)" ] && [ -f "$tmp/sandboxes/new/vm/share/agent-desktop-guest.app/Contents/MacOS/agent-desktop-guest" ] || fail 'the clone has no guest agent or host build in its shared folder'
for leftover in id_ed25519 known_hosts guest_user; do
  [ ! -e "$tmp/sandboxes/new/$leftover" ] || fail "the clone holds the leftover $leftover"
done

printf 'provider tests passed\n'
