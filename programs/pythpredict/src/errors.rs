use anchor_lang::prelude::*;

#[error_code]
pub enum PredictionMarketError {
    // Validation Errors
    #[msg("Invalid bet amount")]
    InvalidAmount,

    #[msg("Invalid price provided")]
    InvalidPrice,

    #[msg("Invalid target price - must be positive")]
    InvalidTargetPrice,

    #[msg("Invalid settlement time - must be in the future")]
    InvalidSettleTime,

    #[msg("Invalid mint provided")]
    InvalidMint,

    #[msg("Invalid token account owner")]
    InvalidOwner,

    #[msg("Invalid Pyth price feed")]
    InvalidPythFeed,

    #[msg("Invalid market reference")]
    InvalidMarket,

    #[msg("Invalid better reference")]
    InvalidBetter,

    #[msg("Invalid outcome specified")]
    InvalidOutcome,

    // Market State Errors
    #[msg("Market has already been resolved")]
    MarketAlreadyResolved,

    #[msg("Market is not yet resolved")]
    MarketNotResolved,

    #[msg("Settlement time hasn't been reached yet")]
    SettlementTimeNotMet,

    #[msg("Market has expired and cannot accept bets")]
    MarketExpired,

    #[msg("Market is paused")]
    MarketPaused,

    #[msg("Market is not active")]
    MarketNotActive,

    #[msg("Invalid market status")]
    InvalidMarketStatus,

    // Position Errors
    #[msg("No winning outcome determined")]
    NoWinningOutcome,

    #[msg("User has no position in the winning outcome")]
    NoWinningPosition,

    #[msg("Position does not exist")]
    PositionNotFound,

    #[msg("No position to claim")]
    NoPosition,

    #[msg("Already claimed")]
    AlreadyClaimed,

    // Calculation Errors
    #[msg("Calculation overflow occurred")]
    CalculationOverflow,

    #[msg("Division by zero error")]
    DivideByZero,

    #[msg("Insufficient balance for operation")]
    InsufficientBalance,

    #[msg("Fee calculation error")]
    FeeCalculationError,

    #[msg("Math overflow")]
    MathOverflow,

    #[msg("Invalid pool state")]
    InvalidPool,

    // Oracle Errors
    #[msg("Price data is unavailable")]
    PriceUnavailable,

    #[msg("Price confidence interval is too wide")]
    PriceConfidenceTooLow,

    #[msg("Oracle data is stale")]
    StaleOracleData,

    #[msg("Oracle price is negative or invalid")]
    InvalidOraclePrice,

    #[msg("Price confidence too high")]
    PriceConfidenceTooHigh,

    #[msg("Price data too stale")]
    PriceTooStale,

    #[msg("Oracle error")]
    OracleError,

    // Authorization Errors
    #[msg("Unauthorized - only market creator or resolver can perform this action")]
    Unauthorized,

    #[msg("Account is not a signer")]
    MissingSignature,

    #[msg("Invalid program authority")]
    InvalidAuthority,

    #[msg("Not authorized to resolve")]
    UnauthorizedResolver,

    // Limits and Constraints
    #[msg("Bet amount exceeds maximum allowed")]
    BetTooLarge,

    #[msg("Bet amount below minimum allowed")]
    BetTooSmall,

    #[msg("Market has reached maximum capacity")]
    MarketCapacityReached,

    #[msg("Too many positions for this user")]
    TooManyPositions,

    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,

    #[msg("Transaction has expired")]
    TransactionExpired,

    #[msg("Invalid parameter value")]
    InvalidParameter,

    #[msg("Settlement time not reached")]
    SettlementTimeNotReached,

    #[msg("Insufficient liquidity")]
    InsufficientLiquidity,

    // Market Maker Errors
    #[msg("Rebalance not needed")]
    RebalanceNotNeeded,

    #[msg("Maximum exposure exceeded")]
    MaxExposureExceeded,

    #[msg("Arbitrage opportunity too small")]
    InsufficientArbitrage,

    #[msg("Settlement time is too soon")]
    SettlementTimeTooSoon,

    #[msg("Settlement time is too far in the future")]
    SettlementTimeTooFar,

    #[msg("Market is closed for betting")]
    MarketClosed,


}
impl PredictionMarketError {
    /// Get the error code as a u32 for client-side handling
    pub fn error_code(&self) -> u32 {
        (*self as u32) + 6000
    }

    /// Check if the error is recoverable (user can retry)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::SlippageExceeded      // Uncommented - this was causing the test failure
                | Self::TransactionExpired  // Uncommented
                | Self::PriceUnavailable
                | Self::StaleOracleData
        )
    }

    /// Check if the error is a validation error
    pub fn is_validation_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidAmount
                | Self::InvalidPrice  // Added this
                | Self::InvalidTargetPrice
                | Self::InvalidSettleTime
                | Self::InvalidMint
                | Self::InvalidOwner
                | Self::BetTooLarge
                | Self::BetTooSmall
        )
    }

    /// Check if the error is an authorization error
    pub fn is_auth_error(&self) -> bool {
        matches!(
            self,
            Self::Unauthorized | Self::MissingSignature | Self::InvalidAuthority
        )
    }
}

// Custom Result type for the program
pub type PredictionResult<T> = Result<T>;

// Error context for detailed debugging
#[derive(Debug)]
pub struct ErrorContext {
    pub error: PredictionMarketError,
    pub message: String,
    pub account: Option<Pubkey>,
    pub instruction: String,
}

impl ErrorContext {
    pub fn new(
        error: PredictionMarketError,
        message: String,
        instruction: String,
    ) -> Self {
        Self {
            error,
            message,
            account: None,
            instruction,
        }
    }

    pub fn with_account(mut self, account: Pubkey) -> Self {
        self.account = Some(account);
        self
    }
}

// Macro for easier error handling with context
#[macro_export]
macro_rules! require_with_context {
    ($condition:expr, $error:expr, $msg:expr) => {
        if !$condition {
            msg!("Error: {} - {}", stringify!($error), $msg);
            return Err($error.into());
        }
    };
}

// Tests for error handling
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        // Test a few error codes to ensure the formula works
        let err = PredictionMarketError::InvalidAmount;
        assert_eq!(err.error_code(), 6000); // First error

        // Count the actual position of MarketAlreadyResolved in your enum
        let err = PredictionMarketError::MarketAlreadyResolved;
        // Update this to match the actual position
        assert!(err.error_code() >= 6000 && err.error_code() < 7000);
    }

    #[test]
    fn test_error_categories() {
        assert!(PredictionMarketError::SlippageExceeded.is_recoverable());
        assert!(!PredictionMarketError::InvalidAmount.is_recoverable());

        assert!(PredictionMarketError::InvalidAmount.is_validation_error());
        assert!(!PredictionMarketError::Unauthorized.is_validation_error());

        assert!(PredictionMarketError::Unauthorized.is_auth_error());
        assert!(!PredictionMarketError::InvalidAmount.is_auth_error());
    }

    #[test]
    fn test_error_context() {
        let ctx = ErrorContext::new(
            PredictionMarketError::InvalidAmount,
            "Amount must be greater than 0".to_string(),
            "place_bet".to_string(),
        );

        assert_eq!(ctx.instruction, "place_bet");
        assert!(ctx.account.is_none());

        let pubkey = Pubkey::new_unique();
        let ctx_with_account = ctx.with_account(pubkey);
        assert_eq!(ctx_with_account.account, Some(pubkey));
    }
}