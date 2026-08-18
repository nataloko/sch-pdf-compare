#!/usr/bin/env bash
# The whole portable build, in one container: dependencies, Qt, the
# application, the image.
#
#   ./packaging/appimage/portable.sh
#
# The container is thrown away each time, so the distribution packages are
# installed on every run — about a minute. Qt is not: it lands in
# `packaging/appimage/toolchain/`, which is ignored by git and reused, so only
# the first run pays for it.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
exec "$here/in-container.sh" bash -c '
    set -euo pipefail
    ./packaging/appimage/install-build-deps.sh
    ./packaging/appimage/build-qt.sh
    ./packaging/appimage/build.sh
'
