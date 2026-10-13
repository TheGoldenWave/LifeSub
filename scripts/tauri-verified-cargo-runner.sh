#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec "$SCRIPT_DIR/with-sherpa-runtime.sh" "${LIFESUB_CARGO_BINARY:-cargo}" "$@"
