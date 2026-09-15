#!/bin/sh
# Pins one release's prebuilt libraries in zucchini-sys/prebuilt.txt.
#
# Downloads the release archives, checks them against its SHA256SUMS, and verifies with
# `gh attestation verify` that each one was built by this repository's prebuilt workflow, from the
# release tag, on a GitHub-hosted runner. Only then are the sums written.
#
# usage: scripts/pin-prebuilt.sh <tag>
set -eu

if [ $# -ne 1 ]; then
  echo "usage: scripts/pin-prebuilt.sh <tag>" >&2
  exit 1
fi
tag=$1
cd "$(git rev-parse --show-toplevel)"
repo=$(gh repo view --json nameWithOwner --jq .nameWithOwner)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

gh release download "$tag" --repo "$repo" --dir "$tmp" --pattern 'zucchini-*.tar.gz' --pattern SHA256SUMS
(cd "$tmp" && sha256sum -c SHA256SUMS)
for archive in "$tmp"/zucchini-*.tar.gz; do
  gh attestation verify "$archive" \
    --repo "$repo" \
    --signer-workflow "$repo/.github/workflows/prebuilt.yml" \
    --source-ref "refs/tags/$tag" \
    --deny-self-hosted-runners
done

pins=zucchini-sys/prebuilt.txt
{
  grep '^#' "$pins" || true
  echo "release https://github.com/$repo/releases/download/$tag"
  cat "$tmp/SHA256SUMS"
} > "$pins.new"
mv "$pins.new" "$pins"
echo "pinned $(grep -c . "$tmp/SHA256SUMS") archives from $tag in $pins"
