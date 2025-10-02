#!/bin/bash
set -e

echo "🔧 Setting up DEVNET environment..."

# Create devnet keypair if it doesn't exist
if [ ! -f .wallets/devnet-keypair.json ]; then
    echo "📝 Generating devnet keypair..."
    solana-keygen new --no-bip39-passphrase -o .wallets/devnet-keypair.json
fi

# Configure Solana for devnet
solana config set --url devnet
solana config set --keypair .wallets/devnet-keypair.json

echo "✅ Devnet setup complete!"
echo "Keypair: .wallets/devnet-keypair.json"
PUBKEY=$(solana address)
echo "Address: $PUBKEY"

# Check balance
BALANCE=$(solana balance)
echo "Balance: $BALANCE"

# Airdrop if balance is low
if [[ "$BALANCE" == "0 SOL" ]]; then
    echo "💰 Requesting airdrop..."
    solana airdrop 2
    echo "New balance: $(solana balance)"
fi

echo ""
echo "🚀 Commands:"
echo "  - Build: anchor build"
echo "  - Deploy: anchor deploy"
echo "  - Test: anchor test --skip-local-validator"