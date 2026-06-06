//use std::mem::transmute;

use pinocchio::{
    AccountView, Address, error::ProgramError
};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fundraiser {
    maker: [u8; 32],
    mint_to_raise: [u8; 32],
    amount_to_raise: [u8; 8],
    current_amount: [u8; 8],
    time_started: [u8; 8],
    pub duration: u8,
    pub bump: u8,
}

impl Fundraiser {
    pub const LEN: usize = 32 + 32 + 8 + 8 + 8 + 1 + 1;

    pub fn from_account_info(account_info: &mut AccountView) -> Result<&mut Self, ProgramError> {
        let data = unsafe { account_info.borrow_unchecked_mut() };
        if data.len() != Fundraiser::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self) })
    }

    pub fn maker(&self) -> &Address {
        // unsafe { &*(&self.maker as *const [u8; 32] as *const Address) }
        unsafe { &*(self.maker.as_ptr() as *const Address) }
    }

    pub fn set_maker(&mut self, maker: &Address) {
        self.maker.copy_from_slice(maker.as_ref());
    }

    pub fn _mint_to_raise(&self) -> &Address {
        // unsafe { &*(&self.mint_to_raise as *const [u8; 32] as *const Address ) }
        unsafe { &*(self.mint_to_raise.as_ptr() as *const Address) }
    }

    pub fn set_mint_to_raise(&mut self, mint: &Address) {
        self.mint_to_raise.copy_from_slice(mint.as_ref());
    }

    pub fn amount_to_raise(&self) -> u64 {
        u64::from_le_bytes(self.amount_to_raise)
        // unsafe { *( self.amount_to_raise.as_ptr() as *const u64 ) }
    }

    pub fn set_amount_to_raise(&mut self, amount: u64) {
        self.amount_to_raise = amount.to_le_bytes()
    }

    pub fn current_amount(&self) -> u64 {
        u64::from_le_bytes(self.current_amount)
        // unsafe { *( self.current_amount.as_ptr() as *const u64 ) }
    }

    pub fn set_current_amount(&mut self, amount: u64) {
        self.current_amount = amount.to_le_bytes()
    }

    pub fn time_started(&self) -> i64 {
        i64::from_le_bytes(self.time_started)
        //unsafe { *( self.time_started.as_ptr() as *const i64 ) }
    }

    pub fn set_time_started(&mut self, time: i64) {
        //self.time_started = unsafe { transmute(time.to_le_bytes()) }
        self.time_started = time.to_le_bytes()
    }

}