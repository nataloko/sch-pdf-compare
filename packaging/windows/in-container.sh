#!/usr/bin/env bash
# Run a command inside the pinned Fedora image, with this repository mounted.
#
#   ./packaging/windows/in-container.sh ./packaging/windows/build.sh
#   ./packaging/windows/in-container.sh bash        # a shell in there
#
# Fedora because of Qt: cross-building the shell needs a MinGW Qt 6 whose moc,
# rcc and uic run on the build machine, and Fedora is the distribution that
# packages one. Everything else here — the MinGW toolchain, NSIS's compiler,
# Wine — is available anywhere; Qt is what settles it.
#
# The same shape as `../appimage/in-container.sh`, and for the same reasons.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
. "$here/toolchain.env"

# podman on this machine may only exist outside the development container, so
# fall back to running it on the host. The bind mount below is a host path
# either way, which is why the two must agree.
if command -v podman >/dev/null; then
    engine=(podman)
elif command -v docker >/dev/null; then
    engine=(docker)
elif command -v distrobox-host-exec >/dev/null; then
    engine=(distrobox-host-exec podman)
    distrobox-host-exec test -d "$root" || {
        echo "container: the host cannot see $root, so it cannot be mounted" >&2
        exit 2
    }
else
    echo "container: no podman and no docker" >&2
    exit 2
fi

# Cargo's state under the repository rather than in the caller's home: the
# container runs as root, and root writing into a real ~/.cargo is how a
# development machine ends up with files it cannot delete.
home=$here/toolchain/home
mkdir -p "$home"

exec "${engine[@]}" run --rm --security-opt label=disable \
    --cpus "${BUILD_CPUS:-6}" \
    --volume "$root:/repo:rw" \
    --workdir /repo \
    --env "HOME=/repo/packaging/windows/toolchain/home" \
    --env "CARGO_HOME=/repo/packaging/windows/toolchain/home/.cargo" \
    --env "CMAKE_BUILD_PARALLEL_LEVEL=${BUILD_JOBS:-6}" \
    --env "CARGO_BUILD_JOBS=${BUILD_JOBS:-6}" \
    "$FEDORA_IMAGE" \
    bash -c 'exec "$@"' _ "$@"
