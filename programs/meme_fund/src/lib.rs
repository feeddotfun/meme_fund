use anchor_lang::prelude::*;

declare_id!("FQRP7BsLL83pktuo4yYHABntASh9xa4wo9nCpDpwydzy");

const MIN_SOL_AMOUNT: u64 = 100_000_000; // 0.1 SOL in lamports
const MAX_SOL_AMOUNT: u64 = 2_000_000_000; // 2 SOL in lamports
const MAX_FUND_LIMIT: u64 = 20_000_000_000; // 20 SOL in lamports
const MAX_COMMISSION_RATE: u8 = 10; // 10%
const MAX_TOKEN_CLAIM_AVAILABLE_TIME: i64 = 60 * 60 * 24; // 24 hours

#[program]
pub mod meme_fund {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        fee_recipient: Pubkey, 
        initial_min_buy_amount: u64, 
        initial_max_buy_amount: u64, 
        initial_fund_duration: i64, 
        initial_max_fund_limit: u64,
        initial_commission_rate: u8,
        initial_token_claim_available_time: i64,
    ) -> Result<()> {
        // Ensure the initial fun duration is greater than 0
        require!(initial_fund_duration > 0, MemeError::InvalidFundDuration);

        // Ensure the initial min buy amount is less than or equal to the initial max buy amount
        require!(initial_min_buy_amount <= initial_max_buy_amount, MemeError::InvalidBuyAmount);

        let state = &mut ctx.accounts.state;
        state.fee_recipient = fee_recipient;
        state.min_buy_amount = initial_min_buy_amount.max(MIN_SOL_AMOUNT);
        state.max_buy_amount = initial_max_buy_amount.min(MAX_SOL_AMOUNT);
        state.fund_duration = initial_fund_duration;      
        state.max_fund_limit = initial_max_fund_limit.min(MAX_FUND_LIMIT);
        state.commission_rate = initial_commission_rate.min(MAX_COMMISSION_RATE);
        state.token_claim_available_time = initial_token_claim_available_time.min(MAX_TOKEN_CLAIM_AVAILABLE_TIME);
        state.authority = ctx.accounts.authority.key();
       
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 8 + 8 + 32 + 8 + 8 + 1 + 8, // discriminator + fee_recipient + min_buy_amount + max_buy_amount + authority + fund_duration + max_fund_limit + commission_rate + token_claim_available_time
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum MemeError {
    #[msg("Invalid fund duration")]
    InvalidFundDuration,
    #[msg("Invalid buy amount: minimum exceeds maximum")]
    InvalidBuyAmount,
}