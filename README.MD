# PythPredict - Decentralized Prediction Market on Solana

A decentralized prediction market protocol built on Solana that allows users to bet on price movements of cryptocurrencies using real-time price feeds from Pyth Network.

## 🎯 Overview

PythPredict enables users to create binary prediction markets where participants can bet YES or NO on whether an asset's price will move within a specified timeframe. The protocol uses Pyth Network's high-fidelity price oracles to ensure fair and transparent market resolution.

### Key Features

- **Real-Time Price Feeds**: Integration with Pyth Network for accurate BTC/USD and ETH/USD price data
- **Binary Markets**: Simple YES/NO betting on price movements
- **Automated Resolution**: Markets resolve automatically based on oracle prices at settlement time
- **Fee System**: 1% platform fee on all bets
- **Fair Payout Distribution**: Winners receive proportional shares of the total pool
- **Token Conservation**: Guaranteed token conservation with no loss of funds

## 📋 Prerequisites

- Node.js v16+ and Yarn
- Rust 1.70+
- Solana CLI tools 1.17+
- Anchor Framework 0.31.1

## 🚀 Installation

1. **Clone the repository**
```bash
git clone https://github.com/TrebuchetNetwork/PythPredict
cd PythPredict
```

2. **Install dependencies**
```bash
yarn install
```

3. **Build the Solana program**
```bash
anchor build
```

4. **Set up local validator** (optional for local testing)
```bash
./scripts/setup-local.sh
```

## 🧪 Running Tests

The project includes a comprehensive test suite (`master-test.ts`) that validates all core functionality.

### Run the full test suite
```bash
anchor test
```

### What the Main Test Suite Does

The master test suite (`tests/master-test.ts`) performs extensive testing across 5 main sections:

#### Section 1: Core Functionality (6 tests)
- Creates prediction markets with correct parameters
- Places bets and validates fee calculations (1% fee)
- Updates odds based on bet distribution
- Resolves markets after settlement time
- Distributes winnings proportionally to winners
- Prevents double-claiming of winnings

#### Section 2: Edge Cases (3 tests)  
- Handles minimum bet amounts
- Maintains perfect token conservation (no tokens lost)
- Resolves empty markets with no bets

#### Section 3: Multi-Market Stress Test (1 test)
- Creates and manages multiple concurrent markets
- Places randomized bets across markets

#### Section 4: Live BTC Price Integration (5 tests)
- Fetches real BTC price from Pyth Network mainnet
- Creates market at actual BTC price (~$118,829 in example)
- Simulates different trading strategies
- Monitors price movement for 20 seconds
- Resolves based on actual price changes

### Test Output Example
```
📊 TEST SUITE FINAL REPORT
============================================================
📈 Test Results:
   Total Tests: 15
   Passed: 15 (100.0%)
   Failed: 0

💰 Token Conservation:
   Initial Supply: 3500.00 tokens
   User Balances: 2922.18 tokens

✅ EXCELLENT: All critical tests passed!
```

## 💻 Development Commands

```bash
# Build the program
anchor build

# Run tests
anchor test

# Deploy to devnet
anchor deploy --provider.cluster devnet

# Monitor test coverage
./coverage.sh
```

## 🏗️ Architecture

### Program Structure
```
programs/pythpredict/
├── src/
│   ├── lib.rs           # Program entry point
│   ├── instructions.rs  # Core instruction handlers
│   ├── state.rs         # Account structures
│   └── errors.rs        # Error definitions
```

### Key Components

1. **Market Account**: Stores market parameters, pools, and resolution data
2. **Position Account**: Tracks user bets and payouts
3. **Vault Accounts**: Hold YES/NO pools and fees
4. **Oracle Integration**: Pyth price feeds for resolution

### Core Instructions

- `initialize_market`: Create a new prediction market
- `place_bet`: Place a YES or NO bet
- `resolve_market`: Resolve using oracle price
- `resolve_with_external_price`: Resolve with manual price (testing)
- `claim_winnings`: Claim payouts after resolution

## 🔧 Configuration

### Anchor.toml
- Program ID: `J7TLVPzbd47RpiHV8BBPLQuixU53P5qijkrwkvN4u98W`
- Default cluster: Localnet
- Test script: `yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/master-test.ts`

### Test Wallets
Test wallets are stored in `.wallets/` directory:
- alice, bob, charlie, dave, eve (test participants)
- Each funded with SOL and test tokens

## 📊 How It Works

1. **Market Creation**: Creator sets initial price, settlement time, and oracle feed
2. **Betting Phase**: Users bet YES (price will move) or NO (price stays same)
3. **Price Monitoring**: Oracle prices tracked in real-time
4. **Resolution**: At settlement, compare final vs initial price
5. **Payout**: Winners share the total pool proportionally

### Payout Calculation
```
User Payout = (User Stake / Winning Pool) * Total Pool
```

Example: If YES pool has 100 tokens, NO pool has 50 tokens, and YES wins:
- YES bettors share 150 tokens proportionally
- Each receives their stake + proportional share of losing pool

## 🔐 Security Features

- PDA-based account derivation
- Signer verification for all critical operations
- Overflow protection with checked math
- Time-based settlement enforcement
- Oracle confidence validation

## 📝 License

MIT

## 🤝 Contributing

Contributions are welcome! Please read our contributing guidelines before submitting PRs.

## 📞 Support

For issues, questions, or suggestions, please open an issue on GitHub.