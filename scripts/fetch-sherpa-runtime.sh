#!/usr/bin/env bash
#
# fetch-sherpa-runtime.sh
#
# Downloads and verifies the sherpa-onnx v1.13.5 macOS ARM64 static library
# archive. The archive is written outside the repository tree to a cache
# directory and its path is printed to stdout so it can be consumed by
# `SHERPA_ONNX_ARCHIVE_DIR`.
#
# Usage:
#   SHERPA_ONNX_ARCHIVE_DIR="$(scripts/fetch-sherpa-runtime.sh)"
#   export SHERPA_ONNX_ARCHIVE_DIR
#
# The archive is downloaded only once; subsequent invocations verify the
# cached copy and re-download only if the hash mismatches.

set -euo pipefail

ARCHIVE_NAME="sherpa-onnx-v1.13.5-osx-arm64-static-lib.tar.bz2"
ARCHIVE_URL="https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.5/${ARCHIVE_NAME}"
EXPECTED_SIZE=19862746
EXPECTED_SHA256="339c8fc19bb4b26e118c80792bbc4546eb263040fac36ef0cc027ec29c756b44"

CACHE_DIR="${HOME}/.cache/lifesub/sherpa-onnx"
mkdir -p "${CACHE_DIR}"

ARCHIVE_PATH="${CACHE_DIR}/${ARCHIVE_NAME}"

# Download if the archive is not present.
if [[ ! -f "${ARCHIVE_PATH}" ]]; then
    echo "==> Downloading ${ARCHIVE_URL}" >&2
    curl -fSL --progress-bar -o "${ARCHIVE_PATH}.tmp" "${ARCHIVE_URL}"
    mv "${ARCHIVE_PATH}.tmp" "${ARCHIVE_PATH}"
fi

# Verify size.
actual_size=$(wc -c < "${ARCHIVE_PATH}" | tr -d ' ')
if [[ "${actual_size}" -ne "${EXPECTED_SIZE}" ]]; then
    echo "ERROR: size mismatch: expected ${EXPECTED_SIZE}, got ${actual_size}" >&2
    echo "       removing cached archive; re-run to re-download." >&2
    rm -f "${ARCHIVE_PATH}"
    exit 1
fi

# Verify SHA-256.
if command -v shasum >/dev/null 2>&1; then
    actual_sha256=$(shasum -a 256 "${ARCHIVE_PATH}" | awk '{print $1}')
else
    actual_sha256=$(sha256sum "${ARCHIVE_PATH}" | awk '{print $1}')
fi

if [[ "${actual_sha256}" != "${EXPECTED_SHA256}" ]]; then
    echo "ERROR: SHA-256 mismatch: expected ${EXPECTED_SHA256}, got ${actual_sha256}" >&2
    echo "       removing cached archive; re-run to re-download." >&2
    rm -f "${ARCHIVE_PATH}"
    exit 1
fi

echo "==> Verified ${ARCHIVE_NAME} (${actual_size} bytes, SHA-256 ok)" >&2

# Print the cache directory so the caller can set SHERPA_ONNX_ARCHIVE_DIR.
echo "${CACHE_DIR}"