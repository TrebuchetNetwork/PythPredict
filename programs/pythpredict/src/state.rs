// programs/pythpredict/src/state.rs
use anchor_lang::prelude::*;
use anchor_lang::error::Error as AnchorError;
use crate::errors::PredictionMarketError;

// ---------- Enums ----------

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Yes = 0,
    No = 1,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum MarketStatus {
    PendingLiquidity,
    Active,
    Resolved,
    Disputed,
    Cancelled,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum MarketCategory {
    Crypto,
    Sports,
    Politics,
    Weather,
    Entertainment,
    Other,
}

// ---------- Accounts ----------

#[account]
pub struct Market {
    pub creator: Pubkey,                // 32
    pub pyth_feed: Pubkey,              // 32
    pub target_price: i64,              // 8
    pub settle_time: i64,               // 8
    pub yes_pool: u64,                  // 8
    pub no_pool: u64,                   // 8
    pub collateral_mint: Pubkey,        // 32
    pub is_resolved: bool,              // 1
    pub winning_outcome: Option<u8>,    // 1(tag) + 1 = 2
    pub nonce: u64,                     // 8
    pub bump: u8,                       // 1
    pub resolver_authority: Pubkey,     // 32
    pub total_volume: u64,              // 8
    pub fee_bps: u16,                   // 2
    pub final_price: Option<i64>,       // 1(tag) + 8 = 9
    // Extended
    pub total_fees_collected: u64,      // 8
    pub fee_collector: Pubkey,          // 32
    pub oracle_confidence: u64,         // 8
    pub min_bet_amount: u64,            // 8
    pub max_bet_amount: u64,            // 8
    pub market_status: MarketStatus,    // 1 (enum as u8)
    pub created_at: i64,                // 8
    pub description: [u8; 128],         // 128
    pub category: MarketCategory,       // 1 (enum as u8)
    pub oracle_last_update: i64,        // 8
    pub emergency_paused: bool,         // 1
    pub min_liquidity: u64,             // 8
    pub liquidity_locked_until: i64,    // 8
}

impl Market {
    // Sum(fields) = 418 → +8 discriminator = 426
    pub const SIZE: usize = 8 + 418;

    pub fn calculate_odds(&self) -> (f64, f64) {
        let total = self.yes_pool.saturating_add(self.no_pool);
        if total == 0 {
            return (0.5, 0.5);
        }
        (
            self.yes_pool as f64 / total as f64,
            self.no_pool as f64 / total as f64,
        )
    }

    pub fn get_total_pot(&self) -> u64 {
        self.yes_pool.saturating_add(self.no_pool)
    }

    pub fn is_active(&self) -> bool {
        self.market_status == MarketStatus::Active
            && !self.is_resolved
            && !self.emergency_paused
    }

    pub fn can_resolve(&self, current_time: i64) -> bool {
        !self.is_resolved && current_time >= self.settle_time
    }

    pub fn has_minimum_liquidity(&self) -> bool {
        self.get_total_pot() >= self.min_liquidity
    }

    pub fn validate_bet_amount(&self, amount: u64) -> Result<()> {
        require!(amount >= self.min_bet_amount, PredictionMarketError::BetTooSmall);
        require!(amount <= self.max_bet_amount, PredictionMarketError::BetTooLarge);
        Ok(())
    }

    pub fn validate_oracle_price(
        &self,
        price_confidence: u64,
        last_update: i64,
        current_time: i64,
    ) -> Result<()> {
        require!(
            price_confidence <= self.oracle_confidence,
            PredictionMarketError::PriceConfidenceTooHigh
        );
        require!(
            current_time - last_update <= 60,
            PredictionMarketError::PriceTooStale
        );
        Ok(())
    }

    /// Returns spot prices as basis points of total (e.g. 5000 == 50%)
    pub fn get_spot_prices(&self) -> (u64, u64) {
        let total = self.get_total_pot();
        if total == 0 {
            return (5000, 5000);
        }
        let yes = ((self.yes_pool as u128) * 10_000u128 / (total as u128)) as u64;
        let no = 10_000u64.saturating_sub(yes);
        (yes, no)
    }

    /// Simple price impact approximation in bps for adding `amount` to one side.
pub fn calculate_price_impact(&self, amount: u64, side: Outcome) -> Result<u64> {
    let (mut y, mut n) = (self.yes_pool as u128, self.no_pool as u128);
    let before = if y + n == 0 { 5_000u128 } else { y * 10_000 / (y + n) };

    match side {
        Outcome::Yes => {
            y = y.checked_add(amount as u128).ok_or_else(overflow_err)?;
        }
        Outcome::No => {
            n = n.checked_add(amount as u128).ok_or_else(overflow_err)?;
        }
    }

    let after = (y * 10_000) / (y + n);
    let impact = if after > before { after - before } else { before - after };

    // ↓ Concrete AnchorError, no type inference ambiguity
    let val: u64 = u64::try_from(impact).map_err(|_| overflow_err())?;
    Ok(val)
}

    /// If external price (in bps) deviates enough from internal odds, return arbitrage.
    /// MIN_PROFIT_BPS = 100 (1%)
    pub fn calculate_arbitrage_opportunity(&self, ext_yes_bps: u64) -> Option<(Outcome, u64)> {
        const MIN_PROFIT_BPS: u64 = 100;
        let (yes_bps, no_bps) = self.get_spot_prices();
        if ext_yes_bps > yes_bps + MIN_PROFIT_BPS {
            Some((Outcome::Yes, ext_yes_bps - yes_bps))
        } else if (10_000 - ext_yes_bps) > no_bps + MIN_PROFIT_BPS {
            Some((Outcome::No, (10_000 - ext_yes_bps) - no_bps))
        } else {
            None
        }
    }

    /// Expected payout for a fresh bet of `amount` on `side` given current pools (no fee logic).
    pub fn get_expected_payout(&self, amount: u64, side: Outcome) -> Result<u64> {
        let (win_pool, lose_pool) = match side {
            Outcome::Yes => (self.yes_pool, self.no_pool),
            Outcome::No => (self.no_pool, self.yes_pool),
        };
        if win_pool == 0 {
            return amount.checked_mul(2).ok_or_else(overflow_err);
        }
        let total_after = win_pool
            .checked_add(lose_pool)
            .and_then(|t| t.checked_add(amount))
            .ok_or_else(overflow_err)?;
        amount
            .checked_mul(total_after)
            .ok_or_else(overflow_err)?
            .checked_div(win_pool.checked_add(amount).ok_or_else(overflow_err)?)
            .ok_or_else(overflow_err)
    }
}

#[account]
pub struct Position {
    pub market: Pubkey,         // 32
    pub better: Pubkey,         // 32
    pub yes_amount: u64,        // 8
    pub no_amount: u64,         // 8
    pub claimed: bool,          // 1
    // tracking
    pub entry_odds_yes: u64,    // 8
    pub entry_odds_no: u64,     // 8
    pub bet_timestamp: i64,     // 8
    pub total_invested: u64,    // 8
    pub pending_payout: u64,    // 8
}
impl Position {
    // Sum(fields) = 121 → +8 discriminator = 129
    pub const SIZE: usize = 8 + 121;

    pub fn get_total_stake(&self) -> u64 { self.yes_amount.saturating_add(self.no_amount) }
    pub fn has_position(&self) -> bool { self.yes_amount > 0 || self.no_amount > 0 }

    pub fn get_winning_stake(&self, winning_outcome: u8) -> u64 {
        match winning_outcome {
            0 => self.yes_amount,
            1 => self.no_amount,
            _ => 0,
        }
    }

    pub fn calculate_pending_payout(&self, market: &Market) -> u64 {
        if !market.is_resolved { return 0; }
        let Some(winner) = market.winning_outcome else { return 0; };
        let stake = self.get_winning_stake(winner);
        if stake == 0 { return 0; }

        let (win, lose) = match winner {
            0 => (market.yes_pool, market.no_pool),
            _ => (market.no_pool, market.yes_pool),
        };

        if win == 0 { return stake; }

        let total = win.saturating_add(lose);
        let fees = (total as u128) * (market.fee_bps as u128) / 10_000u128;
        let distributable = total.saturating_sub(fees as u64);
        (stake as u128 * distributable as u128 / win as u128) as u64
    }
}

#[account]
pub struct MarketMaker {
    pub market: Pubkey,             // 32
    pub authority: Pubkey,          // 32
    pub target_spread_bps: u64,     // 8
    pub max_exposure: u64,          // 8
    pub current_exposure: u64,      // 8
    pub total_volume_provided: u64, // 8
    pub fees_earned: u64,           // 8
    pub last_rebalance: i64,        // 8
    pub is_active: bool,            // 1
}
impl MarketMaker {
    // Sum(fields) = 113 → +8 discriminator = 121
    pub const SIZE: usize = 8 + 113;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct FeeDistribution {
    pub treasury_bps: u16,
    pub liquidity_bps: u16,
    pub creator_bps: u16,
}

#[account]
pub struct FeeCollector {
    pub authority: Pubkey,              // 32
    pub total_fees_collected: u64,      // 8
    pub treasury: Pubkey,               // 32
    pub fee_distribution: FeeDistribution, // 6
}
impl FeeCollector {
    // Sum(fields) = 78 → +8 discriminator = 86
    pub const SIZE: usize = 8 + 78;
}

// ---------- Params ----------

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct MarketParams {
    pub min_bet_amount: u64,
    pub max_bet_amount: u64,
    pub min_settlement_time: i64,
    pub max_settlement_time: i64,
    pub max_price_confidence: u64,
    pub min_liquidity: u64,
    pub oracle_staleness_threshold: i64,
}

impl Default for MarketParams {
    fn default() -> Self {
        Self {
            min_bet_amount: 100_000,               // 0.0001 tokens w/ 6 decimals
            max_bet_amount: 1_000_000_000_000,     // 1M tokens
            min_settlement_time: 10,             // 10 sec
            max_settlement_time: 365 * 24 * 3600,  // 1 year
            max_price_confidence: 5,               // 0.5%
            min_liquidity: 10_000_000,             // 10 tokens
            oracle_staleness_threshold: 60,        // 60s
        }
    }
}

// ---------- Error helpers & math ----------

#[inline(always)]
fn overflow_err() -> AnchorError {
    PredictionMarketError::MathOverflow.into()
}

pub fn calculate_payout(user_stake: u64, winning_pool: u64, losing_pool: u64) -> Result<u64> {
    if winning_pool == 0 {
        return Err(PredictionMarketError::InvalidPool.into());
    }
    if losing_pool == 0 {
        return Ok(user_stake);
    }
    let total = winning_pool
        .checked_add(losing_pool)
        .ok_or_else(overflow_err)?;
    let v = user_stake
        .checked_mul(total).ok_or_else(overflow_err)?
        .checked_div(winning_pool).ok_or_else(overflow_err)?;
    Ok(v)
}

pub fn calculate_fee(amount: u64, fee_bps: u16) -> Result<(u64, u64)> {
    let fee = amount
        .checked_mul(fee_bps as u64).ok_or_else(overflow_err)?
        .checked_div(10_000).ok_or_else(overflow_err)?;
    let after = amount.checked_sub(fee).ok_or_else(overflow_err)?;
    Ok((fee, after))
}
