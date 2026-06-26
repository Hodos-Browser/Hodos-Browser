#!/usr/bin/env python3
"""
Cryptographically verify the auto-update signatures in a generated appcast
against the REAL release artifacts, using the SAME public keys the shipped
client embeds. Fail-closed: any mandatory verification failure exits non-zero
BEFORE the release is promoted.

Why this exists (T4(a), Track-0 Pillar A): the publish job previously only
*grepped* that signatures were present/non-empty in the appcast XML — which
passes while the update is cryptographically broken. This step proves the
signatures actually verify, so a mis-signed feed can never be promoted.

Verification matrix (matches release.yml's signing steps + the client today):
  - Windows DSA  (MANDATORY): the load-bearing Windows signature. Inverts the
                 WinSparkle "double SHA1" sign:
                   openssl dgst -sha1 -binary < installer | openssl dgst -sha1 -sign
                 -> verify with the DSA public key embedded in AutoUpdater.cpp.
  - macOS EdDSA  (MANDATORY): Sparkle 2's Ed25519 signature over the whole DMG.
                 -> verify with SUPublicEDKey from Info.plist.
  - Windows EdDSA (CONDITIONAL today, MANDATORY at the client cutover): verified
                 if present. While the shipped client is WinSparkle 0.8.1 (DSA), its
                 absence is allowed with a loud tripwire. When the client flips to
                 EdDSA-only (release.yml "commit 3b"), CI must pass
                 --require-windows-eddsa to make it mandatory (else a DSA-only feed
                 would pass here yet be rejected by every field client).

Public keys are additionally PINNED: the key read from source must equal a
checked-in expected value, so an accidental/co-rotated signing-key swap fails
closed instead of passing a self-consistent green check (a rotation must
deliberately update the pin in a reviewed commit).

Anti-no-op guarantee (the important part): a verifier that *silently always
passes* is indistinguishable from a real one on a green CI run. So for EVERY
signature we verify, we ALSO flip a byte and assert the verification now FAILS.
If a corrupted signature still "verifies", this script hard-fails — proving the
verify has teeth on this exact run. This mutation self-test stays in CI forever
as a regression guard.

Keys are read FROM SOURCE at runtime (AutoUpdater.cpp, Info.plist) so there is
never a second, drift-prone copy of a public key in CI config.

Dependencies: python3 + openssl 3.x (Ed25519 -rawin). Both ship on the
ubuntu-24.04 publish runner. No pip installs.

Exit codes: 0 = all mandatory verifications + self-tests passed. Non-zero = a
mandatory signature failed to verify, a corrupted signature wrongly verified
(no-op verifier), or an input was missing/malformed.
"""

import argparse
import base64
import hashlib
import os
import re
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

SPARKLE_NS = "http://www.andymatuschak.org/xml-namespaces/sparkle"
# Fixed DER prefix for an Ed25519 SubjectPublicKeyInfo wrapping a 32-byte raw key.
ED25519_SPKI_PREFIX = bytes.fromhex("302a300506032b6570032100")

# Pinned, known-good DEPLOYED public keys. The keys are still read from source at
# runtime (no drift-prone duplicate of the key material), but we ALSO assert the
# extracted key equals these pins. Effect: a signing-key rotation must DELIBERATELY
# update these constants in a separately-reviewed commit. An accidental or co-rotated
# key swap (source key changed in the same release that re-signs with the new key)
# then fails closed here, instead of passing a self-consistent green check while
# breaking auto-update for every ALREADY-INSTALLED client (the "forced reinstall"
# worst case). Mirrors the EXPECTED_PUB pin already used by release.yml's EdDSA sign step.
EXPECTED_DSA_PUB_DER_SHA256 = "b6fa23c04b13044f0db4c949121b7c6a8b54a545204e31bbf359d3cd403a814c"
EXPECTED_SUPUBLIC_ED_KEY_B64 = "GVq3mpDl8eelsG0A5wqC5FBYZd3fy7U3we9iZ9+Tq3Q="


def die(msg):
    print(f"::error::{msg}")
    sys.exit(1)


def run(cmd, *, stdin_bytes=None):
    """Run a command, return (returncode, combined_output_str)."""
    p = subprocess.run(
        cmd, input=stdin_bytes,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
    )
    return p.returncode, p.stdout.decode("utf-8", "replace").strip()


# ---------- key extraction (from source) ----------

def extract_dsa_pub_pem(autoupdater_cpp, out_path):
    """Reconstruct the DSA public-key PEM from the DSA_PUB_KEY C-string literal."""
    src = open(autoupdater_cpp, encoding="utf-8").read()
    m = re.search(r"DSA_PUB_KEY\s*=\s*(.*?);", src, re.S)
    if not m:
        die(f"DSA_PUB_KEY definition not found in {autoupdater_cpp}")
    # Concatenate every C string literal in the initializer, then interpret \n.
    parts = re.findall(r'"((?:[^"\\]|\\.)*)"', m.group(1))
    if not parts:
        die("DSA_PUB_KEY initializer had no string literals")
    pem = "".join(parts).encode().decode("unicode_escape")
    if "BEGIN PUBLIC KEY" not in pem or "END PUBLIC KEY" not in pem:
        die("reconstructed DSA PEM is missing its BEGIN/END markers")
    with open(out_path, "w", newline="\n") as f:
        f.write(pem if pem.endswith("\n") else pem + "\n")
    rc, out = run(["openssl", "pkey", "-pubin", "-in", out_path, "-noout"])
    if rc != 0:
        die(f"extracted DSA public key does not load in openssl: {out}")
    # Pin: the extracted key MUST equal the known-good deployed key (see the constants).
    der = subprocess.run(["openssl", "pkey", "-pubin", "-in", out_path, "-outform", "DER"],
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if der.returncode != 0:
        die(f"could not DER-encode DSA public key: {der.stderr.decode('utf-8', 'replace')}")
    got = hashlib.sha256(der.stdout).hexdigest()
    if got != EXPECTED_DSA_PUB_DER_SHA256:
        die(f"DSA public key in {autoupdater_cpp} (sha256 {got}) does not match the pinned "
            f"deployed key ({EXPECTED_DSA_PUB_DER_SHA256}). If this is a DELIBERATE key "
            f"rotation, update EXPECTED_DSA_PUB_DER_SHA256 in a separately-reviewed commit; "
            f"otherwise an unexpected key change would break auto-update for installed clients.")
    print(f"  DSA public key extracted from {autoupdater_cpp}, loads OK, matches pinned key")


def extract_ed25519_pub_pem(info_plist, out_path):
    """Build an Ed25519 public-key PEM from the SUPublicEDKey in Info.plist."""
    txt = open(info_plist, encoding="utf-8").read()
    m = re.search(r"<key>SUPublicEDKey</key>\s*<string>([^<]+)</string>", txt)
    if not m:
        die(f"SUPublicEDKey not found in {info_plist}")
    b64 = m.group(1).strip()
    # Pin: a co-rotated Ed25519 key fails closed unless the constant is deliberately updated.
    if b64 != EXPECTED_SUPUBLIC_ED_KEY_B64:
        die(f"SUPublicEDKey in {info_plist} ({b64}) does not match the pinned deployed key "
            f"({EXPECTED_SUPUBLIC_ED_KEY_B64}). If this is a DELIBERATE key rotation, update "
            f"EXPECTED_SUPUBLIC_ED_KEY_B64 in a separately-reviewed commit.")
    raw = base64.b64decode(b64)
    if len(raw) != 32:
        die(f"SUPublicEDKey is {len(raw)} bytes, expected 32 (raw Ed25519)")
    der = ED25519_SPKI_PREFIX + raw
    der_path = out_path + ".der"
    with open(der_path, "wb") as f:
        f.write(der)
    rc, out = run(["openssl", "pkey", "-pubin", "-inform", "DER",
                   "-in", der_path, "-out", out_path])
    if rc != 0:
        die(f"failed to build Ed25519 public key from SUPublicEDKey: {out}")
    print(f"  Ed25519 public key built from SUPublicEDKey ({info_plist})")


# ---------- appcast parsing ----------

def parse_appcast(appcast_path):
    """Return {'windows': {...attrs}, 'macos': {...attrs}} from enclosure tags."""
    tree = ET.parse(appcast_path)
    out = {}
    for item in tree.getroot().iter("item"):
        os_el = item.find(f"{{{SPARKLE_NS}}}os")
        enc = item.find("enclosure")
        if os_el is None or enc is None:
            continue
        out[os_el.text.strip()] = {
            "url": enc.get("url", ""),
            "dsa": enc.get(f"{{{SPARKLE_NS}}}dsaSignature"),
            "ed": enc.get(f"{{{SPARKLE_NS}}}edSignature"),
        }
    return out


# ---------- verification primitives ----------

def _write_sig(b64, tmpdir, name):
    raw = base64.b64decode(b64)
    path = os.path.join(tmpdir, name)
    with open(path, "wb") as f:
        f.write(raw)
    return path, raw


def verify_dsa(installer, dsa_pub_pem, sig_b64, tmpdir):
    """True iff the WinSparkle double-SHA1 DSA signature verifies (openssl rc authoritative)."""
    sig_path, _ = _write_sig(sig_b64, tmpdir, "dsa.sig")
    with open(installer, "rb") as fh:
        p = subprocess.run(["openssl", "dgst", "-sha1", "-binary"], stdin=fh,
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if p.returncode != 0:
        die(f"openssl could not SHA1-hash the installer: {p.stderr.decode('utf-8', 'replace')}")
    rc, out = run(
        ["openssl", "dgst", "-sha1", "-verify", dsa_pub_pem, "-signature", sig_path],
        stdin_bytes=p.stdout,
    )
    # openssl exit code is authoritative (0 = verified, 1 = failed). The success string is
    # only a secondary sanity log, so a cosmetic openssl text change can't false-FAIL a real
    # release. The mutation self-test guarantees a corrupted sig still drives rc != 0.
    if rc == 0 and "Verified OK" not in out:
        print(f"::warning::DSA verify returned success without the expected text: {out}")
    return rc == 0


def verify_ed25519(payload_file, ed_pub_pem, sig_b64, tmpdir):
    """True iff the Ed25519 signature over the whole file verifies (openssl rc authoritative)."""
    sig_path, _ = _write_sig(sig_b64, tmpdir, "ed.sig")
    rc, out = run([
        "openssl", "pkeyutl", "-verify", "-pubin", "-inkey", ed_pub_pem,
        "-rawin", "-in", payload_file, "-sigfile", sig_path,
    ])
    if rc == 0 and "Signature Verified Successfully" not in out:
        print(f"::warning::Ed25519 verify returned success without the expected text: {out}")
    return rc == 0


def corrupt_b64_sig(sig_b64):
    """Return a base64 signature with exactly one signature byte flipped."""
    raw = bytearray(base64.b64decode(sig_b64))
    raw[len(raw) // 2] ^= 0xFF
    return base64.b64encode(bytes(raw)).decode()


def check(label, payload_file, verify_fn, sig_b64, pub_pem, tmpdir):
    """Run a verification + its mutation self-test. die() on any failure."""
    if not verify_fn(payload_file, pub_pem, sig_b64, tmpdir):
        die(f"{label}: signature did NOT verify against the embedded public key")
    # Anti-no-op: a one-byte-corrupted signature MUST fail.
    if verify_fn(payload_file, pub_pem, corrupt_b64_sig(sig_b64), tmpdir):
        die(f"{label}: a CORRUPTED signature still verified — the verifier is a "
            f"no-op and would let a mis-signed update ship. Failing closed.")
    print(f"  OK: {label} - genuine signature verifies, corrupted signature rejected")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--appcast", required=True)
    ap.add_argument("--windows-installer", required=True)
    ap.add_argument("--macos-dmg", required=True)
    ap.add_argument("--autoupdater-src", required=True)
    ap.add_argument("--info-plist", required=True)
    ap.add_argument(
        "--require-windows-eddsa", action="store_true",
        help="Make the Windows EdDSA signature MANDATORY. Pass this once the shipped "
             "WinSparkle client is EdDSA-only (release.yml 'commit 3b'); until then the "
             "client is DSA-mandatory and Windows EdDSA is optional.")
    args = ap.parse_args()

    for f in (args.appcast, args.windows_installer, args.macos_dmg,
              args.autoupdater_src, args.info_plist):
        if not os.path.isfile(f):
            die(f"required input not found: {f}")

    with tempfile.TemporaryDirectory() as tmp:
        dsa_pub = os.path.join(tmp, "dsa_pub.pem")
        ed_pub = os.path.join(tmp, "ed_pub.pem")
        print("Extracting public keys from source...")
        extract_dsa_pub_pem(args.autoupdater_src, dsa_pub)
        extract_ed25519_pub_pem(args.info_plist, ed_pub)

        enc = parse_appcast(args.appcast)
        if "windows" not in enc:
            die("appcast has no Windows enclosure")
        if "macos" not in enc:
            die("appcast has no macOS enclosure")

        print("Verifying signatures against real artifacts...")

        # MANDATORY: Windows DSA.
        if not enc["windows"].get("dsa"):
            die("Windows enclosure has no DSA signature (mandatory)")
        check("Windows DSA", args.windows_installer, verify_dsa,
              enc["windows"]["dsa"], dsa_pub, tmp)

        # MANDATORY: macOS EdDSA.
        if not enc["macos"].get("ed"):
            die("macOS enclosure has no EdDSA signature (mandatory)")
        check("macOS EdDSA", args.macos_dmg, verify_ed25519,
              enc["macos"]["ed"], ed_pub, tmp)

        # Windows EdDSA. Today the shipped client is WinSparkle 0.8.1 (DSA-mandatory),
        # so EdDSA is OPTIONAL here. At the EdDSA-only client cutover (release.yml
        # "commit 3b"), CI MUST pass --require-windows-eddsa, or a DSA-only feed would
        # pass this gate while every field client rejects it (a forced reinstall).
        if enc["windows"].get("ed"):
            check("Windows EdDSA", args.windows_installer, verify_ed25519,
                  enc["windows"]["ed"], ed_pub, tmp)
        elif args.require_windows_eddsa:
            die("Windows EdDSA signature is REQUIRED (--require-windows-eddsa) but the "
                "appcast's Windows enclosure has none. An EdDSA-only client would reject "
                "this update. Failing closed.")
        else:
            print("  TRIPWIRE: Windows EdDSA absent - allowed ONLY while the shipped client "
                  "is WinSparkle 0.8.1/DSA. At the EdDSA-only client cutover (release.yml "
                  "'commit 3b'), CI MUST add --require-windows-eddsa or this turns into a "
                  "silent fail-open. See POST_BETA16_PLAN T4(a).")

    print("All mandatory update signatures cryptographically verified "
          "(with mutation self-test). Safe to promote.")


if __name__ == "__main__":
    main()
