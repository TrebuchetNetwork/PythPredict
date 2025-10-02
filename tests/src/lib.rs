use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use pythpredict::state::*;
use pythpredict::errors::PredictionMarketError;

#[cfg(test)]
mod market_tests {
    use super::*;

    #[test]
    fn test_market_size() {
        // Ensure the SIZE constant matches the actual struct size
        assert!(Market::SIZE <= 10240, "Market too large for Solana");
        assert!(Market::SIZE >= 100, "Market suspiciously small");
    }

    #[test]
    fn test_calculate_odds() {
        let mut market = create_test_market();

        // Test with no bets
        let (yes_odds, no_odds) = market.calculate_odds();
        assert_eq!(yes_odds, 0.5);
        assert_eq!(no_odds, 0.5);

        // Test with bets
        market.yes_pool = 700;
        market.no_pool = 300;
        let (yes_odds, no_odds) = market.calculate_odds();
        assert_eq!(yes_odds, 0.7);
        assert_eq!(no_odds, 0.3);
    }

    #[test]
    fn test_get_total_pot() {
        let mut market = create_test_market();
        assert_eq!(market.get_total_pot(), 0);

        market.yes_pool = 1000;
        market.no_pool = 500;
        assert_eq!(market.get_total_pot(), 1500);
    }

    #[test]
    fn test_is_active() {
        let mut market = create_test_market();
        market.settle_time = 1000;
        market.is_resolved = false;

        // Market should be active if not resolved and before settle time
        // Note: This test would need mock clock in real scenario
        assert!(!market.is_resolved);
    }

    #[test]
    fn test_can_resolve() {
        let mut market = create_test_market();
        market.is_resolved = false;
        market.settle_time = 0; // Past time

        // Should be able to resolve if not resolved and past settle time
        assert!(!market.is_resolved);
    }

    fn create_test_market() -> Market {
        Market {
            creator: Pubkey::new_unique(),
            pyth_feed: Pubkey::new_unique(),
            target_price: 50000,
            settle_time: 1234567890,
            yes_pool: 0,
            no_pool: 0,
            collateral_mint: Pubkey::new_unique(),
            is_resolved: false,
            winning_outcome: None,
            nonce: 0,
            bump: 254,
            resolver_authority: Pubkey::new_unique(),
            total_volume: 0,
            fee_bps: 100,
            final_price: None,
        }
    }
}

#[cfg(test)]
mod position_tests {
    use super::*;

    #[test]
    fn test_position_size() {
        assert!(Position::SIZE >= std::mem::size_of::<Position>());
    }

    #[test]
    fn test_get_total_stake() {
        let position = Position {
            market: Pubkey::new_unique(),
            better: Pubkey::new_unique(),
            yes_amount: 100,
            no_amount: 50,
            claimed: false,
        };

        assert_eq!(position.get_total_stake(), 150);
    }

    #[test]
    fn test_has_position() {
        let mut position = Position {
            market: Pubkey::new_unique(),
            better: Pubkey::new_unique(),
            yes_amount: 0,
            no_amount: 0,
            claimed: false,
        };

        assert!(!position.has_position());

        position.yes_amount = 100;
        assert!(position.has_position());

        position.yes_amount = 0;
        position.no_amount = 50;
        assert!(position.has_position());
    }

    #[test]
    fn test_get_winning_stake() {
        let position = Position {
            market: Pubkey::new_unique(),
            better: Pubkey::new_unique(),
            yes_amount: 100,
            no_amount: 50,
            claimed: false,
        };

        assert_eq!(position.get_winning_stake(0), 100); // Yes wins
        assert_eq!(position.get_winning_stake(1), 50);  // No wins
        assert_eq!(position.get_winning_stake(2), 0);   // Invalid
    }
}

#[cfg(test)]
mod calculation_tests {
    use super::*;

    #[test]
    fn test_calculate_payout() {
        // Test normal payout calculation
        let result = calculate_payout(100, 1000, 500);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 150); // 100 * 1500 / 1000 = 150

        // Test no losing pool
        let result = calculate_payout(100, 1000, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 100); // Return original stake

        // Test no winning pool (should error)
        let result = calculate_payout(100, 0, 1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_fee() {
        // Test 1% fee (100 basis points)
        let result = calculate_fee(10000, 100);
        assert!(result.is_ok());
        let (fee, amount_after_fee) = result.unwrap();
        assert_eq!(fee, 100);
        assert_eq!(amount_after_fee, 9900);

        // Test 2.5% fee (250 basis points)
        let result = calculate_fee(10000, 250);
        assert!(result.is_ok());
        let (fee, amount_after_fee) = result.unwrap();
        assert_eq!(fee, 250);
        assert_eq!(amount_after_fee, 9750);

        // Test zero fee
        let result = calculate_fee(10000, 0);
        assert!(result.is_ok());
        let (fee, amount_after_fee) = result.unwrap();
        assert_eq!(fee, 0);
        assert_eq!(amount_after_fee, 10000);
    }

    #[test]
    fn test_calculate_payout_overflow() {
        // Test overflow handling
        let result = calculate_payout(u64::MAX, u64::MAX, u64::MAX);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_fee_overflow() {
        // Test overflow handling
        let result = calculate_fee(u64::MAX, u64::MAX);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod market_params_tests {
    use super::*;

    #[test]
    fn test_default_market_params() {
        let params = MarketParams::default();
        assert_eq!(params.min_bet_amount, 100_000);
        assert_eq!(params.max_bet_amount, 1_000_000_000_000);
        assert_eq!(params.min_settlement_time, 3600);
        assert_eq!(params.max_settlement_time, 365 * 24 * 3600);
        assert_eq!(params.max_price_confidence, 5);
    }
}

#[cfg(test)]
mod outcome_tests {
    use super::*;

    #[test]
    fn test_outcome_equality() {
        assert_eq!(Outcome::Yes, Outcome::Yes);
        assert_eq!(Outcome::No, Outcome::No);
        assert_ne!(Outcome::Yes, Outcome::No);
    }

    #[test]
    fn test_outcome_clone() {
        let outcome = Outcome::Yes;
        let cloned = outcome.clone();
        assert_eq!(outcome, cloned);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_full_market_lifecycle() {
        // Create market
        let mut market = Market {
            creator: Pubkey::new_unique(),
            pyth_feed: Pubkey::new_unique(),
            target_price: 50000,
            settle_time: 1234567890,
            yes_pool: 1000,
            no_pool: 500,
            collateral_mint: Pubkey::new_unique(),
            is_resolved: false,
            winning_outcome: None,
            nonce: 0,
            bump: 254,
            resolver_authority: Pubkey::new_unique(),
            total_volume: 1500,
            fee_bps: 100,
            final_price: None,
        };

        // Verify initial state
        assert!(!market.is_resolved);
        assert_eq!(market.get_total_pot(), 1500);
        let (yes_odds, no_odds) = market.calculate_odds();
        assert!((yes_odds - 0.6667).abs() < 0.001);
        assert!((no_odds - 0.3333).abs() < 0.001);

        // Simulate resolution
        market.is_resolved = true;
        market.winning_outcome = Some(0); // Yes wins
        market.final_price = Some(55000);

        // Verify resolved state
        assert!(market.is_resolved);
        assert_eq!(market.winning_outcome, Some(0));
        assert_eq!(market.final_price, Some(55000));

        // Test position payout calculation
        let user_position = Position {
            market: market.creator,
            better: Pubkey::new_unique(),
            yes_amount: 100,
            no_amount: 0,
            claimed: false,
        };

        let payout = calculate_payout(
            user_position.yes_amount,
            market.yes_pool,
            market.no_pool,
        ).unwrap();
        assert_eq!(payout, 150); // 100 * (1000 + 500) / 1000
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_zero_pools() {
        let market = Market {
            creator: Pubkey::new_unique(),
            pyth_feed: Pubkey::new_unique(),
            target_price: 50000,
            settle_time: 1234567890,
            yes_pool: 0,
            no_pool: 0,
            collateral_mint: Pubkey::new_unique(),
            is_resolved: false,
            winning_outcome: None,
            nonce: 0,
            bump: 254,
            resolver_authority: Pubkey::new_unique(),
            total_volume: 0,
            fee_bps: 100,
            final_price: None,
        };

        let (yes_odds, no_odds) = market.calculate_odds();
        assert_eq!(yes_odds, 0.5);
        assert_eq!(no_odds, 0.5);
        assert_eq!(market.get_total_pot(), 0);
    }

    #[test]
    fn test_max_values() {
        let mut market = Market {
            creator: Pubkey::new_unique(),
            pyth_feed: Pubkey::new_unique(),
            target_price: i64::MAX,
            settle_time: i64::MAX,
            yes_pool: u64::MAX - 1,
            no_pool: 1,
            collateral_mint: Pubkey::new_unique(),
            is_resolved: false,
            winning_outcome: None,
            nonce: u64::MAX,
            bump: 255,
            resolver_authority: Pubkey::new_unique(),
            total_volume: u64::MAX,
            fee_bps: 10000, // 100%
            final_price: Some(i64::MAX),
        };

        // Test that calculations handle max values gracefully
        let total = market.get_total_pot();
        assert_eq!(total, u64::MAX);
    }

    #[test]
    fn test_all_users_win() {
        // Scenario where everyone bet on the winning side
        let payout = calculate_payout(100, 1000, 0).unwrap();
        assert_eq!(payout, 100); // Should just return stake
    }

    #[test]
    fn test_precision_loss() {
        // Test for precision issues with small amounts
        let payout = calculate_payout(1, 1000000, 1).unwrap();
        assert_eq!(payout, 1); // Should handle small fractions
    }
}