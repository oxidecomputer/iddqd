# shellcheck shell=bash
# shellcheck disable=SC2034

# A pinned Soteria release.
#
# This is sourced by soteria-install and soteria-rust. Bump the tag and both
# hashes together (hashes come from the release's SHA256SUMS.txt).

SOTERIA_PIN_REPO="giltho/soteria"
SOTERIA_PIN_TAG="v0.2.3"
SOTERIA_PIN_SHA256_LINUX_X86_64="ada8a07c8711573719d19c40f0b4ffee9d326aa382477bfc6b537a7d76ce6e09"
SOTERIA_PIN_SHA256_MACOS_ARM64="8a96ff226ee1c415581d2c0cf36cb35ea244dea29a0045131f75423bc96296ff"

SOTERIA_PIN_INSTALL_DIR="${SOTERIA_HOME:-$HOME/.soteria}/$SOTERIA_PIN_TAG"
SOTERIA_PIN_INSTALL_MARKER="$SOTERIA_PIN_INSTALL_DIR/.install-complete"

soteria_export_env() {
    local bin="$SOTERIA_PIN_INSTALL_DIR/bin"
    export SOTERIA_RUST_PLUGINS="$SOTERIA_PIN_INSTALL_DIR/plugins"
    export SOTERIA_OBOL_PATH="$bin/obol"
    export SOTERIA_CHARON_PATH="$bin/charon"
    export SOTERIA_Z3_PATH="$bin/z3"
    export PATH="$bin:$PATH"
}
