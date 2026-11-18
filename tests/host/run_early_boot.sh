#!/bin/sh
# Linux host only. No server, device, Android SDK, or key material is needed.
set -eu
cd "$(dirname "$0")/../.."
build=$(mktemp -d "${TMPDIR:?set TMPDIR to a scratch directory}/early-boot.XXXXXX")
trap 'rm -rf "$build"' EXIT
cxx=${CXX:-c++}
set -- -std=c++20 -O2 -DNDEBUG -Wall -Wextra -Werror -fno-exceptions -fno-rtti -pthread "$@"
"$cxx" "$@" tests/host/early_boot_test.cpp -o "$build/early_boot_test"
# Discard server-only sections; the test uses the real config parser, commit result and wire framing.
"$cxx" "$@" -ffunction-sections -fdata-sections -Wl,--gc-sections -Itests/host/include \
  -Iinjector/include -Irust/teesim-km/include tests/host/config_ack_test.cpp common/json.cpp \
  -o "$build/config_ack_test"
timeout 20s "$build/early_boot_test"
timeout 20s "$build/config_ack_test"
