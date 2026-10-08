#!/bin/sh
# `shunt` launcher: runs target/release/shunt of the checkout this script lives in.
# `shunt tui` (alias `top`) first rebuilds the binary (with the `tui` feature) when
# a source file is newer than it, so you never rebuild by hand. Every other
# command, including `shunt run` under launchd, starts the binary as it is and
# never waits for a build. A failed build warns and runs the old binary.
# Link it:  ln -sf <checkout>/packaging/shunt-launcher.sh ~/.local/bin/shunt
# Resolves the symlink. POSIX sh; needs cargo only for the rebuild.
self=$0
while [ -L "$self" ]; do
  dir=$(cd "$(dirname "$self")" && pwd)
  self=$(readlink "$self")
  case $self in /*) ;; *) self=$dir/$self ;; esac
done
root=$(cd "$(dirname "$self")/.." && pwd -P)
exe=$root/target/release/shunt

stale() {
  [ -x "$exe" ] || return 0
  [ -n "$(find "$root/src" "$root/blueprints" "$root/Cargo.toml" "$root/Cargo.lock" -type f -newer "$exe" -print -quit 2>/dev/null)" ]
}

case ${1-} in
  tui | top)
    if stale; then
      echo "shunt: building the TUI (sources changed)…" >&2
      PATH="$HOME/.cargo/bin:$PATH" cargo build --locked --release --features tui --quiet \
        --manifest-path "$root/Cargo.toml" --bin shunt \
        || echo "shunt: build failed; using the previous binary" >&2
    fi
    ;;
esac
[ -x "$exe" ] || { echo "shunt: $exe is missing and could not be built" >&2; exit 1; }
exec "$exe" "$@"
