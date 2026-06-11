#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use litesvm::LiteSVM;
    use litesvm_token::{
        CreateAssociatedTokenAccount, CreateMint, MintTo,
        spl_token::{self},
    };
    use solana_sdk::clock::Clock;
    use solana_instruction::{AccountMeta, Instruction};
    use solana_keypair::Keypair;
    use solana_message::Message;
    use solana_native_token::LAMPORTS_PER_SOL;
    use solana_pubkey::Pubkey;
    use solana_signer::Signer;
    use solana_transaction::Transaction;

    const TOKEN_PROGRAM_ID: Pubkey = spl_token::ID;
    const SECONDS_TO_DAYS: i64 = 86_400;
    const DECIMALS: u8 = 6;


    fn min_raise() -> u64 {
        crate::MIN_AMOUNT_TO_RAISE
            .checked_mul(10u64.pow(DECIMALS as u32))
            .unwrap()
    }


    fn program_id() -> Pubkey {
        Pubkey::from(crate::ID)
    }

    fn so_path() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for subdir in &["sbpf-solana-solana", "sbf-solana-solana"] {
            let p = manifest_dir
                .join("target")
                .join(subdir)
                .join("release/pinocchio_fundraiser.so");
            if p.exists() {
                return p;
            }
        }
        manifest_dir.join("target/deploy/pinocchio_fundraiser.so")
    }

    fn setup() -> (LiteSVM, Keypair) {
        let mut svm = LiteSVM::new();
        let payer = Keypair::new();
        svm.airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL)
            .expect("Airdrop failed");
        let program_data =
            std::fs::read(so_path()).expect("Failed to read .so — run `cargo build-sbf` first");
        svm.add_program(program_id(), &program_data)
            .expect("Failed to add program");
        (svm, payer)
    }


    fn fundraiser_pda(maker: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"fundraiser", maker.as_ref()], &program_id())
    }

    fn contributor_pda(contributor: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"contributor", contributor.as_ref()], &program_id())
    }


    fn ata_program() -> Pubkey {
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
            .parse()
            .unwrap()
    }

    fn system_program() -> Pubkey {
        solana_sdk_ids::system_program::ID
    }


    fn read_token_balance(svm: &LiteSVM, ata: &Pubkey) -> u64 {
        let acct = svm.get_account(ata).expect("token account not found");
        u64::from_le_bytes(acct.data[64..72].try_into().unwrap())
    }


    fn read_fundraiser_amount_to_raise(svm: &LiteSVM, fundraiser: &Pubkey) -> u64 {
        let acct = svm.get_account(fundraiser).expect("fundraiser not found");
        u64::from_le_bytes(acct.data[64..72].try_into().unwrap())
    }

    fn read_fundraiser_current_amount(svm: &LiteSVM, fundraiser: &Pubkey) -> u64 {
        let acct = svm.get_account(fundraiser).expect("fundraiser not found");
        u64::from_le_bytes(acct.data[72..80].try_into().unwrap())
    }


    fn set_clock(svm: &mut LiteSVM, unix_timestamp: i64) {
        let mut clock = Clock::default();
        clock.unix_timestamp = unix_timestamp;
        svm.set_sysvar(&clock);
    }


    struct FundraiserSetup {
        svm: LiteSVM,
        maker: Keypair,
        mint_to_raise: Pubkey,
        fundraiser: Pubkey,
        fundraiser_bump: u8,
        vault: Pubkey,
        amount_to_raise: u64,
        time_started: i64,
        duration_days: u8,
    }

    struct ContributeSetup {
        svm: LiteSVM,
        maker: Keypair,
        mint_to_raise: Pubkey,
        contributor: Keypair,
        contributor_ata: Pubkey,
        contributor_account: Pubkey,
        fundraiser: Pubkey,
        fundraiser_bump: u8,
        contributor_bump: u8,
        vault: Pubkey,
        amount_contributed: u64,
        amount_to_raise: u64,
    }

    //Initialize

    fn setup_initialize(amount_to_raise: u64, duration_days: u8) -> FundraiserSetup {

        assert!(
            amount_to_raise >= min_raise(),
            "amount_to_raise {amount_to_raise} is below program minimum {}",
            min_raise()
        );

        let (mut svm, maker) = setup();

        let mint_to_raise = CreateMint::new(&mut svm, &maker)
            .decimals(DECIMALS)
            .authority(&maker.pubkey())
            .send()
            .unwrap();

        let (fundraiser, fundraiser_bump) = fundraiser_pda(&maker.pubkey());
        let vault =
            spl_associated_token_account::get_associated_token_address(&fundraiser, &mint_to_raise);


        let time_started: i64 = 0;
        set_clock(&mut svm, time_started);

        let ix = Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new(maker.pubkey(), true),
                AccountMeta::new(mint_to_raise, false),
                AccountMeta::new(fundraiser, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(system_program(), false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(ata_program(), false),
            ],
            data: [
                vec![0u8],
                amount_to_raise.to_le_bytes().to_vec(),
                time_started.to_le_bytes().to_vec(),
                vec![duration_days],
                vec![fundraiser_bump],
            ]
            .concat(),
        };

        let blockhash = svm.latest_blockhash();
        let tx = Transaction::new(&[&maker], Message::new(&[ix], Some(&maker.pubkey())), blockhash);
        let meta = svm.send_transaction(tx).expect("Initialize instruction failed");
        println!("Initialize CU: {}", meta.compute_units_consumed);

        FundraiserSetup {
            svm,
            maker,
            mint_to_raise,
            fundraiser,
            fundraiser_bump,
            vault,
            amount_to_raise,
            time_started,
            duration_days,
        }
    }




    // contribute

    fn setup_contribute(
        fs: FundraiserSetup,
        contribute_amount: u64,
        mint_supply: u64,

    ) -> ContributeSetup {

        let FundraiserSetup {
            mut svm,
            maker,
            mint_to_raise,
            fundraiser,
            fundraiser_bump,
            vault,
            amount_to_raise,
            time_started,
            duration_days,
        } = fs;

        let past_deadline = time_started + (duration_days as i64 + 1) * SECONDS_TO_DAYS;
        set_clock(&mut svm, past_deadline);

        let contributor = Keypair::new();
        svm.airdrop(&contributor.pubkey(), 10 * LAMPORTS_PER_SOL)
            .expect("Airdrop failed");

        let contributor_ata =
            CreateAssociatedTokenAccount::new(&mut svm, &contributor, &mint_to_raise)
                .owner(&contributor.pubkey())
                .send()
                .unwrap();

        MintTo::new(&mut svm, &maker, &mint_to_raise, &contributor_ata, mint_supply)
            .send()
            .unwrap();

        let (contributor_account, contributor_bump) = contributor_pda(&contributor.pubkey());

        let ix = Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new(contributor.pubkey(), true),
                AccountMeta::new(mint_to_raise, false),
                AccountMeta::new(fundraiser, false),
                AccountMeta::new(contributor_account, false),
                AccountMeta::new(contributor_ata, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(system_program(), false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(ata_program(), false),
            ],
            data: [
                vec![1u8],
                contribute_amount.to_le_bytes().to_vec(),
                vec![contributor_bump],
            ]
            .concat(),
        };

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new(
            &[&contributor],
            Message::new(&[ix], Some(&contributor.pubkey())),
            blockhash,
        );

        let meta = svm.send_transaction(tx).expect("Contribute instruction failed");
        println!("Contribute CU: {}", meta.compute_units_consumed);

        ContributeSetup {
            svm,
            maker,
            mint_to_raise,
            contributor,
            contributor_ata,
            contributor_account,
            fundraiser,
            fundraiser_bump,
            contributor_bump,
            vault,
            amount_contributed: contribute_amount,
            amount_to_raise,
        }
    }





    // checker

    fn run_checker(
        svm: &mut LiteSVM,
        maker: &Keypair,
        mint_to_raise: &Pubkey,
        fundraiser: &Pubkey,
        fundraiser_bump: u8,
        vault: &Pubkey,
    ) -> Pubkey {

        let maker_ata = CreateAssociatedTokenAccount::new(svm, maker, mint_to_raise)
            .owner(&maker.pubkey())
            .send()
            .unwrap();

        let ix = Instruction {
            program_id: program_id(),

            accounts: vec![
                AccountMeta::new(maker.pubkey(), true),
                AccountMeta::new(*mint_to_raise, false),
                AccountMeta::new(*fundraiser, false),
                AccountMeta::new(*vault, false),
                AccountMeta::new(maker_ata, false),
                AccountMeta::new_readonly(system_program(), false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            ],
            data: vec![2u8, fundraiser_bump],
        };

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new(&[maker], Message::new(&[ix], Some(&maker.pubkey())), blockhash);
        
        let meta = svm.send_transaction(tx).expect("Checker instruction failed");
        
        println!("Checker CU: {}", meta.compute_units_consumed);

        maker_ata
    }





    // refund

    fn run_refund(
        svm: &mut LiteSVM,
        contributor: &Keypair,
        maker_pubkey: &Pubkey,
        mint_to_raise: &Pubkey,
        fundraiser: &Pubkey,
        fundraiser_bump: u8,
        contributor_bump: u8,
        contributor_account: &Pubkey,
        contributor_ata: &Pubkey,
        vault: &Pubkey,
        active_timestamp: i64,
    ) {
        set_clock(svm, active_timestamp);

        let ix = Instruction {

            program_id: program_id(),

            accounts: vec![
                AccountMeta::new(contributor.pubkey(), true),
                AccountMeta::new(*maker_pubkey, false),
                AccountMeta::new(*mint_to_raise, false),
                AccountMeta::new(*fundraiser, false),
                AccountMeta::new(*contributor_account, false),
                AccountMeta::new(*contributor_ata, false),
                AccountMeta::new(*vault, false),
                AccountMeta::new_readonly(system_program(), false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            ],
            data: vec![3u8, fundraiser_bump, contributor_bump],
        };

        let blockhash = svm.latest_blockhash();

        let tx = Transaction::new(
            &[contributor],
            Message::new(&[ix], Some(&contributor.pubkey())),
            blockhash,
        );

        let meta = svm.send_transaction(tx).expect("Refund instruction failed");

        println!("Refund CU: {}", meta.compute_units_consumed);
    }


    




    #[test]
    fn test_initialize() {
        let amount_to_raise = min_raise() * 2;
        let s = setup_initialize(amount_to_raise, 30);

        let acct = s.svm.get_account(&s.fundraiser).expect("fundraiser not found");
        assert_eq!(acct.owner, program_id(), "fundraiser should be owned by the program");
        assert_eq!(acct.data.len(), crate::state::Fundraiser::LEN, "wrong account size");

        assert_eq!(
            read_fundraiser_amount_to_raise(&s.svm, &s.fundraiser),
            amount_to_raise,
            "stored amount_to_raise mismatch"
        );
        assert_eq!(
            read_fundraiser_current_amount(&s.svm, &s.fundraiser),
            0,
            "current_amount should be 0 after initialize"
        );
        assert_eq!(
            read_token_balance(&s.svm, &s.vault),
            0,
            "vault should be empty after initialize"
        );

        println!("test_initialize passed");
    }


    


    #[test]
    fn test_contribute() {
        let amount_to_raise   = min_raise() * 2;
        let contribute_amount = amount_to_raise / 15;
        let mint_supply       = amount_to_raise * 2;

        let fs = setup_initialize(amount_to_raise, 30);
        let vault      = fs.vault;
        let fundraiser = fs.fundraiser;

        let cs = setup_contribute(fs, contribute_amount, mint_supply);

        assert_eq!(
            read_token_balance(&cs.svm, &vault),
            contribute_amount,
            "vault should hold contributed tokens"
        );
        assert_eq!(
            read_token_balance(&cs.svm, &cs.contributor_ata),
            mint_supply - contribute_amount,
            "contributor ATA should decrease by contributed amount"
        );
        assert_eq!(
            read_fundraiser_current_amount(&cs.svm, &fundraiser),
            contribute_amount,
            "current_amount should reflect the contribution"
        );

        println!("test_contribute passed");
    }


    
    /*
    #[test]
    fn test_checker() {
        let amount_to_raise = min_raise() * 2;
        let contribute_amount = amount_to_raise / 15;
        let mint_supply     = amount_to_raise * 2;

        let fs = setup_initialize(amount_to_raise, 30);

        let cs = setup_contribute(fs, contribute_amount, mint_supply);

        let ContributeSetup {
            mut svm,
            maker,
            mint_to_raise,
            fundraiser,
            fundraiser_bump,
            vault,
            ..
        } = cs;

        let maker_ata =
            run_checker(&mut svm, &maker, &mint_to_raise, &fundraiser, fundraiser_bump, &vault);

        assert_eq!(
            read_token_balance(&svm, &maker_ata),
            amount_to_raise,
            "maker should receive all raised tokens"
        );
        assert_eq!(
            read_token_balance(&svm, &vault),
            0,
            "vault should be empty after checker"
        );
        assert!(
            !svm.get_account(&fundraiser).is_none(),
            "fundraiser account should be closed after checker"
        );

        println!("test_checker passed");
    }
    */

    

    #[test]
    fn test_checker() {
        let amount_to_raise = min_raise();
        let duration_days = 7;
        let fs = setup_initialize(amount_to_raise, duration_days);

        let per_person_limit = amount_to_raise / 15;
        

        let cs = setup_contribute(fs, per_person_limit, per_person_limit);
        
        let ContributeSetup {
            mut svm,
            maker,
            mint_to_raise,
            fundraiser,
            fundraiser_bump,
            vault,
            ..
        } = cs;
        
        let mut total_in_vault = per_person_limit;

        while total_in_vault < amount_to_raise {
            let next_contributor = Keypair::new();
            svm.airdrop(&next_contributor.pubkey(), 10 * LAMPORTS_PER_SOL)
                .expect("Airdrop failed");

            let next_ata = CreateAssociatedTokenAccount::new(&mut svm, &next_contributor, &mint_to_raise)
                .owner(&next_contributor.pubkey())
                .send()
                .unwrap();

            MintTo::new(&mut svm, &maker, &mint_to_raise, &next_ata, per_person_limit)
                .send()
                .unwrap();

            let (contributor_account, contributor_bump) = contributor_pda(&next_contributor.pubkey());

            let contribute_ix = Instruction {
                program_id: program_id(),
                accounts: vec![
                    AccountMeta::new(next_contributor.pubkey(), true),
                    AccountMeta::new(mint_to_raise, false),
                    AccountMeta::new(fundraiser, false),
                    AccountMeta::new(contributor_account, false),
                    AccountMeta::new(next_ata, false),
                    AccountMeta::new(vault, false),
                    AccountMeta::new_readonly(system_program(), false),
                    AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                    AccountMeta::new_readonly(ata_program(), false),
                ],
                data: [vec![1u8], per_person_limit.to_le_bytes().to_vec(), vec![contributor_bump]].concat(),
            };

            let blockhash = svm.latest_blockhash();
            let tx = Transaction::new(
                &[&next_contributor],
                Message::new(&[contribute_ix], Some(&next_contributor.pubkey())),
                blockhash,
            );
            svm.send_transaction(tx).expect("Subsequent contribution failed");
            
            total_in_vault += per_person_limit;
        }



        let maker_ata =
            run_checker(&mut svm, &maker, &mint_to_raise, &fundraiser, fundraiser_bump, &vault);

        assert_eq!(
            read_token_balance(&svm, &maker_ata),
            amount_to_raise,
            "maker should receive all raised tokens"
        );
        assert_eq!(
            read_token_balance(&svm, &vault),
            0,
            "vault should be empty after checker"
        );
        assert!(
            svm.get_account(&fundraiser).is_none(),
            "fundraiser account should be closed after checker"
        );

        println!("test_checker passed");

    }




    #[test]
    fn test_refund() {
        let amount_to_raise   = min_raise() * 2;
        let contribute_amount = amount_to_raise / 15;
        let mint_supply       = amount_to_raise * 2;

        let fs = setup_initialize(amount_to_raise, 30);
        let fundraiser_bump = fs.fundraiser_bump;
        let mint_to_raise   = fs.mint_to_raise;
        let maker_pubkey    = fs.maker.pubkey();

        let active_timestamp: i64 = 15 * SECONDS_TO_DAYS;

        let cs = setup_contribute(fs, contribute_amount, mint_supply);

        let ContributeSetup {
            mut svm,
            contributor,
            contributor_ata,
            contributor_account,
            contributor_bump,
            fundraiser,
            vault,
            ..
        } = cs;

        run_refund(
            &mut svm,
            &contributor,
            &maker_pubkey,
            &mint_to_raise,
            &fundraiser,
            fundraiser_bump,
            contributor_bump,
            &contributor_account,
            &contributor_ata,
            &vault,
            active_timestamp,
        );

        assert_eq!(
            read_token_balance(&svm, &contributor_ata),
            mint_supply,
            "contributor should recover full balance after refund"
        );
        assert_eq!(
            read_token_balance(&svm, &vault),
            0,
            "vault should be empty after refund"
        );
        assert!(
            svm.get_account(&contributor_account).is_none(),
            "contributor_account PDA should be closed after refund"
        );

        println!("test_refund passed");
    }



    #[test]
    fn test_binary_size() {
        let path = so_path();
        let metadata = std::fs::metadata(&path).expect("Failed to get metadata");
        let size_kb = metadata.len() as f64 / 1024.0;
        
        println!("Final Binary Size: {:.2} KB", size_kb);
        
        assert!(metadata.len() < 32_768, "Binary is too large for L1 iCache optimization!");
    }

}