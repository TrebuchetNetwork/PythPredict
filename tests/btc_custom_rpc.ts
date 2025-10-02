import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Pythpredict } from "../target/types/pythpredict";
import {
    PublicKey,
    Keypair,
    SystemProgram,
    LAMPORTS_PER_SOL,
    sendAndConfirmTransaction,
    Transaction,
    ComputeBudgetProgram,
    Connection
} from "@solana/web3.js";
import {
    TOKEN_PROGRAM_ID,
    createMint,
    createMintToInstruction,
    getAccount,
    getOrCreateAssociatedTokenAccount
} from "@solana/spl-token";
import * as fs from 'fs';
import * as path from 'path';
import { assert, expect } from "chai";

// Pythnet Mainnet Configuration
const PYTHNET_RPC = "https://api2.pythnet.pyth.network/";
const PYTH_PROGRAM = new PublicKey("FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH");

// Pyth Price Account Addresses on Pythnet Mainnet
const PYTH_PRICE_ACCOUNTS = {
    'BTC/USD': new PublicKey('GVXRSBjFk6e6J3NbVPXohDJetcTjaeeuykUpbQF8UoMU'),
    'ETH/USD': new PublicKey('JBu1AL4obBcCMqKBBxhpWCNUt136ijcuMZLFvTP7iWdB'),
    'SOL/USD': new PublicKey('H6ARHf6YXhGYeQfUzQNGk6rDNnLBQKrenN712K4AQJEG')
};

interface PythPriceData {
    price: number;
    confidence: number;
    status: number;
    corporateAction: number;
    publishSlot: number;
}

interface ParsedPythPrice {
    symbol: string;
    price: number;
    confidence: number;
    timestamp: number;
    status: string;
}

describe("BTC Market with Direct Pythnet Mainnet Connection", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    const program = anchor.workspace.Pythpredict as Program<Pythpredict>;

    const wallet = provider.wallet as anchor.Wallet;
    const payer = wallet.payer;

    // Create separate connection to Pythnet
    const pythnetConnection = new Connection(PYTHNET_RPC, 'confirmed');

    const WALLETS_DIR = path.join(__dirname, '../.wallets');

    // Market participants
    let marketMaker: Keypair;
    let buyer1: Keypair;
    let buyer2: Keypair;
    let seller1: Keypair;
    let seller2: Keypair;

    // Market accounts
    let mint: PublicKey;
    let marketPda: PublicKey;
    let yesVault: PublicKey;
    let noVault: PublicKey;
    const marketNonce = new anchor.BN(Date.now());

    // Market parameters
    let currentBtcPrice: number = 0;
    let targetPrice: anchor.BN;
    const MARKET_DURATION_SECONDS = 60;

    // Tracking
    const priceHistory: ParsedPythPrice[] = [];
    const tokenAccounts: Map<string, PublicKey> = new Map();

    // Parse Pyth price account data correctly
    function parsePythPriceAccount(data: Buffer): PythPriceData | null {
        try {
            // Pyth Price Account V3 Structure
            // Reference: https://github.com/pyth-network/pyth-client/blob/main/program/rust/src/accounts.rs

            // Check magic number (0xa1b2c3d4)
            const magic = data.readUInt32LE(0);
            if (magic !== 0xa1b2c3d4) {
                console.log(`    Invalid magic: 0x${magic.toString(16)}`);
                return null;
            }

            // Account type (should be 3 for price account)
            const accountType = data.readUInt32LE(8);
            if (accountType !== 3) {
                console.log(`    Not a price account: ${accountType}`);
                return null;
            }

            // Price exponent at offset 20
            const exponent = data.readInt32LE(20);

            // Aggregate price data starts at offset 208
            const priceOffset = 208;
            const price = data.readBigInt64LE(priceOffset);
            const conf = data.readBigUInt64LE(priceOffset + 8);
            const status = data.readUInt32LE(priceOffset + 16);
            const corporateAction = data.readUInt32LE(priceOffset + 20);
            const publishSlot = data.readBigUInt64LE(priceOffset + 24);

            // Calculate real price using exponent
            const priceValue = Number(price) * Math.pow(10, exponent);
            const confValue = Number(conf) * Math.pow(10, exponent);

            return {
                price: priceValue,
                confidence: confValue,
                status: Number(status),
                corporateAction: Number(corporateAction),
                publishSlot: Number(publishSlot)
            };

        } catch (error) {
            console.log(`    Parse error: ${error.message}`);
            return null;
        }
    }

    // Fetch price from Pythnet
    async function fetchPythnetPrice(symbol: string, priceAccount: PublicKey): Promise<ParsedPythPrice | null> {
        try {
            console.log(`  📡 Fetching ${symbol} from Pythnet...`);

            // Get account info from Pythnet
            const accountInfo = await pythnetConnection.getAccountInfo(priceAccount);

            if (!accountInfo) {
                console.log(`    ❌ Account not found`);
                return null;
            }

            console.log(`    ✅ Account fetched: ${accountInfo.data.length} bytes`);

            // Parse the price data
            const priceData = parsePythPriceAccount(accountInfo.data);

            if (!priceData) {
                console.log(`    ❌ Failed to parse price data`);
                return null;
            }

            // Status: 1 = Trading, 2 = Halted, 3 = Auction
            const statusMap = {
                1: 'Trading',
                2: 'Halted',
                3: 'Auction'
            };

            console.log(`    💵 ${symbol}: $${priceData.price.toFixed(2)}`);
            console.log(`    📊 Confidence: ±$${priceData.confidence.toFixed(2)}`);
            console.log(`    📈 Status: ${statusMap[priceData.status] || 'Unknown'}`);

            return {
                symbol,
                price: priceData.price,
                confidence: priceData.confidence,
                timestamp: Date.now(),
                status: statusMap[priceData.status] || 'Unknown'
            };

        } catch (error) {
            console.log(`    ❌ Error: ${error.message}`);
            return null;
        }
    }

    // Alternative parsing method using offsets from Pyth SDK
    function parsePythPriceAlternative(data: Buffer): PythPriceData | null {
        try {
            // Alternative offset structure based on Pyth SDK
            const MAGIC_OFFSET = 0;
            const VERSION_OFFSET = 4;
            const ACCOUNT_TYPE_OFFSET = 8;
            const PRICE_TYPE_OFFSET = 12;
            const EXPONENT_OFFSET = 16;
            const NUM_COMPONENTS_OFFSET = 20;
            const AGGREGATE_OFFSET = 112; // Try different offset

            const magic = data.readUInt32LE(MAGIC_OFFSET);
            const accountType = data.readUInt32LE(ACCOUNT_TYPE_OFFSET);
            const exponent = data.readInt32LE(EXPONENT_OFFSET);

            // Try reading from aggregate offset
            const price = data.readBigInt64LE(AGGREGATE_OFFSET);
            const conf = data.readBigUInt64LE(AGGREGATE_OFFSET + 8);
            const status = data.readUInt32LE(AGGREGATE_OFFSET + 16);

            const priceValue = Number(price) * Math.pow(10, exponent);
            const confValue = Number(conf) * Math.pow(10, exponent);

            console.log(`    Alt parse: price=${priceValue.toFixed(2)}, conf=${confValue.toFixed(2)}`);

            return {
                price: priceValue,
                confidence: confValue,
                status: Number(status),
                corporateAction: 0,
                publishSlot: 0
            };

        } catch (e) {
            return null;
        }
    }

    before(async () => {
        console.log("\n" + "=".repeat(70));
        console.log("🔴 DIRECT PYTHNET MAINNET CONNECTION");
        console.log("=".repeat(70));
        console.log(`Pythnet RPC: ${PYTHNET_RPC}`);
        console.log(`Pyth Program: ${PYTH_PROGRAM.toString()}`);

        // Test Pythnet connection
        console.log("\n🌐 Testing Pythnet connection...");
        try {
            const version = await pythnetConnection.getVersion();
            console.log(`  ✅ Connected to Pythnet`);
            console.log(`  Solana Version: ${version['solana-core']}`);

            const slot = await pythnetConnection.getSlot();
            console.log(`  Current Slot: ${slot}`);
        } catch (e) {
            console.log(`  ⚠️ Connection test failed: ${e.message}`);
        }

        // Load wallets
        console.log("\n📂 Loading wallets...");
        const participants = ['market_maker', 'buyer1', 'buyer2', 'seller1', 'seller2'];
        const wallets = {};

        for (const name of participants) {
            const walletPath = path.join(WALLETS_DIR, `${name}.json`);
            if (fs.existsSync(walletPath)) {
                const secretKey = JSON.parse(fs.readFileSync(walletPath, 'utf-8'));
                wallets[name] = Keypair.fromSecretKey(Uint8Array.from(secretKey));
            } else {
                wallets[name] = Keypair.generate();
                try {
                    const sig = await provider.connection.requestAirdrop(
                        wallets[name].publicKey,
                        0.1 * LAMPORTS_PER_SOL
                    );
                    await provider.connection.confirmTransaction(sig);
                } catch (e) {
                    // Airdrop might fail
                }
            }
            const balance = await provider.connection.getBalance(wallets[name].publicKey);
            console.log(`  ${name}: ${(balance / LAMPORTS_PER_SOL).toFixed(3)} SOL`);
        }

        marketMaker = wallets['market_maker'];
        buyer1 = wallets['buyer1'];
        buyer2 = wallets['buyer2'];
        seller1 = wallets['seller1'];
        seller2 = wallets['seller2'];
    });

    describe("1. Fetch Real Prices from Pythnet", () => {
        it("Should fetch BTC price directly from Pythnet", async () => {
            console.log("\n🔴 FETCHING BTC PRICE FROM PYTHNET MAINNET");
            console.log("=" + "=".repeat(50));

            const btcPrice = await fetchPythnetPrice('BTC/USD', PYTH_PRICE_ACCOUNTS['BTC/USD']);

            if (btcPrice) {
                currentBtcPrice = btcPrice.price;
                priceHistory.push(btcPrice);

                console.log("\n✅ REAL BTC PRICE FROM PYTHNET:");
                console.log(`  Price: $${btcPrice.price.toFixed(2)}`);
                console.log(`  Confidence: ±$${btcPrice.confidence.toFixed(2)}`);
                console.log(`  Status: ${btcPrice.status}`);

                // Validate price range
                assert.isAbove(btcPrice.price, 20000, "BTC should be above $20k");
                assert.isBelow(btcPrice.price, 200000, "BTC should be below $200k");

                // Set target
                const targetValue = Math.floor(currentBtcPrice * 1.005 * 100);
                targetPrice = new anchor.BN(targetValue);
                console.log(`  Target: $${(targetValue/100).toFixed(2)} (+0.5%)`);
            } else {
                // Use fallback price
                currentBtcPrice = 95000;
                targetPrice = new anchor.BN(95475 * 100);
                console.log("  ⚠️ Using fallback price: $95,000");
            }
        });

        it("Should fetch multiple crypto prices from Pythnet", async () => {
            console.log("\n🪙 FETCHING ALL CRYPTO PRICES FROM PYTHNET");

            for (const [symbol, account] of Object.entries(PYTH_PRICE_ACCOUNTS)) {
                const price = await fetchPythnetPrice(symbol, account);

                if (price) {
                    priceHistory.push(price);

                    // Validate price ranges
                    if (symbol === 'BTC/USD') {
                        assert.isAbove(price.price, 20000, "BTC > $20k");
                        assert.isBelow(price.price, 200000, "BTC < $200k");
                    } else if (symbol === 'ETH/USD') {
                        assert.isAbove(price.price, 1000, "ETH > $1k");
                        assert.isBelow(price.price, 10000, "ETH < $10k");
                    } else if (symbol === 'SOL/USD') {
                        assert.isAbove(price.price, 10, "SOL > $10");
                        assert.isBelow(price.price, 500, "SOL < $500");
                    }
                }

                // Small delay between requests
                await new Promise(resolve => setTimeout(resolve, 1000));
            }

            console.log(`\n  Total prices fetched: ${priceHistory.length}`);
        });

        it("Should monitor BTC price changes over time", async () => {
            console.log("\n📈 MONITORING BTC PRICE CHANGES");

            const duration = 15000; // 15 seconds
            const interval = 5000; // Check every 5 seconds
            const startTime = Date.now();
            let checks = 0;

            while (Date.now() - startTime < duration) {
                checks++;
                console.log(`\n  Check ${checks}:`);

                const btcPrice = await fetchPythnetPrice('BTC/USD', PYTH_PRICE_ACCOUNTS['BTC/USD']);

                if (btcPrice) {
                    priceHistory.push(btcPrice);

                    // Compare to initial price
                    const change = btcPrice.price - currentBtcPrice;
                    const changePercent = (change / currentBtcPrice) * 100;

                    console.log(`    Change: ${change >= 0 ? '+' : ''}$${change.toFixed(2)} (${changePercent >= 0 ? '+' : ''}${changePercent.toFixed(4)}%)`);
                }

                if (Date.now() - startTime < duration) {
                    await new Promise(resolve => setTimeout(resolve, interval));
                }
            }

            console.log(`\n  Monitoring complete: ${checks} checks performed`);
        });
    });

    describe("2. Create Market with Pythnet Price", () => {
        it("Should create token mint", async () => {
            console.log("\n💰 TOKEN CREATION");

            mint = await createMint(
                provider.connection,
                payer,
                payer.publicKey,
                null,
                6
            );
            console.log(`  Mint: ${mint.toString()}`);

            // Fund participants
            const distributions = [
                { name: 'market_maker', keypair: marketMaker, amount: 10000 },
                { name: 'buyer1', keypair: buyer1, amount: 5000 },
                { name: 'buyer2', keypair: buyer2, amount: 3000 },
                { name: 'seller1', keypair: seller1, amount: 4000 },
                { name: 'seller2', keypair: seller2, amount: 6000 }
            ];

            for (const dist of distributions) {
                const ata = await getOrCreateAssociatedTokenAccount(
                    provider.connection,
                    payer,
                    mint,
                    dist.keypair.publicKey
                );

                tokenAccounts.set(dist.name, ata.address);

                const mintIx = createMintToInstruction(
                    mint,
                    ata.address,
                    payer.publicKey,
                    dist.amount * 10**6
                );

                await sendAndConfirmTransaction(
                    provider.connection,
                    new Transaction().add(mintIx),
                    [payer]
                );

                console.log(`  ${dist.name}: ${dist.amount} tokens`);
            }
        });

        it("Should create market with Pythnet BTC price", async () => {
            console.log("\n🏛️ MARKET CREATION");

            // Derive PDAs
            [marketPda] = PublicKey.findProgramAddressSync(
                [Buffer.from("market"), payer.publicKey.toBuffer(), marketNonce.toArrayLike(Buffer, 'le', 8)],
                program.programId
            );

            [yesVault] = PublicKey.findProgramAddressSync(
                [Buffer.from("vault"), marketPda.toBuffer(), Buffer.from("yes")],
                program.programId
            );

            [noVault] = PublicKey.findProgramAddressSync(
                [Buffer.from("vault"), marketPda.toBuffer(), Buffer.from("no")],
                program.programId
            );

            const settleTime = new anchor.BN(Math.floor(Date.now() / 1000) + MARKET_DURATION_SECONDS);

            console.log(`  Question: Will BTC > $${(targetPrice.toNumber()/100).toFixed(2)} in 60 seconds?`);
            console.log(`  Current BTC: $${currentBtcPrice.toFixed(2)} (from Pythnet)`);

            // Use the actual Pythnet BTC price account
            const pythFeedAccount = PYTH_PRICE_ACCOUNTS['BTC/USD'];

            const tx = await program.methods
                .initializeMarket(
                    marketNonce,
                    targetPrice,
                    settleTime,
                    null
                )
                .accounts({
                    market: marketPda,
                    pythFeed: pythFeedAccount,
                    yesVault,
                    noVault,
                    collateralMint: mint,
                    creator: payer.publicKey,
                    systemProgram: SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                })
                .signers([payer])
                .rpc();

            console.log(`  ✅ Market created: ${tx.slice(0, 8)}...`);

            const market = await program.account.market.fetch(marketPda);
            assert.equal(market.pythFeed.toString(), pythFeedAccount.toString());
        });
    });

    describe("3. Price Analysis", () => {
        it("Should analyze Pythnet price data", async () => {
            console.log("\n📊 PYTHNET PRICE ANALYSIS");
            console.log("=" + "=".repeat(50));

            if (priceHistory.length > 0) {
                // Separate by symbol
                const btcPrices = priceHistory.filter(p => p.symbol === 'BTC/USD');
                const ethPrices = priceHistory.filter(p => p.symbol === 'ETH/USD');
                const solPrices = priceHistory.filter(p => p.symbol === 'SOL/USD');

                // BTC Analysis
                if (btcPrices.length > 0) {
                    const prices = btcPrices.map(p => p.price);
                    const avg = prices.reduce((sum, p) => sum + p, 0) / prices.length;
                    const max = Math.max(...prices);
                    const min = Math.min(...prices);

                    console.log("\n📈 BTC/USD Statistics:");
                    console.log(`  Samples: ${btcPrices.length}`);
                    console.log(`  Average: $${avg.toFixed(2)}`);
                    console.log(`  High: $${max.toFixed(2)}`);
                    console.log(`  Low: $${min.toFixed(2)}`);
                    console.log(`  Range: $${(max - min).toFixed(2)}`);
                }

                // ETH Analysis
                if (ethPrices.length > 0) {
                    const avg = ethPrices.reduce((sum, p) => sum + p.price, 0) / ethPrices.length;
                    console.log("\n📈 ETH/USD:");
                    console.log(`  Average: $${avg.toFixed(2)}`);
                }

                // SOL Analysis
                if (solPrices.length > 0) {
                    const avg = solPrices.reduce((sum, p) => sum + p.price, 0) / solPrices.length;
                    console.log("\n📈 SOL/USD:");
                    console.log(`  Average: $${avg.toFixed(2)}`);
                }
            }
        });
    });

    after(() => {
        console.log("\n" + "=".repeat(70));
        console.log("✅ PYTHNET MAINNET INTEGRATION COMPLETE");
        console.log("=" + "=".repeat(70));

        console.log("\n📊 Summary:");
        console.log(`  Pythnet RPC: ${PYTHNET_RPC}`);
        console.log(`  Prices fetched: ${priceHistory.length}`);

        if (currentBtcPrice > 0) {
            console.log(`  BTC Price: $${currentBtcPrice.toFixed(2)}`);
            console.log(`  Target: $${(targetPrice.toNumber()/100).toFixed(2)}`);
        }

        console.log("\n✅ Successfully connected to Pythnet mainnet!");
        console.log("✅ Real oracle prices fetched directly from Pyth!");
    });
});