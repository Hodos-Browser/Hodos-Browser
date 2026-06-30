#!/usr/bin/env python3
"""Generate + sign the expected-new-manifest for the Windows silent auto-updater
(WINDOWS_AUTOUPDATE_PLAN commit 6b.3 / AUTOUPDATE_6B_SUPERVISOR_DESIGN.md §9 v3, B4/V3-8).

Walks the **post-Authenticode-signed** staging tree and emits a content manifest
({relative-path -> sha256-hex}) plus a DETACHED Ed25519 signature sidecar. At apply
time the supervisor (hodos-update-helper.exe) verifies the sidecar signature with the
EMBEDDED key, then checks every installed {app} file's sha256 against the manifest —
catching a power-loss-truncated install, version-skew, or a swapped binary that the
verified installer alone wouldn't.

⚠️ ORDERING IS LOAD-BEARING: run this AFTER the staging exes/DLLs are Azure-signed.
Hashing pre-signing produces hashes that never match the installed (signed) files →
the gate fails on EVERY apply → fleet-wide pause. (Same discipline as §H.2 ZIP signing.)

Manifest JSON shape MUST match hodos::FileManifest (UpdateApply.cpp::SerializeManifest):
    {"schema":1,"files":{ "<normkey>":"<sha256hex>", ... }}
Keys MUST match hodos::NormalizeManifestKey: backslashes->forward slashes, lower-cased,
leading "./" / "/" stripped. The signature prefix MUST equal
hodos::updatefs::ManifestSignaturePrefix() byte-for-byte.

Usage:
    python3 generate-tree-manifest.py --staging staging/HodosBrowser \
        --out dist/expected-new-manifest.json --key ed25519_priv.pem
(writes <out> and <out>.ed). The manifest is written OUTSIDE the staging tree so it
never references itself.
"""

import argparse
import base64
import hashlib
import json
import os
import subprocess
import sys
import tempfile

# MUST equal hodos::updatefs::ManifestSignaturePrefix() exactly (bytes, incl. the \n).
MANIFEST_SIGNATURE_PREFIX = b"hodos-manifest-v1\n"


def normkey(rel: str) -> str:
    # Mirror hodos::NormalizeManifestKey: \ -> /, lowercase, strip leading ./ and /.
    s = rel.replace("\\", "/").lower()
    while s.startswith("./"):
        s = s[2:]
    while s.startswith("/"):
        s = s[1:]
    return s


def sha256_file(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


# Mirrors hodos-browser.iss [Files] EXACTLY — the manifest must describe what Inno
# installs, not a superset (an extra file => false "missing" => false rollback on a
# good build). Root level installs only these extensions (non-recursive globs); the
# locales\ and frontend\ subdirs install recursively. ⚠️ If [Files] changes, change this.
_ROOT_INSTALL_EXTS = {".exe", ".dll", ".bin", ".dat", ".pak", ".json"}
_RECURSIVE_DIRS = {"locales", "frontend"}


def _is_installed(rel: str) -> bool:
    norm = rel.replace("\\", "/")
    if "/" not in norm:  # root-level file
        return os.path.splitext(norm)[1].lower() in _ROOT_INSTALL_EXTS
    top = norm.split("/", 1)[0].lower()
    return top in _RECURSIVE_DIRS  # everything under locales\ / frontend\


def build_manifest(staging: str, build_number: int) -> dict:
    files = {}
    for root, _dirs, names in os.walk(staging):
        for name in names:
            full = os.path.join(root, name)
            rel = os.path.relpath(full, staging)
            if not _is_installed(rel):
                continue  # not in [Files] -> not installed -> not in the manifest
            files[normkey(rel)] = sha256_file(full)
    # buildNumber is bound into the SIGNED bytes so apply-time anti-rollback can trust
    # it instead of the plaintext marker (review #2). Key order matches the C++
    # SerializeManifest output for human-diff parity (not required for verification —
    # the client hashes the bytes we write). Deterministic file ordering.
    return {"schema": 1, "buildNumber": build_number, "files": dict(sorted(files.items()))}


def sign_detached(message: bytes, key_pem: str) -> bytes:
    msg_path = sig_path = None
    try:
        with tempfile.NamedTemporaryFile(delete=False) as mt:
            mt.write(message)
            msg_path = mt.name
        sig_path = msg_path + ".sig"
        subprocess.run(
            ["openssl", "pkeyutl", "-sign", "-rawin", "-inkey", key_pem,
             "-in", msg_path, "-out", sig_path],
            check=True,
        )
        with open(sig_path, "rb") as f:
            return f.read()
    finally:
        for p in (msg_path, sig_path):
            if p and os.path.exists(p):
                os.unlink(p)


def main():
    ap = argparse.ArgumentParser(description="Generate + sign the expected-new-manifest")
    ap.add_argument("--staging", required=True, help="post-signed staging tree (e.g. staging/HodosBrowser)")
    ap.add_argument("--out", required=True, help="manifest output path (written OUTSIDE staging)")
    ap.add_argument("--key", required=True, help="Ed25519 private key (PEM / PKCS#8) — same key as the appcast")
    ap.add_argument("--build-number", type=int, required=True,
                    help="monotonic integer build number (bound into the signed manifest for anti-rollback)")
    args = ap.parse_args()
    if args.build_number <= 0:
        sys.exit("ERROR: --build-number must be a positive integer")

    if not os.path.isdir(args.staging):
        sys.exit(f"ERROR: staging dir not found: {args.staging}")
    out_abs = os.path.abspath(args.out)
    staging_abs = os.path.abspath(args.staging)
    if out_abs.startswith(staging_abs + os.sep):
        sys.exit("ERROR: --out must be OUTSIDE --staging (else the manifest references itself)")

    manifest = build_manifest(args.staging, args.build_number)
    # Compact, stable bytes — what we sign + what the client re-reads. Match the
    # C++ json.dump(2) only in *content*; the client hashes the BYTES we write, so
    # write once and sign the exact same bytes.
    body = json.dumps(manifest, indent=2).encode("utf-8")

    os.makedirs(os.path.dirname(out_abs) or ".", exist_ok=True)
    with open(out_abs, "wb") as f:
        f.write(body)

    sig = sign_detached(MANIFEST_SIGNATURE_PREFIX + body, args.key)
    if len(sig) != 64:
        sys.exit(f"ERROR: expected a 64-byte Ed25519 signature, got {len(sig)}")
    with open(out_abs + ".ed", "w", newline="") as f:
        f.write(base64.b64encode(sig).decode("ascii"))

    print(f"Wrote {out_abs} ({len(manifest['files'])} files) + {out_abs}.ed")


if __name__ == "__main__":
    main()
