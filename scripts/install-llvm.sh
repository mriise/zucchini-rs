#!/bin/sh
# Installs the LLVM release CI builds zucchini-sys with, checked against a pinned SHA-256, and puts
# it first on the GitHub Actions PATH.
#
# Linux and Windows use LLVM's official release archives, extracting only the compiler. LLVM
# publishes no macOS archive for this release, so macOS builds use the runner's Xcode clang. Every
# prebuilt archive records the compiler that built it in BUILD_INFO.
#
# usage: scripts/install-llvm.sh <install dir>
set -eu

version=21.1.7
if [ $# -ne 1 ]; then
  echo "usage: scripts/install-llvm.sh <install dir>" >&2
  exit 1
fi

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)
    archive=LLVM-$version-Linux-X64.tar.xz
    sha256=621ab8424178ffc28db0facc5aefd3fc11f5dea339aac171b36fa0b8d4b368cb
    members="*/bin/clang */bin/clang++ */bin/clang-21 */lib/clang/*"
    ;;
  Linux-aarch64)
    archive=LLVM-$version-Linux-ARM64.tar.xz
    sha256=aa85ddc8ba95ac5f2febddb51a6891ee0e57ac058c6455395d5a4bfa5650d44b
    members="*/bin/clang */bin/clang++ */bin/clang-21 */lib/clang/*"
    ;;
  MINGW*-x86_64 | MSYS*-x86_64)
    archive=clang+llvm-$version-x86_64-pc-windows-msvc.tar.xz
    sha256=70a2b73f2f14f787557f90abf380e7170b54e97b893218999144de5284b4f8f8
    members="*/bin/clang.exe */bin/clang-cl.exe */lib/clang/*"
    ;;
  Darwin-*)
    echo "macOS builds use Xcode clang: $(clang --version | head -n 1)"
    exit 0
    ;;
  *)
    echo "no LLVM is pinned for $(uname -s) $(uname -m)" >&2
    exit 1
    ;;
esac

mkdir -p "$1"
# A POSIX path, so tar on Windows does not read the drive letter as a remote host.
dest=$(cd "$1" && pwd)

if [ ! -e "$dest/.installed-$version" ]; then
  echo "downloading $archive"
  curl -fL --retry 3 -o "$dest/$archive" \
    "https://github.com/llvm/llvm-project/releases/download/llvmorg-$version/$archive"
  (cd "$dest" && echo "$sha256  $archive" | sha256sum -c -)
  echo "extracting the compiler; xz reads the whole archive to find it"
  # shellcheck disable=SC2086 # the member patterns are meant to split
  (cd "$dest" && tar -xvJf "$archive" --strip-components=1 --wildcards $members)
  rm "$dest/$archive"
  touch "$dest/.installed-$version"
fi

bin=$(cd "$dest/bin" && (pwd -W 2>/dev/null || pwd))
echo "$bin" >> "${GITHUB_PATH:-/dev/null}"
"$dest/bin/clang" --version | head -n 1
