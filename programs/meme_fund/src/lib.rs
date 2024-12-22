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


    // Contribute to a meme vault
    pub fn contribute(ctx: Context<Contribute>, meme_id: [u8; 16], amount: u64) -> Result<()> {
        let state = &ctx.accounts.state;  
        let registry = &mut ctx.accounts.registry;
        let contribution = &mut ctx.accounts.contribution;
        let clock = Clock::get().unwrap();

        // Ensure the amount is greater than or equal to the minimum allowed
        require!(amount >= state.min_buy_amount, MemeError::BelowMinAmount);

        // Ensure the amount does not exceed the maximum allowed
        require!(amount <= state.max_buy_amount, MemeError::ExceedsMaxAmount);

        // Ensure the meme id is valid
        require!(registry.meme_id == meme_id, MemeError::InvalidMemeId);

        let current_time = clock.unix_timestamp;

        // Ensure the meme registry has not expired 
        require!(current_time < registry.end_time, MemeError::FundExpired);

        // Check if the contributor has enough balance
        require!(ctx.accounts.contributor.lamports() >= amount, MemeError::InsufficientBalance);

        // Calculate the commission amount and contribution amount
        let commission_amount = amount
            .checked_mul(state.commission_rate as u64)
            .ok_or(MemeError::ArithmeticOverflow)?
            .checked_div(100)
            .ok_or(MemeError::ArithmeticOverflow)?;

        let net_contribution_amount = amount
            .checked_sub(commission_amount)
            .ok_or(MemeError::ArithmeticOverflow)?;

        contribution.meme_id = meme_id.clone();
        contribution.contributor = ctx.accounts.contributor.key();
        contribution.amount = net_contribution_amount;
        contribution.timestamp = clock.unix_timestamp;

        registry.total_funds = registry.total_funds
            .checked_add(net_contribution_amount)
            .ok_or(MemeError::ArithmeticOverflow)?;

        // Check if adding this contribution would exceed the max fund limit
        require!(
            registry.total_funds + amount <= state.max_fund_limit,
            MemeError::ExceedsMaxFundLimit
        );
            
        registry.contributor_count = registry.contributor_count
            .checked_add(1)
            .ok_or(MemeError::MaxContributorsReached)?;

        // Transfer commission
        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.contributor.key(),
                &ctx.accounts.fee_recipient.key(),
                commission_amount,
            ),
            &[
                ctx.accounts.contributor.to_account_info(),
                ctx.accounts.fee_recipient.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        // Transfer net contribution
        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.contributor.key(),
                &ctx.accounts.vault.key(),
                net_contribution_amount,
            ),
            &[
                ctx.accounts.contributor.to_account_info(),
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;        

        // Emit event
        emit!(ContributionMade {
            meme_id: meme_id.clone(),
            contributor: ctx.accounts.contributor.key(),
            amount,
            commission_amount,
            net_contribution_amount,
            timestamp: current_time,
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

#[account]
pub struct Contribution {
    pub meme_id: [u8; 16],
    pub contributor: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
    pub is_claimed: bool,
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

#[derive(Accounts)]
#[instruction(meme_id: [u8; 16])]
pub struct Contribute<'info> {
    /// CHECK: This account is only used as a PDA for receiving SOL
    #[account(
        mut,
        seeds = [b"vault", meme_id.as_ref()],
        bump,
    )]
    pub vault: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [b"registry", meme_id.as_ref()],
        bump,
    )]
    pub registry: Account<'info, MemeRegistry>,
    #[account(
        init,
        payer = contributor,
        space = 8 + 16 + 32 + 8 + 8 + 1, // discriminator + meme_id + contributor + amount + timestamp + is_claimed
        seeds = [b"contribution", meme_id.as_ref(), contributor.key().as_ref()],
        bump
    )]
    pub contribution: Account<'info, Contribution>,
    #[account(mut)]
    pub contributor: Signer<'info>,
    #[account(
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, State>,
    /// CHECK: This is safe because we're checking the address against the one stored in the state account
    #[account(
        mut,
        address = state.fee_recipient @ MemeError::InvalidFeeRecipient
    )]
    pub fee_recipient: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}


// Events
#[event]
pub struct MemeRegistryCreated {
    pub meme_id: [u8; 16],
    pub start_time: i64,
    pub end_time: i64,
}

#[event]
pub struct ContributionMade {
    pub meme_id: [u8; 16],
    pub contributor: Pubkey,
    pub amount: u64,
    pub commission_amount: u64,
    pub net_contribution_amount: u64,
    pub timestamp: i64,
}

#[error_code]
pub enum MemeError {
    #[msg("Invalid fund duration")]
    InvalidFundDuration,
    #[msg("Invalid buy amount: minimum exceeds maximum")]
    InvalidBuyAmount,
    #[msg("Invalid fee recipient address")]
    InvalidFeeRecipient,
    #[msg("Contribution is below the minimum allowed amount")]
    BelowMinAmount,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("Contribution exceeds maximum allowed amount")]
    ExceedsMaxAmount,
    #[msg("Invalid Meme ID")]
    InvalidMemeId,
    #[msg("Fund has expired")]
    FundExpired,
    #[msg("Insufficient balance")]
    InsufficientBalance,
    #[msg("Exceeds maximum fund limit")]
    ExceedsMaxFundLimit,
    #[msg("Maximum number of contributors reached")]
    MaxContributorsReached,
}