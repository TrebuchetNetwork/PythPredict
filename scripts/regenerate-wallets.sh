#!/bin/bash

# Regenerate wallets for Anchor/Solana project
# This script creates new keypairs for all test wallets with backup and validation

set -e

WALLET_DIR=".wallets"
BACKUP_DIR=".wallets/backup_$(date +%Y%m%d_%H%M%S)"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# List of wallets to generate
WALLETS=(
    "alice"
    "bob"
    "charlie"
    "buyer1"
    "buyer2"
    "seller1"
    "seller2"
    "market_maker"
)

echo -e "${BLUE}╔════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   Wallet Regeneration Script          ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════╝${NC}"
echo ""

# Check if solana-keygen is available
if ! command -v solana-keygen &> /dev/null; then
    echo -e "${RED}✗ Error: solana-keygen not found${NC}"
    echo "Please install Solana CLI tools first:"
    echo "  sh -c \"\$(curl -sSfL https://release.solana.com/stable/install)\""
    exit 1
fi

# Create .wallets directory if it doesn't exist
mkdir -p "$WALLET_DIR"

# Check if wallets already exist and offer to backup
if [ -f "$WALLET_DIR/addresses.json" ]; then
    echo -e "${YELLOW}⚠️  Existing wallets found!${NC}"
    echo ""
    read -p "Backup existing wallets? (y/N): " -n 1 -r
    echo
    
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${BLUE}Creating backup...${NC}"
        mkdir -p "$BACKUP_DIR"
        
        # Backup all existing wallet files
        for wallet in "${WALLETS[@]}"; do
            if [ -f "$WALLET_DIR/${wallet}.json" ]; then
                cp "$WALLET_DIR/${wallet}.json" "$BACKUP_DIR/"
                echo -e "${GREEN}  ✓ Backed up ${wallet}.json${NC}"
            fi
        done
        
        # Backup addresses.json
        if [ -f "$WALLET_DIR/addresses.json" ]; then
            cp "$WALLET_DIR/addresses.json" "$BACKUP_DIR/"
            echo -e "${GREEN}  ✓ Backed up addresses.json${NC}"
        fi
        
        echo -e "${GREEN}✓ Backup created at: ${BACKUP_DIR}${NC}"
        echo ""
    fi
    
    echo -e "${YELLOW}⚠️  This will OVERWRITE existing wallets!${NC}"
    read -p "Continue? (y/N): " -n 1 -r
    echo
    
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Aborted."
        exit 0
    fi
fi

echo ""
echo -e "${BLUE}Generating new wallets...${NC}"
echo ""

# Generate each wallet
for wallet in "${WALLETS[@]}"; do
    WALLET_PATH="$WALLET_DIR/${wallet}.json"
    echo -e "${BLUE}Generating ${wallet}...${NC}"
    
    if solana-keygen new --no-bip39-passphrase --silent --outfile "$WALLET_PATH" --force; then
        ADDRESS=$(solana-keygen pubkey "$WALLET_PATH")
        echo -e "${GREEN}  ✓ ${wallet}: ${ADDRESS:0:8}...${ADDRESS: -8}${NC}"
    else
        echo -e "${RED}  ✗ Failed to generate ${wallet}${NC}"
        exit 1
    fi
done

echo ""
echo -e "${BLUE}Generating addresses.json...${NC}"

# Start JSON object
echo "{" > "$WALLET_DIR/addresses.json"

# Add each wallet's public key
for i in "${!WALLETS[@]}"; do
    wallet="${WALLETS[$i]}"
    WALLET_PATH="$WALLET_DIR/${wallet}.json"
    
    if [ ! -f "$WALLET_PATH" ]; then
        echo -e "${RED}✗ Error: ${wallet}.json not found${NC}"
        exit 1
    fi
    
    ADDRESS=$(solana-keygen pubkey "$WALLET_PATH")
    
    # Add comma for all but the last entry
    if [ $i -eq $((${#WALLETS[@]} - 1)) ]; then
        echo "  \"${wallet}\": \"${ADDRESS}\"" >> "$WALLET_DIR/addresses.json"
    else
        echo "  \"${wallet}\": \"${ADDRESS}\"," >> "$WALLET_DIR/addresses.json"
    fi
done

# Close JSON object
echo "}" >> "$WALLET_DIR/addresses.json"

echo -e "${GREEN}✓ addresses.json created${NC}"
echo ""

# Validate addresses.json
if command -v jq &> /dev/null; then
    echo -e "${BLUE}Validating addresses.json...${NC}"
    if jq empty "$WALLET_DIR/addresses.json" 2>/dev/null; then
        echo -e "${GREEN}✓ Valid JSON format${NC}"
    else
        echo -e "${RED}✗ Invalid JSON format${NC}"
        exit 1
    fi
    echo ""
fi

# Display summary
echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
echo -e "${GREEN}✓ All wallets regenerated successfully!${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
echo ""
echo -e "${BLUE}Wallet Summary:${NC}"
cat "$WALLET_DIR/addresses.json"
echo ""

# Update .gitignore if needed
if [ -f ".gitignore" ]; then
    if ! grep -q ".wallets/\*.json" .gitignore; then
        echo -e "${YELLOW}⚠️  Updating .gitignore...${NC}"
        echo "" >> .gitignore
        echo "# Wallets - NEVER commit!" >> .gitignore
        echo ".wallets/*.json" >> .gitignore
        echo ".wallets/**/*.json" >> .gitignore
        echo "!.wallets/README.md" >> .gitignore
        echo -e "${GREEN}✓ .gitignore updated${NC}"
        echo ""
    fi
fi

# Create README in wallets directory
cat > "$WALLET_DIR/README.md" << 'EOF'
# Wallets Directory

**⚠️ NEVER commit wallet JSON files to git!**

## Files
- `*.json` - Keypair files (KEEP SECURE!)
- `addresses.json` - Public addresses only (safe to commit)
- `backup_*/` - Backup directories (excluded from git)

## Wallets
- `alice`, `bob`, `charlie` - Test users
- `buyer1`, `buyer2` - Buyers in market tests
- `seller1`, `seller2` - Sellers in market tests  
- `market_maker` - Market maker account

## Regenerating Wallets
Run: `./scripts/regenerate-wallets.sh`

## Security
- ✅ `addresses.json` - Safe to commit (public keys only)
- ❌ `*.json` files - NEVER commit (contain private keys)
- 🔒 Mainnet wallets should be stored separately with encryption

## Backup
Backups are automatically created in `backup_*` directories when regenerating.
EOF

echo -e "${GREEN}✓ Created README.md${NC}"
echo ""

# Show wallet count and verification
WALLET_COUNT=$(ls -1 "$WALLET_DIR"/*.json 2>/dev/null | grep -v addresses.json | wc -l | tr -d ' ')
echo -e "${BLUE}Total wallets: ${WALLET_COUNT}${NC}"

# Verify all wallets can be read
echo ""
echo -e "${BLUE}Verifying all wallets...${NC}"
ALL_VALID=true

for wallet in "${WALLETS[@]}"; do
    WALLET_PATH="$WALLET_DIR/${wallet}.json"
    if solana-keygen pubkey "$WALLET_PATH" > /dev/null 2>&1; then
        echo -e "${GREEN}  ✓ ${wallet}.json is valid${NC}"
    else
        echo -e "${RED}  ✗ ${wallet}.json is invalid${NC}"
        ALL_VALID=false
    fi
done

echo ""
if [ "$ALL_VALID" = true ]; then
    echo -e "${GREEN}✓ All wallets verified successfully!${NC}"
else
    echo -e "${RED}✗ Some wallets failed verification${NC}"
    exit 1
fi

echo ""
echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"
echo -e "${YELLOW}⚠️  IMPORTANT REMINDERS:${NC}"
echo -e "${YELLOW}   • These are TEST wallets only${NC}"
echo -e "${YELLOW}   • Never use these for mainnet${NC}"
echo -e "${YELLOW}   • Wallet files are in .gitignore${NC}"
echo -e "${YELLOW}   • Run 'anchor test' to use these wallets${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════${NC}"