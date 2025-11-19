# Babbage Browser Go Wallet

A production-ready Bitcoin SV wallet backend with BRC-100 authentication and BEEF/SPV support.

## Features

- **HD Wallet Support** - Hierarchical Deterministic wallet with BIP44 derivation
- **BRC-100 Authentication** - Complete BRC-100 protocol implementation
- **BEEF/SPV Integration** - Real blockchain integration with Merkle proofs
- **WebSocket Support** - Real-time communication for BRC-100 sessions
- **Multi-API Support** - WhatsOnChain, GorillaPool, and TAAL integration
- **Transaction Broadcasting** - Real BSV transaction creation and broadcasting

## Quick Start

### Option 1: Run Executable
```bash
# Start the wallet daemon
./babbage-wallet.exe

# Or use the batch file (Windows)
start-wallet.bat
```

### Option 2: Run from Source
```bash
go run main.go hd_wallet.go transaction_builder.go transaction_broadcaster.go utxo_manager.go brc100_api.go
```

## API Endpoints

### Wallet Endpoints
- `GET /health` - Health check
- `GET /wallet/info` - Get wallet information
- `GET /wallet/addresses` - Get all addresses
- `POST /wallet/address/generate` - Generate new address
- `POST /transaction/send` - Send complete transaction

### BRC-100 Endpoints
- `GET /brc100/status` - BRC-100 service status
- `POST /brc100/identity/generate` - Generate identity certificate
- `POST /brc100/auth/challenge` - Generate authentication challenge
- `POST /brc100/auth/authenticate` - Authenticate with challenge
- `POST /brc100/session/create` - Create authentication session
- `POST /brc100/beef/create` - Create BRC-100 BEEF transaction
- `POST /brc100/beef/create-from-tx` - Create BEEF with SPV data from transaction
- `WS /brc100/ws` - WebSocket for real-time communication

## Configuration

The wallet automatically creates a `wallet.json` file in:
- Windows: `%APPDATA%/BabbageBrowser/wallet/wallet.json`
- Linux/Mac: `~/.babbage-browser/wallet/wallet.json`

## Testing

Run the test suite:
```bash
go run test_real_beef_workflow.go
```

## Architecture

- **HD Wallet** - Manages private keys and address generation
- **Transaction Builder** - Creates and signs Bitcoin SV transactions
- **BRC-100 Manager** - Handles authentication and identity management
- **BEEF Manager** - Manages BEEF transactions with SPV data
- **Blockchain Client** - Interfaces with multiple blockchain APIs
- **WebSocket Handler** - Real-time communication for BRC-100 sessions

## Dependencies

- `github.com/bsv-blockchain/go-sdk` - Bitcoin SV SDK
- `github.com/sirupsen/logrus` - Logging
- `github.com/gorilla/websocket` - WebSocket support

## License

Part of the Babbage Browser project.
