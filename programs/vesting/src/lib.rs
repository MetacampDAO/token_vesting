use anchor_lang::prelude::*;
use anchor_spl::token::{TokenAccount, Transfer, Token, Mint};
use anchor_spl::token;

declare_id!("cBvCy7Qi492GybgwLbARVPPTk3cBCKbatMZaZwZR8is");

#[program]
pub mod vesting {

    use super::*;

    pub fn create(ctx: Context<Create>, release_interval: Vec<u64>, amount_interval: Vec<u64>, _seedphase: String) -> Result<()> {
        // Create
        // Populate vesting_account
        let vesting_account = &mut ctx.accounts.vesting_account;
        vesting_account.src_token_account = ctx.accounts.src_token_account.key();
        vesting_account.src_token_account_owner = ctx.accounts.initializer.key();
        vesting_account.destination_token_account = ctx.accounts.dst_token_account.key();
        vesting_account.destination_token_account_owner = ctx.accounts.dst_token_account_owner.key();
        vesting_account.mint_key = ctx.accounts.mint_address.key();
        
        // Check if release interval and amount interval is the same length
        require!(release_interval.len() == amount_interval.len(), ErrorCode::InvalidIntervalInput);
        
        let mut schedules: Vec<VestingSchedule> = vec![];
        for i in 0..release_interval.len() {
            let schedule = VestingSchedule {
                release_time: release_interval[i],
                amount: amount_interval[i]
            };
            schedules.push(schedule);
        }
        vesting_account.schedules = schedules;

        // Transfer amount to escrow
        let total_amount: u64 = amount_interval.iter().sum();
        token::transfer(
            ctx.accounts.transfer_into_escrow(),
            total_amount
        )?;

        Ok(())
    }

    // Unlock
    pub fn unlock(ctx: Context<Unlock>, seedphase: String) -> Result<()> {
        let mut total_amount_to_transfer: u64 = 0;

        for s in ctx.accounts.vesting_account.schedules.iter_mut() {
            if ctx.accounts.clock.unix_timestamp as u64 > s.release_time {
                total_amount_to_transfer += s.amount;
                s.amount = 0;
            }
        }

        require!(total_amount_to_transfer > 0, ErrorCode::ZeroUnlockAmount);

        let (_key, bump) = Pubkey::find_program_address(&[
            seedphase.as_bytes()
            ], ctx.program_id);

        let signer_seed = [
            seedphase.as_bytes(),
            &[bump]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vesting_token_account.to_account_info().clone(),
            to: ctx.accounts.dst_token_account.to_account_info().clone(),
            authority: ctx.accounts.vesting_account.to_account_info().clone(),
        };
        
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info().clone(), 
                cpi_accounts,
                &[&signer_seed[..]]
            ), 
            total_amount_to_transfer
        )?;
        
        Ok(())
    }

    // Change Distination
    pub fn change_destination(ctx: Context<ChangeDestination>, seedphase: String) -> Result<()> {
        let vesting_account = &mut ctx.accounts.vesting_account;
        vesting_account.destination_token_account = ctx.accounts.new_destination_token_account.key();
        vesting_account.destination_token_account_owner = ctx.accounts.new_destination_token_account_owner.key();
    
        Ok(())
    }

    // Close
    pub fn close_account(ctx: Context<CloseAccount>, seedphase: String) -> Result<()> {
        let mut amount_pass_unlock: u64 = 0;
        let mut total_amount_to_transfer: u64 = 0;

        for s in ctx.accounts.vesting_account.schedules.iter_mut() {
            if ctx.accounts.clock.unix_timestamp as u64 > s.release_time {
                amount_pass_unlock += s.amount;
                s.amount = 0;
            } else {
                total_amount_to_transfer += s.amount
            }
        }

        require!(amount_pass_unlock == 0, ErrorCode::UnlockAmountFirst);
        
        // Transfer remaining amount to src
        let (_key, bump) = Pubkey::find_program_address(&[
            seedphase.as_bytes()
            ], ctx.program_id);

        let signer_seed = [
            seedphase.as_bytes(),
            &[bump]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vesting_token_account.to_account_info().clone(),
            to: ctx.accounts.src_token_account.to_account_info().clone(),
            authority: ctx.accounts.vesting_account.to_account_info().clone(),
        };
        
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info().clone(), 
                cpi_accounts,
                &[&signer_seed[..]]
            ), 
            total_amount_to_transfer
        )?;
        
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(release_interval: Vec<u64>, amount_interval: Vec<u64>, seedphase: String)]
pub struct Create<'info> {
    #[account(mut)]
    pub initializer: Signer<'info>,
    #[account(
        init,
        space = VestingScheduleHeader::LEN() 
        + VestingSchedule::LEN() * release_interval.len(),
        seeds = [seedphase.as_ref()],
        bump,
        payer = initializer,
    )]
    pub vesting_account: Account<'info, VestingScheduleHeader>,
    #[account(mut, token::authority = initializer.key())]
    pub src_token_account: Box<Account<'info, TokenAccount>>,
    /// CHECK: Just make sure authority of dst_token_account is this account
    pub dst_token_account_owner: UncheckedAccount<'info>,
    #[account(
        mut,
        token::mint = mint_address,
        token::authority = dst_token_account_owner.key(),
    )]
    pub dst_token_account: Box<Account<'info, TokenAccount>>,
    #[account(
        init,
        seeds = [mint_address.key().as_ref(), vesting_account.key().as_ref()],
        bump,
        payer = initializer,
        token::mint = mint_address,
        token::authority = vesting_account,
    )]
    pub vesting_token_account: Account<'info, TokenAccount>,
    pub mint_address: Box<Account<'info, Mint>>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>
}

impl<'info> Create<'info> {
    fn transfer_into_escrow(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.src_token_account.to_account_info().clone(),
            to: self.vesting_token_account.to_account_info().clone(),
            authority: self.initializer.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }
}

#[derive(Accounts)]
#[instruction(seedphase: String)]
pub struct Unlock<'info> {
    #[account(
        mut, seeds = [seedphase.as_ref()], bump, 
        constraint = vesting_account.destination_token_account == dst_token_account.key(),
    )]
    pub vesting_account: Account<'info, VestingScheduleHeader>,
    #[account(
        mut,
        seeds = [mint_address.key().as_ref(), vesting_account.key().as_ref()],
        bump,
        token::mint = mint_address,
        token::authority = vesting_account,
    )]
    pub vesting_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub dst_token_account: Box<Account<'info, TokenAccount>>,
    #[account(constraint = mint_address.key() == vesting_account.mint_key)]
    pub mint_address: Box<Account<'info, Mint>>,
    pub clock: Sysvar<'info, Clock>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(seedphase: String)]
pub struct ChangeDestination<'info> {
    #[account(
        mut, seeds = [seedphase.as_ref()], bump, 
        constraint = vesting_account.destination_token_account == current_destination_token_account.key(),
        constraint = vesting_account.destination_token_account_owner == current_destination_token_account_owner.key()
    )]
    pub vesting_account: Account<'info, VestingScheduleHeader>,
    pub current_destination_token_account_owner: Signer<'info>,
    #[account(token::authority = current_destination_token_account_owner.key())]
    pub current_destination_token_account: Box<Account<'info, TokenAccount>>,
    /// CHECK: Just make sure authority of new_destination_token_account is this account
    pub new_destination_token_account_owner: UncheckedAccount<'info>,
    #[account(token::authority = new_destination_token_account_owner.key())]
    pub new_destination_token_account: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(seedphase: String)]
pub struct CloseAccount<'info> {
    #[account(
        mut, seeds = [seedphase.as_ref()], bump, 
        constraint = vesting_account.src_token_account == src_token_account.key(),
        constraint = vesting_account.src_token_account_owner == initializer.key(),
        close = initializer
    )]
    pub vesting_account: Account<'info, VestingScheduleHeader>,
    #[account(mut)]
    pub initializer: Signer<'info>,
    #[account(
        mut,
        seeds = [mint_address.key().as_ref(), vesting_account.key().as_ref()],
        bump,
        token::mint = mint_address,
        token::authority = vesting_account,
    )]
    pub vesting_token_account: Account<'info, TokenAccount>,
    #[account(mut, token::authority = initializer.key())]
    pub src_token_account: Box<Account<'info, TokenAccount>>,
    #[account(constraint = mint_address.key() == vesting_account.mint_key)]
    pub mint_address: Box<Account<'info, Mint>>,
    pub clock: Sysvar<'info, Clock>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct VestingSchedule {
    pub release_time: u64,
    pub amount: u64,
}
#[account]
pub struct VestingScheduleHeader {
    pub src_token_account: Pubkey,
    pub src_token_account_owner: Pubkey,
    pub destination_token_account: Pubkey,
    pub destination_token_account_owner: Pubkey,
    pub mint_key: Pubkey,
    pub schedules: Vec<VestingSchedule>,
}

const DISCRIMINATOR: usize = 8;
const PUBKEY: usize = 32;
const U64: usize = 32;

impl VestingScheduleHeader {
    fn LEN() -> usize {
        DISCRIMINATOR + PUBKEY + PUBKEY + PUBKEY + PUBKEY + PUBKEY
    }
}

impl VestingSchedule {
    fn LEN() -> usize {
        U64 + U64
    }
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid releaseInterval and amountInterval. Must be the same length.")]
    InvalidIntervalInput,
    #[msg("No outstanding unlockable balance.")]
    ZeroUnlockAmount,
    #[msg("There are outstanding unlockable balance. Please unlock balance first")]
    UnlockAmountFirst,
}