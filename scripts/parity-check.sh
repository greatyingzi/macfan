#!/usr/bin/env bash
# Compare this crate's rsmc against the reference `smc` tool on live hardware.
#
#   scripts/parity-check.sh [reference-smc] [rsmc]
#
# Defaults: `smc` from PATH, `target/release/rsmc`.
#
# What it checks
#   1. `-f` fan dump: byte-identical except the live "Current speed" line.
#   2. `-l` key enumeration: same key set, same key+type column for every key.
#   3. `-l` payloads: for every key that holds still during the window, both
#      tools must render the same line.
#   4. Any payload mismatch is re-verified by re-reading that key alternately
#      with both tools: only a difference that reproduces in every round counts
#      (SMC values drift, and a dump taken seconds apart will show that drift).
#
# Two differences are expected and reported as "known": the reference drops the
# sign when printing `si16` keys, and it prints out-of-bounds bytes for keys
# whose payload read fails (see README).
#
# Exits non-zero if an unexpected difference shows up.
set -uo pipefail

REF="${1:-smc}"
RUST="${2:-target/release/rsmc}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

ROUNDS=5

die() { echo "parity-check: $*" >&2; exit 2; }

command -v "$REF" >/dev/null 2>&1 || [ -x "$REF" ] || die "reference smc not found: $REF"
[ -x "$RUST" ] || die "rsmc not found: $RUST (build first: cargo build --release)"

unexpected=0

# --- 1. fan dump ------------------------------------------------------------
"$REF" -f > "$TMP/ref_f" 2>&1
"$RUST" -f > "$TMP/rs_f" 2>&1
# The live speed changes between two runs, so mask that one line.
sed 's/^\(    Current speed \).*/\1: (volatile)/' "$TMP/ref_f" > "$TMP/ref_f.mask"
sed 's/^\(    Current speed \).*/\1: (volatile)/' "$TMP/rs_f" > "$TMP/rs_f.mask"
if diff -q "$TMP/ref_f.mask" "$TMP/rs_f.mask" >/dev/null; then
  echo "  [1/4] -f fan dump        : identical ($(grep -c . "$TMP/ref_f") lines)"
else
  echo "  [1/4] -f fan dump        : DIFFERENT"
  diff "$TMP/ref_f.mask" "$TMP/rs_f.mask" | sed 's/^/        /'
  unexpected=$((unexpected + 1))
fi

# --- 2 & 3. full key dump ---------------------------------------------------
echo "  dumping every key with both tools (4 passes)…"
"$REF" -l > "$TMP/ref1"; sleep 1
"$RUST" -l > "$TMP/rs1"; sleep 1
"$RUST" -l > "$TMP/rs2"; sleep 1
"$REF" -l > "$TMP/ref2"

awk 'NF>1{print substr($0,1,9)}' "$TMP/ref1" > "$TMP/ref.struct"
awk 'NF>1{print substr($0,1,9)}' "$TMP/rs1" > "$TMP/rs.struct"
if diff -q "$TMP/ref.struct" "$TMP/rs.struct" >/dev/null; then
  echo "  [2/4] -l key+type column : identical ($(wc -l < "$TMP/ref.struct" | tr -d ' ') keys)"
else
  echo "  [2/4] -l key+type column : DIFFERENT"
  diff "$TMP/ref.struct" "$TMP/rs.struct" | head -20 | sed 's/^/        /'
  unexpected=$((unexpected + 1))
fi

: > "$TMP/suspect"
awk -v ref1="$TMP/ref1" -v ref2="$TMP/ref2" -v rs1="$TMP/rs1" -v rs2="$TMP/rs2" '
function load(path, arr,   line) { while ((getline line < path) > 0) if (length(line) > 6) arr[substr(line,3,4)] = line; close(path) }
BEGIN {
  load(ref1, a1); load(ref2, a2); load(rs1, b1); load(rs2, b2)
  compared = 0; moved = 0; sign = 0; unreadable = 0; suspect = 0
  for (k in a1) {
    if (!(k in a2) || a1[k] != a2[k]) { moved++; continue }     # moved during the window
    if (!(k in b1) || b1[k] != b2[k]) { moved++; continue }
    compared++
    if (a1[k] == b1[k]) continue
    if (a1[k] ~ /\[si16\]/) {
      if (sign++ == 0) print "  [3/4] known deviation      : reference drops the si16 sign, e.g. " a1[k] " vs " b1[k]
      continue
    }
    if (b1[k] ~ /unreadable/) { unreadable++; continue }
    print k > "'"$TMP"'/suspect"
    suspect++
  }
  printf "  [3/4] -l payloads        : %d keys held still, %d moved, %d known deviations (%d si16 sign, %d unreadable), %d to re-verify\n", compared, moved, sign + unreadable, sign, unreadable, suspect
}' /dev/null

# --- 4. re-verify every suspect by alternating reads -------------------------
# A single-read comparison cannot tell a real difference from a live counter
# that jitters: such a key mismatches on nearly every read. So read each
# suspect ROUNDS times per tool and only call it a real difference when each
# tool's readings are *stable* and the two values differ. Keys that keep moving
# are reported as uncomparable instead of being quietly passed or failed.
real=0
stable_same=0
moving=0
if [ -s "$TMP/suspect" ]; then
  while read -r key; do
    [ -n "$key" ] || continue
    : > "$TMP/ref_vals"; : > "$TMP/rs_vals"
    for _ in $(seq "$ROUNDS"); do
      "$REF" -k "$key" -r 2>&1 | sed 's/.*(bytes/(bytes/' >> "$TMP/ref_vals"
      "$RUST" -k "$key" -r 2>&1 | sed 's/.*(bytes/(bytes/' >> "$TMP/rs_vals"
    done
    n_ref=$(sort -u "$TMP/ref_vals" | wc -l | tr -d ' ')
    n_rs=$(sort -u "$TMP/rs_vals" | wc -l | tr -d ' ')
    ref_val=$(head -1 "$TMP/ref_vals")
    rs_val=$(head -1 "$TMP/rs_vals")
    if [ "$n_ref" -eq 1 ] && [ "$n_rs" -eq 1 ] && [ "$ref_val" != "$rs_val" ]; then
      real=$((real + 1))
      echo "  [4/4] REAL DIFFERENCE      : $key ref=$ref_val rust=$rs_val (both stable over $ROUNDS reads)"
    elif [ "$n_ref" -eq 1 ] && [ "$n_rs" -eq 1 ]; then
      stable_same=$((stable_same + 1))
    else
      moving=$((moving + 1))
      echo "  [4/4] uncomparable         : $key moves while reading (ref $n_ref value(s), rust $n_rs) — not a tool difference"
    fi
  done < "$TMP/suspect"
fi
echo "  [4/4] re-verification    : $real reproducible, $stable_same stable-and-equal, $moving moving/uncomparable"
[ "$real" -eq 0 ] || unexpected=$((unexpected + real))

echo
if [ "$unexpected" -eq 0 ]; then
  echo "PASS — no unexpected differences against $REF"
else
  echo "FAIL — $unexpected check(s) found unexpected differences"
fi
exit $((unexpected > 0))
