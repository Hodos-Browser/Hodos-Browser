/**
 * Extract hex from Rust logs and compare
 *
 * This script can:
 * 1. Extract hex from a log file
 * 2. Or accept hex directly as argument
 * 3. Parse and display the structure for comparison
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// Load the comparison function
const compareScript = require('./compare_csr_bytes.js');

function extractHexFromLogs(logText) {
    // Look for "Serialized request (hex, FULL):"
    const hexMatch = logText.match(/Serialized request \(hex, FULL\):\s*([a-fA-F0-9]+)/);
    if (hexMatch) {
        return hexMatch[1];
    }

    // Also try base64 and convert
    const base64Match = logText.match(/Serialized request \(base64, FULL\):\s*([A-Za-z0-9+/=]+)/);
    if (base64Match) {
        const base64 = base64Match[1];
        const hex = Buffer.from(base64, 'base64').toString('hex');
        return hex;
    }

    return null;
}

function main() {
    const args = process.argv.slice(2);

    if (args.length === 0) {
        console.log('CSR Byte Extraction and Comparison');
        console.log('===================================\n');
        console.log('Usage:');
        console.log('  node extract_and_compare.js <hex_string>');
        console.log('  node extract_and_compare.js --file <log_file>');
        console.log('  node extract_and_compare.js --from-logs');
        console.log('\nExamples:');
        console.log('  node extract_and_compare.js "deadbeef1234..."');
        console.log('  node extract_and_compare.js --file rust_output.txt');
        console.log('  node extract_and_compare.js --from-logs  (searches for recent log files)');
        return;
    }

    let hexString = null;

    if (args[0] === '--file') {
        // Read from file
        const logFile = args[1];
        if (!fs.existsSync(logFile)) {
            console.error(`❌ Log file not found: ${logFile}`);
            process.exit(1);
        }
        const logText = fs.readFileSync(logFile, 'utf8');
        hexString = extractHexFromLogs(logText);
        if (!hexString) {
            console.error('❌ Could not find hex string in log file');
            console.log('Looking for: "Serialized request (hex, FULL):" or base64 version');
            process.exit(1);
        }
        console.log('✅ Extracted hex from log file\n');
    } else if (args[0] === '--from-logs') {
        // Search for recent log files
        console.log('Searching for recent log files...');
        const possibleLogFiles = [
            'rust_output.txt',
            'test_output_rust.txt',
            '../logs',
            'target/debug/*.log'
        ];
        // For now, just prompt for hex
        console.log('Please provide the hex string directly or use --file <log_file>');
        return;
    } else {
        // Direct hex string
        hexString = args[0];
    }

    if (!hexString) {
        console.error('❌ No hex string provided');
        process.exit(1);
    }

    // Validate hex string
    if (!/^[a-fA-F0-9]+$/.test(hexString)) {
        console.error('❌ Invalid hex string (must contain only 0-9, a-f, A-F)');
        process.exit(1);
    }

    console.log(`\n📋 Analyzing ${hexString.length / 2} bytes of serialized request...\n`);

    // Use the comparison script to parse
    try {
        // Call the parse function from compare_csr_bytes.js
        const compareModule = require('./compare_csr_bytes.js');
        if (typeof compareModule.parseSerializedRequest === 'function') {
            compareModule.parseSerializedRequest(hexString);
        } else {
            // Fallback: use the script directly
            const { spawn } = require('child_process');
            const child = spawn('node', ['compare_csr_bytes.js', hexString], {
                stdio: 'inherit'
            });
            child.on('exit', (code) => {
                process.exit(code || 0);
            });
        }
    } catch (error) {
        console.error('❌ Error parsing:', error.message);
        process.exit(1);
    }
}

if (require.main === module) {
    main();
}

module.exports = { extractHexFromLogs, main };

