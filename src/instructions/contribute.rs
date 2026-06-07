use pinocchio::{
    AccountView, ProgramResult, cpi::{Seed, Signer}, error::ProgramError, sysvars::{Sysvar, clock::Clock, rent::Rent}
};
use pinocchio_pubkey::derive_address;
use pinocchio_system::instructions::CreateAccount;

use crate::{state::{Fundraiser, Contributor}, SECONDS_TO_DAYS, MAX_CONTRIBUTION_PERCENTAGE, PERCENTAGE_SCALER};

pub fn process_contribute_instruction(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [
        contributor,
        mint_to_raise,
        fundraiser,
        contributor_account,
        contributor_ata,
        vault,
        _system_program,
        _token_program,
        _associated_token_program
    ] = accounts 
    else {
        return Err(ProgramError::NotEnoughAccountKeys)
    };

    {
        let contributor_ata_state = pinocchio_token::state::Account::from_account_view(contributor_ata)?;
        if contributor_ata_state.owner() != contributor.address() {
            return Err(ProgramError::IllegalOwner);
        }
        if contributor_ata_state.mint() != mint_to_raise.address() {
            return Err(ProgramError::InvalidAccountData);
        }
    }


    let amount = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let bump = data[8];

    let seed = [b"contributor".as_ref(), contributor.address().as_ref(), &[bump]];

    let contributor_account_pda = derive_address(&seed, None, &crate::ID.to_bytes());
    assert_eq!(contributor_account_pda, *contributor_account.address().as_array());

    let fundraiser_data = Fundraiser::from_account_info(fundraiser)?;
    
    let mint_data = mint_to_raise.try_borrow()?;
    if mint_data.len() < 45 {
        return Err(ProgramError::InvalidAccountData);
    }

    let decimals = mint_data[44];

    if amount <= 1u8.pow(decimals as u32) as u64 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    
    if amount > (fundraiser_data.amount_to_raise() * MAX_CONTRIBUTION_PERCENTAGE) / PERCENTAGE_SCALER {
        return Err(ProgramError::ArithmeticOverflow);
    }
    

    let current_time = Clock::get()?.unix_timestamp;
    if fundraiser_data.duration >= ((current_time - fundraiser_data.time_started())/SECONDS_TO_DAYS) as u8 {
        return Err(ProgramError::MaxInstructionTraceLengthExceeded);
    }


    if !contributor_account.owned_by(&crate::ID) {

        let bump_bytes = [bump];
        let signer_seeds = [
            Seed::from(b"contributor"),
            Seed::from(contributor.address().as_array()),
            Seed::from(bump_bytes.as_ref()),
        ];
        let signer = Signer::from(&signer_seeds);

        CreateAccount {
            from: contributor,
            to: contributor_account,
            lamports: Rent::get()?.try_minimum_balance(Contributor::LEN)?,
            space: Contributor::LEN as u64,
            owner: &crate::ID
        }
        .invoke_signed(&[signer])?;
    }


    let contributor_data = Contributor::from_account_info(contributor_account)?;

    
    if (contributor_data.amount() > (fundraiser_data.amount_to_raise() * MAX_CONTRIBUTION_PERCENTAGE) / PERCENTAGE_SCALER)
        && (contributor_data.amount() + amount > (fundraiser_data.amount_to_raise() * MAX_CONTRIBUTION_PERCENTAGE) / PERCENTAGE_SCALER) {
        return Err(ProgramError::InvalidArgument);
    }


    let fund_amount = contributor_data.amount();
    contributor_data.set_amount(fund_amount + amount);

    pinocchio_token::instructions::Transfer::new(contributor_ata, vault, contributor, amount)
        .invoke()?;

    let fund_amount = fundraiser_data.current_amount();
    fundraiser_data.set_current_amount(fund_amount + amount);

    Ok(())
}