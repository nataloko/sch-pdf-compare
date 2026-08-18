#!/usr/bin/env bash
# Run a command inside the pinned manylinux_2_28 image, with this repository
# mounted and a bounded share of the machine.
#
#   ./packaging/appimage/in-container.sh ./packaging/appimage/build-qt.sh
#   ./packaging/appimage/in-container.sh bash        # a shell in there
#
# The image is what gives the AppImage its glibc floor, so everything that ends
# up inside the image — Qt, the core, the shell — is compiled in here and
# nothing is compiled outside it.
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

# Cargo and rustup state under the repository rather than in the caller's home:
# the container runs as root, and root writing into a real ~/.cargo is how a
# development machine ends up with files it cannot delete.
home=$here/toolchain/home
mkdir -p "$home"

# --cpus, because Qt Base is the longest build here by a wide margin and a
# container with all sixteen cores makes the desktop unusable for the duration.
# --security-opt label=disable is podman on a SELinux host; docker ignores it.
exec "${engine[@]}" run --rm --security-opt label=disable \
    --cpus "${BUILD_CPUS:-6}" \
    --volume "$root:/repo:rw" \
    --workdir /repo \
    --env "HOME=/repo/packaging/appimage/toolchain/home" \
    --env "CARGO_HOME=/repo/packaging/appimage/toolchain/home/.cargo" \
    --env "RUSTUP_HOME=/repo/packaging/appimage/toolchain/home/.rustup" \
    --env "CMAKE_BUILD_PARALLEL_LEVEL=${BUILD_JOBS:-6}" \
    --env "CARGO_BUILD_JOBS=${BUILD_JOBS:-6}" \
    "$MANYLINUX_IMAGE" \
    bash -lc 'export PATH="$CARGO_HOME/bin:$PATH"; exec "$@"' _ "$@"
