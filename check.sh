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
#
# Reported separately when the difference is merely uncommitted, because that is
# the normal state after changing the ABI and reads as a defect otherwise —
# which cost two pushes that went out with this script showing red.
cargo build --manifest-path "$manifest" -p sc-ffi >/dev/null 2>&1
header=crates/sc-ffi/include/schcompare.h
if git -C "$root" diff --quiet -- "$header"; then
    printf '  ok    generated header\n'
elif git -C "$root" diff --quiet HEAD -- "$header"; then
    printf '  FAIL  generated header does not match the source\n'
    failed=1
else
    printf '  ok    generated header (regenerated, not committed yet)\n'
fi

if [ -d "$root/shell/build" ]; then
    step "shell build"    cmake --build "$root/shell/build"
    step "shell tests"    ctest --test-dir "$root/shell/build" --output-on-failure
else
    echo "  skip  shell (no build directory; run cmake -S shell -B shell/build -G Ninja)"
fi

# Customer drawings, and anything rendered from them, must never be tracked.
if git -C "$root" ls-files | grep -qiE '\.(pdf|png|ppm|jpg)$'; then
    printf '  FAIL  no customer files tracked\n'
    git -C "$root" ls-files | grep -iE '\.(pdf|png|ppm|jpg)$' | sed 's/^/        /'
    failed=1
else
    printf '  ok    no customer files tracked\n'
fi

# Nor their names. The repository is public, and a customer's board codes are as
# much theirs as the drawings — so the tests address the sample sets by the role
# they play and read the filenames from `samples/sets.json`, which is ignored
# along with them.
#
# The words to look for are taken from whatever is in `samples/` right now
# rather than listed here, because a list of forbidden words in a public
# repository is the leak it was meant to prevent. Anyone without the drawings
# simply skips this.
if [ -d "$root/samples" ]; then
    words=$(ls "$root/samples" 2>/dev/null | grep -iE '\.pdf$' \
        | tr -c 'A-Za-z0-9' '\n' \
        | awk 'length($0) >= 4 && $0 !~ /^[0-9]+$/' \
        | grep -viE '^(pdf|rev[a-z]?[0-9]*)$' | sort -u)
    leaked=""
    for w in $words; do
        if git -C "$root" grep -qiF -- "$w" 2>/dev/null; then
            leaked="$leaked $w"
        fi
    done
    if [ -n "$leaked" ]; then
        printf '  FAIL  no customer names tracked\n'
        for w in $leaked; do
            printf '        %s appears in: %s\n' "$w" \
                "$(git -C "$root" grep -liF -- "$w" | tr '\n' ' ')"
        done
        failed=1
    else
        printf '  ok    no customer names tracked\n'
    fi
else
    printf '  skip  customer names (no samples/ on this machine)\n'
fi

if [ "$failed" -ne 0 ]; then
    echo "something failed"
    exit 1
fi
echo "all good"
