# <div align="center"><img src="assets/logo.svg" width="200" height="200" alt="feed.fun"></div>

# Feed.fun Solana Program

Feed.fun is a Solana program that enables decentralized meme creation and investment through time-limited funding rounds.

## Features
- Meme registry management 
- Time-limited investment windows
- Token distribution system
- Security controls

## 📋 Overview
This Solana program handles the core functionality of Feed.fun's meme creation and investment system, including:

Meme registry management
Time-limited investment windows
Token distribution mechanics
Security controls and rate limiting

## 🏗 Architecture
Core Components

## State Management
```bash
pub struct State {
    pub fee_recipient: Pubkey,
    pub max_buy_amount: u64,
    pub min_buy_amount: u64,
    pub authority: Pubkey,
    pub fund_duration: i64,
    pub max_fund_limit: u64,
    pub commission_rate: u8,
    pub token_claim_available_time: i64,
}
```
## Meme Registry
```bash
pub struct MemeRegistry {
    pub meme_id: [u8; 16],
    pub total_funds: u64,
    pub start_time: i64,
    pub end_time: i64,
    pub authority: Pubkey,
    pub contributor_count: u64,
    pub mint: Pubkey,
    pub unclaimed_rewards: u64,
    pub claimed_count: u64,
}
```
## Contribution Tracking
```bash
#[account]
pub struct Contribution {
    pub meme_id: [u8; 16],      // Associated meme
    pub contributor: Pubkey,     // Investor address
    pub amount: u64,            // Investment amount
    pub timestamp: i64,         // Investment timestamp
    pub is_claimed: bool,       // Claim status
}
```
- 
## Security Parameters
```bash
const MIN_SOL_AMOUNT: u64 = 100_000_000;      // 0.1 SOL
const MAX_SOL_AMOUNT: u64 = 2_000_000_000;    // 2 SOL
const MAX_FUND_LIMIT: u64 = 20_000_000_000;   // 20 SOL
const MAX_COMMISSION_RATE: u8 = 10;           // 10%
```

## Program Flow

## 1. Initialization
   * Program state setup
   * Fee recipient configuration
   * Investment parameters
## 2. Meme Creation
   * Registry initialization
   * Time window setup
   * Vault PDA creation
## 3. Investment Process
   * Contribution validation
   * Commission handling
   * Fund tracking
## 4. Token Distribution
   * Token minting
   * Fair distribution calculation
   * Claim processing

## PDAs and Seeds
```bash
// Registry PDA
[b"registry", meme_id]

// Vault PDA
[b"vault", meme_id]

// State PDA
[b"state"]

// Contribution PDA
[b"contribution", meme_id, contributor_pubkey]
```

## Error Handling
```bash
pub enum MemeError {
    InvalidFundDuration,
    InvalidBuyAmount,
    BelowMinAmount,
    ExceedsMaxAmount,
    FundExpired,
    InsufficientBalance,
    ExceedsMaxFundLimit,
    AlreadyClaimed,
    ClaimTimeNotReached,
}
````

## Integration Points

## External Programs
- Token Program (SPL)
- Associated Token Program
- Metadata Program
- Pump Program (Custom)

## CPI (Cross-Program Invocation)
```bash
token::transfer_checked(
    CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        token::TransferChecked {
            from: vault_token_account,
            to: user_token_account,
            authority: vault_pda,
            mint: token_mint,
        },
        &[vault_seeds],
    ),
    amount,
    decimals,
)?;
```

## 🛠 Development Setup
Prerequisites

- Rust
- Solana Tool Suite
- Anchor Framework

## Build
```bash
anchor build
```

## Test
```bash
anchor test
```
