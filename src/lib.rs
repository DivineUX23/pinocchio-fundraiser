#![allow(unexpected_cfgs)]

use pinocchio::{
    AccountView, Address, ProgramResult, address::declare_id, entrypoint, error::ProgramError
};

mod instructions;
mod state;
mod constants;
mod test;

use instructions::*;
use constants::*;

entrypoint!(process_instruction);

declare_id!("96TFrsG998MvvrfuShRQmSemkzN555pnidGF4gquJsKr");

pub fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8]
) -> ProgramResult {
    assert_eq!(program_id, &ID);

    let (discriminator, data) = instruction_data
    .split_first()
    .ok_or(ProgramError::InvalidAccountData)?;

    match FundraiserInstructions::try_from(discriminator)? {
        FundraiserInstructions::Initialize => instructions::process_initialize_instruction(accounts, data)?,
        FundraiserInstructions::Contributor => instructions::process_contribute_instruction(accounts, data)?,
        FundraiserInstructions::Checker => instructions::process_checker_instruction(accounts, data)?,
        FundraiserInstructions::Refund => instructions::process_refund_instruction(accounts, data)?,

    }

    Ok(())
}