#!/bin/sh
# Installs the almena CLI from its GitHub release. Served at
# https://almena.id/install.sh:
#
#   curl -fsSL https://almena.id/install.sh | sh
#
# Only the platforms with a release binary are installed: macOS on Apple
# silicon and Linux on x86_64 (Ubuntu 24.04 or later). The archive is checked
# against the release's SHA256SUMS before anything is copied.
#
# Settings, through the environment (after `| `, e.g. `| ALMENA_INSTALL_DIR=~/bin sh`):
#
#   ALMENA_INSTALL_VERSION   the version to install (e.g. 2026.10.1); empty: the latest
#   ALMENA_INSTALL_DIR       where the binary goes; empty: /usr/local/bin, through
#                            sudo when it is not writable
#
# Everything runs inside main, called on the last line, so a download cut off
# halfway through runs nothing.

set -eu

REPO="almena-id/almena"

say() {
    printf 'almena: %s\n' "$*" >&2
}

fail() {
    printf 'almena: error: %s\n' "$*" >&2
    exit 1
}

need() {
    command -v "$1" > /dev/null 2>&1 || fail "$1 is needed and was not found"
}

# The release target of this machine, or a refusal naming what is supported.
target() {
    os=$(uname -s)
    arch=$(uname -m)
    case "$os" in
        Darwin)
            # A shell running under Rosetta reports x86_64 on Apple silicon.
            if [ "$arch" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2> /dev/null || echo 0)" = 1 ]; then
                arch=arm64
            fi
            case "$arch" in
                arm64 | aarch64) echo aarch64-apple-darwin ;;
                *) fail "macOS on $arch has no binary; only Apple silicon (arm64) is supported" ;;
            esac
            ;;
        Linux)
            case "$arch" in
                x86_64 | amd64) echo x86_64-unknown-linux-gnu ;;
                *) fail "Linux on $arch has no binary; only x86_64 is supported" ;;
            esac
            ;;
        *)
            fail "$os has no install script; on Windows use winget or Scoop (see https://github.com/$REPO)"
            ;;
    esac
}

# The latest release's version, from where /releases/latest redirects to
# (no API call, so no rate limit).
latest_version() {
    url=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest") ||
        fail "could not reach GitHub to find the latest release"
    version=${url##*/tag/v}
    case "$version" in
        "" | */*) fail "$REPO has no published release yet" ;;
    esac
    echo "$version"
}

sha256() {
    if command -v sha256sum > /dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    elif command -v shasum > /dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d ' ' -f 1
    else
        fail "sha256sum or shasum is needed to check the download"
    fi
}

main() {
    need curl
    need tar
    need uname

    target=$(target)
    version=${ALMENA_INSTALL_VERSION:-}
    version=${version#v}
    [ -n "$version" ] || version=$(latest_version)

    name="almena-$version-$target"
    base="https://github.com/$REPO/releases/download/v$version"

    tmp=$(mktemp -d 2> /dev/null || mktemp -d -t almena)
    trap 'rm -rf "$tmp"' EXIT INT TERM

    say "downloading almena $version for $target"
    curl -fsSL --proto '=https' --tlsv1.2 -o "$tmp/$name.tar.gz" "$base/$name.tar.gz" ||
        fail "could not download $base/$name.tar.gz (is $version a published version?)"
    curl -fsSL --proto '=https' --tlsv1.2 -o "$tmp/SHA256SUMS" "$base/SHA256SUMS" ||
        fail "could not download $base/SHA256SUMS"

    expected=$(awk -v file="$name.tar.gz" '$2 == file || $2 == "*" file { print $1 }' "$tmp/SHA256SUMS")
    [ -n "$expected" ] || fail "SHA256SUMS has no line for $name.tar.gz"
    actual=$(sha256 "$tmp/$name.tar.gz")
    [ "$expected" = "$actual" ] || fail "checksum mismatch for $name.tar.gz (expected $expected, got $actual)"

    tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
    [ -f "$tmp/$name/almena" ] || fail "the archive has no almena binary"

    dir=${ALMENA_INSTALL_DIR:-/usr/local/bin}
    sudo=""
    if ! mkdir -p "$dir" 2> /dev/null || [ ! -w "$dir" ]; then
        if [ -n "${ALMENA_INSTALL_DIR:-}" ]; then
            fail "$dir is not writable"
        fi
        command -v sudo > /dev/null 2>&1 ||
            fail "$dir is not writable and sudo was not found; set ALMENA_INSTALL_DIR to a directory of yours"
        say "$dir is not writable; asking sudo"
        sudo="sudo"
        $sudo mkdir -p "$dir"
    fi
    $sudo install -m 755 "$tmp/$name/almena" "$dir/almena"

    say "installed $("$dir/almena" --version) at $dir/almena"
    case ":$PATH:" in
        *":$dir:"*) ;;
        *) say "$dir is not on your PATH; add it to run almena by name" ;;
    esac
    say "next: almena auth login"
}

main "$@"
