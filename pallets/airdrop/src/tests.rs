use crate::{
    Error, Event, PalletId,
    mock::{AccountId, Airdrop, Balances, RuntimeOrigin, System, Test, new_test_ext, run_to_block},
};
use frame_support::{assert_noop, assert_ok, traits::Currency};
use sp_runtime::traits::AccountIdConversion;

#[test]
fn claim_airdrop_works() {
    new_test_ext().execute_with(|| {
        // Account 5 has balance 0 (below threshold of 100)
        let initial_balance = Balances::free_balance(&5);
        assert_eq!(initial_balance, 0);

        // Account 2 should be eligible for airdrop
        assert!(Airdrop::is_eligible_for_airdrop(&5));

        // Claim airdrop
        assert_ok!(Airdrop::claim_airdrop(RuntimeOrigin::signed(5)));

        // Check balance increased
        assert_eq!(Balances::free_balance(&5), 1000);

        // Check airdrop record was created
        let record = Airdrop::airdrop_records(&5).unwrap();
        assert_eq!(record.claims_count, 1);
        assert_eq!(record.last_claim_block, 1);
        assert_eq!(record.total_received, 1000);

        // Check total airdrops counter
        assert_eq!(Airdrop::total_airdrops(), 2); // 1 from genesis + 1 from claim

        // Check airdrops this block counter
        assert_eq!(Airdrop::airdrops_this_block(), 1);
    });
}

#[test]
fn claim_airdrop_fails_for_funded_account() {
    new_test_ext().execute_with(|| {
        // Account 1 has balance 5000 (above threshold of 100)
        assert_eq!(Balances::free_balance(&1), 5000);

        // Should not be eligible
        assert!(!Airdrop::is_eligible_for_airdrop(&1));

        // Claim should fail
        assert_noop!(
            Airdrop::claim_airdrop(RuntimeOrigin::signed(1)),
            Error::<Test>::AccountAlreadyFunded
        );
    });
}

#[test]
fn cooldown_period_works() {
    new_test_ext().execute_with(|| {
        // First claim should work
        assert_ok!(Airdrop::claim_airdrop(RuntimeOrigin::signed(2)));

        // Second claim immediately should fail
        assert_noop!(
            Airdrop::claim_airdrop(RuntimeOrigin::signed(2)),
            Error::<Test>::AccountAlreadyFunded
        );

        // Check cooldown remaining
        assert_eq!(Airdrop::get_cooldown_remaining(&2), 5);

        // spend some funds
        assert_ok!(Balances::burn(RuntimeOrigin::signed(2), 1040, true));
        // Fast forward to block 3 (still within cooldown)
        run_to_block(3);
        assert_eq!(Airdrop::get_cooldown_remaining(&2), 3);

        assert_noop!(
            Airdrop::claim_airdrop(RuntimeOrigin::signed(2)),
            Error::<Test>::CooldownPeriodActive
        );

        // Fast forward to block 6 (cooldown should be over)
        run_to_block(6);
        assert_eq!(Airdrop::get_cooldown_remaining(&2), 0);

        // Reduce balance to make account eligible again
        let _ = Balances::slash(&2, 1000);

        // Should work now
        assert_ok!(Airdrop::claim_airdrop(RuntimeOrigin::signed(2)));

        // Check record was updated
        let record = Airdrop::airdrop_records(&2).unwrap();
        assert_eq!(record.claims_count, 2);
        assert_eq!(record.last_claim_block, 6);
    });
}

#[test]
fn max_airdrops_per_account_works() {
    new_test_ext().execute_with(|| {
        let account = 3;

        // Claim 3 airdrops (maximum allowed)
        for i in 0..3 {
            if i > 0 {
                // Wait for cooldown
                run_to_block(1 + i * 6);
            }
            assert_ok!(Airdrop::claim_airdrop(RuntimeOrigin::signed(account)));
            // spend some funds
            assert_ok!(Balances::burn(RuntimeOrigin::signed(account), 990, true));
        }

        // Check record
        let record = Airdrop::airdrop_records(&account).unwrap();
        assert_eq!(record.claims_count, 3);

        // Fourth claim should fail
        run_to_block(25);
        assert_noop!(
            Airdrop::claim_airdrop(RuntimeOrigin::signed(account)),
            Error::<Test>::MaxAirdropsReached
        );
    });
}

#[test]
fn max_airdrops_per_block_works() {
    new_test_ext().execute_with(|| {
        // Create 10 accounts with low balance
        for i in 10..20 {
            let _ = Balances::deposit_creating(&i, 50);
        }

        // Claim 10 airdrops in the same block (should all work)
        for i in 10..20 {
            assert_ok!(Airdrop::claim_airdrop(RuntimeOrigin::signed(i)));
            // spend some funds
            assert_ok!(Balances::burn(RuntimeOrigin::signed(i), 980, true));
        }

        // 11th claim should fail
        let _ = Balances::deposit_creating(&21, 50);
        assert_noop!(
            Airdrop::claim_airdrop(RuntimeOrigin::signed(21)),
            Error::<Test>::MaxAirdropsPerBlockReached
        );

        // Move to next block and it should work
        run_to_block(2);
        assert_ok!(Airdrop::claim_airdrop(RuntimeOrigin::signed(21)));
    });
}

#[test]
fn insufficient_funds_error() {
    new_test_ext().execute_with(|| {
        // Drain the airdrop pool
        let airdrop_account = Airdrop::airdrop_account_id();
        let _ = Balances::slash(&airdrop_account, 20000);

        // Claim should fail
        assert_noop!(
            Airdrop::claim_airdrop(RuntimeOrigin::signed(2)),
            Error::<Test>::InsufficientAirdropFunds
        );
    });
}

#[test]
fn fund_airdrop_pool_works() {
    new_test_ext().execute_with(|| {
        let airdrop_account = Airdrop::airdrop_account_id();
        let initial_balance = Balances::free_balance(&airdrop_account); // Should be 9000

        // Fund the pool
        assert_ok!(Airdrop::fund_airdrop_pool(RuntimeOrigin::root(), 5000));

        // Check balance increased
        assert_eq!(Balances::free_balance(&airdrop_account), initial_balance + 5000);

        // Check event was emitted
        System::assert_last_event(Event::AirdropFunded { amount: 5000 }.into());
    });
}
#[test]
fn fund_airdrop_pool_fails_with_zero_amount() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Airdrop::fund_airdrop_pool(RuntimeOrigin::root(), 0),
            Error::<Test>::ZeroAirdropAmount
        );
    });
}

#[test]
fn fund_airdrop_pool_requires_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Airdrop::fund_airdrop_pool(RuntimeOrigin::signed(1), 1000),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn events_are_emitted() {
    new_test_ext().execute_with(|| {
        // Clear events
        System::reset_events();

        // Claim airdrop
        assert_ok!(Airdrop::claim_airdrop(RuntimeOrigin::signed(2)));

        // Check event was emitted
        System::assert_last_event(Event::AirdropClaimed { who: 2, amount: 1000 }.into());
    });
}

#[test]
fn on_initialize_resets_counters() {
    new_test_ext().execute_with(|| {
        // Make some claims
        assert_ok!(Airdrop::claim_airdrop(RuntimeOrigin::signed(2)));
        assert_ok!(Airdrop::claim_airdrop(RuntimeOrigin::signed(3)));

        assert_eq!(Airdrop::airdrops_this_block(), 2);
        assert_eq!(Airdrop::last_reset_block(), 1);

        // Move to next block
        run_to_block(2);

        // Counter should be reset
        assert_eq!(Airdrop::airdrops_this_block(), 0);
        assert_eq!(Airdrop::last_reset_block(), 2);
    });
}

#[test]
fn is_eligible_for_airdrop_comprehensive() {
    new_test_ext().execute_with(|| {
        // Account with high balance should not be eligible
        assert!(!Airdrop::is_eligible_for_airdrop(&1));

        // Account with low balance should be eligible
        assert!(Airdrop::is_eligible_for_airdrop(&2));

        // After claiming, should not be eligible due to balance increase
        assert_ok!(Airdrop::claim_airdrop(RuntimeOrigin::signed(2)));
        assert!(!Airdrop::is_eligible_for_airdrop(&2));

        // Account with zero balance should be eligible
        assert!(Airdrop::is_eligible_for_airdrop(&3));

        // Test max airdrops per account
        let account = 5;
        let _ = Balances::deposit_creating(&account, 10);

        // Claim maximum airdrops
        for i in 0..3 {
            if i > 0 {
                run_to_block(1 + i * 6);
                let _ = Balances::slash(&account, 1000); // Reduce balance
            }
            assert_ok!(Airdrop::claim_airdrop(RuntimeOrigin::signed(account)));
        }

        // Should not be eligible for more airdrops
        run_to_block(25);
        let _ = Balances::slash(&account, 1000);
        assert!(!Airdrop::is_eligible_for_airdrop(&account));
    });
}

#[test]
fn airdrop_account_id_is_correct() {
    new_test_ext().execute_with(|| {
        let expected_account: AccountId = PalletId(*b"airdrop!").into_account_truncating();
        assert_eq!(Airdrop::airdrop_account_id(), expected_account);
    });
}

#[test]
fn storage_values_are_correct() {
    new_test_ext().execute_with(|| {
        // Initial values
        assert_eq!(Airdrop::total_airdrops(), 1); // From genesis
        assert_eq!(Airdrop::airdrops_this_block(), 0);

        // After claim
        assert_ok!(Airdrop::claim_airdrop(RuntimeOrigin::signed(2)));
        assert_eq!(Airdrop::total_airdrops(), 2);
        assert_eq!(Airdrop::airdrops_this_block(), 1);
    });
}

#[test]
fn multiple_claims_update_record_correctly() {
    new_test_ext().execute_with(|| {
        let account = 3;

        // First claim
        assert_ok!(Airdrop::claim_airdrop(RuntimeOrigin::signed(account)));
        let record1 = Airdrop::airdrop_records(&account).unwrap();
        assert_eq!(record1.claims_count, 1);
        assert_eq!(record1.total_received, 1000);

        // Second claim after cooldown
        run_to_block(7);
        let _ = Balances::slash(&account, 1000); // Reduce balance
        assert_ok!(Airdrop::claim_airdrop(RuntimeOrigin::signed(account)));

        let record2 = Airdrop::airdrop_records(&account).unwrap();
        assert_eq!(record2.claims_count, 2);
        assert_eq!(record2.total_received, 2000);
        assert_eq!(record2.last_claim_block, 7);
    });
}
