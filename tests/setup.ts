import {
    Connection,
    Keypair,
    LAMPORTS_PER_SOL,
    PublicKey,
    sendAndConfirmTransaction,
    Transaction,
    SystemProgram,
    clusterApiUrl
} from "@solana/web3.js";
import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';
import { dirname } from 'path';
import * as os from 'os';

// ES module fix for __dirname
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const WALLETS_DIR = path.join(__dirname, '../.wallets');
const PARTICIPANTS = ['market_maker', 'buyer1', 'buyer2', 'seller1', 'seller2'];
const SOL_PER_PARTICIPANT = 0.3;

async function setupWallets() {
    console.log("=== PARTICIPANT WALLET SETUP ===\n");

    // Create wallets directory if it doesn't exist
    if (!fs.existsSync(WALLETS_DIR)) {
        fs.mkdirSync(WALLETS_DIR, { recursive: true });
        console.log(`✅ Created wallets directory: ${WALLETS_DIR}`);
    }

    // Load main wallet (payer)
    const walletPath = process.env.ANCHOR_WALLET || path.join(os.homedir(), '.config/solana/id.json');
    if (!fs.existsSync(walletPath)) {
        throw new Error(`Wallet not found at ${walletPath}. Please run 'solana-keygen new' first.`);
    }

    const payerSecretKey = JSON.parse(fs.readFileSync(walletPath, 'utf-8'));
    const payer = Keypair.fromSecretKey(Uint8Array.from(payerSecretKey));

    // Connect to cluster
    const rpcUrl = process.env.ANCHOR_PROVIDER_URL || 'http://127.0.0.1:8899';
    const connection = new Connection(rpcUrl, 'confirmed');

    console.log("Funding wallet:", payer.publicKey.toString());
    console.log("RPC URL:", rpcUrl);

    const payerBalance = await connection.getBalance(payer.publicKey);
    console.log("Payer balance:", payerBalance / LAMPORTS_PER_SOL, "SOL\n");

    const requiredSol = SOL_PER_PARTICIPANT * PARTICIPANTS.length;
    if (payerBalance < requiredSol * LAMPORTS_PER_SOL) {
        console.error(`❌ Insufficient balance. Need ${requiredSol} SOL, have ${payerBalance / LAMPORTS_PER_SOL} SOL`);
        return;
    }

    const wallets: { [key: string]: Keypair } = {};

    for (const participant of PARTICIPANTS) {
        const walletFilePath = path.join(WALLETS_DIR, `${participant}.json`);

        let keypair: Keypair;

        // Check if wallet already exists
        if (fs.existsSync(walletFilePath)) {
            console.log(`📂 Loading existing wallet for ${participant}`);
            const secretKey = JSON.parse(fs.readFileSync(walletFilePath, 'utf-8'));
            keypair = Keypair.fromSecretKey(Uint8Array.from(secretKey));
        } else {
            console.log(`🆕 Creating new wallet for ${participant}`);
            keypair = Keypair.generate();

            // Save wallet to file
            fs.writeFileSync(
                walletFilePath,
                JSON.stringify(Array.from(keypair.secretKey))
            );
        }

        wallets[participant] = keypair;
        console.log(`  Address: ${keypair.publicKey.toString()}`);

        // Check current balance
        const currentBalance = await connection.getBalance(keypair.publicKey);
        console.log(`  Current balance: ${(currentBalance / LAMPORTS_PER_SOL).toFixed(4)} SOL`);

        // Transfer SOL if needed
        if (currentBalance < SOL_PER_PARTICIPANT * LAMPORTS_PER_SOL) {
            const transferAmount = Math.floor((SOL_PER_PARTICIPANT * LAMPORTS_PER_SOL) - currentBalance);

            if (transferAmount > 0) {
                console.log(`  💸 Transferring ${(transferAmount / LAMPORTS_PER_SOL).toFixed(4)} SOL...`);

                const transferTx = new Transaction().add(
                    SystemProgram.transfer({
                        fromPubkey: payer.publicKey,
                        toPubkey: keypair.publicKey,
                        lamports: transferAmount,
                    })
                );

                try {
                    const signature = await sendAndConfirmTransaction(
                        connection,
                        transferTx,
                        [payer],
                        { commitment: 'confirmed' }
                    );
                    console.log(`  ✅ Transfer complete: ${signature.slice(0, 8)}...`);

                    // Verify new balance
                    const newBalance = await connection.getBalance(keypair.publicKey);
                    console.log(`  New balance: ${(newBalance / LAMPORTS_PER_SOL).toFixed(4)} SOL`);
                } catch (error: any) {
                    console.error(`  ❌ Transfer failed:`, error.message);
                }
            }
        } else {
            console.log(`  ✅ Already funded`);
        }

        console.log();
    }

    // Save wallet addresses to a reference file
    const addressesPath = path.join(WALLETS_DIR, 'addresses.json');
    const addresses: { [key: string]: string } = {};

    for (const [name, keypair] of Object.entries(wallets)) {
        addresses[name] = keypair.publicKey.toString();
    }

    fs.writeFileSync(addressesPath, JSON.stringify(addresses, null, 2));
    console.log(`📝 Saved wallet addresses to ${addressesPath}`);

    // Display summary
    console.log("\n=== WALLET SETUP COMPLETE ===");
    console.log("\nParticipant wallets ready:");
    for (const [name, address] of Object.entries(addresses)) {
        const balance = await connection.getBalance(new PublicKey(address));
        console.log(`  ${name}: ${address.slice(0, 8)}... (${(balance / LAMPORTS_PER_SOL).toFixed(3)} SOL)`);
    }

    console.log(`\n✅ All wallets saved in: ${WALLETS_DIR}`);
    console.log("\n📁 Wallet files created:");
    PARTICIPANTS.forEach(p => {
        console.log(`  - ${p}.json`);
    });
    console.log(`  - addresses.json (reference file)`);

    console.log("\n🚀 You can now run the market simulation!");

    // Show environment for verification
    console.log("\n📋 Environment:");
    console.log(`  Cluster: ${rpcUrl}`);
    console.log(`  Wallet: ${walletPath}`);
}

// Run the setup
console.log("Starting wallet setup...\n");

setupWallets()
    .then(() => {
        console.log("\n✅ Setup completed successfully!");
        process.exit(0);
    })
    .catch((error) => {
        console.error("\n❌ Setup failed:", error);
        process.exit(1);
    });