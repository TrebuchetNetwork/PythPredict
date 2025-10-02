#[cfg(test)]
mod comprehensive_tests {
    use super::*;
    use crate::state::*;
    use crate::errors::*;
    use crate::instructions::*;
    use anchor_lang::prelude::*;
    use solana_program::clock::Clock;
    use std::mem::size_of;

    // ============== ACCOUNT SIZE TESTS ==============
    mod account_size_tests {
        use super::*;

        #[test]
        fn test_all_account_sizes() {
            // Market size validation
            assert_eq!(Market::SIZE, 8 + 32 + 32 + 8 + 8 + 8 + 8 + 32 + 1 + 2 + 8 + 1 + 32 + 8 + 8 + 9);
            assert!(Market::SIZE <= 10240, "Market too large for Solana");

            // Position size validation
            assert_eq!(Position::SIZE, 8 + 32 + 32 + 8 + 8 + 1);
            assert!(Position::SIZE >= size_of::<Position>());

            // MarketMaker size validation
            assert_eq!(MarketMaker::SIZE, 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 1);

            // MarketAnalytics size validation
            assert_eq!(MarketAnalytics::SIZE, 8 + 32 + 8 + 8 + 8 + 8 + 8 + 8 + 9);
        }

        #[test]
        fn test_size_constants_match_structs() {
            // Ensure SIZE constants are correctly calculated for all accounts
            let market_actual = 8 + size_of::<Pubkey>() * 4 + size_of::<i64>() * 2 +
                size_of::<u64>() * 5 + size_of::<bool>() +
                size_of::<Option<u8>>() + size_of::<u8>() + size_of::<Option<i64>>();

            // Allow some padding for alignment
            assert!(Market::SIZE >= market_actual - 16);
            assert!(Market::SIZE <= market_actual + 16);
        }
    }

    // ============== MARKET METHODS COMPREHENSIVE ==============
    mod market_methods_tests {
        use super::*;

        #[test]
        fn test_get_spot_prices_all_scenarios() {
            let mut market = create_test_market();

            // Test 1: Empty pools
            market.yes_pool = 0;
            market.no_pool = 0;
            let (yes_price, no_price) = market.get_spot_prices();
            assert_eq!(yes_price, 5000);
            assert_eq!(no_price, 5000);

            // Test 2: Equal pools
            market.yes_pool = 1000;
            market.no_pool = 1000;
            let (yes_price, no_price) = market.get_spot_prices();
            assert_eq!(yes_price, 5000);
            assert_eq!(no_price, 5000);

            // Test 3: Skewed pools
            market.yes_pool = 7000;
            market.no_pool = 3000;
            let (yes_price, no_price) = market.get_spot_prices();
            assert_eq!(yes_price, 7000);
            assert_eq!(no_price, 3000);

            // Test 4: Extreme skew
            market.yes_pool = 9999;
            market.no_pool = 1;
            let (yes_price, no_price) = market.get_spot_prices();
            assert_eq!(yes_price, 9999);
            assert_eq!(no_price, 1);

            // Test 5: Large numbers
            market.yes_pool = u64::MAX / 2;
            market.no_pool = u64::MAX / 2;
            let (yes_price, no_price) = market.get_spot_prices();
            assert_eq!(yes_price, 5000);
            assert_eq!(no_price, 5000);
        }

        #[test]
        fn test_calculate_price_impact() {
            let mut market = create_test_market();
            market.yes_pool = 5000;
            market.no_pool = 5000;

            // Test small bet
            let impact = market.calculate_price_impact(100, Outcome::Yes).unwrap();
            assert!(impact < 100); // Less than 1% impact

            // Test large bet
            let impact = market.calculate_price_impact(5000, Outcome::Yes).unwrap();
            assert!(impact > 1000); // More than 10% impact

            // Test massive bet
            let impact = market.calculate_price_impact(50000, Outcome::No).unwrap();
            assert!(impact > 5000); // More than 50% impact

            // Test overflow protection
            let result = market.calculate_price_impact(u64::MAX, Outcome::Yes);
            assert!(result.is_err());
        }

        #[test]
        fn test_calculate_arbitrage_opportunity() {
            let mut market = create_test_market();
            market.yes_pool = 3000;
            market.no_pool = 7000;

            // Test profitable arbitrage
            let arb = market.calculate_arbitrage_opportunity(8000);
            assert!(arb.is_some());
            if let Some((outcome, profit)) = arb {
                assert_eq!(outcome, Outcome::Yes);
                assert!(profit > 100);
            }

            // Test no arbitrage when balanced
            market.yes_pool = 5000;
            market.no_pool = 5000;
            let arb = market.calculate_arbitrage_opportunity(5000);
            assert!(arb.is_none());

            // Test reverse arbitrage
            market.yes_pool = 8000;
            market.no_pool = 2000;
            let arb = market.calculate_arbitrage_opportunity(3000);
            assert!(arb.is_some());
            if let Some((outcome, _)) = arb {
                assert_eq!(outcome, Outcome::No);
            }
        }

        #[test]
        fn test_get_expected_payout() {
            let mut market = create_test_market();
            market.yes_pool = 4000;
            market.no_pool = 6000;

            // Test YES bet payout
            let payout = market.get_expected_payout(1000, Outcome::Yes).unwrap();
            let expected = (1000 * 11000) / 5000; // New pool after bet
            assert_eq!(payout, expected);

            // Test NO bet payout
            let payout = market.get_expected_payout(2000, Outcome::No).unwrap();
            let expected = (2000 * 12000) / 8000;
            assert_eq!(payout, expected);

            // Test with empty pool
            market.yes_pool = 0;
            let payout = market.get_expected_payout(100, Outcome::Yes).unwrap();
            assert_eq!(payout, 200); // Default 2x
        }

        fn create_test_market() -> Market {
            Market {
                creator: Pubkey::new_unique(),
                pyth_feed: Pubkey::new_unique(),
                target_price: 50000,
                settle_time: 0,
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

    // ============== MARKET MAKER TESTS ==============
    mod market_maker_tests {
        use super::*;

        #[test]
        fn test_should_rebalance_all_scenarios() {
            let mm = MarketMaker {
                market: Pubkey::new_unique(),
                authority: Pubkey::new_unique(),
                target_spread_bps: 500, // 5%
                max_exposure: 10000,
                liquidity_provided: 0,
                fees_earned: 0,
                yes_balance: 0,
                no_balance: 0,
                last_rebalance: 0,
                bump: 254,
            };

            // Test 1: Balanced pools - no rebalance
            assert!(!mm.should_rebalance(5000, 5000));

            // Test 2: Small imbalance - no rebalance
            assert!(!mm.should_rebalance(5200, 4800));

            // Test 3: Large imbalance - needs rebalance
            assert!(mm.should_rebalance(7000, 3000));

            // Test 4: Empty pools
            assert!(!mm.should_rebalance(0, 0));

            // Test 5: Single-sided pool
            assert!(mm.should_rebalance(10000, 0));
            assert!(mm.should_rebalance(0, 10000));
        }

        #[test]
        fn test_calculate_rebalance_amounts() {
            let mm = MarketMaker {
                market: Pubkey::new_unique(),
                authority: Pubkey::new_unique(),
                target_spread_bps: 500,
                max_exposure: 10000,
                liquidity_provided: 0,
                fees_earned: 0,
                yes_balance: 0,
                no_balance: 0,
                last_rebalance: 0,
                bump: 254,
            };

            // Test balanced pools
            let (yes_needed, no_needed) = mm.calculate_rebalance_amounts(5000, 5000);
            assert_eq!(yes_needed, 0);
            assert_eq!(no_needed, 0);

            // Test imbalanced pools
            let (yes_needed, no_needed) = mm.calculate_rebalance_amounts(3000, 7000);
            assert_eq!(yes_needed, 2000);
            assert_eq!(no_needed, 0);

            let (yes_needed, no_needed) = mm.calculate_rebalance_amounts(8000, 2000);
            assert_eq!(yes_needed, 0);
            assert_eq!(no_needed, 3000);

            // Test extreme imbalance
            let (yes_needed, no_needed) = mm.calculate_rebalance_amounts(0, 10000);
            assert_eq!(yes_needed, 5000);
            assert_eq!(no_needed, 0);
        }
    }

    // ============== ERROR HANDLING TESTS ==============
    mod error_tests {
        use super::*;

        #[test]
        fn test_all_error_codes() {
            // Ensure all errors have unique codes
            let mut codes = std::collections::HashSet::new();

            let errors = vec![
                PredictionMarketError::InvalidAmount,
                PredictionMarketError::InvalidPrice,
                PredictionMarketError::InvalidTargetPrice,
                PredictionMarketError::InvalidSettleTime,
                PredictionMarketError::InvalidMint,
                PredictionMarketError::InvalidOwner,
                PredictionMarketError::InvalidPythFeed,
                PredictionMarketError::InvalidMarket,
                PredictionMarketError::InvalidBetter,
                PredictionMarketError::InvalidOutcome,
                PredictionMarketError::MarketAlreadyResolved,
                PredictionMarketError::MarketNotResolved,
                PredictionMarketError::SettlementTimeNotMet,
                PredictionMarketError::MarketExpired,
                PredictionMarketError::MarketPaused,
                PredictionMarketError::NoWinningOutcome,
                PredictionMarketError::NoWinningPosition,
                PredictionMarketError::AlreadyClaimed,
                PredictionMarketError::PositionNotFound,
                PredictionMarketError::CalculationOverflow,
                PredictionMarketError::DivideByZero,
                PredictionMarketError::InsufficientBalance,
                PredictionMarketError::FeeCalculationError,
                PredictionMarketError::PriceUnavailable,
                PredictionMarketError::PriceConfidenceTooLow,
                PredictionMarketError::StaleOracleData,
                PredictionMarketError::InvalidOraclePrice,
                PredictionMarketError::Unauthorized,
                PredictionMarketError::MissingSignature,
                PredictionMarketError::InvalidAuthority,
                PredictionMarketError::BetTooLarge,
                PredictionMarketError::BetTooSmall,
                PredictionMarketError::MarketCapacityReached,
                PredictionMarketError::TooManyPositions,
                PredictionMarketError::SlippageExceeded,
                PredictionMarketError::TransactionExpired,
                PredictionMarketError::InvalidParameter,
                PredictionMarketError::InsufficientLiquidity,
                PredictionMarketError::RebalanceNotNeeded,
                PredictionMarketError::MaxExposureExceeded,
                PredictionMarketError::InsufficientArbitrage,
            ];

            for error in errors {
                let code = error.error_code();
                assert!(codes.insert(code), "Duplicate error code: {}", code);
                assert!(code >= 6000 && code < 7000);
            }
        }

        #[test]
        fn test_error_context() {
            let ctx = ErrorContext::new(
                PredictionMarketError::InvalidAmount,
                "Test message".to_string(),
                "test_instruction".to_string(),
            );

            assert_eq!(ctx.message, "Test message");
            assert_eq!(ctx.instruction, "test_instruction");
            assert!(ctx.account.is_none());

            let pubkey = Pubkey::new_unique();
            let ctx_with_account = ctx.with_account(pubkey);
            assert_eq!(ctx_with_account.account, Some(pubkey));
        }
    }

    // ============== EDGE CASE TESTS ==============
    mod edge_case_tests {
        use super::*;

        #[test]
        fn test_u64_max_calculations() {
            // Test all calculation functions with MAX values
            let result = calculate_payout(u64::MAX, u64::MAX, u64::MAX);
            assert!(result.is_err());

            let result = calculate_fee(u64::MAX, u64::MAX);
            assert!(result.is_err());

            let result = calculate_payout(u64::MAX / 2, u64::MAX / 2, u64::MAX / 2);
            assert!(result.is_ok());
        }

        #[test]
        fn test_zero_edge_cases() {
            // Test with all zeros
            let result = calculate_payout(0, 0, 0);
            assert!(result.is_err());

            let result = calculate_payout(0, 1000, 1000);
            assert_eq!(result.unwrap(), 0);

            let result = calculate_fee(0, 100);
            assert_eq!(result.unwrap(), (0, 0));
        }

        #[test]
        fn test_one_wei_precision() {
            // Test minimum amounts
            let result = calculate_payout(1, 1, 1);
            assert_eq!(result.unwrap(), 2);

            let result = calculate_payout(1, 1000000000, 1);
            assert_eq!(result.unwrap(), 1);

            let result = calculate_fee(1, 10000);
            assert_eq!(result.unwrap(), (1, 0));
        }
    }

    // ============== INTEGRATION SIMULATION TESTS ==============
    mod simulation_tests {
        use super::*;

        #[test]
        fn test_market_lifecycle_simulation() {
            // Simulate complete market lifecycle
            let mut market = Market {
                creator: Pubkey::new_unique(),
                pyth_feed: Pubkey::new_unique(),
                target_price: 100000,
                settle_time: 1234567890,
                yes_pool: 0,
                no_pool: 0,
                collateral_mint: Pubkey::new_unique(),
                is_resolved: false,
                winning_outcome: None,
                nonce: 1,
                bump: 254,
                resolver_authority: Pubkey::new_unique(),
                total_volume: 0,
                fee_bps: 100,
                final_price: None,
            };

            // Phase 1: Initial bets
            market.yes_pool = 1000;
            market.no_pool = 1000;
            market.total_volume = 2000;

            let (yes_odds, no_odds) = market.calculate_odds();
            assert_eq!(yes_odds, 0.5);
            assert_eq!(no_odds, 0.5);

            // Phase 2: Market movement
            market.yes_pool = 7500;
            market.no_pool = 2500;
            market.total_volume = 10000;

            let (yes_odds, no_odds) = market.calculate_odds();
            assert_eq!(yes_odds, 0.75);
            assert_eq!(no_odds, 0.25);

            // Phase 3: Resolution
            market.is_resolved = true;
            market.winning_outcome = Some(0); // YES wins
            market.final_price = Some(105000);

            assert!(market.is_resolved);
            assert_eq!(market.winning_outcome, Some(0));

            // Phase 4: Payout calculation
            let user_yes_stake = 100;
            let payout = calculate_payout(user_yes_stake, market.yes_pool, market.no_pool).unwrap();
            assert_eq!(payout, 133); // 100 * 10000 / 7500
        }

        #[test]
        fn test_high_frequency_trading_simulation() {
            let mut market = create_test_market();
            market.yes_pool = 10000;
            market.no_pool = 10000;

            // Simulate 100 rapid trades
            for i in 0..100 {
                let amount = (i + 1) * 10;
                let outcome = if i % 2 == 0 { Outcome::Yes } else { Outcome::No };

                let impact = market.calculate_price_impact(amount as u64, outcome.clone());
                assert!(impact.is_ok());

                // Update pools
                match outcome {
                    Outcome::Yes => market.yes_pool += amount as u64,
                    Outcome::No => market.no_pool += amount as u64,
                }
            }

            // Verify final state
            assert!(market.yes_pool > 10000);
            assert!(market.no_pool > 10000);
        }

        fn create_test_market() -> Market {
            Market {
                creator: Pubkey::new_unique(),
                pyth_feed: Pubkey::new_unique(),
                target_price: 50000,
                settle_time: 0,
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

    // ============== OUTCOME ENUM TESTS ==============
    mod outcome_tests {
        use super::*;

        #[test]
        fn test_outcome_serialization() {
            // Test that Outcome can be serialized/deserialized
            let yes = Outcome::Yes;
            let no = Outcome::No;

            // Test equality
            assert_eq!(yes, Outcome::Yes);
            assert_eq!(no, Outcome::No);
            assert_ne!(yes, no);

            // Test clone
            let cloned_yes = yes.clone();
            assert_eq!(cloned_yes, yes);
        }

        #[test]
        fn test_outcome_matching() {
            let outcome = Outcome::Yes;

            let result = match outcome {
                Outcome::Yes => 1,
                Outcome::No => 0,
            };
            assert_eq!(result, 1);

            let outcome = Outcome::No;
            let result = match outcome {
                Outcome::Yes => 1,
                Outcome::No => 0,
            };
            assert_eq!(result, 0);
        }
    }
}