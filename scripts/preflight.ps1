<#
.SYNOPSIS
  Local T0 (static gates) + T1 (unit tests) runner for the beta.3 sprint harness.

.DESCRIPTION
  The dev fork's Actions minutes are exhausted until ~2026-09-01, so test.yml and ci.yml
  execute nothing. Everything they would have gated runs here instead. See
  development-docs/0.4.0-beta.3/HARNESS.md.

  Static gates RATCHET: each carries a Baseline and fails when violations EXCEED it. That
  lets a gate land before its fix, while a NEW violation still fails at any baseline --
  which is what preserves the negative control.

.PARAMETER NegativeControl
  Prove each static gate can fail. Injects a synthetic violation into a scanned tree, asserts
  the gate reports MORE than its baseline, then removes it. A gate that stays green under
  this is blind, and is reported as FAIL. Runs no unit tests.

.PARAMETER Full
  Also run the frontend type/build gate (tsc -b && vite build). Slow; off by default.

.PARAMETER Only
  Run a subset by id, e.g. -Only G1,G2

.OUTPUTS
  Exit 0 = PASS.  1 = FAIL.  2 = INCOMPLETE (something was skipped; never report as pass).

.EXAMPLE
  pwsh scripts/preflight.ps1
  pwsh scripts/preflight.ps1 -NegativeControl
#>
[CmdletBinding()]
param(
    [switch]$NegativeControl,
    [switch]$Full,
    [string[]]$Only
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path -Parent $PSScriptRoot

# Invoked via -File, "-Only G1,G2" arrives as ONE string, not an array, so every
# -contains test silently misses and every check is skipped. Split defensively.
if ($Only) { $Only = @($Only) -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ } }

# --------------------------------------------------------------------------------------
# Static gates (T0)
#
# Baseline: the count of KNOWN violations at the time the gate landed. Lowering it is a
# deliberate commit; raising it requires a written reason in HARNESS.md section 4.
# Probe:    a synthetic violating line, used ONLY by -NegativeControl.
# --------------------------------------------------------------------------------------
$Gates = @(
    [pscustomobject]@{
        Id       = 'G1'
        Name     = 'Bare-filename file sinks (relative path resolves against CWD, i.e. {app})'
        Owner    = 'Phase 0'
        # Lowered 52 -> 0 by Phase 0 (P0-A3). All 52 removed: debug_output.log x44
        # (WalletService 19, simple_app 17, my_overlay_render_handler .cpp/.mm 3+3,
        # AddressHandler 2) + startup_log.txt x8 (simple_app). No residuals.
        Baseline = 0
        Target   = 0
        Paths    = @('cef-native/src', 'cef-native/include')
        Include  = @('*.cpp', '*.mm', '*.h')
        Pattern  = '(ofstream|ifstream|fstream|fopen|freopen)[^;]*"[A-Za-z0-9_.\-]+\.(log|txt|json|db|dat|bin)"'
        Probe    = 'std::ofstream probe("preflight_probe.log", std::ios::app);'
        ProbeExt = '.cpp'
    },
    [pscustomobject]@{
        Id       = 'G2'
        Name     = 'Substring origin checks on the internal frontend port (trust boundary)'
        Owner    = 'Phase 0.5'
        # Measured 2026-08-18 by THIS script. The 5: TabManager.cpp:168,
        # TabManager_mac.mm:191, simple_handler.cpp:7962,
        # simple_render_process_handler.cpp:539 and :541.
        # Target 2 = the two TabManager history-exclusion uses -- sloppy, but not a trust
        # boundary; they go with W7 in beta.4. Phase 0.5 fixes the other three.
        # Lowered 5 -> 2 by Phase 0.5 (P0.5-G3). The three trust-boundary gates now use
        # hodos::IsInternalFrontendUrl (a prefix match). The 2 residuals are named in
        # phase-0.5-money-path/PHASE_CONTRACT.md section 6: TabManager.cpp:176 and
        # TabManager_mac.mm:191, both history-exclusion, not a trust boundary. They go
        # with W7 in beta.4.
        Baseline = 2
        Target   = 2
        Paths    = @('cef-native/src', 'cef-native/include')
        Include  = @('*.cpp', '*.mm', '*.h')
        Pattern  = '(find|rfind)\("(http://)?(127\.0\.0\.1|localhost):5137'
        # A prefix check -- rfind(X, 0) == 0, or find(X) != 0 -- is the CORRECT form and
        # must not be flagged, or the gate teaches the wrong lesson. Only unanchored
        # substring searches are violations.
        Exclude  = '(rfind\([^)]*, *0\)|find\([^)]*\) *!= *0)'
        Probe    = 'bool probe = url.find("127.0.0.1:5137") != std::string::npos;'
        ProbeExt = '.cpp'
    },
    [pscustomobject]@{
        Id       = 'G3'
        Name     = 'F8 secret-log gate (Rust) - secret material into a log/print sink'
        Owner    = 'test.yml (ported)'
        Baseline = 0
        Target   = 0
        Paths    = @('rust-wallet/src', 'adblock-engine/src')
        Include  = @('*.rs')
        Pattern  = '(log::(info|debug|trace|warn)!|println!|eprintln!|print!)\(.*(hex::encode|base64|\.encode)\(&?[A-Za-z0-9_]*(priv|secret|symmetric|mnemonic|seed|wif|xpriv|revelation|hmac_output|hmac_secret)'
        Probe    = 'log::info!("{}", hex::encode(&mnemonic_secret));'
        ProbeExt = '.rs'
    },
    [pscustomobject]@{
        Id       = 'G4'
        Name     = 'F8 secret-log gate (C++) - secret material into a log/print sink'
        Owner    = 'test.yml (ported)'
        Baseline = 0
        Target   = 0
        Paths    = @('cef-native/src', 'cef-native/include')
        Include  = @('*.cpp', '*.mm', '*.h')
        Pattern  = '(std::cout|std::cerr|printf|fprintf|OutputDebugString)[^;]*(\["(mnemonic|privateKey|private_key|seed)"\]|<<[^;]*\b(mnemonic|privateKey)\b)'
        Probe    = 'std::cerr << "probe" << mnemonic << std::endl;'
        ProbeExt = '.cpp'
    },
    [pscustomobject]@{
        Id       = 'G8'
        Name     = 'Raw (unconverted) coordinate assigned to a CefMouseEvent - the OSR overlay DPI bug'
        Owner    = 'Phase 1 (WS1)'
        # The ~15 overlays are WINDOWLESS CEF browsers. Their WndProcs receive PHYSICAL client
        # pixels; CefMouseEvent's x/y are VIEW coordinates, and our view is LOGICAL
        # (MyOverlayRenderHandler::GetViewRect divides by the DPI scale). Both are `int`, so
        # nothing in the type system distinguishes them -- which is how 47 call sites shipped
        # unconverted. MEASURED at 125%: the user aimed at one control and a DIFFERENT control
        # received the click, and the bottom 20% of every overlay was dead.
        # All 47 sites now go through hodos::ClientToViewPoint (include/core/OverlayMouse.h),
        # which does not match this pattern. A new direct assignment is the regression.
        # ⚠️ Windows only, deliberately. macOS assigns from NSView `location`, which is already
        #    in logical points -- 62 such lines in cef_browser_shell_mac.mm are CORRECT. Adding
        #    .mm here would produce a 62-violation baseline that hides the one line that matters.
        Baseline = 0
        Target   = 0
        Paths    = @('cef-native')
        Include  = @('*.cpp', '*.h')
        Pattern  = '\w*[Ee]vent\.(x|y) *= *[^;]'
        Probe    = 'mouse_event.x = pt.x;'
        ProbeExt = '.cpp'
    },
    [pscustomobject]@{
        Id       = 'G5'
        Name     = 'Full wallet HTTP response bodies reaching a sink (the mnemonic-leak shape)'
        Owner    = 'Phase 0'
        # Measured 2026-08-18 by THIS script. A hand-rolled grep gave 11 -- a narrower
        # pattern. Always baseline with the tool that will do the measuring.
        # Lowered 15 -> 0 by Phase 0 (P0-A8). Response BODIES are gone entirely; the
        # remaining prose hits ("...HTTP response...") were reworded, and txid/error are
        # now extracted into named locals BEFORE the log call rather than reached through
        # `response` inside it. No residuals.
        Baseline = 0
        Target   = 0
        Paths    = @('cef-native/src/core')
        Include  = @('WalletService.cpp', 'WalletService_mac.cpp', '__preflight_probe.cpp')
        Pattern  = '(debugLog *<<|LOG_[A-Z_]+ *\()[^;]*\b(responseBody|response)\b'
        Probe    = 'debugLog << "   Response: " << responseBody << std::endl;'
        ProbeExt = '.cpp'
    },
    [pscustomobject]@{
        Id       = 'G11'
        Name     = 'Window-scoped work resolved through a process-global (the wrong-window bug)'
        Owner    = 'Phase 3 (WS2); 60 -> 59 by beta.3 Phase 8c O8 (2026-09-13); driven to target by beta.4'
        # A second launch of the SAME profile does not start a second process -- it forwards
        # over a named pipe and opens a second WINDOW in the running one. So any code that
        # asks a process-global "which window am I?" can act on the wrong one. MEASURED
        # symptoms, both owner-observed 2026-08-26: Ctrl+F in the second window opened the
        # find bar in the FIRST and raised it (switching virtual desktops), and HTML5 video
        # fullscreen in either window hid the OTHER window's header and resized its tabs to
        # the primary window's rect.
        #
        # The correct APIs already exist and are already used in ~18 places:
        #   GetOwnerWindow()->hwnd / ->header_browser, TabManager::GetActiveTabForWindow(id),
        #   and, inside a WndProc, GetWindowLongPtr(hwnd, GWLP_USERDATA).
        # This is a HALF-FINISHED MIGRATION, not a missing capability -- which is exactly why
        # a ratchet is the right instrument: it cannot be finished in one phase, and it must
        # not be allowed to grow while it waits.
        #
        # ⛔ NOT every global is wrong. g_file_dialog_active and g_wallet_overlay_prevent_close
        # are genuinely process-wide and are deliberately NOT matched here. Likewise SaveSession
        # and ShutdownApplication legitimately enumerate ALL tabs in ALL windows. The test is
        # never "is it global" but "does this have one value per process, or one per window?"
        #
        # ⚠️ Baseline MUST be set by running this script, never from the hand counts in
        # phase-3-window-identity/MEASUREMENTS.md M9.4 -- those counted raw lines (including
        # `extern` declarations) and overstated the defect by ~25%. Same lesson as G2/G5.
        # ⚠️ Windows only: cef_browser_shell_mac.mm has a structurally different window model
        # and would produce a large baseline that hides the lines that matter (the G8 lesson).
        #
        # Baseline MEASURED BY THIS SCRIPT 2026-08-30: 60. ⭐ The hand count in M9.4 predicted
        # ~19 on the tab axis and did not account for the 18 static Get*Browser() accessors all
        # routing through GetPrimaryWindow(). Third time this sprint a hand count has been wrong
        # where the tool was right -- which is the whole reason HARNESS.md §9 requires this.
        # Phase 3 fixed the two REPORTED symptoms (Ctrl+F/Ctrl+L, fullscreen) but those sites
        # used GetHeaderBrowser()/g_is_fullscreen, not the two patterns matched here, so the
        # count is unchanged by Phase 3 -- that is expected, not a failure to fix anything.
        # (Phase 3.5 did not move it either -- HARNESS.md section 4 records why: its file and
        # its globals are outside this gate's paths and pattern.)
        #
        # 2026-09-13, beta.3 Phase 8c O8: 59, MEASURED BY THIS SCRIPT after the backup-overlay
        # deletion removed CreateBackupOverlayWithSeparateProcess()'s GetPrimaryWindow() lookup
        # in simple_app.cpp. Lowered in its own commit per HARNESS.md section 4 / working rule 6,
        # with -NegativeControl re-run.
        Baseline = 59
        Target   = 0
        Paths    = @('cef-native/src/handlers', 'cef-native/src/core')
        Include  = @('*.cpp', '__preflight_probe.cpp')
        Pattern  = '(GetPrimaryWindow *\(\)|TabManager::GetInstance\(\)\.GetActiveTab *\(\))'
        Probe    = 'auto* t = TabManager::GetInstance().GetActiveTab();'
        ProbeExt = '.cpp'
    },
    [pscustomobject]@{
        Id       = 'G12'
        Name     = 'Unanchored host:port URL matchers on the wallet trust boundary'
        Owner    = 'Phase 5'
        # G2's sibling. G2 owns the FRONTEND port (5137); this owns every other
        # host:port matcher -- our wallet port and the foreign bridge ports.
        #
        # Why it exists: an unanchored find() over the whole URL admits any URL that
        # merely CONTAINS the host:port, including in a query string the page author
        # controls. MEASURED 2026-09-02 in a running browser
        # (phase-5-loopback-routing/MEASUREMENTS.md M2): a page's own request to
        # example.com was rewritten, downgraded https->http, and answered by our
        # wallet, because the page put the text in its own query.
        #
        # ⛔ And the reason this is a TRUST gate rather than a tidiness gate: C++ is
        # what stamps X-Requesting-Domain, and Rust reads a MISSING header as
        # internal + fully trusted. A matcher that misses does not leave traffic
        # ungated, it leaves it TRUSTED. Correct form is a parsed authority --
        # hodos::IsWalletOrigin / IsOurWalletOrigin (include/core/PortConfig.h).
        #
        # Baseline 4, measured by THIS script 2026-09-02, all in PortConfig.h and all
        # deliberate: IsWalletHostPort (2 lines) and IsLoopbackHostPort (2 lines) are
        # kept ONLY to back hodos::LegacyWalletGateMatch, the W3 shadow predicate that
        # decides nothing and exists so the new gate's disagreements can be logged.
        # They are retired together in beta.4 (ticket W8), which is what drives this
        # to its target of 0. Residuals are named in
        # phase-5-loopback-routing/PHASE_CONTRACT.md section 6.
        Baseline = 4
        Target   = 0
        Paths    = @('cef-native/src', 'cef-native/include')
        Include  = @('*.cpp', '*.mm', '*.h')
        Pattern  = '(find|rfind)\("(http://)?(127\.0\.0\.1|localhost):'
        # A prefix check -- rfind(X, 0) == 0, or find(X) != 0 -- is the CORRECT form
        # and must not be flagged. Same rule as G2. 5137 belongs to G2; excluding it
        # here keeps one owner per defect and stops a single fix moving two baselines.
        Exclude  = '(rfind\([^)]*, *0\)|find\([^)]*\) *!= *0|:5137)'
        Probe    = 'bool probe = url.find("127.0.0.1:31301") != std::string::npos;'
        ProbeExt = '.cpp'
    }
)

# --------------------------------------------------------------------------------------
# Helpers
# --------------------------------------------------------------------------------------
function Get-GateHits {
    param([Parameter(Mandatory)]$Gate)
    $files = @()
    foreach ($p in $Gate.Paths) {
        $full = Join-Path $RepoRoot $p
        if (-not (Test-Path $full)) { continue }
        $files += Get-ChildItem -Path $full -Recurse -File -Include $Gate.Include -ErrorAction SilentlyContinue
    }
    if ($files.Count -eq 0) { return @() }
    $hits = @($files | Select-String -Pattern $Gate.Pattern -CaseSensitive)
    # Drop whole-line comments: a commented example is documentation, not a violation.
    $hits = @($hits | Where-Object { $_.Line -notmatch '^\s*(//|#)' })
    # Per-gate exclusion for forms that are correct by construction.
    if ($Gate.PSObject.Properties.Name -contains 'Exclude' -and $Gate.Exclude) {
        $hits = @($hits | Where-Object { $_.Line -notmatch $Gate.Exclude })
    }
    return ,$hits
}

$script:Results = New-Object System.Collections.Generic.List[object]

function Add-Result {
    param([string]$Id, [string]$Name, [string]$Status, [string]$Detail)
    $script:Results.Add([pscustomobject]@{ Id = $Id; Name = $Name; Status = $Status; Detail = $Detail })
    $colour = switch ($Status) {
        'PASS'    { 'Green' }
        'FAIL'    { 'Red' }
        'SKIPPED' { 'Yellow' }
        default   { 'Gray' }
    }
    Write-Host ("  [{0,-7}] {1,-4} {2}" -f $Status, $Id, $Name) -ForegroundColor $colour
    if ($Detail) { Write-Host ("            {0}" -f $Detail) -ForegroundColor DarkGray }
}

function Test-Selected {
    param([string]$Id)
    if (-not $Only) { return $true }
    return $Only -contains $Id
}

# --------------------------------------------------------------------------------------
# -NegativeControl : prove every gate can fail
# --------------------------------------------------------------------------------------
if ($NegativeControl) {
    Write-Host ''
    Write-Host 'PREFLIGHT - NEGATIVE CONTROL' -ForegroundColor Cyan
    Write-Host 'Proving each static gate reports MORE than its baseline when a violation is injected.' -ForegroundColor DarkGray
    Write-Host 'A gate that stays green here is blind, and so is anything it was supposed to protect.' -ForegroundColor DarkGray
    Write-Host ''

    foreach ($g in $Gates) {
        if (-not (Test-Selected $g.Id)) { continue }
        $scanDir = Join-Path $RepoRoot $g.Paths[0]
        if (-not (Test-Path $scanDir)) {
            Add-Result $g.Id $g.Name 'SKIPPED' "scan path missing: $($g.Paths[0])"
            continue
        }
        $probeFile = Join-Path $scanDir ('__preflight_probe' + $g.ProbeExt)
        if (Test-Path $probeFile) {
            Add-Result $g.Id $g.Name 'SKIPPED' "probe file already exists, refusing to overwrite: $probeFile"
            continue
        }
        try {
            Set-Content -Path $probeFile -Value $g.Probe -Encoding UTF8
            $n = (Get-GateHits -Gate $g).Count   # function returns a real array via ,$hits
            if ($n -gt $g.Baseline) {
                Add-Result $g.Id $g.Name 'PASS' "detected the injected violation ($n > baseline $($g.Baseline))"
            } else {
                Add-Result $g.Id $g.Name 'FAIL' "GATE IS BLIND: injected a violation and counted $n (baseline $($g.Baseline)). The pattern does not match its own probe."
            }
        } finally {
            if (Test-Path $probeFile) { Remove-Item $probeFile -Force }
        }
    }

    # T1e negative control (Phase 0.6): reintroduce the slice(8) scheme-strip trap in
    # the real bip21.ts / qr-scanner-logic.js and prove the GREEN assertion catches it
    # (a bsv: address then truncates or fails closed). The harness exits 0 when the trap
    # is caught -- i.e. when the test is NOT blind, which is the PASS this block wants.
    if (Test-Selected 'T1e') {
        $harness = Join-Path $RepoRoot 'development-docs/0.4.0-beta.3/phase-0.6-qr-bsv-uri/probes/qr_scheme_t1e.mjs'
        $lbl = 'QR scheme classifier trap (slice(8) reintroduced)'
        if (-not (Test-Path $harness)) {
            Add-Result 'T1e' $lbl 'SKIPPED' "harness missing: $harness"
        } elseif (-not (Get-Command node -ErrorAction SilentlyContinue)) {
            Add-Result 'T1e' $lbl 'SKIPPED' 'node not on PATH'
        } else {
            $prevEap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
            try { & node --experimental-strip-types $harness --negative-control 2>&1 | Out-String | Write-Verbose } finally { $ErrorActionPreference = $prevEap }
            if ($LASTEXITCODE -eq 0) { Add-Result 'T1e' $lbl 'PASS' 'bsv: truncates/fails-closed under slice(8); GREEN assertion is not blind' }
            else { Add-Result 'T1e' $lbl 'FAIL' "could not demonstrate the trap (exited $LASTEXITCODE)" }
        }
    }

    # T1f negative control (Phase 0.8): rewrite manifestConsent.ts so BRC-73's
    # monthly satoshis are read as a per-transaction USD cap -- the exact R-CAPS
    # breach -- and assert the A5 check SEES it. MEASURED 2026-08-22: with only the
    # 5,000,000-satoshi fixture this did NOT trip, because that value is above the
    # magnitude sanity cap and was rejected for the wrong reason. The harness now
    # also drives BRC-73's own 10,000 example, which isolates the unit rule.
    if (Test-Selected 'T1f') {
        $harness = Join-Path $RepoRoot 'development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/probes/manifest_consent_t1f.mjs'
        $lbl = 'NC: connect-modal consent rule (R-CAPS broken on purpose)'
        if (-not (Test-Path $harness)) {
            Add-Result 'T1f' $lbl 'SKIPPED' "harness missing: $harness"
        } elseif (-not (Get-Command node -ErrorAction SilentlyContinue)) {
            Add-Result 'T1f' $lbl 'SKIPPED' 'node not on PATH'
        } else {
            $prevEap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
            try { & node --experimental-strip-types $harness --negative-control 2>&1 | Out-String | Write-Verbose } finally { $ErrorActionPreference = $prevEap }
            if ($LASTEXITCODE -eq 0) { Add-Result 'T1f' $lbl 'PASS' 'a monthly-satoshis-as-cap regression is caught; the A5 check is not blind' }
            else { Add-Result 'T1f' $lbl 'FAIL' "could not demonstrate the breach (exited $LASTEXITCODE)" }
        }
    }

    # T1g negative control (Phase 0.8): restore the background-without-colour
    # override that SHIPPED in 8f3982c and assert the contrast check sees it.
    # MEASURED: the breach reads 1.08:1 -- near-white on near-white -- which is
    # exactly what the owner saw as an empty box on 2026-08-23.
    if (Test-Selected 'T1g') {
        $harness = Join-Path $RepoRoot 'development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/probes/limit_field_contrast_t1g.mjs'
        $lbl = 'NC: limit-field contrast (invisible-value defect restored)'
        if (-not (Test-Path $harness)) {
            Add-Result 'T1g' $lbl 'SKIPPED' "harness missing: $harness"
        } elseif (-not (Get-Command node -ErrorAction SilentlyContinue)) {
            Add-Result 'T1g' $lbl 'SKIPPED' 'node not on PATH'
        } else {
            $prevEap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
            try { & node $harness --negative-control 2>&1 | Out-String | Write-Verbose } finally { $ErrorActionPreference = $prevEap }
            if ($LASTEXITCODE -eq 0) { Add-Result 'T1g' $lbl 'PASS' 'an unreadable consent value is caught; the contrast check is not blind' }
            else { Add-Result 'T1g' $lbl 'FAIL' "could not demonstrate the breach (exited $LASTEXITCODE)" }
        }
    }

    Write-Host ''
    if ($script:Results.Count -eq 0) {
        Write-Host 'NEGATIVE CONTROL: INCOMPLETE - ZERO gates exercised. This is NOT a pass.' -ForegroundColor Yellow
        exit 2
    }
    $ncFailed  = @($script:Results | Where-Object { $_.Status -eq 'FAIL' }).Count
    $ncSkipped = @($script:Results | Where-Object { $_.Status -eq 'SKIPPED' }).Count
    if ($ncFailed -gt 0) {
        Write-Host "NEGATIVE CONTROL: FAIL - $ncFailed gate(s) could not detect their own probe." -ForegroundColor Red
        exit 1
    }
    if ($ncSkipped -gt 0) {
        Write-Host "NEGATIVE CONTROL: INCOMPLETE - $ncSkipped skipped." -ForegroundColor Yellow
        exit 2
    }
    Write-Host 'NEGATIVE CONTROL: PASS - every gate was seen to fail.' -ForegroundColor Green
    exit 0
}

# --------------------------------------------------------------------------------------
# Normal run: T0 then T1
# --------------------------------------------------------------------------------------
Write-Host ''
Write-Host 'PREFLIGHT - T0 static gates' -ForegroundColor Cyan
Write-Host 'CI note: test.yml/ci.yml execute nothing until the Actions quota resets (~2026-09-01).' -ForegroundColor DarkGray
Write-Host ''

foreach ($g in $Gates) {
    if (-not (Test-Selected $g.Id)) { continue }
    $hits = Get-GateHits -Gate $g        # already an array (,$hits) -- do NOT re-wrap in @(), that nests
    $n = $hits.Count
    if ($n -gt $g.Baseline) {
        $sample = $hits | Select-Object -First 5 | ForEach-Object {
            "$($_.Path -replace [regex]::Escape($RepoRoot + [System.IO.Path]::DirectorySeparatorChar), ''):$($_.LineNumber)"
        }
        Add-Result $g.Id $g.Name 'FAIL' "$n violations, over baseline $($g.Baseline) [$($g.Owner), target $($g.Target)]. e.g. $($sample -join '; ')"
    } elseif ($n -lt $g.Baseline) {
        Add-Result $g.Id $g.Name 'PASS' "$n violations, BELOW baseline $($g.Baseline) - lower the baseline in HARNESS.md section 4 [$($g.Owner), target $($g.Target)]"
    } else {
        Add-Result $g.Id $g.Name 'PASS' "$n violations, at baseline [$($g.Owner), target $($g.Target)]"
    }
}

Write-Host ''
Write-Host 'PREFLIGHT - T1 unit tests' -ForegroundColor Cyan
Write-Host ''

function Invoke-CargoTest {
    param([string]$Id, [string]$Manifest, [string]$Label)
    if (-not (Test-Selected $Id)) { return }
    $path = Join-Path $RepoRoot $Manifest
    if (-not (Test-Path $path)) { Add-Result $Id $Label 'SKIPPED' "no manifest at $Manifest"; return }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { Add-Result $Id $Label 'SKIPPED' 'cargo not on PATH'; return }
    # cargo writes warnings to stderr. With EAP=Stop that becomes a terminating
    # NativeCommandError and the run dies on a *warning* -- so judge by exit code only.
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { & cargo test --manifest-path $path --quiet 2>&1 | Out-String | Write-Verbose }
    finally { $ErrorActionPreference = $prevEap }
    if ($LASTEXITCODE -eq 0) { Add-Result $Id $Label 'PASS' '' }
    else { Add-Result $Id $Label 'FAIL' "cargo test exited $LASTEXITCODE - re-run with -Verbose for output" }
}

Invoke-CargoTest -Id 'T1a' -Manifest 'rust-wallet/Cargo.toml'    -Label 'cargo test - rust-wallet'
Invoke-CargoTest -Id 'T1b' -Manifest 'adblock-engine/Cargo.toml' -Label 'cargo test - adblock-engine'

if (Test-Selected 'T1c') {
    # CMake puts the target wherever RUNTIME_OUTPUT_DIRECTORY says -- measured 2026-08-18 it
    # lands in bin/Release, NOT tests/Release as cef-native/tests/CMakeLists.txt's header
    # comment claims. Probe the real candidates rather than trusting one hard-coded path:
    # a wrong path here reads as "not built" and SKIPS, which is a silent loss of coverage.
    $exe = @(
        'cef-native/build/bin/Release/hodos_tests.exe',
        'cef-native/build/tests/Release/hodos_tests.exe',
        'cef-native/build/bin/hodos_tests.exe',
        'cef-native/build/tests/hodos_tests'
    ) | ForEach-Object { Join-Path $RepoRoot $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
    if ($exe) {
        $prevEap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
        try { & $exe 2>&1 | Out-String | Write-Verbose } finally { $ErrorActionPreference = $prevEap }
        if ($LASTEXITCODE -eq 0) { Add-Result 'T1c' 'hodos_tests (C++)' 'PASS' '' }
        else { Add-Result 'T1c' 'hodos_tests (C++)' 'FAIL' "exited $LASTEXITCODE" }
    } else {
        Add-Result 'T1c' 'hodos_tests (C++)' 'SKIPPED' 'not built: cmake -S cef-native -B cef-native/build -DHODOS_BUILD_TESTS=ON; cmake --build cef-native/build --config Release --target hodos_tests'
    }
}

if (Test-Selected 'T1d') {
    if (-not $Full) {
        Add-Result 'T1d' 'frontend build (tsc -b, vite build)' 'SKIPPED' 'not requested - pass -Full'
    } elseif (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
        Add-Result 'T1d' 'frontend build (tsc -b, vite build)' 'SKIPPED' 'npm not on PATH'
    } else {
        Push-Location (Join-Path $RepoRoot 'frontend')
        try {
            $prevEap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
            try { & npm run build 2>&1 | Out-String | Write-Verbose } finally { $ErrorActionPreference = $prevEap }
            if ($LASTEXITCODE -eq 0) { Add-Result 'T1d' 'frontend build (tsc -b, vite build)' 'PASS' '' }
            else { Add-Result 'T1d' 'frontend build (tsc -b, vite build)' 'FAIL' "npm run build exited $LASTEXITCODE" }
        } finally { Pop-Location }
    }
}

# T1e: QR scheme classifier (Phase 0.6). Exercises the REAL shipped sources --
# frontend/src/utils/bip21.ts (Node type-stripping) and
# cef-native/build_tools/qr-scanner-logic.js (real IIFE in a vm sandbox) -- and
# asserts on the extracted ADDRESS STRING, not scan success. Its --negative-control
# mode (run only under -NegativeControl above) reintroduces the slice(8) trap.
if (Test-Selected 'T1e') {
    $harness = Join-Path $RepoRoot 'development-docs/0.4.0-beta.3/phase-0.6-qr-bsv-uri/probes/qr_scheme_t1e.mjs'
    $lbl = 'QR scheme classifier (bip21.ts + qr-scanner-logic.js)'
    if (-not (Test-Path $harness)) {
        Add-Result 'T1e' $lbl 'SKIPPED' "harness missing: $harness"
    } elseif (-not (Get-Command node -ErrorAction SilentlyContinue)) {
        Add-Result 'T1e' $lbl 'SKIPPED' 'node not on PATH'
    } else {
        $prevEap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
        try { & node --experimental-strip-types $harness 2>&1 | Out-String | Write-Verbose } finally { $ErrorActionPreference = $prevEap }
        if ($LASTEXITCODE -eq 0) { Add-Result 'T1e' $lbl 'PASS' '' }
        else { Add-Result 'T1e' $lbl 'FAIL' "harness exited $LASTEXITCODE - re-run with -Verbose" }
    }
}

# T1f: connect-modal consent rule (Phase 0.8). Exercises the REAL shipped source --
# frontend/src/utils/manifestConsent.ts (Node type-stripping) -- and asserts on WHOSE
# NUMBER lands in each limit field (`sourceOf`), not merely that a value came back.
# Its --negative-control mode (run only under -NegativeControl above) rewrites the real
# source so BRC-73's monthly satoshis are treated as a per-transaction USD cap, and
# proves the A5 assertion catches it.
if (Test-Selected 'T1f') {
    $harness = Join-Path $RepoRoot 'development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/probes/manifest_consent_t1f.mjs'
    $lbl = 'connect-modal consent rule (manifestConsent.ts)'
    if (-not (Test-Path $harness)) {
        Add-Result 'T1f' $lbl 'SKIPPED' "harness missing: $harness"
    } elseif (-not (Get-Command node -ErrorAction SilentlyContinue)) {
        Add-Result 'T1f' $lbl 'SKIPPED' 'node not on PATH'
    } else {
        $prevEap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
        try { & node --experimental-strip-types $harness 2>&1 | Out-String | Write-Verbose } finally { $ErrorActionPreference = $prevEap }
        if ($LASTEXITCODE -eq 0) { Add-Result 'T1f' $lbl 'PASS' '' }
        else { Add-Result 'T1f' $lbl 'FAIL' "harness exited $LASTEXITCODE - re-run with -Verbose" }
    }
}

# T1g: limit-field contrast (Phase 0.8). The consent screen's spending caps must be
# READABLE, in both provenance states. Guards the surface T1f cannot see: round 2
# shipped a marked-field style that put near-white text on a near-white box (1.07:1),
# hiding the very number the site had chosen. Reads the real .tsx and runs the real
# WCAG relative-luminance formula -- not a string match, which any restyle defeats.
if (Test-Selected 'T1g') {
    $harness = Join-Path $RepoRoot 'development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/probes/limit_field_contrast_t1g.mjs'
    $lbl = 'limit-field contrast (BRC100AuthOverlayRoot.tsx)'
    if (-not (Test-Path $harness)) {
        Add-Result 'T1g' $lbl 'SKIPPED' "harness missing: $harness"
    } elseif (-not (Get-Command node -ErrorAction SilentlyContinue)) {
        Add-Result 'T1g' $lbl 'SKIPPED' 'node not on PATH'
    } else {
        $prevEap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
        try { & node $harness 2>&1 | Out-String | Write-Verbose } finally { $ErrorActionPreference = $prevEap }
        if ($LASTEXITCODE -eq 0) { Add-Result 'T1g' $lbl 'PASS' '' }
        else { Add-Result 'T1g' $lbl 'FAIL' "harness exited $LASTEXITCODE - re-run with -Verbose" }
    }
}

# --------------------------------------------------------------------------------------
# Verdict. A skip is never a pass.
# --------------------------------------------------------------------------------------
$failed  = @($script:Results | Where-Object { $_.Status -eq 'FAIL' })
$skipped = @($script:Results | Where-Object { $_.Status -eq 'SKIPPED' })

Write-Host ''
Write-Host ('-' * 78)
# A run that executed nothing is the single most dangerous "green" this project can
# produce -- it is the shape of all four farbling harnesses. Never call it a pass.
if ($script:Results.Count -eq 0) {
    Write-Host 'PREFLIGHT: INCOMPLETE - ZERO checks executed. This is NOT a pass.' -ForegroundColor Yellow
    Write-Host 'Most likely an -Only filter that matched nothing. Run with no -Only to see the full set.' -ForegroundColor Yellow
    exit 2
}
if ($failed.Count -gt 0) {
    Write-Host "PREFLIGHT: FAIL - $($failed.Count) check(s) failed, $($skipped.Count) skipped." -ForegroundColor Red
    exit 1
}
if ($skipped.Count -gt 0) {
    Write-Host "PREFLIGHT: INCOMPLETE - 0 failed, $($skipped.Count) skipped. This is NOT a pass." -ForegroundColor Yellow
    Write-Host 'Record it as INCOMPLETE in the phase contract sign-off table.' -ForegroundColor Yellow
    exit 2
}
Write-Host 'PREFLIGHT: PASS - all checks ran and passed.' -ForegroundColor Green
Write-Host 'Reminder: a green preflight is T0+T1 only. It says nothing about T2/T3.' -ForegroundColor DarkGray
exit 0
