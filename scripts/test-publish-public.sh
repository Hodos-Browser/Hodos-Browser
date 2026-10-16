#!/usr/bin/env bash
# Controls for the public-export gate (scripts/publish-public.sh + .githooks/pre-push): every
# GREEN must pass and every RED must be refused. Builds throwaway git objects only; pushes nothing.
#   bash scripts/test-publish-public.sh                   test the working-tree script and hook
#   bash scripts/test-publish-public.sh <script> <hook>   test other copies (negative control:
#                                                         an older version must FAIL rows)
# The fixtures are built from the INDEX (git write-tree): stage a change before testing it.
# ⚠️ The hook always calls the WORKING-TREE scripts/publish-public.sh, so a <script> argument
# reaches the --check rows but not the hook rows.
# Needs git and openssl. Run from the root of the private repo, with release/main fetched.
# Prints one line per control: expected, got, PASS/FAIL. Exit 1 if any FAIL.
set -u
S=$(mktemp -d); trap 'rm -rf "$S"' EXIT
SCRIPT=${1:-scripts/publish-public.sh}; HOOK=${2:-.githooks/pre-push}; fails=0
H="PRIVATE KEY-"'----'   # never written whole here, or the gate would refuse this file
T=$(git write-tree); BASE=$(git commit-tree "$T" -p HEAD -m tmp)
EX=$(bash "$SCRIPT" --source "$BASE" 2>/dev/null | awk '/^Export:/{print $2}')
[ -n "$EX" ] || { echo "FAIL: no export built (git write-tree reads the INDEX: stage the change)"; exit 1; }
KEY=$(openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 2>/dev/null | tr -d '\r')
[ -n "$KEY" ] || { echo "FAIL: openssl could not make a test key"; exit 1; }
ok() { # name expected-rc got-rc
    if [ "$2" = "$3" ]; then r=PASS; else r=FAIL; fails=$((fails+1)); fi
    printf '%-58s want=%s got=%s %s\n' "$1" "$2" "$3" "$r"; }
mkf() { # path file -> commit = export + that file
    local idx=$S/idx.c; rm -f "$idx"; GIT_INDEX_FILE=$idx git read-tree "$EX"
    GIT_INDEX_FILE=$idx git update-index --add --cacheinfo 100644,"$(git hash-object -w --no-filters "$2")","$1"
    local t; t=$(GIT_INDEX_FILE=$idx git write-tree); rm -f "$idx"; git commit-tree "$t" -p "$EX" -m fixture; }
chk() { bash "$SCRIPT" --check "$1" >/dev/null 2>&1; echo $?; }
chkl() { LC_ALL=en_US.UTF-8 bash "$SCRIPT" --check "$1" >/dev/null 2>&1; echo $?; }

ok "GREEN export passes --check" 0 "$(chk "$EX")"
ok "GREEN export has ~602 files" 1 "$([ "$(git ls-tree -r "$EX" | wc -l)" -ge 590 ] && echo 1 || echo 0)"
ok "RED raw HEAD refused" 1 "$(chk "$BASE")"

# key shapes
printf '%s\n' "$KEY" > "$S/k1"; ok "RED plain PEM" 1 "$(chk "$(mkf scripts/a.txt "$S/k1")")"
printf '%s\n' "$KEY" | awk '{ printf "%s\r\n", $0 }' > "$S/k2"; ok "RED CRLF PEM" 1 "$(chk "$(mkf scripts/a.txt "$S/k2")")"
printf 'k: |\n%s\n' "$(printf '%s\n' "$KEY" | sed 's/^/    /')" > "$S/k3"; ok "RED indented PEM" 1 "$(chk "$(mkf scripts/a.yml "$S/k3")")"
printf '{"private_key":"%s"}\n' "$(printf '%s\n' "$KEY" | awk '{printf "%s\\n",$0}')" > "$S/k4"; ok "RED JSON-escaped PEM" 1 "$(chk "$(mkf scripts/a.json "$S/k4")")"
printf -- '-----BEGIN RSA %s\nProc-Type: 4,ENCRYPTED\n\nx\n' "$H" > "$S/k5"; ok "RED encrypted PEM" 1 "$(chk "$(mkf scripts/a.txt "$S/k5")")"
{ printf 'b\0'; printf '%s\n' "$KEY"; printf '\0\377\376\200'; } > "$S/k6"; F=$(mkf scripts/a.bin "$S/k6")
ok "RED PEM in binary (C locale)" 1 "$(chk "$F")"; ok "RED PEM in binary (en_US.UTF-8)" 1 "$(chkl "$F")"
printf '{"o":"Jos\351","k":"%s"}\n' "$(printf '%s\n' "$KEY" | awk '{printf "%s\\n",$0}')" > "$S/k7"; ok "RED Latin-1 JSON PEM (en_US.UTF-8)" 1 "$(chkl "$(mkf scripts/b.json "$S/k7")")"
printf -- '-----BEGIN EC %s\nMCUCAQEEIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n-----END EC %s\n' "$H" "$H" > "$S/k8"; ok "RED short SEC1 body" 1 "$(chk "$(mkf scripts/a.txt "$S/k8")")"
printf '%s\n' "$KEY" | sed -e '/^-----/d' | tr -d '\n' | fold -w 48 | { echo "-----BEGIN $H"; cat; echo; echo "-----END $H"; } > "$S/k9"; ok "RED 48-column PEM" 1 "$(chk "$(mkf scripts/a.txt "$S/k9")")"
printf '%s\n' "$KEY" > "$S/k10"; ok "RED non-ASCII path with PEM" 1 "$(chk "$(mkf 'scripts/é.txt' "$S/k10")")"
printf 'opaque' > "$S/k11"; ok "RED .p12 by extension" 1 "$(chk "$(mkf scripts/c.p12 "$S/k11")")"
ok "RED .KEY by extension (case)" 1 "$(chk "$(mkf scripts/x.KEY "$S/k11")")"
ok "RED non-ASCII .der by extension" 1 "$(chk "$(mkf 'docs/ключ.der' "$S/k11")")"
ok "RED .ppk by extension" 1 "$(chk "$(mkf scripts/c.ppk "$S/k11")")"
ok "RED key in a path joining two allowed names" 1 "$(chk "$(mkf '.github/workflows/check-signing-key.yml .github/workflows/release.yml' "$S/k1")")"
printf -- '-----BEGIN PGP %s BLOCK-----\n\nlQdGBF\n' "PRIVATE KEY" > "$S/k12"; ok "RED PGP private key block" 1 "$(chk "$(mkf scripts/a.asc "$S/k12")")"
ok "color.grep=always really set (git >= 2.31)" always "$(GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=color.grep GIT_CONFIG_VALUE_0=always git config color.grep)"
ok "GREEN export passes with color.grep=always" 0 "$(GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=color.grep GIT_CONFIG_VALUE_0=always bash "$SCRIPT" --check "$EX" >/dev/null 2>&1; echo $?)"

# allowlist hardening
WIDE=$(git show "$EX:scripts/public-export-allowlist.txt" | grep -v CLAUDE.md; printf '+ CLAUDE.md\n+ NOTES_parallel_work.md\n+ Development-Docs/\n')
mka() { local idx=$S/idx.a; rm -f "$idx"; GIT_INDEX_FILE=$idx git read-tree "$EX"
    printf '%s\n' "$1" > "$S/al"; GIT_INDEX_FILE=$idx git update-index --add --cacheinfo 100644,"$(git hash-object -w "$S/al")",scripts/public-export-allowlist.txt
    printf 'x' > "$S/xf"; GIT_INDEX_FILE=$idx git update-index --add --cacheinfo 100644,"$(git hash-object -w "$S/xf")","$2"
    local t; t=$(GIT_INDEX_FILE=$idx git write-tree); rm -f "$idx"; git commit-tree "$t" -p "$EX" -m fixture; }
for f in Development-Docs/x.md rust-wallet/claude.md frontend/.claude/settings.local.json NOTES_parallel_work.md CLAUDE.md cef-native/CLAUDE.md; do
    ok "RED widened allowlist: $f" 1 "$(chk "$(mka "$WIDE" "$f")")"; done
ok "RED allowlist '+ **/' rejected" 2 "$(chk "$(mka "$(printf '+ **/\n')" README.md)")"
ok "RED allowlist '+ /' rejected" 2 "$(chk "$(mka "$(printf '+ /\n')" README.md)")"
NF=$(bash "$SCRIPT" --source "$(mka "$(git show "$EX:scripts/public-export-allowlist.txt")" newfolder/x.md)" 2>/dev/null | awk '/^Export:/{print $2}')
ok "RED new top-level folder: export built" 1 "$([ -n "$NF" ] && echo 1 || echo 0)"
ok "RED new top-level folder not exported" 0 "$(git ls-tree -r --name-only "${NF:-none}" 2>/dev/null | grep -c '^newfolder/')"

# hook
hk() { printf '%s %s %s %s\n' refs/heads/x "$1" refs/heads/main "$(git rev-parse release/main)" | bash "$HOOK" release "$2" >/dev/null 2>&1; echo $?; }
for u in https://github.com/Hodos-Browser/Hodos-Browser.git 'https://github.com/Hodos%2DBrowser/Hodos%2DBrowser.git' 'https://github.com/Hodos-Browser/Hodos-Browser//' git@github.com:Hodos-Browser/Hodos-Browser.git \
         'https://github.com/bsvarchie/hodos-browser\c/../../../Hodos-Browser/Hodos-Browser.git' https://gitlab.com/bsvarchie/hodos-browser /tmp/x/bsvarchie/hodos-browser; do
    ok "RED hook raw HEAD -> $u" 1 "$(hk "$BASE" "$u")"; done
X=$(git commit-tree "$(git rev-parse "$EX^{tree}")" -p "$BASE" -m clean-tip)
ok "RED hook clean tip over private history" 1 "$(hk "$X" https://github.com/Hodos-Browser/Hodos-Browser.git)"
ok "GREEN hook export -> public" 0 "$(hk "$EX" https://github.com/Hodos-Browser/Hodos-Browser.git)"
for u in https://github.com/BSVArchie/Hodos-Browser.git https://github.com/BSVArchie/Hodos-Browser git@github.com:BSVArchie/Hodos-Browser.git; do
    ok "GREEN hook raw HEAD -> private $u" 0 "$(hk "$BASE" "$u")"; done
echo "failures: $fails"; [ "$fails" -eq 0 ]
