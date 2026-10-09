#!/bin/sh
# Exercise the real shared-library ABI with both C and C++ consumers.
set -eu
cd "$(dirname "$0")/../.."
python3 zpl-c/generate-config.py --check
cargo build --locked -p zpl-c
zpl_target=$(cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
zpl_tmp=$(mktemp -d)
trap 'rm -rf "$zpl_tmp"' EXIT HUP INT TERM
case $(uname -s) in
    Linux|Darwin) ;;
    *) echo 'run-c.sh currently supports Linux and macOS' >&2; exit 1 ;;
esac
"${CC:-cc}" -std=c11 -Wall -Wextra -Werror -pedantic -Izpl-c/include \
    zpl-c/tests/smoke.c -L"$zpl_target/debug" -lzpl_c \
    -Wl,-rpath,"$zpl_target/debug" -o "$zpl_tmp/smoke"
"$zpl_tmp/smoke"
"${CXX:-c++}" -x c++ -std=c++11 -Wall -Wextra -Werror -pedantic -Izpl-c/include \
    zpl-c/tests/smoke.c -L"$zpl_target/debug" -lzpl_c \
    -Wl,-rpath,"$zpl_target/debug" -o "$zpl_tmp/smoke-cpp"
"$zpl_tmp/smoke-cpp"
"${CC:-cc}" -std=c11 -Wall -Wextra -Werror -pedantic -Izpl-c/include \
    zpl-c/examples/render.c -L"$zpl_target/debug" -lzpl_c \
    -Wl,-rpath,"$zpl_target/debug" -o "$zpl_tmp/render"
(cd "$zpl_tmp" && ./render && test -s label.png)
