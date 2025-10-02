🚀 PYTHPREDICT COMPLETE TEST SUITE

========================================
   PYTHPREDICT TEST SUITE
========================================

🔧 Initializing test environment...

   📂 Loading wallets...
   💰 Funding alice...
   💰 Funding bob...
   💰 Funding charlie...
   💰 Funding dave...
   💰 Funding eve...
   ✅ Created token mint: 3nKY314X2rqJmvyTabh6wixgN9S6q4HsREh6wBWpGe16

   ✅ Minted 1000 tokens to alice
   ✅ Minted 1000 tokens to bob
   ✅ Minted 500 tokens to charlie
   ✅ Minted 500 tokens to dave
   ✅ Minted 500 tokens to eve

   💰 Total token supply: 3,500 tokens
   ✨ Environment ready!

    📋 Section 1: Core Functionality Tests
   ✅ Market 'coreTestMarket' created: A7P52Np3...
      ✔ Should create a market with correct parameters (456ms)
      ✔ Should place bets with correct fee calculation (452ms)
      ✔ Should update odds correctly after multiple bets (928ms)

   ⏳ Waiting for market to reach settlement time...
   ✅ Market 'coreTestMarket' resolved. Winner: UNKNOWN
      ✔ Should resolve market and determine winner correctly (12358ms)
      ✔ Should distribute winnings correctly to winners (910ms)
   ⚠️ Attempt 1 failed, retrying in 1000ms...
   ⚠️ Attempt 2 failed, retrying in 2000ms...
      ✔ Should prevent double claiming (3032ms)
    🔬 Section 2: Edge Cases and Precision Tests
   ✅ Market 'edgeTestMarket' created: P9XM7yYp...
      ✔ Should handle minimum bet amounts (602ms)
   📊 Token Conservation Check:
      Initial: 3500.00
      Current: 3500.00
      Deviation: 0.000000
      ✔ Should maintain token conservation (934ms)
   ✅ Market 'emptyMarket' created: 5ALc3Gmi...
   ✅ Market 'emptyMarket' resolved. Winner: NO
   ✅ Successfully resolved empty market
      ✔ Should handle market with no bets (17927ms)
    ⚡ Section 3: Multi-Market Stress Test

   🔥 Creating multiple markets concurrently...
   ✅ Market 'stress3' created: 7ycovG4i...
   ✅ Market 'stress1' created: FttQJDRG...
   ✅ Market 'stress2' created: 8khjM6ua...
   ✅ Created 3 markets
   ✅ Placed 9 bets
      ✔ Should handle multiple concurrent markets (4618ms)
    🌐 Section 4: Live BTC Price Market

   🌐 Fetching live BTC price from Pyth Network...
      Raw price: 11882901963831, exponent: -8, final: $118829.02
   ✅ Live BTC Price: $118829.02
      Confidence: ±$34.50
      ✔ Should fetch real BTC price from Pyth Network (1057ms)
   ✅ Market 'liveBtcMarket' created: 3ZbTzbck...
   📈 Market created at price: $118829.01
      ✔ Should create market with live BTC price (309ms)

   🎲 Placing strategic bets:
      alice: $50 on YES - Momentum trader - expects continuation
      bob: $75 on NO - Mean reversion - expects stability
      charlie: $25 on YES - Following Alice

   📊 Market Sentiment:
      YES (price will move): 50.0%
      NO (price stays same): 50.0%
      ✔ Should place bets based on market sentiment (1378ms)

   ⏱️  Monitoring BTC price for 20 seconds...
   📍 Starting price: $118829.02
      Raw price: 11883066624765, exponent: -8, final: $118830.67
      [5s] $118830.67 (+1.65 | +0.001%)
      Raw price: 11883010157971, exponent: -8, final: $118830.10
      [10s] $118830.10 (+1.08 | +0.001%)
      Raw price: 11883036702392, exponent: -8, final: $118830.37
      [15s] $118830.37 (+1.35 | +0.001%)
      Raw price: 11883042034601, exponent: -8, final: $118830.42

   📍 Final price: $118830.42
   📈 Total change: +$1.40
   ✅ Market 'liveBtcMarket' resolved. Winner: UNKNOWN
   🏆 Winner: YES (price moved)
      ✔ Should monitor price and resolve with actual movement (22988ms)

   💰 Processing payouts based on price movement:
      alice: +$99.00 ✅
      charlie: +$49.50 ✅
      bob: Correctly rejected claim ❌

   📊 Summary:
      Price moved → YES wins
      Total payouts: $148.50
      ✔ Should distribute winnings based on actual price movement (1391ms)

============================================================
📊 TEST SUITE FINAL REPORT
============================================================

📈 Test Results:
   Total Tests: 15
   Passed: 15 (100.0%)
   Failed: 0

💰 Token Conservation:
   Initial Supply: 3500.00 tokens
   User Balances: 2922.18 tokens

✅ EXCELLENT: All critical tests passed!