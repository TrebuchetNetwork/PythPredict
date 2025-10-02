use anchor_lang::{prelude::*, InstructionData, ToAccountMetas};
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction,
    signature::{Keypair, Signer},
    system_instruction, system_program, transaction::Transaction,
};
use spl_token::{
    instruction as token_ix,
    state::{Account as TokenAccount, Mint},
    ID as TOKEN_PROGRAM_ID,
};

use pythpredict::{accounts as accts, instruction as ix, state::Outcome};

/// --------- Helpers ----------

async fn latest_blockhash(banks: &mut BanksClient) -> solana_sdk::hash::Hash {
    banks.get_latest_blockhash().await.unwrap()
}

async fn rent_lamports(banks: &mut BanksClient, size: usize) -> u64 {
    let rent = banks.get_rent().await.unwrap();
    rent.minimum_balance(size)
}

async fn pay(
    banks: &mut BanksClient,
    fee_payer: &Keypair,
    from: &Keypair,
    to: &Pubkey,
    lamports: u64,
) {
    let bh = latest_blockhash(banks).await;
    let tx = Transaction::new_signed_with_payer(
        &[system_instruction::transfer(&from.pubkey(), to, lamports)],
        Some(&fee_payer.pubkey()),
        &[fee_payer, from],
        bh,
    );
    banks.process_transaction(tx).await.unwrap();
}

/// Create an SPL mint with `decimals` and `mint_authority`.
async fn create_mint(
    banks: &mut BanksClient,
    fee_payer: &Keypair,
    mint_kp: &Keypair,
    mint_authority: &Pubkey,
    decimals: u8,
) {
    let lamports = rent_lamports(banks, Mint::LEN).await;
    let bh = latest_blockhash(banks).await;

    let tx = Transaction::new_signed_with_payer(
        &[
            system_instruction::create_account(
                &fee_payer.pubkey(),
                &mint_kp.pubkey(),
                lamports,
                Mint::LEN as u64,
                &TOKEN_PROGRAM_ID,
            ),
            token_ix::initialize_mint(
                &TOKEN_PROGRAM_ID,
                &mint_kp.pubkey(),
                mint_authority,
                None,
                decimals,
            )
            .unwrap(),
        ],
        Some(&fee_payer.pubkey()),
        &[fee_payer, mint_kp],
        bh,
    );
    banks.process_transaction(tx).await.unwrap();
}

/// Create a plain SPL token account owned by `owner` for `mint`.
async fn create_token_account(
    banks: &mut BanksClient,
    fee_payer: &Keypair,
    owner: &Pubkey,
    mint: &Pubkey,
) -> Keypair {
    let ta = Keypair::new();
    let lamports = rent_lamports(banks, TokenAccount::LEN).await;
    let bh = latest_blockhash(banks).await;

    let tx = Transaction::new_signed_with_payer(
        &[
            system_instruction::create_account(
                &fee_payer.pubkey(),
                &ta.pubkey(),
                lamports,
                TokenAccount::LEN as u64,
                &TOKEN_PROGRAM_ID,
            ),
            token_ix::initialize_account(&TOKEN_PROGRAM_ID, &ta.pubkey(), mint, owner).unwrap(),
        ],
        Some(&fee_payer.pubkey()),
        &[fee_payer, &ta],
        bh,
    );
    banks.process_transaction(tx).await.unwrap();
    ta
}

/// Mint `amount` tokens to `dest_ta` using `mint_authority`.
async fn mint_to(
    banks: &mut BanksClient,
    fee_payer: &Keypair,
    mint: &Pubkey,
    dest_ta: &Pubkey,
    mint_authority: &Keypair,
    amount: u64,
) {
    let bh = latest_blockhash(banks).await;
    let tx = Transaction::new_signed_with_payer(
        &[token_ix::mint_to(
            &TOKEN_PROGRAM_ID,
            mint,
            dest_ta,
            &mint_authority.pubkey(),
            &[],
            amount,
        )
        .unwrap()],
        Some(&fee_payer.pubkey()),
        &[fee_payer, mint_authority],
        bh,
    );
    banks.process_transaction(tx).await.unwrap();
}

async fn read_token_amount(banks: &mut BanksClient, account: &Pubkey) -> u64 {
    use spl_token::state::Account as TokenAccState;
    use spl_token::state::AccountState;
    use spl_token::state::Pack;

    let acc = banks.get_account(*account).await.unwrap().unwrap();
    let parsed = TokenAccState::unpack_from_slice(&acc.data).unwrap();
    assert_eq!(parsed.state, AccountState::Initialized);
    parsed.amount
}

/// Derive all PDAs we need (market, vaults, fee collector).
fn derive_pdas(
    program_id: &Pubkey,
    creator: &Pubkey,
    market_nonce: u64,
) -> (Pubkey, u8, Pubkey, Pubkey, Pubkey, Pubkey) {
    let (market, bump) = Pubkey::find_program_address(
        &[b"market", creator.as_ref(), &market_nonce.to_le_bytes()],
        program_id,
    );
    let (yes_vault, _) = Pubkey::find_program_address(&[b"yes_vault", market.as_ref()], program_id);
    let (no_vault, _) = Pubkey::find_program_address(&[b"no_vault", market.as_ref()], program_id);
    let (fee_vault, _) = Pubkey::find_program_address(&[b"fee_vault", market.as_ref()], program_id);
    let (fee_collector, _) = Pubkey::find_program_address(&[b"fee_collector"], program_id);
    (market, bump, yes_vault, no_vault, fee_vault, fee_collector)
}

/// Derive the position PDA for (market, better)
fn derive_position_pda(program_id: &Pubkey, market: &Pubkey, better: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"position", market.as_ref(), better.as_ref()],
        program_id,
    )
    .0
}

/// -------------------- TESTS --------------------

#[tokio::test]
async fn test_initialize_market_place_bet_resolve_and_claim() {
    // Use your program id from `declare_id!` in lib.rs
    let program_id = pythpredict::id();

    // Register the program entrypoint
    let mut pt = ProgramTest::new("pythpredict", program_id, processor!(pythpredict::entry));

    // Spin up bank
    let (mut banks, payer, _recent) = pt.start().await;

    // Actors
    let creator = Keypair::new();
    let better = Keypair::new();

    // Fund creator & better
    pay(&mut banks, &payer, &payer, &creator.pubkey(), 10_000_000_000).await;
    pay(&mut banks, &payer, &payer, &better.pubkey(), 10_000_000_000).await;

    // Create mint (6 decimals) with creator as mint authority
    let decimals = 6u8;
    let collateral_mint = Keypair::new();
    create_mint(
        &mut banks,
        &payer,
        &collateral_mint,
        &creator.pubkey(),
        decimals,
    )
    .await;

    // Create user's token account, mint them some collateral
    let user_ta = create_token_account(
        &mut banks,
        &payer,
        &better.pubkey(),
        &collateral_mint.pubkey(),
    )
    .await;
    mint_to(
        &mut banks,
        &payer,
        &collateral_mint.pubkey(),
        &user_ta.pubkey(),
        &creator,
        5_000_000_000, // 5000 tokens (w/ 6 decimals)
    )
    .await;

    // Derive PDAs
    let market_nonce: u64 = 42;
    let (market_pda, _mbump, yes_vault, no_vault, fee_vault, fee_collector) =
        derive_pdas(&program_id, &creator.pubkey(), market_nonce);

    // Dummy Pyth account (unchecked by our test path)
    let pyth_feed = Keypair::new().pubkey();

    // --- Initialize market ---
    let init_accounts = accts::InitializeMarket {
        market: market_pda,
        yes_vault,
        no_vault,
        fee_vault,
        fee_collector,
        collateral_mint: collateral_mint.pubkey(),
        pyth_feed,
        creator: creator.pubkey(),
        system_program: system_program::id(),
        token_program: TOKEN_PROGRAM_ID,
    };
    let init_data = ix::InitializeMarket {
        market_nonce,
        initial_price: 95_000_00, // $95,000 with expo -2 convention
        target_change_bps: 0,
        settle_time: 2_000_000_000, // safely in the future
        resolver_authority: None,
    }
    .data();
    let init_ix = Instruction {
        program_id,
        accounts: init_accounts.to_account_metas(None),
        data: init_data,
    };
    {
        let bh = latest_blockhash(&mut banks).await;
        let mut tx = Transaction::new_with_payer(&[init_ix], Some(&payer.pubkey()));
        tx.sign(&[&payer, &creator], bh);
        banks.process_transaction(tx).await.unwrap();
    }

    // --- Place a YES bet (1 token) ---
    let amount: u64 = 1_000_000; // 1.0 token (6dp)
    let position_pda = derive_position_pda(&program_id, &market_pda, &better.pubkey());

    let bet_accounts = accts::Bet {
        market: market_pda,
        position: position_pda,
        user_token_account: user_ta.pubkey(),
        yes_vault,
        no_vault,
        fee_vault,
        better: better.pubkey(),
        token_program: TOKEN_PROGRAM_ID,
        system_program: system_program::id(),
    };
    let bet_data = ix::PlaceBet {
        amount,
        outcome: Outcome::Yes,
    }
    .data();
    let bet_ix = Instruction {
        program_id,
        accounts: bet_accounts.to_account_metas(None),
        data: bet_data,
    };
    {
        let bh = latest_blockhash(&mut banks).await;
        let mut tx = Transaction::new_with_payer(&[bet_ix], Some(&payer.pubkey()));
        tx.sign(&[&payer, &better], bh);
        banks.process_transaction(tx).await.unwrap();
    }

    // --- Resolve with external price (different from initial → YES wins) ---
    let resolve_accounts = accts::ResolveWithExternalPrice {
        market: market_pda,
        resolver: creator.pubkey(), // default resolver is creator when None was passed
    };
    let resolve_data = ix::ResolveWithExternalPrice {
        final_price: 96_000_00, // moved → YES wins
    }
    .data();
    let resolve_ix = Instruction {
        program_id,
        accounts: resolve_accounts.to_account_metas(None),
        data: resolve_data,
    };
    {
        let bh = latest_blockhash(&mut banks).await;
        let mut tx = Transaction::new_with_payer(&[resolve_ix], Some(&payer.pubkey()));
        tx.sign(&[&payer, &creator], bh);
        banks.process_transaction(tx).await.unwrap();
    }

    // --- Claim winnings ---
    // record user TA balance before
    let before = read_token_amount(&mut banks, &user_ta.pubkey()).await;

    let claim_accounts = accts::ClaimWinnings {
        market: market_pda,
        position: position_pda,
        yes_vault,
        no_vault,
        user_token_account: user_ta.pubkey(),
        claimer: better.pubkey(),
        token_program: TOKEN_PROGRAM_ID,
    };
    let claim_ix = Instruction {
        program_id,
        accounts: claim_accounts.to_account_metas(None),
        data: ix::ClaimWinnings {}.data(),
    };
    {
        let bh = latest_blockhash(&mut banks).await;
        let mut tx = Transaction::new_with_payer(&[claim_ix], Some(&payer.pubkey()));
        tx.sign(&[&payer, &better], bh);
        banks.process_transaction(tx).await.unwrap();
    }

    // After: with only one YES bet and YES winning, payout == stake_after_fee.
    // Your fee_bps is 100 (1%), so fee = 0.01 * amount → 10_000; stake_after_fee = 990_000.
    let after = read_token_amount(&mut banks, &user_ta.pubkey()).await;
    assert!(after > before, "user should receive payout");
    assert_eq!(after - before, 990_000, "payout should equal amount minus fee");
}

/// (Optional) tiny smoke test for `resolve_market` path that reads a Pyth account:
/// You can extend this by writing the expected bytes at offsets [208..228] if you
/// want to exercise `parse_pyth_price_alternative` end-to-end as well.
#[tokio::test]
async fn test_initialize_only() {
    let program_id = pythpredict::id();
    let mut pt = ProgramTest::new("pythpredict", program_id, processor!(pythpredict::entry));
    let (mut banks, payer, _) = pt.start().await;

    let creator = Keypair::new();
    pay(&mut banks, &payer, &payer, &creator.pubkey(), 5_000_000_000).await;

    let mint = Keypair::new();
    create_mint(&mut banks, &payer, &mint, &creator.pubkey(), 6).await;

    let market_nonce = 7u64;
    let (market, _, yes_vault, no_vault, fee_vault, fee_collector) =
        derive_pdas(&program_id, &creator.pubkey(), market_nonce);
    let pyth_feed = Keypair::new().pubkey();

    let accounts = accts::InitializeMarket {
        market,
        yes_vault,
        no_vault,
        fee_vault,
        fee_collector,
        collateral_mint: mint.pubkey(),
        pyth_feed,
        creator: creator.pubkey(),
        system_program: system_program::id(),
        token_program: TOKEN_PROGRAM_ID,
    };
    let data = ix::InitializeMarket {
        market_nonce,
        initial_price: 123_456_00,
        target_change_bps: 0,
        settle_time: 2_000_000_000,
        resolver_authority: None,
    }
    .data();

    let instruction = Instruction {
        program_id,
        accounts: accounts.to_account_metas(None),
        data,
    };
    let bh = latest_blockhash(&mut banks).await;
    let mut tx = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    tx.sign(&[&payer, &creator], bh);
    banks.process_transaction(tx).await.unwrap();

    // market account must now exist and have at least SIZE bytes
    let acc = banks.get_account(market).await.unwrap().unwrap();
    assert!(acc.data.len() >= pythpredict::state::Market::SIZE);
}
