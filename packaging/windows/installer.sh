#!/usr/bin/env bash
# The whole Windows installer, in one container: dependencies, the cross-build,
# the staging, NSIS.
#
#   ./packaging/windows/installer.sh              build it
#   ./packaging/windows/installer.sh --verify     ...and then install it under
#                                                 Wine, start it, uninstall it
#
# The container is thrown away each time, so the distribution packages are
# installed on every run — about a minute and a half. Everything else lands in
# `packaging/windows/build/`, which is ignored by git and reused.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
# Wine is only fetched when it is going to be used: it is several hundred
# megabytes, and it has no part in building the artefact.
verify=""
[ "${1:-}" = "--verify" ] && verify="
    dnf -y install --setopt=install_weak_deps=False wine >/dev/null
    ./packaging/windows/verify.sh"

exec "$here/in-container.sh" bash -c "
    set -euo pipefail
    ./packaging/windows/install-build-deps.sh >/dev/null
    ./packaging/windows/build.sh
    $verify
"
