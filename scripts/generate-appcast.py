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
        --output appcast.xml

The generated XML follows the Sparkle appcast format used by both WinSparkle and Sparkle 2.
"""

import argparse
import xml.etree.ElementTree as ET
from datetime import datetime, timezone


def generate_appcast(args):
    # Register namespace prefixes so ElementTree uses 'sparkle:' instead of 'ns0:'
    ET.register_namespace('sparkle', 'http://www.andymatuschak.org/xml-namespaces/sparkle')
    ET.register_namespace('dc', 'http://purl.org/dc/elements/1.1/')

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
    if args.windows_url:
        item = ET.SubElement(channel, 'item')
        ET.SubElement(item, 'title').text = f'Version {args.version}'
        ET.SubElement(item, 'pubDate').text = pub_date
        ET.SubElement(item, '{http://www.andymatuschak.org/xml-namespaces/sparkle}version').text = args.version
        ET.SubElement(item, '{http://www.andymatuschak.org/xml-namespaces/sparkle}os').text = 'windows'

        enclosure_attrs = {
            'url': args.windows_url,
            'length': str(args.windows_size or 0),
            'type': 'application/octet-stream',
        }
        if args.windows_signature:
            enclosure_attrs['{http://www.andymatuschak.org/xml-namespaces/sparkle}dsaSignature'] = args.windows_signature
        ET.SubElement(item, 'enclosure', enclosure_attrs)

    # macOS item
    if args.macos_url:
        item = ET.SubElement(channel, 'item')
        ET.SubElement(item, 'title').text = f'Version {args.version}'
        ET.SubElement(item, 'pubDate').text = pub_date
        ET.SubElement(item, '{http://www.andymatuschak.org/xml-namespaces/sparkle}version').text = args.version
        ET.SubElement(item, '{http://www.andymatuschak.org/xml-namespaces/sparkle}os').text = 'macos'

        enclosure_attrs = {
            'url': args.macos_url,
            'length': str(args.macos_size or 0),
            'type': 'application/octet-stream',
        }
        if args.macos_signature:
            enclosure_attrs['{http://www.andymatuschak.org/xml-namespaces/sparkle}edSignature'] = args.macos_signature
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


def main():
    parser = argparse.ArgumentParser(description='Generate Sparkle appcast.xml for auto-updates')
    parser.add_argument('--version', required=True, help='Release version (e.g. 0.2.0-beta.1)')
    parser.add_argument('--windows-url', help='Windows installer download URL')
    parser.add_argument('--windows-size', type=int, help='Windows installer file size in bytes')
    parser.add_argument('--windows-signature', help='Windows DSA signature')
    parser.add_argument('--macos-url', help='macOS DMG download URL')
    parser.add_argument('--macos-size', type=int, help='macOS DMG file size in bytes')
    parser.add_argument('--macos-signature', help='macOS EdDSA signature')
    parser.add_argument('--output', default='appcast.xml', help='Output file path')
    args = parser.parse_args()

    generate_appcast(args)


if __name__ == '__main__':
    main()
