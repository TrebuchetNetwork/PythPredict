#!/bin/bash
set -e

echo "🔧 Setting up LOCAL development environment..."

# Create local keypair if it doesn't exist
if [ ! -f .wallets/local-keypair.json ]; then
    echo "📝 Generating local keypair..."
    solana-keygen new --no-bip39-passphrase -o .wallets/local-keypair.json
fi

# Configure Solana for localhost
solana config set --url localhost
solana config set --keypair .wallets/local-keypair.json

echo "✅ Local setup complete!"
echo "Keypair: .wallets/local-keypair.json"
solana address

# Optional: Start local validator if not running
# solana-test-validator &

echo ""
echo "🚀 Run 'anchor test' to build and test locally"