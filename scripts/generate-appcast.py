#!/usr/bin/env python3
"""
Generate appcast.xml for WinSparkle (Windows) and Sparkle 2 (macOS) auto-updates.

Usage:
    python3 generate-appcast.py \
        --version 0.2.0-beta.1 \
        --windows-url https://github.com/Hodos-Browser/Hodos-Browser/releases/download/v0.2.0-beta.1/HodosBrowser-0.2.0-beta.1-setup.exe \
        --windows-size 95000000 \
        --windows-signature "MEUCIQD..." \
        --macos-url https://github.com/Hodos-Browser/Hodos-Browser/releases/download/v0.2.0-beta.1/HodosBrowser-0.2.0-beta.1.dmg \
        --macos-size 180000000 \
        --macos-signature "MEUCIQD..." \
        --macos-minimum-system-version 12.0 \
        --output appcast.xml

The generated XML follows the Sparkle appcast format used by both WinSparkle and Sparkle 2.
"""

import argparse
import sys
import xml.etree.ElementTree as ET
from datetime import datetime, timezone


def generate_appcast(args):
    # Register namespace prefixes so ElementTree uses 'sparkle:' instead of 'ns0:'
    ET.register_namespace('sparkle', 'http://www.andymatuschak.org/xml-namespaces/sparkle')
    ET.register_namespace('dc', 'http://purl.org/dc/elements/1.1/')

    SPARKLE_NS = 'http://www.andymatuschak.org/xml-namespaces/sparkle'

    rss = ET.Element('rss', {
        'version': '2.0',
    })

    channel = ET.SubElement(rss, 'channel')
    ET.SubElement(channel, 'title').text = 'Hodos Browser Updates'
    ET.SubElement(channel, 'link').text = 'https://hodosbrowser.com'
    ET.SubElement(channel, 'description').text = 'Auto-update feed for Hodos Browser'
    ET.SubElement(channel, 'language').text = 'en'

    pub_date = datetime.now(timezone.utc).strftime('%a, %d %b %Y %H:%M:%S +0000')

    # Windows item
    # WinSparkle handles full version strings ("0.3.0-beta.X") in
    # sparkle:version directly. Keeping that contract unchanged.
    if args.windows_url:
        item = ET.SubElement(channel, 'item')
        ET.SubElement(item, 'title').text = f'Version {args.version}'
        ET.SubElement(item, 'pubDate').text = pub_date
        ET.SubElement(item, f'{{{SPARKLE_NS}}}version').text = args.version
        ET.SubElement(item, f'{{{SPARKLE_NS}}}os').text = 'windows'
        # Hodos-read monotonic integer build number. sparkle:version stays the
        # full version STRING (WinSparkle 0.8.1 compares it directly), so the
        # Hodos-driven silent updater reads this dedicated element for its
        # integer anti-rollback gate. WinSparkle ignores unknown elements.
        # Keep this number byte-identical to the macOS build-number formula.
        if args.build_number:
            ET.SubElement(item, 'hodosBuildNumber').text = str(args.build_number)

        enclosure_attrs = {
            'url': args.windows_url,
            'length': str(args.windows_size or 0),
            'type': 'application/octet-stream',
        }
        # Fail closed (Goal-2 feed integrity): a Windows item with no signature is an
        # UNSIGNED enclosure that WinSparkle would still try to install. Refuse to emit
        # one rather than silently omitting the attribute.
        if not args.windows_signature:
            sys.exit('ERROR: --windows-url given without --windows-signature — refusing to emit an unsigned Windows enclosure')
        enclosure_attrs[f'{{{SPARKLE_NS}}}dsaSignature'] = args.windows_signature
        # Dual-signed DSA→EdDSA transition: also emit the EdDSA signature so WinSparkle
        # 0.9.x clients verify via EdDSA, while already-installed 0.8.1 (DSA-only) clients
        # keep verifying via dsaSignature. (Drop dsaSignature only after 0.8.1 drains.)
        if args.windows_ed_signature:
            enclosure_attrs[f'{{{SPARKLE_NS}}}edSignature'] = args.windows_ed_signature
        ET.SubElement(item, 'enclosure', enclosure_attrs)

    # macOS item
    # Sparkle 2 compares sparkle:version against the running app's
    # CFBundleVersion. Apple's spec is that CFBundleVersion is a
    # monotonic integer (build number). Sparkle's SUStandardVersion
    # Comparator silently fails on suffixed strings like
    # "0.3.0-beta.12" vs "0.3.0-beta.13" — it returns "up to date"
    # despite the higher number. So we emit the integer build number
    # as sparkle:version (matches CFBundleVersion in the .app) and
    # the human-readable string as sparkle:shortVersionString (matches
    # CFBundleShortVersionString, used in the update dialog).
    if args.macos_url:
        item = ET.SubElement(channel, 'item')
        ET.SubElement(item, 'title').text = f'Version {args.version}'
        ET.SubElement(item, 'pubDate').text = pub_date

        # macOS-only: integer build number for comparison
        macos_sparkle_version = str(args.build_number) if args.build_number else args.version
        ET.SubElement(item, f'{{{SPARKLE_NS}}}version').text = macos_sparkle_version
        if args.build_number:
            ET.SubElement(item, f'{{{SPARKLE_NS}}}shortVersionString').text = args.version
        ET.SubElement(item, f'{{{SPARKLE_NS}}}os').text = 'macos'

        # ⛔ The OS floor. Without it Sparkle offers this update to EVERY macOS client,
        # including ones dyld will refuse to launch it on — the update installs, replaces
        # the working app, and leaves no in-product way back. That is the exact shape of
        # TICKET_appcast_missing_minimum_system_version.md: our floor moved 11.0 → 12.0
        # with CEF 150 while the feed kept advertising no constraint at all, and every
        # feed we have ever published carries zero occurrences of this element.
        #
        # ⛔ REQUIRED, never defaulted. A default is a second copy of the floor that rots
        # silently on the next bump — which is how the DOC (BUILD_AND_RELEASE.md §4.5) came
        # to specify `11.0` correctly while the implementation emitted nothing. Same
        # fail-closed posture as the signature check below.
        # ⭐ release.yml passes the value MEASURED by `vtool -show-build` on the built
        # framework (the minos guard), not a literal, so the feed cannot disagree with
        # the binary it points at.
        if not args.macos_minimum_system_version:
            sys.exit('ERROR: --macos-url given without --macos-minimum-system-version — '
                     'refusing to emit a macOS item with no OS floor (it would be offered '
                     'to clients that cannot launch it)')
        ET.SubElement(item, f'{{{SPARKLE_NS}}}minimumSystemVersion').text = \
            args.macos_minimum_system_version

        enclosure_attrs = {
            'url': args.macos_url,
            'length': str(args.macos_size or 0),
            'type': 'application/octet-stream',
        }
        # Fail closed (Goal-2 feed integrity): never emit an unsigned macOS enclosure.
        if not args.macos_signature:
            sys.exit('ERROR: --macos-url given without --macos-signature — refusing to emit an unsigned macOS enclosure')
        enclosure_attrs[f'{{{SPARKLE_NS}}}edSignature'] = args.macos_signature
        ET.SubElement(item, 'enclosure', enclosure_attrs)

    # Write XML
    tree = ET.ElementTree(rss)
    ET.indent(tree, space='  ')

    with open(args.output, 'wb') as f:
        tree.write(f, encoding='utf-8', xml_declaration=True)

    print(f'Generated {args.output} for version {args.version}')
    if args.windows_url:
        print(f'  Windows: {args.windows_url}')
    if args.macos_url:
        print(f'  macOS: {args.macos_url}')
        print(f'  macOS minimumSystemVersion: {args.macos_minimum_system_version}')


def main():
    parser = argparse.ArgumentParser(description='Generate Sparkle appcast.xml for auto-updates')
    parser.add_argument('--version', required=True, help='Release version string (e.g. 0.3.0-beta.14). Used directly for Windows sparkle:version and as macOS sparkle:shortVersionString (display name).')
    parser.add_argument('--build-number', type=int, help='Monotonic integer build number (e.g. 14 for v0.3.0-beta.14). Emitted as macOS sparkle:version for comparison against CFBundleVersion. If omitted, falls back to --version.')
    parser.add_argument('--windows-url', help='Windows installer download URL')
    parser.add_argument('--windows-size', type=int, help='Windows installer file size in bytes')
    parser.add_argument('--windows-signature', help='Windows DSA signature (legacy; kept during the DSA→EdDSA transition)')
    parser.add_argument('--windows-ed-signature', help='Windows EdDSA (Ed25519) signature from winsparkle-tool — dual-emitted alongside the DSA signature')
    parser.add_argument('--macos-url', help='macOS DMG download URL')
    parser.add_argument('--macos-size', type=int, help='macOS DMG file size in bytes')
    parser.add_argument('--macos-signature', help='macOS EdDSA signature')
    parser.add_argument('--macos-minimum-system-version',
                        help='Minimum macOS version for the macOS item, e.g. "12.0". REQUIRED whenever '
                             '--macos-url is given; deliberately has no default. Pass the value MEASURED '
                             'from the built Mach-O (release.yml derives it from the minos guard), never a literal.')
    parser.add_argument('--output', default='appcast.xml', help='Output file path')
    args = parser.parse_args()

    generate_appcast(args)


if __name__ == '__main__':
    main()
