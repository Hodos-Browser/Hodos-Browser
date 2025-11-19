// Simple WebSocket test client
const WebSocket = require('ws');

console.log('🔌 Connecting to BRC-100 WebSocket...');

const ws = new WebSocket('ws://localhost:8080/brc100/ws');

ws.on('open', function open() {
    console.log('✅ Connected to BRC-100 WebSocket server');

    // Send a ping message
    const pingMessage = {
        type: 'ping',
        timestamp: new Date().toISOString()
    };

    console.log('📤 Sending ping message...');
    ws.send(JSON.stringify(pingMessage));

    // Send an authentication request
    setTimeout(() => {
        const authMessage = {
            type: 'auth_request',
            data: {
                authRequest: {
                    appDomain: 'test-app.example.com',
                    purpose: 'Testing BRC-100 WebSocket authentication'
                }
            },
            timestamp: new Date().toISOString()
        };

        console.log('📤 Sending authentication request...');
        ws.send(JSON.stringify(authMessage));
    }, 1000);
});

ws.on('message', function message(data) {
    console.log('📥 Received message:', JSON.parse(data.toString()));
});

ws.on('close', function close() {
    console.log('❌ WebSocket connection closed');
});

ws.on('error', function error(err) {
    console.error('❌ WebSocket error:', err);
});

// Close connection after 5 seconds
setTimeout(() => {
    console.log('🔌 Closing connection...');
    ws.close();
}, 5000);
