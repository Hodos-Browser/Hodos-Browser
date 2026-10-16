#!/usr/bin/env bash
# Publish a FILTERED export of our code to the public release repo (Hodos-Browser/Hodos-Browser).
#
# The public repo never receives our branches. It receives one export commit per release:
#   tree    = the source commit's tree, filtered to scripts/public-export-allowlist.txt
#   parent  = the public repo's current main
#   message = "Public export of <sha>" + a "Source-Commit: <full sha>" trailer
# This is the pattern of Google's Copybara (allowlisted origin_files, a GitOrigin-RevId trailer),
# as a short script. Design and reasons: development-docs/DevOps-CICD/BUILD_AND_RELEASE.md Step 2.
#
# Usage (Git Bash on Windows, or macOS bash):
#   scripts/publish-public.sh                      build the export of origin/main, print it, push nothing
#   scripts/publish-public.sh --push               ...and push it to release/main
#   scripts/publish-public.sh --source <ref> [--remote <name>]
#   scripts/publish-public.sh --check <commit>     exit 0 only if <commit>'s tree may be public
#                                                  (used by .githooks/pre-push)
#
# Every check fails closed: a missing allowlist, an unparseable rule, a path git cannot list
# safely, or a failed key scan stops the run. Nothing is pushed until every check has passed.
set -euo pipefail

ALLOWLIST=scripts/public-export-allowlist.txt
# Refused whatever the allowlist inside a commit says, so widening the allowlist by mistake
# still cannot publish these. Matched case-insensitively.
NEVER_PUBLIC='development-docs/ archived-docs/ sessions/ tmp/ **/.claude/ **/claude.md **/notes_parallel_work.md'

die() { echo "publish-public: $*" >&2; exit 1; }

# filter <tree-ish> keep|reject — print the `git ls-tree -r` lines of <tree-ish> that the
# allowlist stored IN <tree-ish> admits (keep) or does not admit (reject).
filter() {
    local rules
    rules=$(git show "$1:$ALLOWLIST" 2>/dev/null) || die "$1 has no $ALLOWLIST"
    # Paths are read NUL-separated and turned into lines, so a path holding a newline would be
    # mis-split: refuse such a tree outright.
    [ "$(git ls-tree -r -z --full-tree "$1" | tr -dc '\n' | wc -c)" -eq 0 ] \
        || die "$1 has a path containing a newline"
    # The rules go through the environment: macOS awk drops the newlines of a multi-line -v value.
    git ls-tree -r -z --full-tree "$1" | tr '\0' '\n' | RULES="$rules" awk -v mode="$2" -v never="$NEVER_PUBLIC" '
        # dir/ = under dir · **/dir/ = a dir of that name at any depth · **/name = that file
        # name at any depth · anything else = one exact path
        function hit(r, p,   b) {
            if (substr(r, 1, 3) == "**/" && substr(r, length(r)) == "/") return index("/" p, "/" substr(r, 4)) > 0
            if (substr(r, length(r)) == "/") return index(p, r) == 1
            if (substr(r, 1, 3) == "**/") { b = p; sub(/.*\//, "", b); return b == substr(r, 4) }
            return p == r
        }
        BEGIN {
            n = split(ENVIRON["RULES"], L, "\n")
            for (i = 1; i <= n; i++) {
                line = L[i]; sub(/\r$/, "", line)
                if (line ~ /^[ \t]*(#|$)/) continue
                # A rule must name something: "+ **/" or "+ /" would match everything.
                if (line ~ /^\+ (\*\*\/)?[A-Za-z0-9._][A-Za-z0-9._\/-]*$/)      inc[++ni] = substr(line, 3)
                else if (line ~ /^- (\*\*\/)?[A-Za-z0-9._][A-Za-z0-9._\/-]*$/)  exc[++ne] = substr(line, 3)
                else { print "publish-public: unreadable allowlist rule: " line > "/dev/stderr"; bad = 1; exit 2 }
            }
            nn = split(never, nev, " ")
        }
        {
            p = substr($0, index($0, "\t") + 1)
            ok = 0
            for (i = 1; i <= ni; i++) if (hit(inc[i], p)) { ok = 1; break }
            for (i = 1; i <= ne; i++) if (hit(exc[i], p)) ok = 0
            for (i = 1; i <= nn; i++) if (hit(nev[i], tolower(p))) ok = 0
            if ((mode == "keep") == (ok == 1)) print
        }
        END { if (bad) exit 2 }'
}

# key_scan <tree-ish> — fail on a key file by extension, or on ANY file that contains a private-key
# header (PEM: the words PRIVATE KEY and five dashes; PGP: PRIVATE KEY BLOCK; any case, text or
# binary), whatever follows it, unless the path is one of the three exact paths below that
# assemble a PEM from a CI secret at run time. No guessing at what a key body looks like.
# ⚠️ Not caught: a key with no header (hex, WIF, raw base64 — our test fixtures carry public SDK
# test vectors in that shape), UTF-16 text, and a key pasted into one of the three files.
# The allowlist is the first line of defence.
key_scan() {
    local named hits rc path found=0
    named=$(git -c core.quotePath=false ls-tree -r -z --name-only "$1" | tr '\0' '\n' \
        | LC_ALL=C grep -iE '\.(pem|key|p12|pfx|der|p8|jks|keystore|ppk|gpg)$' || true)
    if [ -n "$named" ]; then printf '%s\n' "$named" | sed 's/^/publish-public: key file /' >&2; found=1; fi
    set +e
    hits=$(LC_ALL=C git -c core.quotePath=false -c color.grep=false grep -a -z -l -i -F -e "PRIVATE KEY-"'----' -e "PRIVATE KEY BLOCK-"'----' "$1" -- | tr '\0' '\n')
    rc=$?
    set -e
    [ "$rc" -le 1 ] || die "key scan could not run (git grep exit $rc)"
    while IFS= read -r path; do
        [ -n "$path" ] || continue
        path=${path#"$1":}
        case $path in   # exact paths only; adding one is a reviewed decision
            .github/workflows/check-signing-key.yml | .github/workflows/release.yml | scripts/check-sparkle-key.ps1) continue ;;
        esac
        echo "publish-public: private-key header in $path" >&2; found=1
    done <<< "$hits"
    [ "$found" -eq 0 ] || die "refusing: a private key would be published"
}

# check <tree-ish> — every path admitted, and no key.
check() {
    local bad
    bad=$(filter "$1" reject)
    if [ -n "$bad" ]; then
        printf '%s\n' "$bad" | cut -f2 | head -10 >&2 || true   # head may close the pipe early
        die "refusing: $(printf '%s\n' "$bad" | wc -l | tr -d ' ') path(s) in $1 are not on the public allowlist"
    fi
    key_scan "$1"
}

SOURCE=origin/main REMOTE=release PUSH=0 CHECK=
while [ $# -gt 0 ]; do
    case "$1" in
        --source) SOURCE=${2:?}; shift 2 ;;
        --remote) REMOTE=${2:?}; shift 2 ;;
        --push)   PUSH=1; shift ;;
        --check)  CHECK=${2:?}; shift 2 ;;
        *) die "unknown argument: $1" ;;
    esac
done

if [ -n "$CHECK" ]; then
    commit=$(git rev-parse --verify -q "$CHECK^{commit}") || die "not a commit: $CHECK"
    check "$commit"
    exit 0
fi

git fetch -q "$REMOTE" main || die "could not fetch $REMOTE main"
src=$(git rev-parse --verify -q "$SOURCE^{commit}") || die "not a commit: $SOURCE"
parent=$(git rev-parse --verify -q "refs/remotes/$REMOTE/main^{commit}") || die "no $REMOTE/main"

idx=$(mktemp); rm -f "$idx"
trap 'rm -f "$idx"' EXIT
filter "$src" keep | GIT_INDEX_FILE="$idx" git update-index --index-info
tree=$(GIT_INDEX_FILE="$idx" git write-tree)
[ "$(git ls-tree -r "$tree" | wc -l)" -gt 0 ] || die "the export is empty"

# The export is checked as a tree in its own right, not trusted because filter() built it.
check "$tree"

echo "Source:   $src ($SOURCE)"
echo "Withheld: $(filter "$src" reject | wc -l | tr -d ' ') file(s), by top-level path:"
filter "$src" reject | cut -f2 | awk -F/ '{ print "  " (NF > 1 ? $1 "/" : $1) }' | sort | uniq -c
echo "Public:   $(git ls-tree -r "$tree" | wc -l | tr -d ' ') file(s)"

if [ "$tree" = "$(git rev-parse "$parent^{tree}")" ]; then
    echo "Nothing to export: $REMOTE/main already has this tree."
    exit 0
fi
export_commit=$(git commit-tree "$tree" -p "$parent" -m "Public export of ${src:0:12}" -m "Source-Commit: $src")
echo "Export:   $export_commit (parent $REMOTE/main ${parent:0:12})"

if [ "$PUSH" -eq 1 ]; then
    git push "$REMOTE" "$export_commit:refs/heads/main"
    echo "Pushed. Read back what landed:  git fetch $REMOTE && git ls-tree --name-only $REMOTE/main"
else
    echo "Not pushed. To publish:  git push $REMOTE $export_commit:refs/heads/main   (or re-run with --push)"
fi
