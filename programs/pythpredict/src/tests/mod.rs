#[cfg(test)]
mod tests {
    use crate::state::*;
    use crate::errors::PredictionMarketError;
    use anchor_lang::prelude::*;

    mod state_coverage_tests;
    mod instruction_helpers;
    mod error_helper_methods;
    mod integration_tests;

    mod state_tests {
        use super::*;

        #[test]
        fn test_market_all_methods() {
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

            // Test calculate_odds
            let (yes_odds, no_odds) = market.calculate_odds();
            assert!((yes_odds - 0.6667).abs() < 0.001);
            assert!((no_odds - 0.3333).abs() < 0.001);

            // Test get_total_pot
            assert_eq!(market.get_total_pot(), 1500);

            // Note: is_active() and can_resolve() require Clock::get() which isn't available in unit tests
            // These would need to be tested in integration tests with a test validator
        }

        #[test]
        fn test_position_all_methods() {
            let position = Position {
                market: Pubkey::new_unique(),
                better: Pubkey::new_unique(),
                yes_amount: 100,
                no_amount: 50,
                claimed: false,
            };

            // Test get_total_stake
            assert_eq!(position.get_total_stake(), 150);

            // Test has_position
            assert!(position.has_position());

            // Test get_winning_stake for all outcomes
            assert_eq!(position.get_winning_stake(0), 100);
            assert_eq!(position.get_winning_stake(1), 50);
            assert_eq!(position.get_winning_stake(2), 0);
            assert_eq!(position.get_winning_stake(255), 0);
        }

        #[test]
        fn test_position_edge_cases() {
            // Test empty position
            let empty_position = Position {
                market: Pubkey::new_unique(),
                better: Pubkey::new_unique(),
                yes_amount: 0,
                no_amount: 0,
                claimed: false,
            };
            assert!(!empty_position.has_position());
            assert_eq!(empty_position.get_total_stake(), 0);

            // Test max values
            let max_position = Position {
                market: Pubkey::new_unique(),
                better: Pubkey::new_unique(),
                yes_amount: u64::MAX,
                no_amount: u64::MAX,
                claimed: true,
            };
            assert!(max_position.has_position());
            // This will overflow in get_total_stake - testing saturating add
            assert_eq!(max_position.get_total_stake(), u64::MAX);
        }

        #[test]
        fn test_calculate_payout_all_scenarios() {
            // Normal case
            assert_eq!(calculate_payout(100, 1000, 500).unwrap(), 150);

            // No losing bets
            assert_eq!(calculate_payout(100, 1000, 0).unwrap(), 100);

            // Equal pools
            assert_eq!(calculate_payout(100, 1000, 1000).unwrap(), 200);

            // Small winning pool
            assert_eq!(calculate_payout(1, 10, 1000).unwrap(), 101);

            // Large numbers
            assert_eq!(calculate_payout(1_000_000, 10_000_000, 5_000_000).unwrap(), 1_500_000);

            // No winning pool (error case)
            assert!(calculate_payout(100, 0, 1000).is_err());

            // Overflow case
            assert!(calculate_payout(u64::MAX, u64::MAX, u64::MAX).is_err());
        }

        #[test]
        fn test_calculate_fee_all_scenarios() {
            // Standard fees
            let (fee, after) = calculate_fee(10000, 100).unwrap();
            assert_eq!(fee, 100);
            assert_eq!(after, 9900);

            // Zero fee
            let (fee, after) = calculate_fee(10000, 0).unwrap();
            assert_eq!(fee, 0);
            assert_eq!(after, 10000);

            // Max fee (100%)
            let (fee, after) = calculate_fee(10000, 10000).unwrap();
            assert_eq!(fee, 10000);
            assert_eq!(after, 0);

            // Small amounts
            let (fee, after) = calculate_fee(1, 100).unwrap();
            assert_eq!(fee, 0); // Rounds down
            assert_eq!(after, 1);

            // Large amounts
            let (fee, after) = calculate_fee(1_000_000_000, 250).unwrap();
            assert_eq!(fee, 25_000_000);
            assert_eq!(after, 975_000_000);

            // Overflow protection
            assert!(calculate_fee(u64::MAX, u64::MAX).is_err());
        }

        #[test]
        fn test_market_params_default() {
            let params = MarketParams::default();
            assert_eq!(params.min_bet_amount, 100_000);
            assert_eq!(params.max_bet_amount, 1_000_000_000_000);
            assert_eq!(params.min_settlement_time, 3600);
            assert_eq!(params.max_settlement_time, 365 * 24 * 3600);
            assert_eq!(params.max_price_confidence, 5);
        }

        #[test]
        fn test_market_zero_pools() {
            let market = Market {
                creator: Pubkey::new_unique(),
                pyth_feed: Pubkey::new_unique(),
                target_price: 0,
                settle_time: 0,
                yes_pool: 0,
                no_pool: 0,
                collateral_mint: Pubkey::default(),
                is_resolved: false,
                winning_outcome: None,
                nonce: 0,
                bump: 0,
                resolver_authority: Pubkey::default(),
                total_volume: 0,
                fee_bps: 0,
                final_price: None,
            };

            let (yes_odds, no_odds) = market.calculate_odds();
            assert_eq!(yes_odds, 0.5);
            assert_eq!(no_odds, 0.5);
            assert_eq!(market.get_total_pot(), 0);
        }

        #[test]
        fn test_outcome_enum() {
            // Test PartialEq
            assert_eq!(Outcome::Yes, Outcome::Yes);
            assert_eq!(Outcome::No, Outcome::No);
            assert_ne!(Outcome::Yes, Outcome::No);

            // Test Clone
            let outcome = Outcome::Yes;
            let cloned = outcome.clone();
            assert_eq!(outcome, cloned);

            // Test Debug (just ensure it doesn't panic)
            let _ = format!("{:?}", Outcome::Yes);
            let _ = format!("{:?}", Outcome::No);
        }

        #[test]
        fn test_saturating_operations() {
            // Test saturating add in get_total_pot
            let mut market = Market {
                creator: Pubkey::new_unique(),
                pyth_feed: Pubkey::new_unique(),
                target_price: 50000,
                settle_time: 0,
                yes_pool: u64::MAX - 1,
                no_pool: 2,
                collateral_mint: Pubkey::new_unique(),
                is_resolved: false,
                winning_outcome: None,
                nonce: 0,
                bump: 0,
                resolver_authority: Pubkey::new_unique(),
                total_volume: 0,
                fee_bps: 0,
                final_price: None,
            };

            // Should saturate to u64::MAX
            assert_eq!(market.get_total_pot(), u64::MAX);

            // Test with both at max
            market.yes_pool = u64::MAX;
            market.no_pool = u64::MAX;
            assert_eq!(market.get_total_pot(), u64::MAX);
        }

        #[test]
        fn test_position_winning_stake_edge_cases() {
            let position = Position {
                market: Pubkey::new_unique(),
                better: Pubkey::new_unique(),
                yes_amount: u64::MAX,
                no_amount: u64::MAX,
                claimed: false,
            };

            // Test all possible u8 values for outcome
            for outcome in 0u8..=255u8 {
                let stake = position.get_winning_stake(outcome);
                match outcome {
                    0 => assert_eq!(stake, u64::MAX),
                    1 => assert_eq!(stake, u64::MAX),
                    _ => assert_eq!(stake, 0),
                }
            }
        }

        #[test]
        fn test_calculate_payout_precision() {
            // Test precision with small amounts
            assert_eq!(calculate_payout(1, 1000000, 1).unwrap(), 1);
            assert_eq!(calculate_payout(1, 1, 1000000).unwrap(), 1000001);

            // Test rounding
            assert_eq!(calculate_payout(3, 10, 7).unwrap(), 5); // 3 * 17 / 10 = 5.1, rounds down to 5
            assert_eq!(calculate_payout(7, 10, 3).unwrap(), 9); // 7 * 13 / 10 = 9.1, rounds down to 9
        }

        #[test]
        fn test_calculate_fee_precision() {
            // Test precision loss with small amounts
            let (fee, after) = calculate_fee(99, 100).unwrap(); // 0.99, rounds down to 0
            assert_eq!(fee, 0);
            assert_eq!(after, 99);

            let (fee, after) = calculate_fee(100, 100).unwrap(); // Exactly 1
            assert_eq!(fee, 1);
            assert_eq!(after, 99);

            let (fee, after) = calculate_fee(10001, 100).unwrap(); // 100.01, rounds down to 100
            assert_eq!(fee, 100);
            assert_eq!(after, 9901);
        }
    }

    mod instruction_helpers {
        use super::*;

        #[test]
        fn test_market_size_constant() {
            // Verify SIZE constant is correctly calculated
            let expected = 8 + // discriminator
                32 + // creator
                32 + // pyth_feed
                8 +  // target_price
                8 +  // settle_time
                8 +  // yes_pool
                8 +  // no_pool
                32 + // collateral_mint
                1 +  // is_resolved
                2 +  // winning_outcome Option<u8>
                8 +  // nonce
                1 +  // bump
                32 + // resolver_authority
                8 +  // total_volume
                8 +  // fee_bps
                9;   // final_price Option<i64>

            assert_eq!(Market::SIZE, expected);
        }

        #[test]
        fn test_position_size_constant() {
            let expected = 8 + // discriminator
                32 + // market
                32 + // better
                8 +  // yes_amount
                8 +  // no_amount
                1;   // claimed

            assert_eq!(Position::SIZE, expected);
        }
    }

    mod error_helper_methods {
        use super::*;

        #[test]
        fn test_error_code_calculation() {
            // Test that error codes are consistently calculated
            let err = PredictionMarketError::InvalidAmount;
            let code = err.error_code();
            assert!(code >= 6000);
            assert!(code < 7000);
        }

        #[test]
        fn test_all_error_categories() {
            // Recoverable errors
            assert!(PredictionMarketError::SlippageExceeded.is_recoverable());
            assert!(PredictionMarketError::TransactionExpired.is_recoverable());
            assert!(PredictionMarketError::PriceUnavailable.is_recoverable());
            assert!(PredictionMarketError::StaleOracleData.is_recoverable());

            // Validation errors
            assert!(PredictionMarketError::InvalidAmount.is_validation_error());
            assert!(PredictionMarketError::InvalidTargetPrice.is_validation_error());
            assert!(PredictionMarketError::InvalidSettleTime.is_validation_error());
            assert!(PredictionMarketError::BetTooLarge.is_validation_error());
            assert!(PredictionMarketError::BetTooSmall.is_validation_error());

            // Auth errors
            assert!(PredictionMarketError::Unauthorized.is_auth_error());
            assert!(PredictionMarketError::MissingSignature.is_auth_error());
            assert!(PredictionMarketError::InvalidAuthority.is_auth_error());

            // Non-categorized errors
            assert!(!PredictionMarketError::MarketAlreadyResolved.is_recoverable());
            assert!(!PredictionMarketError::MarketAlreadyResolved.is_validation_error());
            assert!(!PredictionMarketError::MarketAlreadyResolved.is_auth_error());
        }
    }
}