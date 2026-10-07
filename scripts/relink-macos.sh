#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 1 ]]
then
    echo "usage: $0 <staging directory bundle.macOS.frameworks lists>" >&2
    exit 2
fi

STAGING_DIRECTORY="$1"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
EXECUTABLE="${ROOT}/gui/src-tauri/target/release/kdmcourier-gui"
FRAMEWORKS_RPATH="@executable_path/../Frameworks"
SYSTEM_REFERENCE='^(/usr/lib/|/System/)'

# the staged libraries have @rpath install names
if ! otool -l "${EXECUTABLE}" | grep -qF "path ${FRAMEWORKS_RPATH} ("
then
    install_name_tool -add_rpath "${FRAMEWORKS_RPATH}" "${EXECUTABLE}"
fi

references="$(otool -L "${EXECUTABLE}" | awk -v system_reference="${SYSTEM_REFERENCE}" 'NR > 1 && $1 !~ system_reference {print $1}')"
for reference in ${references}
do
    name="$(basename "${reference}")"
    if [[ ! -f "${STAGING_DIRECTORY}/${name}" ]]
    then
        echo "relink-macos: ${EXECUTABLE} loads ${reference}, which ${STAGING_DIRECTORY} does not carry" >&2
        exit 1
    fi
    if [[ "${reference}" != "@rpath/${name}" ]]
    then
        install_name_tool -change "${reference}" "@rpath/${name}" "${EXECUTABLE}"
    fi
done

# every install_name_tool edit breaks the signature arm64 requires
codesign --force --sign - "${EXECUTABLE}"
otool -L "${EXECUTABLE}"
