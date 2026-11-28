#!/bin/bash
# Puts an iOS build's IPA in Google Drive (gdrive:Barkfart), checked whole.
#
# IPAs downloaded on the iPad straight from GitHub Actions arrived cut short
# (2026-11-28: 35,615 bytes missing at the end, so LiveContainer and SideStore
# called them corrupt). This fetches the artifact here, checks it against
# GitHub's own size and sha256 and unzips it in full, uploads it, then checks
# the copy in Drive has the same md5.
#
#   tools/ipa-to-drive.sh            newest successful build on barkfart-upgrades
#   tools/ipa-to-drive.sh <run id>   that build
set -euo pipefail
REPO=CandyIsaChad/barkfart
DEST=gdrive:Barkfart
BRANCH=barkfart-upgrades

run=${1:-$(gh run list --repo "$REPO" --workflow app-ios.yml --branch "$BRANCH" --status success --limit 1 --json databaseId --jq '.[0].databaseId')}
[ -n "$run" ] || { echo "No successful iOS build found." >&2; exit 1; }
read -r aid name size digest < <(gh api "repos/$REPO/actions/runs/$run/artifacts" --jq '.artifacts[] | select(.name | endswith(".ipa")) | "\(.id) \(.name) \(.size_in_bytes) \(.digest)"' | head -1)
[ -n "${aid:-}" ] || { echo "Run $run has no .ipa artifact (expired?)." >&2; exit 1; }

dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
ipa="$dir/$name"
echo "Run $run: $name ($size bytes)"
gh api "repos/$REPO/actions/artifacts/$aid/zip" > "$ipa"

got=$(stat -c%s "$ipa")
[ "$got" = "$size" ] || { echo "Downloaded $got bytes, GitHub says $size." >&2; exit 1; }
if [ "${digest#sha256:}" != "$digest" ]; then
	sum=$(sha256sum "$ipa" | cut -d' ' -f1)
	[ "sha256:$sum" = "$digest" ] || { echo "sha256 differs from GitHub's ($sum)." >&2; exit 1; }
fi
unzip -tq "$ipa" >/dev/null || { echo "The IPA doesn't unzip cleanly." >&2; exit 1; }
unzip -l "$ipa" | grep -q 'Payload/Runner.app/Info.plist' || { echo "No Payload/Runner.app in the IPA." >&2; exit 1; }

rclone copy "$ipa" "$DEST" 2> >(grep -v "shared Google Drive client_id" >&2)
local_md5=$(md5sum "$ipa" | cut -d' ' -f1)
drive_md5=$(rclone md5sum "$DEST/$name" 2>/dev/null | cut -d' ' -f1)
[ "$local_md5" = "$drive_md5" ] || { echo "The copy in Drive differs (md5 $drive_md5, want $local_md5)." >&2; exit 1; }
echo "In Drive: Barkfart/$name ($size bytes, md5 $local_md5)"
