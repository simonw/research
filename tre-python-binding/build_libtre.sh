#!/bin/sh
# Build libtre.so from a fresh clone of laurikari/tre and copy the
# resulting shared library next to this script.
#
# Requirements: autoconf, automake, libtool, autopoint (gettext), gcc, make.
#   On Debian/Ubuntu:
#     sudo apt-get install -y autoconf automake libtool gettext autopoint gcc make

set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
SRC="${TRE_SRC:-/tmp/tre}"

if [ ! -d "$SRC" ]; then
    echo "cloning laurikari/tre into $SRC"
    git clone https://github.com/laurikari/tre.git "$SRC"
fi

cd "$SRC"

# Regenerate the autotools build system.
./utils/autogen.sh

# Build a shared lib (we don't need the static one or agrep).
./configure --enable-shared --disable-static
make -j

cp lib/.libs/libtre.so.5.0.0 "$HERE/libtre.so"
echo "wrote $HERE/libtre.so"
