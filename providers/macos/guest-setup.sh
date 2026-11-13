#!/bin/sh
set -eu

here=$(cd "$(dirname "$0")" && pwd)
name=agent-desktop-guest
label=com.agent-desktop.guest
source_app=$here/$name.app
app=$HOME/Applications/$name.app
agent=$app/Contents/MacOS/$name
plist=$HOME/Library/LaunchAgents/$label.plist
domain=gui/$(id -u)
lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister

if [ ! -f "$source_app/Contents/MacOS/$name" ]; then
  echo "$name.app is not in this folder, so run agent-desktop sandbox start on the host again." >&2
  exit 1
fi

loaded() {
  launchctl print "$domain/$label" >/dev/null 2>&1
}

mkdir -p "$HOME/Applications" "$HOME/Library/LaunchAgents" "$HOME/Library/Logs"
if cmp -s "$source_app/Contents/MacOS/$name" "$agent"; then
  echo "The guest agent at $app is already current."
else
  if loaded; then
    launchctl bootout "$domain/$label" >/dev/null 2>&1 || true
    tries=0
    while loaded && [ "$tries" -lt 50 ]; do
      sleep 0.1
      tries=$((tries + 1))
    done
  fi
  rm -rf "$app"
  cp -R "$source_app" "$app"
  "$lsregister" -f "$app" >/dev/null 2>&1 || true
  tccutil reset Accessibility "$label" >/dev/null 2>&1 || true
  tccutil reset ScreenCapture "$label" >/dev/null 2>&1 || true
  echo "Installed the guest agent at $app."
fi

cat > "$plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>$label</string>
  <key>ProgramArguments</key>
  <array>
    <string>$agent</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>LimitLoadToSessionType</key>
  <string>Aqua</string>
  <key>StandardErrorPath</key>
  <string>$HOME/Library/Logs/$name.log</string>
</dict>
</plist>
PLIST

if loaded; then
  start="kickstart -k $domain/$label"
else
  start="bootstrap $domain $plist"
fi
if ! launchctl $start; then
  echo "The guest agent could not be started. Log in to the desktop, open Terminal there, and run this script again." >&2
  exit 1
fi

echo "One step is left: when macOS asks, open System Settings > Privacy & Security > Accessibility and switch on $name."
echo "For screenshots, switch it on under Screen & System Audio Recording too."
if ! defaults read /Library/Preferences/com.apple.loginwindow autoLoginUser >/dev/null 2>&1; then
  echo "Automatic login is off. Turn it on in System Settings > Users & Groups, or the sandbox cannot reach its desktop when it starts without a window."
fi
