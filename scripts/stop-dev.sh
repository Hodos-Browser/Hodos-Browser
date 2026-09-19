#!/usr/bin/env bash
#
# Stop the DEV browser / wallet / adblock processes on macOS — matched by
# EXECUTABLE PATH, never by process name.
#
# WHY THIS EXISTS
# ---------------
# All three dev processes share their image name with the user's INSTALLED
# build: HodosBrowser, hodos-wallet, hodos-adblock. So this:
#
#     pkill -f hodos-wallet          # ⛔ NEVER
#     killall HodosBrowser           # ⛔ NEVER
#
# also kills the installed browser's wallet backend. 🚨 The Windows form of
# exactly that happened on 2026-09-01 and left the owner's production browser
# reporting "no wallet" to every dApp for ~4 hours — see CLAUDE.md's Dev Runbook
# and TICKET_wallet_backend_death_is_silent_and_unrecovered.md for the product
# half. The owner runs an installed Hodos on this Mac too, so the hazard is not
# Windows-specific; `MAC_RELAY_P7C_ROUND.md` M6 asked for this script.
#
# It refuses to stop anything whose executable path is not under the repo.
# `--dry-run` makes it entirely read-only.
#
# ⛔ WHY `ps -o comm=` AND NOT `pgrep -f` / argv[0]
# -------------------------------------------------
# `pgrep -f` matches the full ARGUMENT VECTOR, and argv[0] is whatever the
# launcher passed. A browser started as `./build/bin/HodosBrowser.app/...` has a
# RELATIVE argv[0] and is invisible to an absolute-prefix match — measured
# 2026-08-26, when it left two browsers running on one profile and looked exactly
# like profile corruption. `ps -o comm=` reports the path the KERNEL executed,
# absolute regardless of how the process was launched. Same lesson as the
# `proc_pidpath` finding in the codec_check.py post-mortem: prefer kernel truth
# over self-reported strings.
#
# USAGE
#   ./scripts/stop-dev.sh
#   ./scripts/stop-dev.sh --dry-run
#   ./scripts/stop-dev.sh --repo-root /path/to/Hodos-Browser
#
set -u

DRY_RUN=0
REPO_ROOT=""

while [ $# -gt 0 ]; do
    case "$1" in
        -n|--dry-run) DRY_RUN=1; shift ;;
        --repo-root)  REPO_ROOT="${2:-}"; shift 2 ;;
        -h|--help)    sed -n '2,45p' "$0"; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

# Resolve the repo root from the script's own location, in the BODY.
# (The Windows script's one defect was resolving this at parameter-binding time,
# where the path variable was still empty — the documented no-argument
# invocation then died before the body ran. Same class of mistake avoided here.)
if [ -z "$REPO_ROOT" ]; then
    here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    REPO_ROOT="$(cd "$here/.." && pwd)"
fi
if [ ! -d "$REPO_ROOT" ]; then
    echo "repo root does not exist: $REPO_ROOT" >&2; exit 2
fi
REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"

# ⚠️ Fragments stay anchored to BUILD-OUTPUT directories. Something as loose as
# 'Hodos-Browser' would match a production install sitting in a similarly named
# folder — and on macOS it would also match the repo's own /Applications symlink
# habits. Each candidate must match the fragment AND live under the repo root.
#
# label | basename prefix | path fragment identifying the DEV build
TARGETS=(
  "dev browser|HodosBrowser|cef-native/build/bin"
  "dev wallet|hodos-wallet|rust-wallet/target/release"
  "dev adblock|hodos-adblock|adblock-engine/target/release"
)

echo "Repo root: $REPO_ROOT"
[ "$DRY_RUN" -eq 1 ] && echo "(--dry-run: nothing will be killed)"
echo

total_stopped=0
total_spared=0

for entry in "${TARGETS[@]}"; do
    IFS='|' read -r label prefix fragment <<< "$entry"

    dev_pids=()
    spared_paths=()

    # Every process whose EXECUTABLE basename starts with the prefix. This
    # deliberately catches the browser's helper processes too
    # ("HodosBrowser Helper (Renderer)" etc.), which live under
    # <bundle>/Contents/Frameworks and would otherwise be orphaned.
    while IFS= read -r line; do
        pid="${line%% *}"
        path="${line#* }"
        [ -z "$path" ] && continue
        # ⛔ Parameter expansion, not `basename`: a login shell reports its comm as
        # "-zsh", and `basename -zsh` is parsed as an OPTION — it printed a usage
        # error for every such process on the first run. `${path##*/}` cannot be
        # tricked by a leading dash.
        base="${path##*/}"
        case "$base" in
            "$prefix"*) ;;
            *) continue ;;
        esac
        # ⚠️ CANONICALISE before trusting the repo-root prefix. The browser spawns
        # the wallet through a relative hop, so its kernel path really looks like
        #   .../build/bin/HodosBrowser.app/Contents/MacOS/../../../../../../rust-wallet/target/release/hodos-wallet
        # A textual prefix test on that would also accept a path that starts
        # inside the repo and then `..`s its way OUT of it — which is precisely
        # the "kill something outside the repo" case this script exists to refuse.
        #
        # 🚨 AND RESOLVE A RELATIVE comm AGAINST *THAT PROCESS'S* CWD, NOT OURS.
        # 📏 MEASURED 2026-09-19, and this script silently failed its whole purpose:
        # a browser launched as `./build/bin/HodosBrowser.app/...` reports a RELATIVE
        # kernel comm. `cd $(dirname …)` then resolves it against **stop-dev.sh's own
        # cwd** — which is wherever the user invoked the script, not where the browser
        # was started. Launched from `cef-native/`, stopped from the repo root, the `cd`
        # FAILS, real_path falls back to the relative string, the `$REPO_ROOT/*` test
        # cannot match a path beginning `./`, and the dev browser is **SPARED and listed
        # as if it were the installed app**. Two consecutive stop-dev runs left it alive,
        # respawning helpers and holding 9322.
        # ⇒ This is the argv[0] family one level up: the script was written to avoid
        #   `pgrep -f`'s relative-path blindness and reintroduced it in the canonicaliser.
        real_dir=""
        case "$path" in
            /*) real_dir="$(cd "$(dirname "$path")" 2>/dev/null && pwd -P)" || real_dir="" ;;
            *)  # Relative: ask the KERNEL for that pid's cwd and resolve against it.
                proc_cwd="$(lsof -p "$pid" -a -d cwd -Fn 2>/dev/null | sed -n 's/^n//p' | head -1)"
                if [ -n "$proc_cwd" ]; then
                    real_dir="$(cd "$proc_cwd" && cd "$(dirname "$path")" 2>/dev/null && pwd -P)" || real_dir=""
                fi
                ;;
        esac
        if [ -n "$real_dir" ]; then
            real_path="$real_dir/$base"
        else
            real_path="$path"
        fi
        # DEV iff the RESOLVED path matches the build-output fragment AND is
        # genuinely under this repo.
        if [[ "$real_path" == *"$fragment"* && "$real_path" == "$REPO_ROOT"/* ]]; then
            dev_pids+=("$pid")
        elif [[ "$real_path" == *"$fragment"* ]]; then
            # ⛔ Fail LOUD, never silently spare. The fragment says this IS a dev build
            # but we could not prove it lives under this repo — so we refuse to kill it
            # (that refusal is the whole safety property) and say so, rather than letting
            # it sit in the "spared, those are the installed build's" list where a human
            # reads it as correct.
            echo "  ⚠️  UNRESOLVED dev-looking process pid $pid: $path"
            echo "      Could not canonicalise it; NOT stopped. Check it by hand:"
            echo "      ps -p $pid -o comm=   # then kill -9 $pid if it is yours"
            spared_paths+=("$real_path (UNRESOLVED — see warning above)")
        else
            spared_paths+=("$real_path")
        fi
    done < <(ps -axo pid=,comm= 2>/dev/null | sed 's/^ *//')

    n_dev=${#dev_pids[@]}
    n_spared=${#spared_paths[@]}
    total=$(( n_dev + n_spared ))

    if [ "$total" -eq 0 ]; then
        printf '%-12s : not running\n' "$label"
        continue
    fi
    printf '%-12s : %d running, %d dev, %d spared\n' "$label" "$total" "$n_dev" "$n_spared"

    # Print what was left alone, by distinct path — "I spared something" is the
    # fact worth seeing, and the path is what tells you whose build it is.
    # A browser has dozens of children, so collapse to unique paths with counts.
    if [ "$n_spared" -gt 0 ]; then
        printf '%s\n' "${spared_paths[@]}" | sort | uniq -c | while read -r cnt p; do
            printf '               spared %3s x  %s\n' "$cnt" "$p"
        done
    fi
    total_spared=$(( total_spared + n_spared ))

    for pid in ${dev_pids[@]+"${dev_pids[@]}"}; do
        p="$(ps -p "$pid" -o comm= 2>/dev/null)"
        if [ "$DRY_RUN" -eq 1 ]; then
            printf '               WOULD STOP pid %s  %s\n' "$pid" "$p"
            total_stopped=$(( total_stopped + 1 ))
            continue
        fi
        if kill -9 "$pid" 2>/dev/null; then
            printf '               STOPPED pid %s\n' "$pid"
            total_stopped=$(( total_stopped + 1 ))
        else
            # A helper that already died with its parent is not a failure.
            printf '               pid %s already gone\n' "$pid"
        fi
    done
done

echo
if [ "$DRY_RUN" -eq 1 ]; then
    echo "Would stop $total_stopped dev process(es); would leave $total_spared non-dev process(es) alone."
else
    echo "Stopped $total_stopped dev process(es); left $total_spared non-dev process(es) alone."
fi
if [ "$total_spared" -gt 0 ]; then
    echo "^ those are the installed build's. Never stop them from here."
fi
exit 0
