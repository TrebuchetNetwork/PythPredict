#[cfg(test)]
mod state_coverage_tests {
    use crate::state::*;
    use crate::errors::PredictionMarketError;
    use anchor_lang::prelude::*;

    #[test]
    fn test_market_maker_should_rebalance() {
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

        // Test empty pools
        assert!(!mm.should_rebalance(0, 0));

        // Test balanced pools
        assert!(!mm.should_rebalance(5000, 5000));

        // Test small imbalance (within threshold)
        assert!(!mm.should_rebalance(5200, 4800));

        // Test large imbalance (needs rebalance)
        assert!(mm.should_rebalance(7000, 3000));
        assert!(mm.should_rebalance(2000, 8000));

        // Test extreme cases
        assert!(mm.should_rebalance(10000, 0));
        assert!(mm.should_rebalance(0, 10000));
        assert!(mm.should_rebalance(9000, 1000));
    }

    #[test]
    fn test_market_maker_calculate_rebalance_amounts() {
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
        let (yes, no) = mm.calculate_rebalance_amounts(5000, 5000);
        assert_eq!(yes, 0);
        assert_eq!(no, 0);

        // Test imbalanced - more NO needed
        let (yes, no) = mm.calculate_rebalance_amounts(3000, 7000);
        assert_eq!(yes, 2000);
        assert_eq!(no, 0);

        // Test imbalanced - more YES needed
        let (yes, no) = mm.calculate_rebalance_amounts(8000, 2000);
        assert_eq!(yes, 0);
        assert_eq!(no, 3000);

        // Test extreme imbalance
        let (yes, no) = mm.calculate_rebalance_amounts(0, 10000);
        assert_eq!(yes, 5000);
        assert_eq!(no, 0);

        let (yes, no) = mm.calculate_rebalance_amounts(10000, 0);
        assert_eq!(yes, 0);
        assert_eq!(no, 5000);

        // Test empty pools
        let (yes, no) = mm.calculate_rebalance_amounts(0, 0);
        assert_eq!(yes, 0);
        assert_eq!(no, 0);
    }

    #[test]
    fn test_market_get_spot_prices() {
        let mut market = create_test_market();

        // Test empty pools
        market.yes_pool = 0;
        market.no_pool = 0;
        let (yes_price, no_price) = market.get_spot_prices();
        assert_eq!(yes_price, 5000);
        assert_eq!(no_price, 5000);

        // Test equal pools
        market.yes_pool = 1000;
        market.no_pool = 1000;
        let (yes_price, no_price) = market.get_spot_prices();
        assert_eq!(yes_price, 5000);
        assert_eq!(no_price, 5000);

        // Test 70/30 split
        market.yes_pool = 7000;
        market.no_pool = 3000;
        let (yes_price, no_price) = market.get_spot_prices();
        assert_eq!(yes_price, 7000);
        assert_eq!(no_price, 3000);

        // Test 90/10 split
        market.yes_pool = 9000;
        market.no_pool = 1000;
        let (yes_price, no_price) = market.get_spot_prices();
        assert_eq!(yes_price, 9000);
        assert_eq!(no_price, 1000);

        // Test extreme case
        market.yes_pool = 9999;
        market.no_pool = 1;
        let (yes_price, no_price) = market.get_spot_prices();
        assert_eq!(yes_price, 9999);
        assert_eq!(no_price, 1);
    }

    #[test]
    fn test_market_calculate_price_impact() {
        let mut market = create_test_market();
        market.yes_pool = 5000;
        market.no_pool = 5000;

        // Small bet - minimal impact
        let impact = market.calculate_price_impact(100, Outcome::Yes).unwrap();
        assert!(impact < 100); // Less than 1%

        // Medium bet
        let impact = market.calculate_price_impact(1000, Outcome::Yes).unwrap();
        assert!(impact > 500 && impact < 2000);

        // Large bet - significant impact
        let impact = market.calculate_price_impact(5000, Outcome::Yes).unwrap();
        assert!(impact > 2000);

        // Test NO side
        let impact = market.calculate_price_impact(2000, Outcome::No).unwrap();
        assert!(impact > 1000);

        // Test with imbalanced pools
        market.yes_pool = 8000;
        market.no_pool = 2000;
        let impact = market.calculate_price_impact(1000, Outcome::No).unwrap();
        assert!(impact > 2000); // Large impact on small pool

        // Test overflow
        let result = market.calculate_price_impact(u64::MAX, Outcome::Yes);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(),
            Error::from(PredictionMarketError::CalculationOverflow)));
    }

    #[test]
    fn test_market_calculate_arbitrage_opportunity() {
        let mut market = create_test_market();

        // Balanced market - external matches internal
        market.yes_pool = 5000;
        market.no_pool = 5000;
        let arb = market.calculate_arbitrage_opportunity(5000);
        assert!(arb.is_none());

        // YES is underpriced internally
        market.yes_pool = 3000;
        market.no_pool = 7000;
        let arb = market.calculate_arbitrage_opportunity(8000);
        assert!(arb.is_some());
        let (outcome, profit) = arb.unwrap();
        assert_eq!(outcome, Outcome::Yes);
        assert!(profit > 100);

        // NO is underpriced internally
        market.yes_pool = 8000;
        market.no_pool = 2000;
        let arb = market.calculate_arbitrage_opportunity(3000);
        assert!(arb.is_some());
        let (outcome, profit) = arb.unwrap();
        assert_eq!(outcome, Outcome::No);
        assert!(profit > 100);

        // Small difference - no arb (below MIN_PROFIT_BPS)
        market.yes_pool = 4950;
        market.no_pool = 5050;
        let arb = market.calculate_arbitrage_opportunity(5000);
        assert!(arb.is_none());
    }

    #[test]
    fn test_market_get_expected_payout() {
        let mut market = create_test_market();

        // Test with empty pool
        market.yes_pool = 0;
        market.no_pool = 5000;
        let payout = market.get_expected_payout(1000, Outcome::Yes).unwrap();
        assert_eq!(payout, 2000); // Default 2x

        // Test normal case
        market.yes_pool = 4000;
        market.no_pool = 6000;
        let payout = market.get_expected_payout(1000, Outcome::Yes).unwrap();
        // (1000 * 11000) / 5000 = 2200
        assert_eq!(payout, 2200);

        // Test NO side
        let payout = market.get_expected_payout(2000, Outcome::No).unwrap();
        // (2000 * 12000) / 8000 = 3000
        assert_eq!(payout, 3000);

        // Test with imbalanced pools
        market.yes_pool = 9000;
        market.no_pool = 1000;
        let payout = market.get_expected_payout(100, Outcome::No).unwrap();
        // (100 * 10100) / 1100 = 918
        assert_eq!(payout, 918);
    }

    #[test]
    fn test_position_methods_comprehensive() {
        // Test empty position
        let pos = Position {
            market: Pubkey::new_unique(),
            better: Pubkey::new_unique(),
            yes_amount: 0,
            no_amount: 0,
            claimed: false,
        };
        assert_eq!(pos.get_total_stake(), 0);
        assert!(!pos.has_position());

        // Test YES only position
        let pos = Position {
            market: Pubkey::new_unique(),
            better: Pubkey::new_unique(),
            yes_amount: 1000,
            no_amount: 0,
            claimed: false,
        };
        assert_eq!(pos.get_total_stake(), 1000);
        assert!(pos.has_position());
        assert_eq!(pos.get_winning_stake(0), 1000);
        assert_eq!(pos.get_winning_stake(1), 0);

        // Test NO only position
        let pos = Position {
            market: Pubkey::new_unique(),
            better: Pubkey::new_unique(),
            yes_amount: 0,
            no_amount: 500,
            claimed: true,
        };
        assert_eq!(pos.get_total_stake(), 500);
        assert!(pos.has_position());
        assert_eq!(pos.get_winning_stake(0), 0);
        assert_eq!(pos.get_winning_stake(1), 500);

        // Test mixed position
        let pos = Position {
            market: Pubkey::new_unique(),
            better: Pubkey::new_unique(),
            yes_amount: 300,
            no_amount: 700,
            claimed: false,
        };
        assert_eq!(pos.get_total_stake(), 1000);
        assert!(pos.has_position());
        assert_eq!(pos.get_winning_stake(0), 300);
        assert_eq!(pos.get_winning_stake(1), 700);
        assert_eq!(pos.get_winning_stake(2), 0);
        assert_eq!(pos.get_winning_stake(255), 0);
    }

    #[test]
    fn test_calculate_payout_comprehensive() {
        // Normal cases
        assert_eq!(calculate_payout(100, 1000, 500).unwrap(), 150);
        assert_eq!(calculate_payout(200, 1000, 1000).unwrap(), 400);
        assert_eq!(calculate_payout(50, 500, 500).unwrap(), 100);

        // No losing pool
        assert_eq!(calculate_payout(100, 1000, 0).unwrap(), 100);
        assert_eq!(calculate_payout(500, 2000, 0).unwrap(), 500);

        // No winning pool (error)
        assert!(calculate_payout(100, 0, 1000).is_err());

        // Large numbers
        assert_eq!(calculate_payout(1_000_000, 10_000_000, 5_000_000).unwrap(), 1_500_000);

        // Precision test
        assert_eq!(calculate_payout(3, 10, 7).unwrap(), 5);
        assert_eq!(calculate_payout(7, 13, 7).unwrap(), 10);

        // Overflow protection
        assert!(calculate_payout(u64::MAX, u64::MAX, u64::MAX).is_err());
        assert!(calculate_payout(u64::MAX / 2, u64::MAX / 2, u64::MAX / 2).is_ok());
    }

    #[test]
    fn test_calculate_fee_comprehensive() {
        // Standard fees
        let (fee, after) = calculate_fee(10000, 100).unwrap();
        assert_eq!(fee, 100);
        assert_eq!(after, 9900);

        // 2.5% fee
        let (fee, after) = calculate_fee(10000, 250).unwrap();
        assert_eq!(fee, 250);
        assert_eq!(after, 9750);

        // Zero fee
        let (fee, after) = calculate_fee(10000, 0).unwrap();
        assert_eq!(fee, 0);
        assert_eq!(after, 10000);

        // 100% fee
        let (fee, after) = calculate_fee(10000, 10000).unwrap();
        assert_eq!(fee, 10000);
        assert_eq!(after, 0);

        // Small amounts (test rounding)
        let (fee, after) = calculate_fee(1, 100).unwrap();
        assert_eq!(fee, 0);
        assert_eq!(after, 1);

        let (fee, after) = calculate_fee(99, 100).unwrap();
        assert_eq!(fee, 0);
        assert_eq!(after, 99);

        let (fee, after) = calculate_fee(100, 100).unwrap();
        assert_eq!(fee, 1);
        assert_eq!(after, 99);

        // Overflow test
        assert!(calculate_fee(u64::MAX, u64::MAX).is_err());
    }

    #[test]
    fn test_outcome_enum() {
        let yes = Outcome::Yes;
        let no = Outcome::No;

        assert_eq!(yes, Outcome::Yes);
        assert_eq!(no, Outcome::No);
        assert_ne!(yes, no);

        let cloned = yes.clone();
        assert_eq!(cloned, Outcome::Yes);

        // Test pattern matching
        match yes {
            Outcome::Yes => assert!(true),
            Outcome::No => assert!(false),
        }
    }

    #[test]
    fn test_size_constants() {
        assert_eq!(MarketMaker::SIZE, 137);
        assert_eq!(Market::SIZE, 8 + Market::INIT_SPACE);
        assert!(Market::SIZE <= 10240);
        assert_eq!(Position::SIZE, 81);
        assert_eq!(MarketAnalytics::SIZE, 89);
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