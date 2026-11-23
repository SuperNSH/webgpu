#!/bin/sh
# Fake pkg-config for `cargo check` only (nothing links): every package exists at a high version.
for a in "$@"; do case "$a" in --modversion) echo 99.0; exit 0;; esac; done
for a in "$@"; do case "$a" in --libs*|--cflags*) echo ""; exit 0;; esac; done
exit 0
