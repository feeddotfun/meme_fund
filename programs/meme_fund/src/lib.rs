use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;

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

    // Create a new meme registry
    pub fn create_meme_registry(ctx: Context<CreateMemeRegistry>, meme_id: [u8; 16]) -> Result<()> {        
        let registry = &mut ctx.accounts.registry;
        let clock = Clock::get().unwrap();
        let state = &ctx.accounts.state;
        
        registry.meme_id = meme_id.clone();
        registry.total_funds = 0;
        registry.start_time = clock.unix_timestamp;
        registry.end_time = clock.unix_timestamp + state.fund_duration;
        registry.authority = ctx.accounts.authority.key();
        registry.contributor_count = 0;
        registry.mint = Pubkey::default();
        registry.unclaimed_rewards = 0;
        registry.claimed_count = 0;

        // Emit event
        emit!(MemeRegistryCreated {
            meme_id,
            start_time: registry.start_time,
            end_time: registry.end_time,
        });

        Ok(())
    }


}

// States
#[account]
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

#[account]
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

#[derive(Accounts)]
#[instruction(meme_id: [u8; 16])]
pub struct CreateMemeRegistry<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 16 + 8 + 8 + 8 + 32 + 8 + 32 + 8 + 8, // discriminator + meme_id + total_funds + start_time + end_time + authority +  contributor_count + mint + unclaimed_rewards + claimed_count
        seeds = [b"registry", meme_id.as_ref()],
        bump
    )]
    pub registry: Account<'info, MemeRegistry>,
    /// CHECK: This account is only used as a PDA for receiving SOL
    #[account(
        seeds = [b"vault", meme_id.as_ref()],
        bump,
    )]
    pub vault: UncheckedAccount<'info>,
    #[account(
        seeds = [b"state"],
        bump,
        has_one = authority
    )]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}


// Events
#[event]
pub struct MemeRegistryCreated {
    pub meme_id: [u8; 16],
    pub start_time: i64,
    pub end_time: i64,
}

#[error_code]
pub enum MemeError {
    #[msg("Invalid fund duration")]
    InvalidFundDuration,
    #[msg("Invalid buy amount: minimum exceeds maximum")]
    InvalidBuyAmount,
}