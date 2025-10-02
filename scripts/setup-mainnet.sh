#!/bin/bash
set -e

echo "⚠️  Setting up MAINNET environment..."
echo "⚠️  WARNING: This is REAL money! Double-check everything!"
echo ""

# Create mainnet keypair if it doesn't exist
if [ ! -f .wallets/mainnet-keypair.json ]; then
    echo "📝 Generating mainnet keypair..."
    echo "⚠️  SAVE THIS KEYPAIR SECURELY!"
    solana-keygen new -o .wallets/mainnet-keypair.json
    echo ""
    echo "🔒 IMPORTANT: Backup .wallets/mainnet-keypair.json securely!"
    echo "Press ENTER to continue..."
    read
fi

# Configure Solana for mainnet
solana config set --url mainnet-beta
solana config set --keypair .wallets/mainnet-keypair.json

echo "✅ Mainnet setup complete!"
echo "Keypair: .wallets/mainnet-keypair.json"
PUBKEY=$(solana address)
echo "Address: $PUBKEY"

# Check balance
BALANCE=$(solana balance)
echo "Balance: $BALANCE"

if [[ "$BALANCE" == "0 SOL" ]]; then
    echo ""
    echo "⚠️  No balance! Fund this address before deploying:"
    echo "   $PUBKEY"
    echo ""
    echo "Recommended: 5-10 SOL for deployment"
fi

echo ""
echo "🚀 Commands:"
echo "  - Build: anchor build"
echo "  - Deploy: anchor deploy"
echo ""
echo "⚠️  Remember: You can't airdrop on mainnet - you need real SOL!"