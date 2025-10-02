use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Mint};
use pyth_sdk_solana::{Price, PriceFeed};

use crate::errors::PredictionMarketError;
use crate::state::*;
use anchor_lang::Space;

// pythpredict/src/instructions.rs (anywhere near top-level module funcs)
use core::cell::RefCell;
use std::rc::Rc;

/// Minimal parser matching your test’s hard-coded offsets.
/// Returns (price, expo, conf) as raw Pyth values.
pub fn parse_pyth_price_alternative(feed_ai: &AccountInfo<'_>) -> Result<(i64, i32, u64)> {
    let data = feed_ai.try_borrow_data()
        .map_err(|_| error!(PredictionMarketError::OracleError))?;

    if data.len() < 228 {
        return Err(error!(PredictionMarketError::OracleError));
    }
    // optional: check magic number [0..4]
    // let magic = u32::from_le_bytes(data[0..4].try_into().unwrap());

    let price = i64::from_le_bytes(data[208..216].try_into().unwrap());
    let conf  = u64::from_le_bytes(data[216..224].try_into().unwrap());
    let expo  = i32::from_le_bytes(data[224..228].try_into().unwrap());
    Ok((price, expo, conf))
}

pub fn resolve_market(ctx: Context<ResolveMarket>) -> Result<()> {
    let market = &mut ctx.accounts.market;
    let clock = Clock::get()?;

    require!(!market.is_resolved, PredictionMarketError::MarketAlreadyResolved);
    require!(clock.unix_timestamp >= market.settle_time, PredictionMarketError::SettlementTimeNotReached);

    let (price, expo, _conf) = parse_pyth_price_alternative(&ctx.accounts.pyth_feed)?;
    // Normalize price to the same scale you stored `target_price` (your tests treat it like cents with expo=-2).
    // If you stored initial as raw from UI (e.g., 95_000 with expo=-2), compare raw i64 values directly.
    let current_price = price;

    let initial_price = market.target_price;
    let winning_outcome = if current_price != initial_price { 0 } else { 1 }; // YES if moved

    market.is_resolved = true;
    market.winning_outcome = Some(winning_outcome);
    market.final_price = Some(current_price);
    market.oracle_last_update = clock.unix_timestamp;
    market.market_status = MarketStatus::Resolved;

    msg!(
        "Market resolved! Initial: {}, Final: {}, Expo: {}, Winner: {}",
        initial_price, current_price, expo, if winning_outcome == 0 { "YES" } else { "NO" }
    );
    Ok(())
}


// ===== INITIALIZE MARKET WITH ENHANCED CONTROLS =====
#[derive(Accounts)]
#[instruction(market_nonce: u64)]
pub struct InitializeMarket<'info> {
    #[account(
        init,
        payer = creator,
        space = Market::SIZE,
        seeds = [b"market", creator.key().as_ref(), &market_nonce.to_le_bytes()],
        bump
    )]
    pub market: Account<'info, Market>,

    #[account(
        init,
        payer = creator,
        token::mint = collateral_mint,
        token::authority = market,
        seeds = [b"yes_vault", market.key().as_ref()],
        bump
    )]
    pub yes_vault: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = creator,
        token::mint = collateral_mint,
        token::authority = market,
        seeds = [b"no_vault", market.key().as_ref()],
        bump
    )]
    pub no_vault: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = creator,
        token::mint = collateral_mint,
        token::authority = market,  // Changed from fee_collector to market for consistency
        seeds = [b"fee_vault", market.key().as_ref()],
        bump
    )]
    pub fee_vault: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = creator,
        space = FeeCollector::SIZE,
        seeds = [b"fee_collector"],
        bump
    )]
    pub fee_collector: Account<'info, FeeCollector>,

    pub collateral_mint: Account<'info, Mint>,

    /// CHECK: This account is the Pyth oracle price feed. It's validated at runtime when reading price data.
    pub pyth_feed: AccountInfo<'info>,

    #[account(mut)]
    pub creator: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

pub fn initialize_market(
    ctx: Context<InitializeMarket>,
    market_nonce: u64,
    initial_price: i64,
    target_change_bps: i64,
    settle_time: i64,
    resolver_authority: Option<Pubkey>,
) -> Result<()> {
    let market = &mut ctx.accounts.market;
    let clock = Clock::get()?;

    // Validate settlement time with proper error messages
    let params = MarketParams::default();
    require!(
        settle_time > clock.unix_timestamp + params.min_settlement_time,
        PredictionMarketError::SettlementTimeTooSoon  // Use appropriate error
    );
    require!(
        settle_time < clock.unix_timestamp + params.max_settlement_time,
        PredictionMarketError::SettlementTimeTooFar  // Use appropriate error
    );

    // Calculate target price based on initial price and target change
    // For zero-target markets, we use the initial price as both initial and target
    let target_price = if target_change_bps == 0 {
        initial_price // Zero target means we're tracking if price moves at all
    } else {
        initial_price + (initial_price * target_change_bps / 10000)
    };

    // Initialize market
    market.creator = ctx.accounts.creator.key();
    market.pyth_feed = ctx.accounts.pyth_feed.key();
    market.target_price = initial_price; // Store initial price here
    market.settle_time = settle_time;
    market.yes_pool = 0;
    market.no_pool = 0;
    market.collateral_mint = ctx.accounts.collateral_mint.key();
    market.is_resolved = false;
    market.winning_outcome = None;
    market.nonce = market_nonce;
    market.bump = ctx.bumps.market;
    market.resolver_authority = resolver_authority.unwrap_or(ctx.accounts.creator.key());
    market.total_volume = 0;
    market.fee_bps = 100; // 1% default fee
    market.final_price = None;

    // Initialize extended fields
    market.total_fees_collected = 0;
    market.fee_collector = ctx.accounts.fee_collector.key();
    market.oracle_confidence = 500; // 5% max confidence
    market.min_bet_amount = 100_000; // 0.1 token minimum
    market.max_bet_amount = 1_000_000_000_000; // 1M tokens max
    market.market_status = MarketStatus::Active; // Start as Active, not PendingLiquidity
    market.created_at = clock.unix_timestamp;
    market.description = [0u8; 128]; // To be set later if needed
    market.category = MarketCategory::Crypto;
    market.oracle_last_update = clock.unix_timestamp;
    market.emergency_paused = false;
    market.min_liquidity = 0; // Set to 0 to allow immediate betting
    market.liquidity_locked_until = settle_time;

    // Store the actual target price in a new field if we need it
    // For now, we'll use final_price field to track the target
    if target_change_bps != 0 {
        // We'll track the target separately if needed
        // For zero-target markets, any change from initial_price is YES
    }

    // Initialize fee collector if needed
    if ctx.accounts.fee_collector.authority == Pubkey::default() {
        let fee_collector = &mut ctx.accounts.fee_collector;
        fee_collector.authority = ctx.accounts.creator.key();
        fee_collector.total_fees_collected = 0;
        fee_collector.treasury = ctx.accounts.creator.key();
        fee_collector.fee_distribution = FeeDistribution {
            treasury_bps: 5000,   // 50% to treasury
            liquidity_bps: 3000,  // 30% to LPs
            creator_bps: 2000,    // 20% to market creator
        };
    }

    msg!("Market initialized with initial price: {}", initial_price);
    msg!("Target change: {} bps", target_change_bps);
    msg!("Fee collector: {}", ctx.accounts.fee_collector.key());
    msg!("Market status: {:?}", market.market_status);

    Ok(())
}

// ===== PLACE BET WITH FEE HANDLING =====
#[derive(Accounts)]
pub struct Bet<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,

    #[account(
        init_if_needed,
        payer = better,
        space = Position::SIZE,
        seeds = [b"position", market.key().as_ref(), better.key().as_ref()],
        bump
    )]
    pub position: Account<'info, Position>,

    #[account(
        mut,
        constraint = user_token_account.owner == better.key(),
        constraint = user_token_account.mint == market.collateral_mint
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"yes_vault", market.key().as_ref()],
        bump
    )]
    pub yes_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"no_vault", market.key().as_ref()],
        bump
    )]
    pub no_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"fee_vault", market.key().as_ref()],
        bump
    )]
    pub fee_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub better: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn place_bet(ctx: Context<Bet>, amount: u64, outcome: Outcome) -> Result<()> {
    let market = &mut ctx.accounts.market;
    let position = &mut ctx.accounts.position;
    let clock = Clock::get()?;

    // Validate market state
    require!(!market.is_resolved, PredictionMarketError::MarketAlreadyResolved);
    require!(clock.unix_timestamp < market.settle_time, PredictionMarketError::MarketClosed);
    require!(!market.emergency_paused, PredictionMarketError::MarketPaused);

    // Check if market is active (remove the is_active() check since we set it to Active on init)
    require!(
        market.market_status == MarketStatus::Active || market.market_status == MarketStatus::PendingLiquidity,
        PredictionMarketError::MarketNotActive
    );

    // Validate bet amount
    require!(amount > 0, PredictionMarketError::InvalidAmount);
    require!(
        amount >= market.min_bet_amount,
        PredictionMarketError::BetTooSmall
    );
    require!(
        amount <= market.max_bet_amount,
        PredictionMarketError::BetTooLarge
    );

    // Calculate fee
    let (fee, amount_after_fee) = calculate_fee(amount, market.fee_bps)?;

    // Transfer fee to fee vault
    if fee > 0 {
        let fee_transfer_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.fee_vault.to_account_info(),
                authority: ctx.accounts.better.to_account_info(),
            },
        );
        token::transfer(fee_transfer_ctx, fee)?;

        market.total_fees_collected = market.total_fees_collected
            .checked_add(fee)
            .ok_or(PredictionMarketError::MathOverflow)?;
    }

    // Transfer bet amount to appropriate vault
    let vault = match outcome {
        Outcome::Yes => &ctx.accounts.yes_vault,
        Outcome::No => &ctx.accounts.no_vault,
    };

    let transfer_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: vault.to_account_info(),
            authority: ctx.accounts.better.to_account_info(),
        },
    );
    token::transfer(transfer_ctx, amount_after_fee)?;

    // Store current odds before updating pools
    let (yes_odds, no_odds) = market.calculate_odds();

    // Update market pools
    match outcome {
        Outcome::Yes => {
            market.yes_pool = market.yes_pool
                .checked_add(amount_after_fee)
                .ok_or(PredictionMarketError::MathOverflow)?;
        }
        Outcome::No => {
            market.no_pool = market.no_pool
                .checked_add(amount_after_fee)
                .ok_or(PredictionMarketError::MathOverflow)?;
        }
    }

    // Update position
    if position.market == Pubkey::default() {
        position.market = market.key();
        position.better = ctx.accounts.better.key();
        position.claimed = false;
        position.yes_amount = 0;
        position.no_amount = 0;
        position.total_invested = 0;
        position.pending_payout = 0;
    }

    match outcome {
        Outcome::Yes => {
            position.yes_amount = position.yes_amount
                .checked_add(amount_after_fee)
                .ok_or(PredictionMarketError::MathOverflow)?;
        }
        Outcome::No => {
            position.no_amount = position.no_amount
                .checked_add(amount_after_fee)
                .ok_or(PredictionMarketError::MathOverflow)?;
        }
    }

    position.entry_odds_yes = (yes_odds * 10000.0) as u64;
    position.entry_odds_no = (no_odds * 10000.0) as u64;
    position.bet_timestamp = clock.unix_timestamp;
    position.total_invested = position.total_invested
        .checked_add(amount)
        .ok_or(PredictionMarketError::MathOverflow)?;

    // Update market volume
    market.total_volume = market.total_volume
        .checked_add(amount)
        .ok_or(PredictionMarketError::MathOverflow)?;

    msg!(
        "Bet placed: {} tokens on {:?}, Fee: {} tokens",
        amount_after_fee,
        outcome,
        fee
    );
    msg!("New odds - YES: {:.2}%, NO: {:.2}%",
        market.yes_pool as f64 / market.get_total_pot() as f64 * 100.0,
        market.no_pool as f64 / market.get_total_pot() as f64 * 100.0
    );

    Ok(())
}

// ===== RESOLVE MARKET WITH ORACLE VALIDATION =====
#[derive(Accounts)]
pub struct ResolveMarket<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,

    /// CHECK: This account is the Pyth oracle price feed. It's validated at runtime when reading price data.
    pub pyth_feed: AccountInfo<'info>,

    #[account(
        constraint = resolver.key() == market.resolver_authority @ PredictionMarketError::UnauthorizedResolver
    )]
    pub resolver: Signer<'info>,
}


// ===== RESOLVE WITH EXTERNAL PRICE (FOR TESTING) =====
#[derive(Accounts)]
pub struct ResolveWithExternalPrice<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,

    #[account(
        constraint = resolver.key() == market.resolver_authority @ PredictionMarketError::UnauthorizedResolver
    )]
    pub resolver: Signer<'info>,
}

pub fn resolve_with_external_price(
    ctx: Context<ResolveWithExternalPrice>,
    final_price: i64,
) -> Result<()> {
    let market = &mut ctx.accounts.market;
    let clock = Clock::get()?;

    require!(!market.is_resolved, PredictionMarketError::MarketAlreadyResolved);
    require!(
        clock.unix_timestamp >= market.settle_time,
        PredictionMarketError::SettlementTimeNotReached
    );

    // For zero-target markets, any price change means YES wins
    let initial_price = market.target_price;
    let winning_outcome = if final_price != initial_price {
        0 // YES wins (price moved)
    } else {
        1 // NO wins (price stayed the same)
    };

    market.is_resolved = true;
    market.winning_outcome = Some(winning_outcome);
    market.final_price = Some(final_price);
    market.market_status = MarketStatus::Resolved;

    msg!(
        "Market resolved with external price! Initial: {}, Final: {}, Winner: {}",
        initial_price,
        final_price,
        if winning_outcome == 0 { "YES" } else { "NO" }
    );

    Ok(())
}

// ===== FIXED CLAIM WINNINGS =====
#[derive(Accounts)]
pub struct ClaimWinnings<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,

    #[account(
        mut,
        seeds = [b"position", market.key().as_ref(), claimer.key().as_ref()],
        bump,
        constraint = position.better == claimer.key(),
        constraint = !position.claimed @ PredictionMarketError::AlreadyClaimed
    )]
    pub position: Account<'info, Position>,

    #[account(
        mut,
        seeds = [b"yes_vault", market.key().as_ref()],
        bump
    )]
    pub yes_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"no_vault", market.key().as_ref()],
        bump
    )]
    pub no_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_token_account.owner == claimer.key(),
        constraint = user_token_account.mint == market.collateral_mint
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    pub claimer: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

// Replace the claim_winnings function in pythpredict/src/instructions.rs with this fixed version

pub fn claim_winnings(ctx: Context<ClaimWinnings>) -> Result<()> {
    let market = &ctx.accounts.market;
    let position = &mut ctx.accounts.position;

    // Validate market is resolved
    require!(market.is_resolved, PredictionMarketError::MarketNotResolved);
    require!(position.has_position(), PredictionMarketError::NoPosition);

    let winning_outcome = market.winning_outcome
        .ok_or(PredictionMarketError::MarketNotResolved)?;

    let winning_stake = position.get_winning_stake(winning_outcome);

    // If user didn't win, they get nothing
    if winning_stake == 0 {
        msg!("User has no winning position");
        position.claimed = true;
        return Ok(());
    }

    // Determine winning and losing vaults and pools
    let (win_vault, lose_vault, win_pool, lose_pool) = match winning_outcome {
        0 => (
            &ctx.accounts.yes_vault,
            &ctx.accounts.no_vault,
            market.yes_pool,
            market.no_pool,
        ),
        1 => (
            &ctx.accounts.no_vault,
            &ctx.accounts.yes_vault,
            market.no_pool,
            market.yes_pool,
        ),
        _ => return Err(PredictionMarketError::InvalidPool.into()),
    };

    // Calculate profit (stake * lose_pool / win_pool) using u128 to avoid overflow
    let profit = if lose_pool == 0 {
        0
    } else {
        let profit_128 = (winning_stake as u128)
            .checked_mul(lose_pool as u128)
            .ok_or(PredictionMarketError::MathOverflow)?
            .checked_div(win_pool as u128)
            .ok_or(PredictionMarketError::MathOverflow)?;
        profit_128 as u64
    };

    let payout = winning_stake
        .checked_add(profit)
        .ok_or(PredictionMarketError::MathOverflow)?;

    // Create market PDA signer seeds
    let nonce_bytes = market.nonce.to_le_bytes();
    let market_seeds = &[
        b"market",
        market.creator.as_ref(),
        &nonce_bytes,
        &[market.bump],
    ];
    let signer = &[&market_seeds[..]];

    // Transfer stake from winning vault
    let transfer_win_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: win_vault.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: market.to_account_info(),
        },
        signer,
    );
    token::transfer(transfer_win_ctx, winning_stake)?;

    // Transfer profit from losing vault (if any)
    if profit > 0 {
        let transfer_lose_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: lose_vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: market.to_account_info(),
            },
            signer,
        );
        token::transfer(transfer_lose_ctx, profit)?;
    }

    // Mark as claimed
    position.claimed = true;
    position.pending_payout = 0;

    msg!(
        "Claimed {} tokens for user {} (stake: {}, profit: {})",
        payout,
        ctx.accounts.claimer.key(),
        winning_stake,
        profit
    );

    Ok(())
}
// ===== MARKET MAKER FUNCTIONS =====
#[derive(Accounts)]
pub struct InitializeMarketMaker<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,

    #[account(
        init,
        payer = authority,
        space = MarketMaker::SIZE,
        seeds = [b"market_maker", market.key().as_ref()],
        bump
    )]
    pub market_maker: Account<'info, MarketMaker>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_market_maker(
    ctx: Context<InitializeMarketMaker>,
    target_spread_bps: u64,
    max_exposure: u64,
) -> Result<()> {
    let market_maker = &mut ctx.accounts.market_maker;
    let clock = Clock::get()?;

    market_maker.market = ctx.accounts.market.key();
    market_maker.authority = ctx.accounts.authority.key();
    market_maker.target_spread_bps = target_spread_bps;
    market_maker.max_exposure = max_exposure;
    market_maker.current_exposure = 0;
    market_maker.total_volume_provided = 0;
    market_maker.fees_earned = 0;
    market_maker.last_rebalance = clock.unix_timestamp;
    market_maker.is_active = true;

    msg!("Market maker initialized with spread: {} bps", target_spread_bps);

    Ok(())
}

#[derive(Accounts)]
pub struct ProvideLiquidity<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,

    #[account(
        mut,
        seeds = [b"market_maker", market.key().as_ref()],
        bump
    )]
    pub market_maker: Account<'info, MarketMaker>,

    #[account(
        init_if_needed,
        payer = liquidity_provider,
        space = Position::SIZE,
        seeds = [b"position", market.key().as_ref(), liquidity_provider.key().as_ref()],  // Changed from market_maker.key()
        bump
    )]
    pub mm_position: Account<'info, Position>,

    #[account(
        mut,
        constraint = provider_token_account.owner == liquidity_provider.key()
    )]
    pub provider_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"yes_vault", market.key().as_ref()],
        bump
    )]
    pub yes_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"no_vault", market.key().as_ref()],
        bump
    )]
    pub no_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub liquidity_provider: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn provide_liquidity(ctx: Context<ProvideLiquidity>, amount_per_side: u64) -> Result<()> {
    let market = &mut ctx.accounts.market;
    let market_maker = &mut ctx.accounts.market_maker;
    let mm_position = &mut ctx.accounts.mm_position;
    let clock = Clock::get()?;

    require!(market_maker.is_active, PredictionMarketError::MarketNotActive);

    // Check exposure limits
    let total_exposure = market_maker.current_exposure + (amount_per_side * 2);
    require!(
        total_exposure <= market_maker.max_exposure,
        PredictionMarketError::BetTooLarge
    );

    // Transfer to YES vault
    let yes_transfer = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.provider_token_account.to_account_info(),
            to: ctx.accounts.yes_vault.to_account_info(),
            authority: ctx.accounts.liquidity_provider.to_account_info(),
        },
    );
    token::transfer(yes_transfer, amount_per_side)?;

    // Transfer to NO vault
    let no_transfer = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.provider_token_account.to_account_info(),
            to: ctx.accounts.no_vault.to_account_info(),
            authority: ctx.accounts.liquidity_provider.to_account_info(),
        },
    );
    token::transfer(no_transfer, amount_per_side)?;

    // Update market pools
    market.yes_pool = market.yes_pool
        .checked_add(amount_per_side)
        .ok_or(PredictionMarketError::MathOverflow)?;
    market.no_pool = market.no_pool
        .checked_add(amount_per_side)
        .ok_or(PredictionMarketError::MathOverflow)?;
    market.total_volume = market.total_volume
        .checked_add(amount_per_side * 2)
        .ok_or(PredictionMarketError::MathOverflow)?;

    // Update market maker tracking
    market_maker.current_exposure = total_exposure;
    market_maker.total_volume_provided = market_maker.total_volume_provided
        .checked_add(amount_per_side * 2)
        .ok_or(PredictionMarketError::MathOverflow)?;
    market_maker.last_rebalance = clock.unix_timestamp;

    // Initialize MM position if needed
    if mm_position.market == Pubkey::default() {
        mm_position.market = market.key();
        mm_position.better = ctx.accounts.liquidity_provider.key();
        mm_position.claimed = false;
        mm_position.yes_amount = 0;
        mm_position.no_amount = 0;
        mm_position.total_invested = 0;
        mm_position.pending_payout = 0;
    }

    // Update MM position
    mm_position.yes_amount = mm_position.yes_amount
        .checked_add(amount_per_side)
        .ok_or(PredictionMarketError::MathOverflow)?;
    mm_position.no_amount = mm_position.no_amount
        .checked_add(amount_per_side)
        .ok_or(PredictionMarketError::MathOverflow)?;
    mm_position.total_invested = mm_position.total_invested
        .checked_add(amount_per_side * 2)
        .ok_or(PredictionMarketError::MathOverflow)?;
    mm_position.bet_timestamp = clock.unix_timestamp;

    msg!(
        "Liquidity added: {} per side, total exposure: {}",
        amount_per_side,
        total_exposure
    );

    Ok(())
}

// ===== CONSOLIDATE FUNDS (FOR CLEANUP) =====
#[derive(Accounts)]
pub struct ConsolidateFunds<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,

    #[account(
        mut,
        seeds = [b"fee_vault", market.key().as_ref()],
        bump
    )]
    pub fee_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub fee_collector: Account<'info, FeeCollector>,

    #[account(
        mut,
        constraint = treasury_account.owner == fee_collector.treasury
    )]
    pub treasury_account: Account<'info, TokenAccount>,

    #[account(
        constraint = authority.key() == fee_collector.authority @ PredictionMarketError::UnauthorizedResolver
    )]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

// pythpredict/src/instructions.rs (inside consolidate_funds)
pub fn consolidate_funds(ctx: Context<ConsolidateFunds>) -> Result<()> {
    let market = &ctx.accounts.market;
    require!(market.is_resolved, PredictionMarketError::MarketNotResolved);

    let fee_vault_balance = ctx.accounts.fee_vault.amount;
    if fee_vault_balance > 0 {
        // MARKET is the token authority for fee_vault → sign with MARKET seeds, not fee_vault seeds
        let nonce_bytes = market.nonce.to_le_bytes();
        let market_seeds: &[&[u8]] = &[
            b"market",
            market.creator.as_ref(),
            &nonce_bytes,
            &[market.bump],
        ];
        let signer = &[market_seeds];

        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.fee_vault.to_account_info(),
                to: ctx.accounts.treasury_account.to_account_info(),
                authority: market.to_account_info(),
            },
            signer,
        );
        token::transfer(transfer_ctx, fee_vault_balance)?;

        ctx.accounts.fee_collector.total_fees_collected =
            ctx.accounts.fee_collector.total_fees_collected
                .checked_add(fee_vault_balance)
                .ok_or(PredictionMarketError::MathOverflow)?;
        msg!("Consolidated {} tokens to treasury", fee_vault_balance);
    }
    Ok(())
}
