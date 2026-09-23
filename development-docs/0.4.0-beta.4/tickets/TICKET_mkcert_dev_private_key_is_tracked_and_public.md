# TICKET — an mkcert dev private key is tracked in git and public on the release repo

**Filed:** 2026-09-23, during the `v0.4.0-beta.3` pre-tag review. **Noted, not chased** — owner's
standing rule. **Severity:** 🟡 low, but non-zero and trivially fixable.

## What is there

```
tmp/smoke-5b/hodos-test.local-key.pem     <- PRIVATE key, tracked
tmp/smoke-5b/hodos-test.local.pem         <- its certificate
```

📏 `openssl x509 -noout -subject -issuer`:

```
subject= O = mkcert development certificate, OU = ARCHBOLD\archb@Archbold (Matt Archbold)
issuer = O = mkcert development CA,          OU = ARCHBOLD\archb@Archbold (Matt Archbold)
notBefore = Jun  1 2026   notAfter = Sep  1 2028
```

Added by `3a1df7b` with the 5.b payment-demo fixture. ⚠️ Already on `release/main`, i.e. **already
public** — this is not a new exposure created by the beta.3 propagation, and it was checked before
that push rather than after.

## Why it is low, and why it is not zero

⛔ **Not** signing material. Not the WinSparkle DSA key, not the Sparkle EdDSA key, not the Azure
Trusted Signing cert — those live in `external/keys/`, which **is** gitignored (verified).

The exposure is bounded to **machines that trust the owner's local mkcert CA**, i.e. his own. Anyone
holding this key could present a valid-looking `hodos-test.local` to those machines. Real, tiny, and
entirely local.

⚠️ The part that is worth fixing anyway: a tracked `*-key.pem` is a shape that teaches the wrong habit
and that a future secret-scanner will flag on every run, training people to ignore it.

## The fix, when someone is in here anyway

1. Regenerate the fixture cert locally; do not commit the new one.
2. `git rm --cached` the two files, add `*-key.pem` / `tmp/` to `.gitignore`, and have the smoke
   fixture generate its cert on demand (mkcert is already a dev prerequisite).
3. ⛔ **Do not rewrite history for this.** The key is 4 months public on a public repo; a force-push
   across `origin` + `release` + every open branch costs far more than rotating a local dev cert,
   and does not un-publish anything. Regenerate, don't rewrite.

## First step

Check whether `tmp/smoke-5b/` is still used by any live fixture — if the 5.b demo has moved on, the
whole directory may just be deletable, which makes steps 1–2 moot.
