/**
 * Parse Rust wallet logs and compare header values with TypeScript SDK expectations
 *
 * Usage: node parse_and_compare_headers.js <log_text>
 * Or paste logs when prompted
 */

const readline = require('readline');

function parseRustLogs(logText) {
    const headers = {};
    const details = {};

    // Extract header values
    const headerPatterns = {
        'x-bsv-auth-version': /x-bsv-auth-version:\s*([^\s]+)/i,
        'x-bsv-auth-identity-key': /x-bsv-auth-identity-key[^:]*:\s*([a-fA-F0-9]{66})/i,
        'x-bsv-auth-nonce': /x-bsv-auth-nonce[^:]*:\s*([A-Za-z0-9+/=]{40,50})/i,
        'x-bsv-auth-your-nonce': /x-bsv-auth-your-nonce[^:]*:\s*([A-Za-z0-9+/=]{40,50})/i,
        'x-bsv-auth-request-id': /x-bsv-auth-request-id[^:]*:\s*([A-Za-z0-9+/=]{40,50})/i,
        'x-bsv-auth-signature': /x-bsv-auth-signature[^:]*:\s*([a-fA-F0-9]{70,150})/i
    };

    for (const [headerName, pattern] of Object.entries(headerPatterns)) {
        const match = logText.match(pattern);
        if (match) {
            headers[headerName] = match[1];
        }
    }

    // Extract serialized request details
    const serializedRequestMatch = logText.match(/Serialized request \(hex, FULL\):\s*([a-fA-F0-9]+)/i);
    if (serializedRequestMatch) {
        details.serializedRequestHex = serializedRequestMatch[1];
    }

    const requestIdMatch = logText.match(/Request ID[^:]*:\s*([A-Za-z0-9+/=]{40,50})/i);
    if (requestIdMatch) {
        details.requestId = requestIdMatch[1];
    }

    const signingNonceMatch = logText.match(/Signing nonce[^:]*:\s*([A-Za-z0-9+/=]{40,50})/i);
    if (signingNonceMatch) {
        details.signingNonce = signingNonceMatch[1];
    }

    const requestNonceMatch = logText.match(/Request nonce[^:]*:\s*([A-Za-z0-9+/=]{40,50})/i);
    if (requestNonceMatch) {
        details.requestNonce = requestNonceMatch[1];
    }

    return { headers, details };
}

function validateHeaders(headers, details) {
    console.log('\n🔍 HEADER VALIDATION RESULTS');
    console.log('='.repeat(70));
    console.log('');

    const issues = [];
    const warnings = [];

    // 1. Check x-bsv-auth-version
    if (headers['x-bsv-auth-version']) {
        if (headers['x-bsv-auth-version'] === '0.1') {
            console.log('✅ x-bsv-auth-version: Correct ("0.1")');
        } else {
            issues.push(`x-bsv-auth-version should be "0.1", got "${headers['x-bsv-auth-version']}"`);
            console.log(`❌ x-bsv-auth-version: Should be "0.1", got "${headers['x-bsv-auth-version']}"`);
        }
    } else {
        warnings.push('x-bsv-auth-version not found in logs');
        console.log('⚠️  x-bsv-auth-version: Not found in logs');
    }

    // 2. Check x-bsv-auth-identity-key
    if (headers['x-bsv-auth-identity-key']) {
        if (headers['x-bsv-auth-identity-key'].length === 66) {
            console.log(`✅ x-bsv-auth-identity-key: Correct format (66 chars)`);
            console.log(`   Value: ${headers['x-bsv-auth-identity-key']}`);
        } else {
            issues.push(`x-bsv-auth-identity-key should be 66 chars (hex), got ${headers['x-bsv-auth-identity-key'].length}`);
            console.log(`❌ x-bsv-auth-identity-key: Should be 66 chars, got ${headers['x-bsv-auth-identity-key'].length}`);
        }
    } else {
        warnings.push('x-bsv-auth-identity-key not found in logs');
        console.log('⚠️  x-bsv-auth-identity-key: Not found in logs');
    }

    // 3. Check x-bsv-auth-nonce (CRITICAL: should be signing nonce, not request nonce)
    if (headers['x-bsv-auth-nonce']) {
        if (headers['x-bsv-auth-nonce'].length === 44) {
            console.log(`✅ x-bsv-auth-nonce: Correct format (44 chars, base64)`);
            console.log(`   Value: ${headers['x-bsv-auth-nonce']}`);

            // Check if it matches signing nonce or request nonce
            if (details.signingNonce && headers['x-bsv-auth-nonce'] === details.signingNonce) {
                console.log(`   ✅ Matches signing nonce (CORRECT)`);
            } else if (details.requestNonce && headers['x-bsv-auth-nonce'] === details.requestNonce) {
                issues.push('x-bsv-auth-nonce is using request nonce instead of signing nonce!');
                console.log(`   ❌ Matches request nonce (WRONG - should be signing nonce!)`);
            } else if (details.signingNonce) {
                warnings.push('x-bsv-auth-nonce does not match signing nonce from logs');
                console.log(`   ⚠️  Does not match signing nonce from logs`);
                console.log(`   Expected (signing nonce): ${details.signingNonce}`);
            }
        } else {
            issues.push(`x-bsv-auth-nonce should be 44 chars (base64), got ${headers['x-bsv-auth-nonce'].length}`);
            console.log(`❌ x-bsv-auth-nonce: Should be 44 chars, got ${headers['x-bsv-auth-nonce'].length}`);
        }
    } else {
        warnings.push('x-bsv-auth-nonce not found in logs');
        console.log('⚠️  x-bsv-auth-nonce: Not found in logs');
    }

    // 4. Check x-bsv-auth-your-nonce
    if (headers['x-bsv-auth-your-nonce']) {
        if (headers['x-bsv-auth-your-nonce'].length === 44) {
            console.log(`✅ x-bsv-auth-your-nonce: Correct format (44 chars, base64)`);
            console.log(`   Value: ${headers['x-bsv-auth-your-nonce']}`);
        } else {
            issues.push(`x-bsv-auth-your-nonce should be 44 chars (base64), got ${headers['x-bsv-auth-your-nonce'].length}`);
            console.log(`❌ x-bsv-auth-your-nonce: Should be 44 chars, got ${headers['x-bsv-auth-your-nonce'].length}`);
        }
    } else {
        warnings.push('x-bsv-auth-your-nonce not found in logs');
        console.log('⚠️  x-bsv-auth-your-nonce: Not found in logs');
    }

    // 5. Check x-bsv-auth-request-id (CRITICAL: must match first 32 bytes of serialized request)
    if (headers['x-bsv-auth-request-id']) {
        if (headers['x-bsv-auth-request-id'].length === 44) {
            console.log(`✅ x-bsv-auth-request-id: Correct format (44 chars, base64)`);
            console.log(`   Value: ${headers['x-bsv-auth-request-id']}`);

            // Verify it matches first 32 bytes of serialized request
            if (details.serializedRequestHex) {
                const first32Bytes = Buffer.from(details.serializedRequestHex.substring(0, 64), 'hex');
                const expectedRequestId = first32Bytes.toString('base64');

                if (headers['x-bsv-auth-request-id'] === expectedRequestId) {
                    console.log(`   ✅ Matches first 32 bytes of serialized request (CORRECT)`);
                } else {
                    issues.push('x-bsv-auth-request-id does not match first 32 bytes of serialized request!');
                    console.log(`   ❌ Does NOT match first 32 bytes of serialized request!`);
                    console.log(`   Expected: ${expectedRequestId}`);
                    console.log(`   Got:      ${headers['x-bsv-auth-request-id']}`);
                }
            } else if (details.requestId) {
                if (headers['x-bsv-auth-request-id'] === details.requestId) {
                    console.log(`   ✅ Matches request ID from logs`);
                } else {
                    warnings.push('x-bsv-auth-request-id does not match request ID from logs');
                    console.log(`   ⚠️  Does not match request ID from logs`);
                    console.log(`   Expected: ${details.requestId}`);
                    console.log(`   Got:      ${headers['x-bsv-auth-request-id']}`);
                }
            }
        } else {
            issues.push(`x-bsv-auth-request-id should be 44 chars (base64), got ${headers['x-bsv-auth-request-id'].length}`);
            console.log(`❌ x-bsv-auth-request-id: Should be 44 chars, got ${headers['x-bsv-auth-request-id'].length}`);
        }
    } else {
        warnings.push('x-bsv-auth-request-id not found in logs');
        console.log('⚠️  x-bsv-auth-request-id: Not found in logs');
    }

    // 6. Check x-bsv-auth-signature
    if (headers['x-bsv-auth-signature']) {
        if (headers['x-bsv-auth-signature'].length >= 70 && headers['x-bsv-auth-signature'].length <= 150) {
            console.log(`✅ x-bsv-auth-signature: Correct format (${headers['x-bsv-auth-signature'].length} chars, hex)`);
            console.log(`   Value (first 40): ${headers['x-bsv-auth-signature'].substring(0, 40)}...`);
        } else {
            warnings.push(`x-bsv-auth-signature length unusual: ${headers['x-bsv-auth-signature'].length} chars (expected 70-150)`);
            console.log(`⚠️  x-bsv-auth-signature: Unusual length: ${headers['x-bsv-auth-signature'].length} chars`);
        }
    } else {
        warnings.push('x-bsv-auth-signature not found in logs');
        console.log('⚠️  x-bsv-auth-signature: Not found in logs');
    }

    console.log('');
    console.log('='.repeat(70));
    console.log('');

    if (issues.length === 0 && warnings.length === 0) {
        console.log('✅ All headers validated successfully!');
    } else {
        if (issues.length > 0) {
            console.log(`❌ Found ${issues.length} issue(s):`);
            issues.forEach((issue, i) => {
                console.log(`   ${i + 1}. ${issue}`);
            });
            console.log('');
        }
        if (warnings.length > 0) {
            console.log(`⚠️  Found ${warnings.length} warning(s):`);
            warnings.forEach((warning, i) => {
                console.log(`   ${i + 1}. ${warning}`);
            });
        }
    }

    return { issues, warnings };
}

// Main
const logText = process.argv[2];

if (logText) {
    // Parse from command line argument
    const { headers, details } = parseRustLogs(logText);
    validateHeaders(headers, details);
} else {
    // Interactive mode
    console.log('📋 Rust Wallet Header Parser');
    console.log('='.repeat(70));
    console.log('');
    console.log('Paste your Rust wallet logs below (the section with headers).');
    console.log('Press Ctrl+D (or Ctrl+Z on Windows) when done, or type "END" on a new line.');
    console.log('');

    const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout
    });

    let logLines = [];

    rl.on('line', (line) => {
        if (line.trim().toUpperCase() === 'END') {
            rl.close();
        } else {
            logLines.push(line);
        }
    });

    rl.on('close', () => {
        const logText = logLines.join('\n');
        const { headers, details } = parseRustLogs(logText);
        validateHeaders(headers, details);
    });
}
