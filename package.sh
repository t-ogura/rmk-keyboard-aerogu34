#!/usr/bin/env bash
# Build both halves, convert them to UF2 and refuse to ship an image that
# reaches into the storage sectors ([storage] start_addr in keyboard.toml).
# Run from anywhere; artefacts land in firmware/.
#
#   ./package.sh                 Vial build (default)      -> firmware/aerogu34_{right,left}.uf2
#   ./package.sh --host rynk     Rynk build                -> firmware/aerogu34_{right,left}_rynk.uf2
#   ./package.sh --host both     both
#   ./package.sh --log           right half with RMK's log on a USB serial port -> .../log/
#   ./package.sh --dev           clear_layout = true: keyboard.toml keymap edits reach the
#                                board without an rmk rebuild (Vial edits then do not persist)
#
# Vial and Rynk are mutually exclusive rmk features, and rmk-macro insists that
# `[host]` in keyboard.toml agrees with the feature. The Rynk build therefore
# compiles with `--no-default-features --features rynk` against a temporary
# copy of keyboard.toml whose `[host]` block is flipped, in its own target
# directory so switching does not rebuild the other flavour from scratch.
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
# nrf-sdc-sys / nrf-mpsl-sys run bindgen, which needs libclang. A system
# install (apt install libclang-dev) is found on its own; a per-user unpack
# (docs/DESIGN.md) is picked up here.
if [ -z "${LIBCLANG_PATH:-}" ] && [ -d "$HOME/.local/opt/libclang/usr/lib/llvm-18/lib" ]; then
  export LIBCLANG_PATH="$HOME/.local/opt/libclang/usr/lib/llvm-18/lib"
  export LD_LIBRARY_PATH="$LIBCLANG_PATH:${LD_LIBRARY_PATH:-}"
fi
HOST=vial
LOG=0
DEV=0
while [ $# -gt 0 ]; do
  case "$1" in
    --host) HOST="$2"; shift 2 ;;
    --log) LOG=1; shift ;;
    --dev) DEV=1; shift ;;
    *) echo "usage: $0 [--host vial|rynk|both] [--log] [--dev]" >&2; exit 2 ;;
  esac
done
STORAGE=$(python3 -c "import tomllib;print(tomllib.load(open('keyboard.toml','rb'))['storage']['start_addr'])")

build_one() { # $1 = vial | rynk
  local flavour="$1" out tmp toml suffix features_right features_left
  tmp=$(mktemp -d)
  toml="$tmp/keyboard.toml"
  # A copy of keyboard.toml with `[host]` flipped for Rynk. `[host]` ends at
  # the next table header.
  python3 - keyboard.toml "$toml" "$flavour" "$DEV" <<'PY'
import re, sys
src, dst, flavour, dev = sys.argv[1:]
s = open(src, encoding="utf-8").read()
if flavour == "rynk":
    s, n = re.subn(r"(?ms)^\[host\]\n.*?(?=^\[)", "[host]\nvial_enabled = false\nrynk_enabled = true\ninsecure = true\n\n", s)
    assert n == 1, "expected exactly one [host] block"
if dev == "1":
    s, n = re.subn(r"(?m)^clear_layout = false$", "clear_layout = true", s)
    assert n == 1, "expected exactly one clear_layout"
open(dst, "w", encoding="utf-8").write(s)
PY
  export KEYBOARD_TOML_PATH="$toml"
  # Both flavours land in firmware/ side by side, the Rynk pair with a
  # `_rynk` suffix, so a Release can attach the whole directory.
  local name_suffix=""
  if [ "$flavour" = rynk ]; then
    out="firmware"; suffix="/rynk"; name_suffix="_rynk"
    features_left="rynk,defmt"; features_right="rynk,defmt"
  else
    out="firmware"; suffix=""
    features_left="vial,defmt"; features_right="vial,defmt"
  fi
  if [ "$LOG" = 1 ]; then
    # Right half only: RMK's log over USB CDC in place of defmt. Own target
    # dir and output dir so it never overwrites the normal build.
    features_right="${features_right//defmt/usb_log}"
    suffix="$suffix/log"; out="$out/log"; mkdir -p "$out"
  fi
  mkdir -p "$out"

  local tgt="target$suffix"
  CARGO_TARGET_DIR="$tgt" cargo build --release --no-default-features --features "$features_right" --bin right
  CARGO_TARGET_DIR="$tgt" cargo objcopy --release --no-default-features --features "$features_right" --bin right -- -O ihex "$tmp/right.hex"
  cargo hex-to-uf2 --input-path "$tmp/right.hex" --output-path "$out/aerogu34_right$name_suffix.uf2" --family nrf52840
  if [ "$LOG" = 0 ]; then
    CARGO_TARGET_DIR="$tgt" cargo build --release --no-default-features --features "$features_left" --bin left
    CARGO_TARGET_DIR="$tgt" cargo objcopy --release --no-default-features --features "$features_left" --bin left -- -O ihex "$tmp/left.hex"
    cargo hex-to-uf2 --input-path "$tmp/left.hex" --output-path "$out/aerogu34_left$name_suffix.uf2" --family nrf52840
  fi

  # Both halves are Xiao BLEs: every image must start at 0x27000 (behind the
  # s140 SoftDevice the bootloader ships with) and end below the storage.
  python3 - "$out" "$STORAGE" "$flavour" "$name_suffix" <<'PY'
import os, struct, sys
out, storage, flavour, sfx = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]
bad = False
for name in (f"aerogu34_right{sfx}.uf2", f"aerogu34_left{sfx}.uf2"):
    path = os.path.join(out, name)
    if not os.path.exists(path):
        continue
    d = open(path, "rb").read(); addrs = []
    for i in range(0, len(d), 512):
        m0, m1, flags, addr, plen, blk, nblk, fam = struct.unpack("<8I", d[i:i+32])
        assert m0 == 0x0A324655 and fam == 0xADA52840, name
        addrs.append(addr)
    start, end = min(addrs), max(addrs) + 256
    ok = start == 0x27000 and end <= storage
    bad |= not ok
    print(f"[{flavour}] {name}: 0x{start:X}..0x{end:X}  storage@0x{storage:X}  {'OK' if ok else 'REFUSED'}")
sys.exit(1 if bad else 0)
PY
  rm -rf "$tmp"
}

case "$HOST" in
  vial) build_one vial ;;
  rynk) build_one rynk ;;
  both) build_one vial; build_one rynk ;;
  *) echo "unknown --host $HOST" >&2; exit 2 ;;
esac
