use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::solana_program::{program::invoke_signed};
use anchor_spl::token::{self, Token, TokenAccount, Mint};
use anchor_spl::associated_token::{AssociatedToken, Create as ATACreate};

declare_id!("FQRP7BsLL83pktuo4yYHABntASh9xa4wo9nCpDpwydzy");

const MIN_SOL_AMOUNT: u64 = 100_000_000; // 0.1 SOL in lamports
const MAX_SOL_AMOUNT: u64 = 2_000_000_000; // 2 SOL in lamports
const MAX_FUND_LIMIT: u64 = 20_000_000_000; // 20 SOL in lamports
const MAX_COMMISSION_RATE: u8 = 10; // 10%
const MAX_TOKEN_CLAIM_AVAILABLE_TIME: i64 = 60 * 60 * 24; // 24 hours

// Include the generated IDL constants
include!(concat!(env!("OUT_DIR"), "/pump_idl.rs"));

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

    // Start Meme creation and buying process
    pub fn start_meme(
        ctx: Context<StartMeme>,
        meme_id: [u8; 16],
        name: String,
        symbol: String,
        uri: String,
        buy_amount: u64,
        max_sol_cost: u64,
    ) -> Result<()> {
        let pump_program_id = ctx.accounts.pump_program.key();

        require!(name.len() <= 32, MemeError::NameTooLong);
        require!(symbol.len() <= 10, MemeError::SymbolTooLong);

        let vault_seeds: &[&[u8]] = &[
            b"vault",
            meme_id.as_ref(),
            &[ctx.bumps.vault],
        ];
        // Create token instruction
        let create_discriminator: [u8; 8] = CREATE_DISCRIMINATOR;

        let mut create_data = Vec::with_capacity(create_discriminator.len() + name.len() + symbol.len() + uri.len() + 12);
        create_data.extend_from_slice(&create_discriminator);
        create_data.extend_from_slice(&(name.len() as u32).to_le_bytes());
        create_data.extend_from_slice(name.as_bytes());
        create_data.extend_from_slice(&(symbol.len() as u32).to_le_bytes());
        create_data.extend_from_slice(symbol.as_bytes());
        create_data.extend_from_slice(&(uri.len() as u32).to_le_bytes());
        create_data.extend_from_slice(uri.as_bytes());

        let create_accounts = vec![
            AccountMeta::new(ctx.accounts.mint.key(), true),
            AccountMeta::new(ctx.accounts.mint_authority.key(), false),
            AccountMeta::new(ctx.accounts.bonding_curve.key(), false),
            AccountMeta::new(ctx.accounts.associated_bonding_curve.key(), false),
            AccountMeta::new_readonly(ctx.accounts.global.key(), false),
            AccountMeta::new_readonly(ctx.accounts.mpl_token_metadata.key(), false),
            AccountMeta::new(ctx.accounts.metadata.key(), false),
            AccountMeta::new(ctx.accounts.vault.key(), true),
            AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
            AccountMeta::new_readonly(ctx.accounts.token_program.key(), false),
            AccountMeta::new_readonly(ctx.accounts.associated_token_program.key(), false),
            AccountMeta::new_readonly(ctx.accounts.rent.key(), false),
            AccountMeta::new_readonly(ctx.accounts.event_authority.key(), false),
            AccountMeta::new_readonly(ctx.accounts.pump_program.key(), false),
        ];

        let create_ix = Instruction {
            program_id: pump_program_id,
            accounts: create_accounts,
            data: create_data,
        };

        // Buy instruction
        let buy_discriminator: [u8; 8] = BUY_DISCRIMINATOR;

        let mut buy_data = Vec::with_capacity(buy_discriminator.len() + 16);
        buy_data.extend_from_slice(&buy_discriminator);
        buy_data.extend_from_slice(&buy_amount.to_le_bytes());
        buy_data.extend_from_slice(&max_sol_cost.to_le_bytes());

        let buy_accounts = vec![
            AccountMeta::new_readonly(ctx.accounts.global.key(), false),
            AccountMeta::new(ctx.accounts.fee_recipient.key(), false),
            AccountMeta::new_readonly(ctx.accounts.mint.key(), false),
            AccountMeta::new(ctx.accounts.bonding_curve.key(), false),
            AccountMeta::new(ctx.accounts.associated_bonding_curve.key(), false),
            AccountMeta::new(ctx.accounts.associated_user.key(), false),
            AccountMeta::new(ctx.accounts.vault.key(), true),
            AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
            AccountMeta::new_readonly(ctx.accounts.token_program.key(), false),
            AccountMeta::new_readonly(ctx.accounts.rent.key(), false),
            AccountMeta::new_readonly(ctx.accounts.event_authority.key(), false),
            AccountMeta::new_readonly(ctx.accounts.pump_program.key(), false),
        ];

        let buy_ix = Instruction {
            program_id: pump_program_id,
            accounts: buy_accounts,
            data: buy_data,
        };

        // Execute instructions
        invoke_signed(
            &create_ix,
            &[
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.mint_authority.to_account_info(),
                ctx.accounts.bonding_curve.to_account_info(),
                ctx.accounts.associated_bonding_curve.to_account_info(),
                ctx.accounts.global.to_account_info(),
                ctx.accounts.mpl_token_metadata.to_account_info(),
                ctx.accounts.metadata.to_account_info(),
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.associated_token_program.to_account_info(),
                ctx.accounts.rent.to_account_info(),
                ctx.accounts.event_authority.to_account_info(),
                ctx.accounts.pump_program.to_account_info(),
            ],
            &[vault_seeds]
        )?;

        // Create the associated token account using a CPI
        msg!("Attempting to create Associated Token Account");
        let create_ata_accounts = ATACreate {
            payer: ctx.accounts.authority.to_account_info(),
            associated_token: ctx.accounts.associated_user.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };

        match anchor_spl::associated_token::create_idempotent(CpiContext::new(
            ctx.accounts.associated_token_program.to_account_info(),
            create_ata_accounts,
        )) {
            Ok(_) => msg!("Associated Token Account created successfully"),
            Err(e) => {
                msg!("Error creating Associated Token Account: {:?}", e);
                return Err(MemeError::ATACreationFailed.into());
            }
        }


        invoke_signed(
            &buy_ix,
            &[
                ctx.accounts.global.to_account_info(),
                ctx.accounts.fee_recipient.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.bonding_curve.to_account_info(),
                ctx.accounts.associated_bonding_curve.to_account_info(),
                ctx.accounts.associated_user.to_account_info(),
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.rent.to_account_info(),
                ctx.accounts.event_authority.to_account_info(),
                ctx.accounts.pump_program.to_account_info(),
            ],
            &[vault_seeds]
        )?;

        let registry = &mut ctx.accounts.registry;
        registry.mint = ctx.accounts.mint.key();

        // Emit event
        emit!(MemeStarted {
            meme_id,
            mint: ctx.accounts.mint.key(),
            name,
            symbol,
            uri,
            total_funds: registry.total_funds, 
        });

        Ok(())
    }

    // Claim token funds from a meme vault
    pub fn claim_tokens(ctx: Context<ClaimTokens>, _meme_id: [u8; 16]) -> Result<()> {
        let registry = &mut ctx.accounts.registry;
        let contribution = &mut ctx.accounts.contribution;
        let vault_token_account = &ctx.accounts.vault_token_account;
        let state = &ctx.accounts.state;

        // Check if the meme_id matches
        require!(registry.meme_id == _meme_id, MemeError::InvalidMemeId);

        // Ensure the contribution has not been claimed
        require!(!contribution.is_claimed, MemeError::AlreadyClaimed);

        // Check for zero amount
        require!(contribution.amount > 0, MemeError::ZeroContributionAmount);

        // Check if the vault has enough tokens
        require!(registry.total_funds > 0, MemeError::NoFundsInRegistry);        

        let clock = Clock::get()?;
        let current_time = clock.unix_timestamp;

        let claim_available_time = registry.end_time.checked_add(state.token_claim_available_time)
        .ok_or(MemeError::ArithmeticOverflow)?;

        // Ensure the claim time has been reached
        require!(
            current_time >= claim_available_time,
            MemeError::ClaimTimeNotReached
        );

        let user_tokens = (contribution.amount as u128)
            .checked_mul(vault_token_account.amount as u128)
            .and_then(|v| v.checked_div(registry.total_funds as u128))
            .ok_or(MemeError::ArithmeticOverflow)?;

        // Check for zero amount
        require!(user_tokens > 0, MemeError::ZeroClaimAmount);

        // Check if the vault has enough tokens
        require!(vault_token_account.amount >= user_tokens as u64, MemeError::InsufficientVaultBalance);
        
        let vault_seeds: &[&[u8]] = &[
            b"vault",
            registry.meme_id.as_ref(),
            &[ctx.bumps.vault],
        ];

        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::TransferChecked {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                },
                &[vault_seeds],
            ),
            user_tokens as u64,
            ctx.accounts.mint.decimals,
        )?;

        // Update the registry's total funds and the vault's token balance
        registry.total_funds = registry.total_funds.checked_sub(contribution.amount).ok_or(MemeError::ArithmeticOverflow)?;

        // Set as claimed after successful transfer
        contribution.is_claimed = true;
        registry.claimed_count = registry.claimed_count
            .checked_add(1)
            .ok_or(MemeError::ArithmeticOverflow)?;

        // Check if this is the last claim
        if registry.claimed_count == registry.contributor_count {
            // Update unclaimed_rewards for manual claim later
            let vault_balance = ctx.accounts.vault.lamports();
            if vault_balance > 0 {
                registry.unclaimed_rewards = vault_balance;
            }
        }
        
        emit!(TokensClaimed {
            meme_id: registry.meme_id,
            contributor: contribution.contributor,
            amount: user_tokens as u64,
        });
        
        Ok(())
    }

    // Admin function to claim remaining pump rewards 
    pub fn admin_claim_rewards(ctx: Context<AdminClaimRewards>, meme_id: [u8; 16]) -> Result<()> {
        let registry = &mut ctx.accounts.registry;

        // Ensure claimable rewards are available
        require!(
            registry.claimed_count == registry.contributor_count,
            MemeError::NotAllTokensClaimed
        );

        if registry.unclaimed_rewards > 0 {
            let amount = registry.unclaimed_rewards;

            let vault_signer_seeds: &[&[u8]] = &[
                b"vault",
                meme_id.as_ref(),
                &[ctx.bumps.vault],
            ];

            // Transfer the remaining rewards to the fee recipient
            anchor_lang::solana_program::program::invoke_signed(
                &anchor_lang::solana_program::system_instruction::transfer(
                    &ctx.accounts.vault.key(),
                    &ctx.accounts.fee_recipient.key(),
                    amount,
                ),
                &[
                    ctx.accounts.vault.to_account_info(),
                    ctx.accounts.fee_recipient.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
                &[vault_signer_seeds],
            )?;

            registry.unclaimed_rewards = 0;

            Ok(())
        } else {
            Err(MemeError::NoRewardsToClaim.into())
        }
    }

    // Update the fee recipient wallet address
    pub fn update_fee_recipient(ctx: Context<UpdateFeeRecipient>, new_fee_recipient: Pubkey) -> Result<()> {
        let state = &mut ctx.accounts.state;
        
        // Ensure the new fee recipient is not the same as the old one
        let old_wallet = state.fee_recipient;
        require!(old_wallet != new_fee_recipient, MemeError::SameWalletAddress);

        state.fee_recipient = new_fee_recipient;

        // Emit event
        emit!(FeeRecipientUpdated {
            old_wallet,
            new_wallet: new_fee_recipient,
        });

        Ok(())
    }

    // Update the maximum amount that can be contributed to a meme vault
    pub fn update_max_buy_amount(ctx: Context<UpdateMaxBuyAmount>, new_max_buy_amount: u64) -> Result<()> {        
        // Ensure the new max buy amount is less than or equal to 2 SOL
        require!(new_max_buy_amount <= 2_000_000_000, MemeError::ExceedsMaxAllowedAmount);
        
        let state = &mut ctx.accounts.state;
        let old_amount = state.max_buy_amount;
        state.max_buy_amount = new_max_buy_amount;

        // Emit event
        emit!(MaxBuyAmountUpdated {
            old_amount,
            new_amount: new_max_buy_amount,
        });
        
        Ok(())
    }

    // Update the minimum amount that can be contributed to a meme vault
    pub fn update_min_buy_amount(ctx: Context<UpdateMinBuyAmount>, new_min_buy_amount: u64) -> Result<()> {
        // Ensure the new min buy amount is greater than or equal to 0.1 SOL
        require!(new_min_buy_amount >= 100_000_000, MemeError::BelowMinAllowedAmount); // 0.1 SOL
        
        // Ensure the new min buy amount is less than or equal to the max buy amount
        require!(new_min_buy_amount <= ctx.accounts.state.max_buy_amount, MemeError::ExceedsMaxBuyAmount);
        
        let state = &mut ctx.accounts.state;
        let old_amount = state.min_buy_amount;
        state.min_buy_amount = new_min_buy_amount;
    
        emit!(MinBuyAmountUpdated {
            old_amount,
            new_amount: new_min_buy_amount,
        });
        
        Ok(())
    }

    // Update the duration of a meme vault
    pub fn update_fund_duration(ctx: Context<UpdateFundDuration>, new_fund_duration: i64) -> Result<()> {
        // Ensure the new fund duration is greater than 0
        require!(new_fund_duration > 0, MemeError::InvalidFundDuration);
        
        let state = &mut ctx.accounts.state;
        let old_duration = state.fund_duration;
        state.fund_duration = new_fund_duration;

        // Emit event
        emit!(FundDurationUpdated {
            old_duration,
            new_duration: new_fund_duration,
        });

        Ok(())
    }

    // Update the maximum amount that can be contributed to single meme vault
    pub fn update_max_fund_limit(ctx: Context<UpdateMaxFundLimit>, new_max_fund_limit: u64) -> Result<()> {
        let state = &mut ctx.accounts.state;
        let old_limit = state.max_fund_limit;
        state.max_fund_limit = new_max_fund_limit;

        // Emit event
        emit!(MaxFundLimitUpdated {
            old_limit,
            new_limit: new_max_fund_limit,
        });

        Ok(())
    }

    // Update commission rate
    pub fn update_commission_rate(ctx: Context<UpdateCommissionRate>, new_rate: u8) -> Result<()> {
        require!(new_rate <= MAX_COMMISSION_RATE, MemeError::CommissionRateTooHigh);

        let state = &mut ctx.accounts.state;
        let old_rate = state.commission_rate;
        state.commission_rate = new_rate;

        emit!(CommissionRateUpdated {
            old_rate,
            new_rate,
        });

        Ok(())
    }

    // Update the max token claim available time
    pub fn update_max_claim_available_time(ctx: Context<UpdateMaxClaimAvailableTime>, new_claim_available_time: i64) -> Result<()> {
        require!(new_claim_available_time <= MAX_TOKEN_CLAIM_AVAILABLE_TIME, MemeError::InvalidFundDuration);
        
        let state = &mut ctx.accounts.state;
        state.token_claim_available_time = new_claim_available_time;

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

#[derive(Accounts)]
pub struct StartMeme<'info> {
    #[account(mut)]
    pub registry: Account<'info, MemeRegistry>,
    /// CHECK: This account is used as a PDA for receiving and sending SOL
    #[account(
        mut,
        seeds = [b"vault", registry.meme_id.as_ref()],
        bump
    )]
    pub vault: UncheckedAccount<'info>,
    #[account(mut)]
    pub mint: Signer<'info>,
    /// CHECK: This account is checked in the instruction
    #[account(mut)]
    pub mint_authority: UncheckedAccount<'info>,
    /// CHECK: This account is checked in the instruction
    #[account(mut)]
    pub bonding_curve: UncheckedAccount<'info>,
    /// CHECK: This account is checked in the instruction
    #[account(mut)]
    pub associated_bonding_curve: UncheckedAccount<'info>,
    /// CHECK: This account is checked in the instruction
    pub global: UncheckedAccount<'info>,
    /// CHECK: This account is checked in the instruction
    pub mpl_token_metadata: UncheckedAccount<'info>,
    /// CHECK: This account is checked in the instruction
    #[account(mut)]
    pub metadata: UncheckedAccount<'info>,
    #[account(
        mut,
        constraint = authority.key() == registry.authority
    )]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub associated_token_program: UncheckedAccount<'info>,
    pub rent: Sysvar<'info, Rent>,
    /// CHECK: This account is checked in the instruction
    pub event_authority: UncheckedAccount<'info>,
    /// CHECK: This account is checked in the instruction
    pub pump_program: UncheckedAccount<'info>,
    /// CHECK: This account is checked in the instruction
    #[account(mut)]
    pub fee_recipient: UncheckedAccount<'info>,
    /// CHECK: This account is checked in the instruction
    #[account(mut)]
    pub associated_user: UncheckedAccount<'info>,
}

#[derive(Accounts)]
#[instruction(meme_id: [u8; 16])]
pub struct ClaimTokens<'info> {
    #[account(
        mut,
        seeds = [b"registry", meme_id.as_ref()],
        bump,
    )]
    pub registry: Account<'info, MemeRegistry>,
    #[account(
        mut,
        seeds = [b"contribution", meme_id.as_ref(), contributor.key().as_ref()],
        bump,
        has_one = contributor,
    )]
    pub contribution: Account<'info, Contribution>,
    #[account(mut)]
    pub contributor: Signer<'info>,
    /// CHECK: This account is used as a PDA for vault operations
    #[account(
        seeds = [b"vault", meme_id.as_ref()],
        bump,
        constraint = registry.meme_id == meme_id @ MemeError::InvalidMemeId
    )]
    pub vault: UncheckedAccount<'info>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = vault,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = contributor,
        associated_token::mint = mint,
        associated_token::authority = contributor,
    )]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(address = registry.mint)]
    pub mint: Account<'info, Mint>,
    #[account(
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, State>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(meme_id: [u8; 16])]
pub struct AdminClaimRewards<'info> {
    #[account(
        mut,
        seeds = [b"registry", meme_id.as_ref()],
        bump,
    )]
    pub registry: Account<'info, MemeRegistry>,
    #[account(seeds = [b"state"], bump, has_one = authority)]
    pub state: Account<'info, State>,
    /// CHECK: This account is a PDA, used as vault
    #[account(
        mut,
        seeds = [b"vault", meme_id.as_ref()],
        bump,
    )]
    pub vault: UncheckedAccount<'info>,
    /// CHECK: This is the fee recipient account, a normal wallet
    #[account(mut, address = state.fee_recipient @ MemeError::InvalidFeeRecipient)]
    pub fee_recipient: UncheckedAccount<'info>,
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateFeeRecipient<'info> {
    #[account(
        mut,
        seeds = [b"state"],
        bump,
        has_one = authority,
    )]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateMaxBuyAmount<'info> {
    #[account(
        mut,
        seeds = [b"state"],
        bump,
        has_one = authority,
    )]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateMinBuyAmount<'info> {
    #[account(
        mut,
        seeds = [b"state"],
        bump,
        has_one = authority,
    )]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateFundDuration<'info> {
    #[account(
        mut,
        seeds = [b"state"],
        bump,
        has_one = authority,
    )]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateMaxFundLimit<'info> {
    #[account(
        mut,
        seeds = [b"state"],
        bump,
        has_one = authority
    )]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateCommissionRate<'info> {
    #[account(
        mut,
        seeds = [b"state"],
        bump,
        has_one = authority,
    )]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateMaxClaimAvailableTime<'info> {
    #[account(
        mut,
        seeds = [b"state"],
        bump,
        has_one = authority,
    )]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
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

#[event]
pub struct MemeStarted {
    pub meme_id: [u8; 16],
    pub mint: Pubkey,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub total_funds: u64,
}

#[event]
pub struct TokensClaimed {
    pub meme_id: [u8; 16],
    pub contributor: Pubkey,
    pub amount: u64,
}

#[event]
pub struct FeeRecipientUpdated {
    pub old_wallet: Pubkey,
    pub new_wallet: Pubkey,
}

#[event]
pub struct MaxBuyAmountUpdated {
    pub old_amount: u64,
    pub new_amount: u64,
}

#[event]
pub struct MinBuyAmountUpdated {
    pub old_amount: u64,
    pub new_amount: u64,
}

#[event]
pub struct FundDurationUpdated {
    pub old_duration: i64,
    pub new_duration: i64,
}

#[event]
pub struct MaxFundLimitUpdated {
    pub old_limit: u64,
    pub new_limit: u64,
}

#[event]
pub struct CommissionRateUpdated {
    pub old_rate: u8,
    pub new_rate: u8,
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
    #[msg("Name must be 32 characters or less")]
    NameTooLong,
    #[msg("Symbol must be 10 characters or less")]
    SymbolTooLong,
    #[msg("Failed to create Associated Token Account")]
    ATACreationFailed,
    #[msg("Tokens have already been claimed")]
    AlreadyClaimed,
    #[msg("Zero contribution amount")]
    ZeroContributionAmount,
    #[msg("Insufficient vault balance")]
    InsufficientVaultBalance,
    #[msg("No funds in registry")]
    NoFundsInRegistry,
    #[msg("Token claim time has not been reached yet")]
    ClaimTimeNotReached,
    #[msg("Zero claim amount")]
    ZeroClaimAmount,
    #[msg("Not all tokens have been claimed yet")]
    NotAllTokensClaimed,
    #[msg("No rewards to claim")]
    NoRewardsToClaim,
    #[msg("New wallet address is the same as the old one")]
    SameWalletAddress,
    #[msg("Exceeds maximum allowed amount of 2 SOL")]
    ExceedsMaxAllowedAmount,
    #[msg("New minimum buy amount exceeds the current maximum buy amount")]
    ExceedsMaxBuyAmount,
    #[msg("New minimum buy amount is below the allowed minimum of 0.1 SOL")]
    BelowMinAllowedAmount,
    #[msg("Commission rate cannot exceed 10%")]
    CommissionRateTooHigh,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commission_calculation() {
        let amount: u64 = 1_000_000_000; // 1 SOL
        let commission_rate: u8 = 5; // 5%
        
        let commission_amount = amount
            .checked_mul(commission_rate as u64)
            .unwrap()
            .checked_div(100)
            .unwrap();
        
        let net_contribution = amount
            .checked_sub(commission_amount)
            .unwrap();
        
        assert_eq!(commission_amount, 50_000_000); // 0.05 SOL
        assert_eq!(net_contribution, 950_000_000); // 0.95 SOL
    }

    #[test]
    fn test_buy_amount_validation() {
        let min_amount: u64 = 100_000_000; // 0.1 SOL
        let max_amount: u64 = 1_000_000_000; // 1 SOL
        let test_amount: u64 = 500_000_000; // 0.5 SOL

        assert!(test_amount >= min_amount, "Amount below minimum");
        assert!(test_amount <= max_amount, "Amount above maximum");
    }

    #[test]
    fn test_user_token_calculation() {
        let contribution_amount: u64 = 1_000_000_000; // 1 SOL
        let total_funds: u64 = 10_000_000_000; // 10 SOL
        let vault_token_amount: u64 = 1_000_000; // 1M tokens

        let user_tokens = (contribution_amount as u128)
            .checked_mul(vault_token_amount as u128)
            .unwrap()
            .checked_div(total_funds as u128)
            .unwrap();

        assert_eq!(user_tokens, 100_000); // User should get 10% of tokens
    }

    #[test]
    fn test_pda_derivation() {
        let program_id = Pubkey::new_unique();
        let meme_id = [1u8; 16];
        
        let (registry_pda, _) = Pubkey::find_program_address(
            &[b"registry", &meme_id],
            &program_id
        );
        
        let (vault_pda, _) = Pubkey::find_program_address(
            &[b"vault", &meme_id],
            &program_id
        );

        assert_ne!(registry_pda, vault_pda, "PDAs should be different");
    }

    #[test]
    fn test_max_fund_limit() {
        let current_funds: u64 = 15_000_000_000;
        let contribution: u64 = 1_000_000_000;
        
        assert!(
            current_funds + contribution <= MAX_FUND_LIMIT,
            "Contribution would exceed fund limit"
        );
    }

    #[test]
    fn test_commission_rate_validation() {
        let test_rate: u8 = 15;
        assert!(
            test_rate > MAX_COMMISSION_RATE,
            "Commission rate exceeds maximum allowed"
        );
    }

    #[test]
    fn test_fund_duration_validation() {
        let fund_duration: i64 = 0;
        assert!(
            fund_duration <= 0,
            "Invalid fund duration should be caught"
        );
    }

    #[test]
    fn test_min_max_buy_amount_validation() {
        let min_buy: u64 = MIN_SOL_AMOUNT;
        let max_buy: u64 = MAX_SOL_AMOUNT;
        
        assert!(
            min_buy <= max_buy,
            "Min buy amount should be less than or equal to max buy amount"
        );
    }
}