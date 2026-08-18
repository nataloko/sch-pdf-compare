#!/usr/bin/env bash
# Everything that has to pass, in one command, with a non-zero exit if any of it
# does not.
#
# This exists because grepping build output for the word "error" is not the same
# as checking whether the build succeeded, and twice it read as clean while the
# test binaries were not compiling at all.
set -uo pipefail

root=$(cd "$(dirname "$0")" && pwd)
manifest="$root/crates/Cargo.toml"
failed=0

step() {
    local name=$1
    shift
    if out=$("$@" 2>&1); then
        printf '  ok    %s\n' "$name"
    else
        printf '  FAIL  %s\n' "$name"
        printf '%s\n' "$out" | tail -25 | sed 's/^/        /'
        failed=1
    fi
}

echo "sch-pdf-compare"

step "cargo build"        cargo build --manifest-path "$manifest" --all-targets
step "cargo test"         cargo test --manifest-path "$manifest" --release
step "cargo fmt"          cargo fmt --manifest-path "$manifest" --all --check
# Warnings are failures here: the workspace is at zero and staying there is
# cheaper than getting back to it.
step "clippy"             cargo clippy --manifest-path "$manifest" --all-targets -- -D warnings

# The committed header must match what the source generates. A renumbered enum
# is an ABI break and this diff is the only place it shows up.
cargo build --manifest-path "$manifest" -p sc-ffi >/dev/null 2>&1
step "generated header"   git -C "$root" diff --exit-code -- crates/sc-ffi/include/schcompare.h

if [ -d "$root/shell/build" ]; then
    step "shell build"    cmake --build "$root/shell/build"
    step "shell tests"    ctest --test-dir "$root/shell/build" --output-on-failure
else
    echo "  skip  shell (no build directory; run cmake -S shell -B shell/build -G Ninja)"
fi

# Customer drawings, and anything rendered from them, must never be tracked.
if git -C "$root" ls-files | grep -qiE '\.(pdf|png|ppm|jpg)$'; then
    printf '  FAIL  no customer material tracked\n'
    git -C "$root" ls-files | grep -iE '\.(pdf|png|ppm|jpg)$' | sed 's/^/        /'
    failed=1
else
    printf '  ok    no customer material tracked\n'
fi

if [ "$failed" -ne 0 ]; then
    echo "something failed"
    exit 1
fi
echo "all good"
