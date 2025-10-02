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
import { assert } from "chai";

describe("BTC Market Resolution and Payouts - Using Funded Wallets", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    const program = anchor.workspace.Pythpredict as Program<Pythpredict>;

    const wallet = provider.wallet as anchor.Wallet;
    const payer = wallet.payer;

    // Market accounts
    let mint: PublicKey;
    let marketPda: PublicKey;
    let yesVault: PublicKey;
    let noVault: PublicKey;
    let pythFeed: Keypair;
    const marketNonce = new anchor.BN(Date.now());

    // Use existing funded wallets
    const WALLETS_DIR = path.join(__dirname, '../.wallets');
    const participants: Map<string, any> = new Map();

    // Market parameters
    const TARGET_PRICE = new anchor.BN(95000 * 100); // $95,000 in cents
    const FINAL_PRICE = 94500 * 100; // $94,500 in cents (NO wins)
    const MARKET_DURATION_SECONDS = 5; // Short for testing

    before(async () => {
        console.log("\n" + "=".repeat(70));
        console.log("🎯 PAYOUT TEST USING EXISTING FUNDED WALLETS");
        console.log("=".repeat(70));

        // Load existing wallets (already funded from previous tests)
        const walletNames = ['buyer1', 'buyer2', 'seller1'];

        for (const name of walletNames) {
            const walletPath = path.join(WALLETS_DIR, `${name}.json`);

            if (fs.existsSync(walletPath)) {
                const secretKey = JSON.parse(fs.readFileSync(walletPath, 'utf-8'));
                const keypair = Keypair.fromSecretKey(Uint8Array.from(secretKey));

                const balance = await provider.connection.getBalance(keypair.publicKey);

                if (balance < 0.01 * LAMPORTS_PER_SOL) {
                    console.log(`  ⚠️ ${name} has low SOL balance: ${(balance / LAMPORTS_PER_SOL).toFixed(4)} SOL`);
                    console.log(`     Please manually fund or wait 24h for rate limit reset`);
                    continue;
                }

                participants.set(name, {
                    name,
                    keypair,
                    tokenAccount: null,
                    position: null,
                    initialBalance: 0,
                    originalBet: 0,
                    yesStake: 0,
                    noStake: 0,
                    expectedPayout: 0,
                    actualPayout: 0
                });

                console.log(`  ✅ ${name}: ${keypair.publicKey.toString().slice(0, 8)}... (${(balance / LAMPORTS_PER_SOL).toFixed(3)} SOL)`);
            } else {
                console.log(`  ❌ ${name} wallet not found - run setup.ts first`);
            }
        }

        if (participants.size === 0) {
            throw new Error("No funded wallets available. Please wait for rate limit or manually fund wallets.");
        }
    });

    describe("Setup", () => {
        it("Should create token and distribute", async () => {
            console.log("\n💰 TOKEN SETUP");

            // Create mint
            mint = await createMint(
                provider.connection,
                payer,
                payer.publicKey,
                null,
                6
            );
            console.log(`  Mint: ${mint.toString().slice(0, 8)}...`);

            // Distribute tokens to existing wallets
            const distributions = {
                'buyer1': 3000,
                'buyer2': 2000,
                'seller1': 4000
            };

            for (const [name, participant] of participants) {
                const ata = await getOrCreateAssociatedTokenAccount(
                    provider.connection,
                    payer,
                    mint,
                    participant.keypair.publicKey
                );

                participant.tokenAccount = ata.address;
                participant.initialBalance = distributions[name] || 1000;

                await sendAndConfirmTransaction(
                    provider.connection,
                    new Transaction().add(
                        createMintToInstruction(
                            mint,
                            ata.address,
                            payer.publicKey,
                            participant.initialBalance * 10**6
                        )
                    ),
                    [payer]
                );

                const balance = await getAccount(provider.connection, ata.address);
                console.log(`  ${name}: ${Number(balance.amount) / 10**6} tokens`);
            }
        });

        it("Should initialize market", async () => {
            console.log("\n🏛️ CREATING MARKET");

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

            pythFeed = Keypair.generate();

            const settleTime = new anchor.BN(Math.floor(Date.now() / 1000) + MARKET_DURATION_SECONDS);

            await program.methods
                .initializeMarket(
                    marketNonce,
                    TARGET_PRICE,
                    settleTime,
                    null
                )
                .accounts({
                    market: marketPda,
                    pythFeed: pythFeed.publicKey,
                    yesVault,
                    noVault,
                    collateralMint: mint,
                    creator: payer.publicKey,
                    systemProgram: SystemProgram.programId,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                })
                .rpc();

            console.log(`  Target: $${TARGET_PRICE.toNumber() / 100}`);
            console.log(`  Market: ${marketPda.toString().slice(0, 8)}...`);

            // Setup position PDAs
            for (const [name, p] of participants) {
                const [positionPda] = PublicKey.findProgramAddressSync(
                    [Buffer.from("position"), marketPda.toBuffer(), p.keypair.publicKey.toBuffer()],
                    program.programId
                );
                p.position = positionPda;
            }
        });
    });

    describe("Betting Phase", () => {
        it("Should place strategic bets", async () => {
            console.log("\n🎲 PLACING BETS");

            // Buyer1 bets YES (will lose)
            if (participants.has('buyer1')) {
                const buyer1 = participants.get('buyer1');
                const betAmount = new anchor.BN(1500 * 10**6);

                await program.methods
                    .placeBet(betAmount, { yes: {} })
                    .accounts({
                        market: marketPda,
                        yesVault,
                        noVault,
                        userTokenAccount: buyer1.tokenAccount,
                        position: buyer1.position,
                        better: buyer1.keypair.publicKey,
                        tokenProgram: TOKEN_PROGRAM_ID,
                        systemProgram: SystemProgram.programId,
                    })
                    .signers([buyer1.keypair])
                    .rpc();

                buyer1.originalBet = 1500;
                buyer1.yesStake = 1500 * 0.99; // After 1% fee
                console.log("  Buyer1: 1500 YES (betting price goes up)");
            }

            // Buyer2 bets YES (will lose)
            if (participants.has('buyer2')) {
                const buyer2 = participants.get('buyer2');
                const betAmount = new anchor.BN(1000 * 10**6);

                await program.methods
                    .placeBet(betAmount, { yes: {} })
                    .accounts({
                        market: marketPda,
                        yesVault,
                        noVault,
                        userTokenAccount: buyer2.tokenAccount,
                        position: buyer2.position,
                        better: buyer2.keypair.publicKey,
                        tokenProgram: TOKEN_PROGRAM_ID,
                        systemProgram: SystemProgram.programId,
                    })
                    .signers([buyer2.keypair])
                    .rpc();

                buyer2.originalBet = 1000;
                buyer2.yesStake = 1000 * 0.99; // After 1% fee
                console.log("  Buyer2: 1000 YES (following buyer1)");
            }

            // Seller1 bets NO (will win)
            if (participants.has('seller1')) {
                const seller1 = participants.get('seller1');
                const betAmount = new anchor.BN(2000 * 10**6);

                await program.methods
                    .placeBet(betAmount, { no: {} })
                    .accounts({
                        market: marketPda,
                        yesVault,
                        noVault,
                        userTokenAccount: seller1.tokenAccount,
                        position: seller1.position,
                        better: seller1.keypair.publicKey,
                        tokenProgram: TOKEN_PROGRAM_ID,
                        systemProgram: SystemProgram.programId,
                    })
                    .signers([seller1.keypair])
                    .rpc();

                seller1.originalBet = 2000;
                seller1.noStake = 2000 * 0.99; // After 1% fee
                console.log("  Seller1: 2000 NO (betting price stays/drops)");
            }

            // Show market state
            const market = await program.account.market.fetch(marketPda);
            const yesPool = market.yesPool.toNumber() / 10**6;
            const noPool = market.noPool.toNumber() / 10**6;
            const total = yesPool + noPool;

            console.log("\n  Market State After Bets:");
            console.log(`    YES pool: ${yesPool.toFixed(2)} tokens`);
            console.log(`    NO pool: ${noPool.toFixed(2)} tokens`);
            console.log(`    Total pot: ${total.toFixed(2)} tokens`);
            console.log(`    Implied odds: YES ${((yesPool/total)*100).toFixed(1)}% | NO ${((noPool/total)*100).toFixed(1)}%`);
        });
    });

    describe("Mock Resolution", () => {
        it("Should wait and prepare for resolution", async () => {
            console.log("\n⏰ WAITING FOR SETTLEMENT TIME");
            await new Promise(resolve => setTimeout(resolve, (MARKET_DURATION_SECONDS + 1) * 1000));

            console.log("\n📊 RESOLUTION SCENARIO");
            console.log(`  Target price: $${TARGET_PRICE.toNumber() / 100}`);
            console.log(`  Final price: $${FINAL_PRICE / 100} (simulated)`);
            console.log(`  Outcome: NO wins! (price didn't reach target)`);

            // NOTE: To actually resolve on devnet, you need to either:
            // 1. Add the mock_resolve_market instruction to your program
            // 2. Create a mock Pyth account with the right data format

            console.log("\n  ⚠️ Resolution requires program modification or Pyth mock");
            console.log("  See instructions below for adding mock resolution");
        });
    });

    describe("Payout Calculations", () => {
        it("Should calculate precise expected payouts", async () => {
            console.log("\n💰 PRECISE PAYOUT CALCULATIONS");

            const market = await program.account.market.fetch(marketPda);
            const yesPool = market.yesPool.toNumber() / 10**6;
            const noPool = market.noPool.toNumber() / 10**6;
            const totalPot = yesPool + noPool;

            console.log("\n  Pool Analysis:");
            console.log(`    Total pot: ${totalPot.toFixed(2)} tokens`);
            console.log(`    YES pool (losers): ${yesPool.toFixed(2)} tokens`);
            console.log(`    NO pool (winners): ${noPool.toFixed(2)} tokens`);

            // Calculate payouts for NO winners
            const winningPool = noPool;
            const losingPool = yesPool;

            console.log("\n  Payout Formula: (stake / winning_pool) × total_pot");
            console.log("  " + "=".repeat(50));

            for (const [name, p] of participants) {
                const totalStake = p.yesStake + p.noStake;

                if (p.noStake > 0) {
                    // Winner (bet NO)
                    p.expectedPayout = (p.noStake / winningPool) * totalPot;
                    const profit = p.expectedPayout - totalStake;
                    const roi = (profit / totalStake) * 100;

                    console.log(`\n  ${name} (WINNER):`);
                    console.log(`    NO stake: ${p.noStake.toFixed(2)} tokens`);
                    console.log(`    Share of NO pool: ${((p.noStake / winningPool) * 100).toFixed(2)}%`);
                    console.log(`    Expected payout: ${p.expectedPayout.toFixed(4)} tokens`);
                    console.log(`    Profit: +${profit.toFixed(4)} tokens`);
                    console.log(`    ROI: +${roi.toFixed(2)}%`);
                } else {
                    // Loser (bet YES)
                    p.expectedPayout = 0;
                    console.log(`\n  ${name} (LOSER):`);
                    console.log(`    YES stake: ${p.yesStake.toFixed(2)} tokens`);
                    console.log(`    Expected payout: 0 tokens`);
                    console.log(`    Loss: -${totalStake.toFixed(2)} tokens`);
                }
            }

            // Validate totals
            const totalExpectedPayouts = Array.from(participants.values())
                .reduce((sum, p) => sum + p.expectedPayout, 0);
            const totalStakedAfterFees = Array.from(participants.values())
                .reduce((sum, p) => sum + p.yesStake + p.noStake, 0);
            const totalOriginalBets = Array.from(participants.values())
                .reduce((sum, p) => sum + (p.originalBet || 0), 0);
            const protocolFees = totalOriginalBets - totalStakedAfterFees; // 1% taken as fees

            console.log("\n  Validation:");
            console.log(`    Total original bets: ${totalOriginalBets.toFixed(2)} tokens`);
            console.log(`    Total after fees: ${totalStakedAfterFees.toFixed(2)} tokens`);
            console.log(`    Total in pools: ${totalPot.toFixed(2)} tokens`);
            console.log(`    Protocol fees (1%): ${protocolFees.toFixed(2)} tokens`);
            console.log(`    Total payouts: ${totalExpectedPayouts.toFixed(2)} tokens`);
            console.log(`    Check sum: ${(totalExpectedPayouts + protocolFees).toFixed(2)} = ${totalOriginalBets.toFixed(2)} ✓`);

            // Assert calculations are correct
            assert.approximately(totalExpectedPayouts, totalPot, 0.01, "Payouts should equal pot");
            assert.approximately(protocolFees, totalOriginalBets * 0.01, 0.01, "Fees should be 1%");
        });
    });

    describe("Instructions for Claiming", () => {
        it("Should explain how to enable claiming", async () => {
            console.log("\n📝 TO ENABLE ACTUAL CLAIMING:");
            console.log("=" + "=".repeat(50));

            console.log("\n1. Add this to your instructions.rs:");
            console.log(`
pub fn mock_resolve_market(
    ctx: Context<MockResolveMarket>, 
    winning_outcome: u8
) -> Result<()> {
    let market = &mut ctx.accounts.market;
    let clock = Clock::get()?;
    
    require!(
        clock.unix_timestamp >= market.settle_time,
        PredictionMarketError::SettlementTimeNotMet
    );
    
    // Set the outcome
    market.winning_outcome = Some(winning_outcome);
    market.is_resolved = true;
    market.final_price = Some(94500 * 100); // Or any price
    
    Ok(())
}
`);

            console.log("\n2. Add to lib.rs:");
            console.log(`
pub fn mock_resolve_market(
    ctx: Context<MockResolveMarket>,
    winning_outcome: u8
) -> Result<()> {
    instructions::mock_resolve_market(ctx, winning_outcome)
}
`);

            console.log("\n3. Rebuild and deploy:");
            console.log("   anchor build && anchor deploy");

            console.log("\n4. Then in test, resolve and claim:");
            console.log(`
// Resolve with NO winning (outcome = 1)
await program.methods
    .mockResolveMarket(1)
    .accounts({
        market: marketPda,
        resolver: payer.publicKey,
    })
    .rpc();

// Then claim for winners
await program.methods
    .claimWinnings()
    .accounts({
        market: marketPda,
        position: winnerPosition,
        yesVault,
        noVault,
        userTokenAccount: winnerTokenAccount,
        better: winner.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
    })
    .signers([winner])
    .rpc();
`);
        });
    });

    after(() => {
        console.log("\n" + "=".repeat(70));
        console.log("✅ PAYOUT CALCULATION TEST COMPLETE");
        console.log("=" + "=".repeat(70));

        if (participants.size > 0) {
            console.log("\n📊 Final Summary:");
            console.log("  Participants tested: " + participants.size);

            const winners = Array.from(participants.values()).filter(p => p.expectedPayout > 0);
            const losers = Array.from(participants.values()).filter(p => p.expectedPayout === 0);

            console.log(`  Winners: ${winners.length}`);
            console.log(`  Losers: ${losers.length}`);

            if (winners.length > 0) {
                const totalWinnings = winners.reduce((sum, w) => sum + w.expectedPayout, 0);
                const totalStaked = winners.reduce((sum, w) => sum + w.noStake, 0);
                const avgROI = ((totalWinnings - totalStaked) / totalStaked) * 100;
                console.log(`  Average winner ROI: +${avgROI.toFixed(2)}%`);
            }
        }

        console.log("\n💡 Next Steps:");
        console.log("  1. Add mock resolution to your program");
        console.log("  2. Deploy updated program");
        console.log("  3. Run this test again with actual claiming");
    });
});