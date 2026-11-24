#!/bin/bash
# Installs the Linux release build for this user, to use like any app:
# - a copy of the build in ~/.local/opt/barkfart (rebuilding never touches
#   a copy that's running),
# - a menu entry and its icon, and `barkfart` on the PATH (~/.local/bin),
# - the sample scripts (docs/barkfart/plugins) in Barkfart Plugins, if
#   they aren't there yet.
# Run it after `flutter build linux --release`, with Barkfart closed.
set -euo pipefail
APP="$(cd "$(dirname "$0")/.." && pwd)"
REPO="$(cd "$APP/.." && pwd)"
BUNDLE="$APP/build/linux/x64/release/bundle"
DEST="$HOME/.local/opt/barkfart"
ID="app.barkfart.barkfart"

if [ ! -x "$BUNDLE/barkfart" ]; then
	echo "No build yet: run flutter build linux --release in app/ first." >&2
	exit 1
fi
if pgrep -f "^$DEST/barkfart" >/dev/null 2>&1; then
	echo "Barkfart is open. Close it, then run this again." >&2
	exit 2
fi

mkdir -p "$DEST" "$HOME/.local/share/applications" "$HOME/.local/bin" \
	"$HOME/.local/share/icons/hicolor/256x256/apps" "$HOME/.local/share/icons/hicolor/512x512/apps"
rsync -a --delete "$BUNDLE/" "$DEST/"
cp "$APP/linux/icon/barkfart-256.png" "$HOME/.local/share/icons/hicolor/256x256/apps/$ID.png"
cp "$APP/linux/icon/barkfart-512.png" "$HOME/.local/share/icons/hicolor/512x512/apps/$ID.png"

cat >"$HOME/.local/share/applications/$ID.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Barkfart
GenericName=Stick figure animation
Comment=Pose and animate stick figures
Exec=$DEST/barkfart
Icon=$ID
Terminal=false
Categories=Graphics;2DGraphics;
StartupWMClass=$ID
EOF
ln -sf "$DEST/barkfart" "$HOME/.local/bin/barkfart"
update-desktop-database "$HOME/.local/share/applications" >/dev/null 2>&1 || true
gtk-update-icon-cache -q -t "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true

# the sample scripts, so the Workshop and + › Script have something in them
DOCS="$(xdg-user-dir DOCUMENTS 2>/dev/null || echo "$HOME/Documents")"
PLUGINS="$DOCS/Barkfart Plugins"
mkdir -p "$PLUGINS"
for f in "$REPO"/docs/barkfart/plugins/*.luau; do
	[ -e "$PLUGINS/$(basename "$f")" ] || cp "$f" "$PLUGINS/"
done

echo "Installed: Barkfart is in the app menu (and \`barkfart\` in a terminal)."
