use anchor_lang::prelude::*;

declare_id!("FQRP7BsLL83pktuo4yYHABntASh9xa4wo9nCpDpwydzy");

#[program]
pub mod meme_fund {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
