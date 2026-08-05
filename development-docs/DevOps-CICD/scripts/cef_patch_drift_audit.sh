#!/usr/bin/env bash
# CEF-2 — CEF patch drift audit. Fast-fail pre-flight for the 10-12 h build.
#
# WHAT THIS IS FOR, and what it deliberately does NOT do:
#
# CEF applies patches with `git apply -p0 --ignore-whitespace` (git_util.py ::
# git_apply_patch_file) -- exact-context, no fuzz, no --3way, no retry. That is a
# real upside: a context mismatch HARD-FAILS the build before compile rather than
# fuzzily landing a hunk in the wrong place. So this script is not defending
# against a silent-misland mode CEF does not have. It exists to surface that
# hard-fail CHEAPLY -- now, in seconds -- instead of 10 hours into a build.
#
# The one signal short of outright failure is a hunk applied at a line OFFSET:
# it still lands, just shifted, and it is the early warning that the next
# milestone jump will break the patch. That is exit 2, not exit 1.
#
# NOT REIMPLEMENTED HERE (they already exist -- this script CALLS them):
#   cef_dist_drift_audit.sh   runtime file-manifest drift (VER-5)
#   cef_gn_args_gate.sh       GN-args / codec gate
# Pass --with-dist <release-dir> [<resources-dir>] to chain the first.
#
# NEVER uses patch_updater.py --reapply/--restore: no dry-run mode, write-capable,
# and it RESAVES the .patch files it is supposed to be validating.
#
# Usage:
#   cef_patch_drift_audit.sh [--with-dist <release-dir> [<resources-dir>]]
#
# Exit: 0 clean · 2 warnings (hunk offsets present; build may proceed with
#       sign-off) · 1 HARD FAIL (patch cannot apply, target file missing, orphan
#       or dangling registry entry) -- build must not start.
set -uo pipefail

CEF_SRC=/c/cef/cef150/chromium/src/cef
CHROMIUM_SRC=/c/cef/cef150/chromium/src
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASELINE="$SCRIPT_DIR/cef_patch_baseline_7871.txt"

# Pre-existing UPSTREAM orphan on branch 7871 @ 94c1726: present in
# patch/patches/ but never registered in patch.cfg. It is not ours. Without this
# allowance the registry check would exit 1 on every single run from day one --
# and a gate that always fails is a gate that gets ignored.
UPSTREAM_ORPHAN_ALLOWANCE="chrome_browser_privacy_1119417"

WARN=0
FAIL=0

note_fail() { echo "  AUDIT_FAIL: $*"; FAIL=1; }
note_warn() { echo "  AUDIT_WARN: $*"; WARN=1; }

# ── R10 GUARD ─────────────────────────────────────────────────────────────────
# patcher.py derives src_dir as the PARENT of the CEF dir. Run from the
# standalone checkout (C:\cef\cef150\cef) that parent is C:\cef\cef150, which is
# not a git repo -- git_util.py then falls back to GNU `patch --force`, which DOES
# fuzz and CAN misland. The exact-context guarantee this whole audit reasons about
# holds only inside chromium/src. So refuse to audit the wrong tree.
if [ ! -d "$CEF_SRC/patch/patches" ]; then
  echo "AUDIT_FAIL: no patch dir at $CEF_SRC/patch/patches"
  exit 1
fi
if ! git -C "$CHROMIUM_SRC" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "AUDIT_FAIL: $CHROMIUM_SRC is not a git checkout."
  echo "            Refusing to run: patcher.py would fall back to GNU patch --force,"
  echo "            which fuzzes. Audit the in-tree CEF copy, never the standalone one."
  exit 1
fi

echo "=== CEF patch drift audit ==="
echo "cef dir      : $CEF_SRC"
echo "chromium src : $CHROMIUM_SRC ($(git -C "$CHROMIUM_SRC" rev-parse --short HEAD))"
echo

# ── 1. REGISTRY INTEGRITY ─────────────────────────────────────────────────────
# patch.cfg is Python and its header comment contains the words name/path/
# condition, so grep -c overcounts. Parse it the way patcher.py does: exec it.
echo "=== 1. Registry integrity (patch.cfg <-> patch/patches/) ==="
PATHMAP=$(mktemp)
trap 'rm -f "$PATHMAP"' EXIT
REG=$(cd "$CEF_SRC" && python - "$UPSTREAM_ORPHAN_ALLOWANCE" "$PATHMAP" <<'PYEOF'
import os, sys
allow = {sys.argv[1]}
scope = {}
exec(compile(open('patch/patch.cfg', 'rb').read(), 'patch.cfg', 'exec'), scope)
patches = scope['patches']
names = [p['name'] for p in patches]
files = set(f[:-6] for f in os.listdir('patch/patches') if f.endswith('.patch'))

hodos = [p for p in patches if p['name'].startswith('hodos_')]
dangling = [n for n in names if n not in files]            # registered, no file
orphan = sorted(files - set(names) - allow)                # file, not registered
dup = sorted(set(n for n in names if names.count(n) > 1))

# name<TAB>path<TAB>condition for EVERY entry. This must cover upstream entries
# too, not just ours: 4 upstream patches target sub-repos (v8,
# third_party/depot_tools, third_party/angle, third_party/dawn) and checking them
# against the Chromium src root produces 4 confident false failures.
with open(sys.argv[2], 'w', newline='\n') as fh:
    for p in patches:
        fh.write('%s\t%s\t%s\n' % (
            p['name'], p.get('path', ''), p.get('condition', '-')))

print('ENTRIES=%d' % len(patches))
print('FILES=%d' % len(files))
print('HODOS=%d' % len(hodos))
print('SUBREPO=%d' % len([p for p in patches if p.get('path')]))
print('DANGLING=%s' % ','.join(sorted(dangling)))
print('ORPHAN=%s' % ','.join(orphan))
print('DUP=%s' % ','.join(dup))
for p in hodos:
    print('HODOS_ENTRY=%s|%s|%s' % (
        p['name'], p.get('condition', '-'), p.get('path', 'src')))
PYEOF
)
if [ -z "$REG" ]; then
  echo "AUDIT_FAIL: could not parse patch.cfg -- this is an AUDIT malfunction, not a finding."
  exit 1
fi
eval "$(echo "$REG" | grep -E '^(ENTRIES|FILES|HODOS|SUBREPO)=')"
DANGLING=$(echo "$REG" | sed -n 's/^DANGLING=//p')
ORPHAN=$(echo "$REG" | sed -n 's/^ORPHAN=//p')
DUP=$(echo "$REG" | sed -n 's/^DUP=//p')

echo "  patch.cfg entries : $ENTRIES"
echo "  .patch files      : $FILES  (+1 allowed upstream orphan: $UPSTREAM_ORPHAN_ALLOWANCE)"
echo "  Hodos entries     : $HODOS"
echo "$REG" | sed -n 's/^HODOS_ENTRY=/    hodos: /p' | tr '|' ' '

# ── SELF-CHECK ────────────────────────────────────────────────────────────────
# A gate that cannot tell ITS OWN failure from the failure it is hunting is worse
# than no gate. If patch.cfg parsed to a handful of entries, every downstream
# verdict is meaningless -- and "0 patches, all clean" would read as PASS.
if [ "${ENTRIES:-0}" -lt 100 ]; then
  echo "  AUDIT_FAIL: only $ENTRIES entries parsed -- expected 114+ upstream."
  echo "              AUDIT malfunction, not a patch finding. Check patch.cfg syntax."
  exit 1
fi
# Second self-check, added after this audit's own first run produced 4 confident
# false failures: it had built a target-dir map for Hodos patches only, so the 4
# upstream sub-repo patches were checked against the Chromium root and "failed".
# A path map that has lost the sub-repo entries is an audit malfunction, and it
# looks exactly like real patch rot. Assert the map is complete.
MAPPED=$(wc -l < "$PATHMAP")
if [ "$MAPPED" -ne "$ENTRIES" ]; then
  echo "  AUDIT_FAIL: path map has $MAPPED rows for $ENTRIES entries -- AUDIT malfunction."
  exit 1
fi
if [ "${SUBREPO:-0}" -lt 1 ]; then
  echo "  AUDIT_FAIL: no sub-repo ('path') entries found -- expected 4 on 7871."
  echo "              AUDIT malfunction (a lost path map fails those patches falsely)."
  exit 1
fi
echo "  self-check OK: $ENTRIES entries parsed, path map complete, $SUBREPO sub-repo entries"

[ -n "$DANGLING" ] && note_fail "registered but no .patch file: $DANGLING"
[ -n "$ORPHAN" ]   && note_fail "orphan .patch file not in patch.cfg: $ORPHAN"
[ -n "$DUP" ]      && note_fail "duplicate patch.cfg names: $DUP"

if [ -f "$BASELINE" ]; then
  NEWP=$(cd "$CEF_SRC/patch/patches" && ls *.patch 2>/dev/null | sed 's/\.patch$//' | sort \
         | comm -23 - <(sort "$BASELINE") | grep -v '^hodos_' || true)
  GONE=$(cd "$CEF_SRC/patch/patches" && ls *.patch 2>/dev/null | sed 's/\.patch$//' | sort \
         | comm -13 - <(sort "$BASELINE") || true)
  [ -n "$NEWP" ] && note_warn "upstream patches added vs baseline: $(echo $NEWP | tr '\n' ' ')"
  [ -n "$GONE" ] && note_warn "baseline patches now absent: $(echo $GONE | tr '\n' ' ')"
  [ -z "$NEWP$GONE" ] && echo "  upstream set matches baseline manifest"
else
  note_warn "no baseline manifest at $BASELINE -- cannot detect upstream set drift"
fi
echo

# ── 2. TARGET-FILE EXISTENCE (Hodos patches only) ─────────────────────────────
# Catches an upstream rename/delete with a clearer message than a raw hunk fail.
echo "=== 2. Hodos patch target files exist ==="
if [ "${HODOS:-0}" -eq 0 ]; then
  echo "  (no Hodos patches registered yet -- nothing to check)"
else
  for pf in "$CEF_SRC"/patch/patches/hodos_*.patch; do
    [ -e "$pf" ] || continue
    pn=$(basename "$pf" .patch)
    # -p0 format: paths are rooted at the tree root, no a/ b/ prefixes.
    tgts=$(grep -E '^\+\+\+ ' "$pf" | sed 's/^+++ //' | sed 's/[[:space:]].*$//' | grep -v '^/dev/null$')
    for t in $tgts; do
      if [ -e "$CHROMIUM_SRC/$t" ]; then
        echo "  OK      $pn -> $t"
      else
        note_fail "$pn targets a missing file: $t (upstream rename/delete?)"
      fi
    done
  done
fi
echo

# ── 3. PATCH APPLY HEALTH (read-only) ─────────────────────────────────────────
# Mirrors git_util.py's order, and it MUST: on an already-patched tree a naive
# forward `git apply --check` fails for every applied patch, which would make this
# audit useless (114 false failures). So reverse-check FIRST -- if the patch
# reverses cleanly it is already applied AND still matches the tree, which is
# itself a strong integrity signal. Only then try forward.
#
# Honest limitation: offsets can only be measured for patches not yet applied.
# Against a fully-patched tree this check proves "present and matching", not
# "would apply cleanly to pristine source". For the latter, run against a fresh
# sync -- which is what the scheduled fork-watcher does.
echo "=== 3. Patch apply health (read-only; reverse-check first) ==="
APPLIED=0; APPLIES=0; OFFSET=0; BAD=0
for pf in "$CEF_SRC"/patch/patches/*.patch; do
  [ -e "$pf" ] || continue
  pn=$(basename "$pf" .patch)
  [ "$pn" = "$UPSTREAM_ORPHAN_ALLOWANCE" ] && continue

  # patch.cfg 'path' -- empty means the Chromium src root. Looked up from the
  # complete map (all entries, upstream included), never inferred.
  rel=$(awk -F'\t' -v n="$pn" '$1==n {print $2; exit}' "$PATHMAP")
  if [ -z "$rel" ]; then
    tdir="$CHROMIUM_SRC"
  else
    tdir="$CHROMIUM_SRC/$rel"
  fi
  [ -d "$tdir" ] || { note_warn "$pn: target dir missing ($tdir) -- patcher would skip"; continue; }

  # CEF converts CRLF->LF before applying (git_util.py does this on win32); match it.
  if tr -d '\r' < "$pf" | git -C "$tdir" apply -p0 --ignore-whitespace --reverse --check - 2>/dev/null; then
    APPLIED=$((APPLIED+1)); continue
  fi
  ERR=$(tr -d '\r' < "$pf" | git -C "$tdir" apply -p0 --ignore-whitespace --check - 2>&1)
  if [ -z "$ERR" ]; then
    APPLIES=$((APPLIES+1))
  elif echo "$ERR" | grep -q 'error:'; then
    note_fail "$pn WILL NOT APPLY -- this aborts the build before compile:"
    echo "$ERR" | sed 's/^/            /' | head -6
    BAD=$((BAD+1))
  else
    APPLIES=$((APPLIES+1))
  fi
  if echo "$ERR" | grep -q 'offset'; then
    OFFSET=$((OFFSET+1))
    note_warn "$pn applies at a line OFFSET -- drifting, likely breaks on the next milestone jump:"
    echo "$ERR" | grep 'offset' | sed 's/^/            /' | head -4
  fi
done
echo "  already applied (reverses cleanly) : $APPLIED"
echo "  would apply cleanly                : $APPLIES"
echo "  applies but at an offset (warn)     : $OFFSET"
echo "  WILL NOT APPLY (hard fail)          : $BAD"
echo

# ── 4/5. CHAIN THE EXISTING AUDITS (do not duplicate them) ────────────────────
if [ "${1:-}" = "--with-dist" ]; then
  echo "=== 4. Runtime file-manifest drift (delegating to cef_dist_drift_audit.sh) ==="
  if [ -x "$SCRIPT_DIR/cef_dist_drift_audit.sh" ] || [ -f "$SCRIPT_DIR/cef_dist_drift_audit.sh" ]; then
    bash "$SCRIPT_DIR/cef_dist_drift_audit.sh" "${2:-}" "${3:-}" || note_warn "dist drift audit reported findings (exit $?)"
  else
    note_warn "cef_dist_drift_audit.sh not found beside this script"
  fi
  echo
else
  echo "=== 4. Runtime file-manifest drift: SKIPPED (pass --with-dist <release-dir>) ==="
  echo "    Not run is NOT the same as clean. The installer's extension whitelist"
  echo "    silently drops unknown extensions at packaging -- run this before shipping."
  echo
fi

echo "=== 5. GN-args / codec gate ==="
echo "    NOT run here: cef_gn_args_gate.sh generates GN projects (slow, and it"
echo "    writes out-dirs). It is already a mandatory pre-build gate on its own."
echo "    Run it separately before the build -- this audit does not replace it."
echo

# ── VERDICT ───────────────────────────────────────────────────────────────────
echo "=== VERDICT ==="
if [ "$FAIL" -ne 0 ]; then
  echo "AUDIT_RESULT: HARD FAIL — DO NOT START THE BUILD (exit 1)"
  exit 1
elif [ "$WARN" -ne 0 ]; then
  echo "AUDIT_RESULT: WARNINGS — build may proceed with sign-off (exit 2)"
  exit 2
fi
echo "AUDIT_RESULT: CLEAN (exit 0)"
exit 0
