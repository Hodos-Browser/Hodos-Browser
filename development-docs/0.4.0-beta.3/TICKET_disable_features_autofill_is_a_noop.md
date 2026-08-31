# `--disable-features=Autofill` is a no-op

**Status:** ❔ **NEEDS OWNER REVIEW** — `simple_app.cpp` DOES append `disable-features=Autofill,…`; whether it is still a no-op needs a runtime check. Labelled 2026-08-31 by the triage pass; see `TICKET_TRIAGE_2026-08-31.md`.

**Opened 2026-08-24** (found during the Phase 0.9 Group B research). **Status:** 🔵 OPEN.
Low severity, but it means a stated privacy control is not in effect.

## The defect

`cef-native/src/handlers/simple_app.cpp :: OnBeforeCommandLineProcessing` passes:

```cpp
command_line->AppendSwitchWithValue("disable-features", "Autofill,AutofillServerCommunication,GlicActorUi");
```

**There is no Chromium feature named `Autofill`.** `grep -rn "BASE_FEATURE(kAutofill," ` across
`chromium/src` returns nothing. Chromium derives a feature's name from its variable, so a feature
called `Autofill` would require `BASE_FEATURE(kAutofill, ...)`, which does not exist. The token is
silently ignored.

The other two are real and do work:

- `features::debug::kAutofillServerCommunication` — `autofill_crowdsourcing_manager.cc:143`
- `kGlicActorUi` — `chrome_features.cc:242`. ⛔ This one is a **hard crash fix** for CEF 150 — do not
  drop it while tidying.

## Why it matters

The comment above the switch says it disables "Chromium's built-in autofill". It does not.
Chromium's autofill and password-manager infrastructure is live — which is also why the
save-password bubble machinery is attached at all (Phase 0.9 contract §8).

⚠️ **Unmeasured:** whether any of this is user-visible in Hodos today. Someone should check whether
autofill dropdowns or a save-password bubble actually appear before deciding what the switch *should*
say. Do not assume either way — assuming is how this survived.

## Fix

Decide what was actually intended, name real features, and correct the comment. Or delete the token
and the claim. ⛔ Keep `GlicActorUi` regardless.

## Negative control

Whatever real feature name replaces it: confirm the behaviour is **present** with the switch removed
and **absent** with it applied. A switch naming a non-existent feature passes silently and changes
nothing — which is exactly how this went unnoticed.
