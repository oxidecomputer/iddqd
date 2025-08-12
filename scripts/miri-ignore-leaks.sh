#!/usr/bin/env bash

# A nextest wrapper script that instructs Miri to ignore leaks.

MIRIFLAGS="${MIRIFLAGS} -Zmiri-ignore-leaks" "$@"
