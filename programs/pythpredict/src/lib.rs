use anchor_lang::prelude::*;

declare_id!("J7TLVPzbd47RpiHV8BBPLQuixU53P5qijkrwkvN4u98W");

pub mod errors;
pub mod instructions;
pub mod state;

use state::Outcome;
pub use instructions::*;

#[program]
pub mod pythpredict {
    use super::*;

    pub fn initialize_market(
        ctx: Context<InitializeMarket>,
        market_nonce: u64,
        initial_price: i64,
        target_change_bps: i64,  // Add this parameter
        settle_time: i64,
        resolver_authority: Option<Pubkey>,
    ) -> Result<()> {
        instructions::initialize_market(
            ctx,
            market_nonce,
            initial_price,
            target_change_bps,  // Pass it through
            settle_time,
            resolver_authority
        )
    }

    pub fn place_bet(ctx: Context<Bet>, amount: u64, outcome: Outcome) -> Result<()> {
        instructions::place_bet(ctx, amount, outcome)
    }

    pub fn resolve_with_external_price(
        ctx: Context<ResolveWithExternalPrice>,
        final_price: i64,
    ) -> Result<()> {
        instructions::resolve_with_external_price(ctx, final_price)
    }

    pub fn claim_winnings(ctx: Context<ClaimWinnings>) -> Result<()> {
        instructions::claim_winnings(ctx)
    }

    // Keep old resolve_market for backwards compatibility if needed
    pub fn resolve_market(ctx: Context<ResolveMarket>) -> Result<()> {
        instructions::resolve_market(ctx)
    }

    pub fn initialize_market_maker(
        ctx: Context<InitializeMarketMaker>,
        target_spread_bps: u64,
        max_exposure: u64,
    ) -> Result<()> {
        instructions::initialize_market_maker(ctx, target_spread_bps, max_exposure)
    }

    pub fn provide_liquidity(
        ctx: Context<ProvideLiquidity>,
        amount_per_side: u64,
    ) -> Result<()> {
        instructions::provide_liquidity(ctx, amount_per_side)
    }



    pub fn consolidate_funds(ctx: Context<ConsolidateFunds>) -> Result<()> {
        instructions::consolidate_funds(ctx)
    }
}