#!/bin/sh
# Checks out the Chromium files zucchini-sys builds from, and nothing else.
#
# zucchini-sys/chromium is a submodule of chromium/src pinned to one commit. A plain submodule
# update would clone all of Chromium, so .gitmodules marks it `update = none` and this script
# fetches it instead: the pinned commit at depth 1 without file contents, then a sparse checkout
# of the paths in zucchini-sys/chromium-files.txt, which downloads only those files.
#
# usage: scripts/chromium.sh         check out the pinned commit
#        scripts/chromium.sh <ref>   pin <ref> (a commit, tag or branch) and check it out
set -eu

cd "$(git rev-parse --show-toplevel)"
dir=zucchini-sys/chromium
url=$(git config -f .gitmodules submodule.chromium.url)

if [ $# -gt 0 ]; then
  want=$1
else
  want=$(git ls-files -s -- "$dir" | awk '$1 == "160000" { print $2 }')
  if [ -z "$want" ]; then
    echo "no Chromium commit is pinned at $dir; pass one: scripts/chromium.sh <ref>" >&2
    exit 1
  fi
fi

mkdir -p "$dir"
if [ ! -e "$dir/.git" ]; then
  git -C "$dir" init -q
fi
git -C "$dir" config remote.origin.url "$url"
git -C "$dir" config remote.origin.fetch "+refs/heads/*:refs/remotes/origin/*"
git -C "$dir" config remote.origin.promisor true
git -C "$dir" config remote.origin.partialclonefilter blob:none
git -C "$dir" config core.sparseCheckout true
git -C "$dir" config core.sparseCheckoutCone false

# The patterns go through a file so no shell rewrites their leading slashes.
sparse=$(git -C "$dir" rev-parse --path-format=absolute --git-path info/sparse-checkout)
mkdir -p "$(dirname "$sparse")"
patterns_changed=false
if ! cmp -s zucchini-sys/chromium-files.txt "$sparse"; then
  patterns_changed=true
  cp zucchini-sys/chromium-files.txt "$sparse"
fi

if [ "$(git -C "$dir" rev-parse -q --verify HEAD 2>/dev/null || true)" = "$want" ]; then
  rev=$want
  # Reapplying walks every path in Chromium's index, so only do it for new patterns.
  if [ "$patterns_changed" = true ]; then
    git -C "$dir" sparse-checkout reapply
  fi
else
  git -C "$dir" fetch -q --depth 1 --filter=blob:none origin "$want"
  rev=$(git -C "$dir" rev-parse FETCH_HEAD)
  git -C "$dir" -c advice.detachedHead=false checkout -q --detach "$rev"
fi

if [ $# -gt 0 ]; then
  git update-index --add --cacheinfo "160000,$rev,$dir"
fi
# Registers the submodule in .git/config; with `update = none` this clones nothing.
git submodule init -q -- "$dir"
if [ -d "$dir/.git" ]; then
  git submodule absorbgitdirs -- "$dir"
fi

files=$(git -C "$dir" ls-files -t | grep -c '^H' || true)
echo "chromium $rev: $files files checked out in $dir"
