#!/usr/bin/env python3
"""Sign a Hodos appcast document for the silent auto-updater (WINDOWS_AUTOUPDATE_PLAN
commit 4c — whole-appcast-document signature / anti-replay).

Produces a DETACHED Ed25519 signature over (PREFIX || appcast-bytes) and writes
it base64-encoded to a sidecar file (default: <in>.ed). The client
(UpdateStager::VerifyAppcastDocument) fetches both appcast.xml and
appcast.xml.ed and verifies the signature over the same prefix||body BEFORE
parsing any item — so a tampered or replayed feed never reaches the parser.

The PREFIX is a domain-separation tag that MUST stay byte-identical to
UpdateStager::AppcastSignaturePrefix() in the C++ client. Tagging only the
appcast (the installer stays raw, for Sparkle/winsparkle-tool compatibility)
keeps the two signing domains provably disjoint.

Shells out to `openssl` (Ed25519) so there is no extra Python dependency —
matches how the localhost test rig and the macOS/Windows installer signing work.

Usage:
    python3 sign-appcast.py --in appcast.xml --key ed25519_priv.pem [--out appcast.xml.ed]

The key must be an Ed25519 private key in PEM (PKCS#8). The localhost rig
generates one with `openssl genpkey -algorithm ed25519`; CI converts the
Sparkle Ed25519 key to PEM first.
"""

import argparse
import base64
import os
import subprocess
import sys
import tempfile

# MUST match UpdateStager::AppcastSignaturePrefix() exactly (bytes, incl. the \n).
APPCAST_SIGNATURE_PREFIX = b"hodos-appcast-v1\n"


def main():
    ap = argparse.ArgumentParser(description="Detached Ed25519 signature for a Hodos appcast")
    ap.add_argument("--in", dest="infile", required=True, help="appcast.xml to sign")
    ap.add_argument("--key", required=True, help="Ed25519 private key (PEM / PKCS#8)")
    ap.add_argument("--out", help="sidecar output path (default: <in>.ed)")
    args = ap.parse_args()
    out_path = args.out or (args.infile + ".ed")

    with open(args.infile, "rb") as f:
        body = f.read()
    message = APPCAST_SIGNATURE_PREFIX + body

    msg_path = None
    sig_path = None
    try:
        with tempfile.NamedTemporaryFile(delete=False) as mt:
            mt.write(message)
            msg_path = mt.name
        sig_path = msg_path + ".sig"
        # Ed25519 is one-shot over the raw message → -rawin (no pre-hash).
        subprocess.run(
            ["openssl", "pkeyutl", "-sign", "-rawin", "-inkey", args.key,
             "-in", msg_path, "-out", sig_path],
            check=True,
        )
        with open(sig_path, "rb") as f:
            sig = f.read()
    finally:
        for p in (msg_path, sig_path):
            if p and os.path.exists(p):
                os.unlink(p)

    if len(sig) != 64:
        sys.exit(f"ERROR: expected a 64-byte Ed25519 signature, got {len(sig)} bytes")

    with open(out_path, "w", newline="") as f:
        f.write(base64.b64encode(sig).decode("ascii"))
    print(f"Wrote appcast signature sidecar: {out_path}")


if __name__ == "__main__":
    main()
