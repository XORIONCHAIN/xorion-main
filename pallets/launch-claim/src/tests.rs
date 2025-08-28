use crate::mock::{
    Balances, LaunchClaim, RuntimeOrigin, System, Test, VestingPeriod, XOR, new_test_ext,
};
use frame_support::{assert_noop, assert_ok};

const USDT: u128 = 1_000_000;
#[test]
fn add_claim_works() {
    new_test_ext().execute_with(|| {
        // Relayer adds claim for user 1
        assert_ok!(LaunchClaim::add_claim(RuntimeOrigin::signed(10), 1, USDT));

        // User 1 should now have 20 claimable tokens
        assert_eq!(LaunchClaim::claims(1).total, 20 * XOR);

        assert_ok!(LaunchClaim::add_claim(RuntimeOrigin::signed(10), 1, 10 * USDT));

        // User 1 should now have 220 claimable tokens
        assert_eq!(LaunchClaim::claims(1).total, 220 * XOR);

        // 0.1 USDT
        assert_ok!(LaunchClaim::add_claim(RuntimeOrigin::signed(10), 8, USDT / 10));
        // User 8 should now have 20 claimable tokens
        assert_eq!(LaunchClaim::claims(8).total, 2 * XOR);
        // 0.01 USDT
        assert_ok!(LaunchClaim::add_claim(RuntimeOrigin::signed(10), 18, USDT / 100));
        // User 18 should now have 20 claimable tokens
        assert_eq!(LaunchClaim::claims(18).total, 2 * XOR / 10);
    });
}

#[test]
fn claim_full_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(LaunchClaim::add_claim(RuntimeOrigin::signed(10), 31, 100 * USDT));

        // User claims full
        assert_eq!(LaunchClaim::claims(31).total, 2_000 * XOR);
        assert_ok!(LaunchClaim::activate(RuntimeOrigin::signed(1)));
        assert_ok!(LaunchClaim::claim_full(RuntimeOrigin::signed(31)));
        // only half at a time
        assert_eq!(Balances::free_balance(31), 1_000 * XOR);
        // source account balance reduced
        assert_eq!(Balances::free_balance(1), 9_000 * XOR);
        assert_eq!(LaunchClaim::claims(31).claimed, 1_000 * XOR);
        assert_noop!(
            LaunchClaim::claim(RuntimeOrigin::signed(31), 1),
            crate::Error::<Test>::InsufficientClaim
        );
    });
}

#[test]
fn claim_full_with_vesting_progress() {
    new_test_ext().execute_with(|| {
        assert_ok!(LaunchClaim::add_claim(RuntimeOrigin::signed(10), 31, 100 * USDT));

        // User claims full
        assert_eq!(LaunchClaim::claims(31).total, 2_000 * XOR);
        assert_ok!(LaunchClaim::activate(RuntimeOrigin::signed(1)));
        assert_ok!(LaunchClaim::claim_full(RuntimeOrigin::signed(31)));
        // only half at a time
        assert_eq!(Balances::free_balance(31), 1_000 * XOR);

        // Fast forward halfway through vesting
        System::set_block_number(VestingPeriod::get() / 2 + 1);
        assert_ok!(LaunchClaim::claim_full(RuntimeOrigin::signed(31)));
        // Should get: half already claimed + half of the remaining half = 75%
        // account for fees
        assert_eq!(Balances::free_balance(31), 1_500 * XOR);
        assert_eq!(LaunchClaim::claims(31).claimed, 1_500 * XOR);
        // Advance to end of vesting
        System::set_block_number(VestingPeriod::get() + 1);
        assert_ok!(LaunchClaim::claim_full(RuntimeOrigin::signed(31)));
        assert_eq!(Balances::free_balance(31), 2_000 * XOR);
        assert_eq!(LaunchClaim::claims(31).claimed, 2_000 * XOR);
        assert_noop!(
            LaunchClaim::claim_full(RuntimeOrigin::signed(31)),
            crate::Error::<Test>::InsufficientClaim
        );
        assert_noop!(
            LaunchClaim::claim(RuntimeOrigin::signed(31), 1),
            crate::Error::<Test>::InsufficientClaim
        );
    })
}

#[test]
fn claim_partial_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(LaunchClaim::add_claim(RuntimeOrigin::signed(10), 31, 50 * USDT));

        // User claims full
        assert_eq!(LaunchClaim::claims(31).total, 1_000 * XOR);
        assert_ok!(LaunchClaim::activate(RuntimeOrigin::signed(1)));
        assert_ok!(LaunchClaim::claim(RuntimeOrigin::signed(31), 500 * XOR));
        assert_eq!(Balances::free_balance(31), 500 * XOR);
        // source account balance reduced
        assert_eq!(Balances::free_balance(1), 9_500 * XOR);
        assert_eq!(LaunchClaim::claims(31).claimed, 500 * XOR);
        assert_noop!(
            LaunchClaim::claim(RuntimeOrigin::signed(31), 1),
            crate::Error::<Test>::InsufficientClaim
        );
    });
}

#[test]
fn cannot_claim_when_inactive() {
    new_test_ext().execute_with(|| {
        assert_ok!(LaunchClaim::add_claim(RuntimeOrigin::signed(10), 31, 50 * USDT));
        assert_noop!(
            LaunchClaim::claim(RuntimeOrigin::signed(31), 1_000 * XOR),
            crate::Error::<Test>::NotActivated
        );
    })
}

#[test]
fn cannot_claim_more_than_available() {
    new_test_ext().execute_with(|| {
        assert_ok!(LaunchClaim::add_claim(RuntimeOrigin::signed(10), 31, 5 * USDT));
        assert_ok!(LaunchClaim::activate(RuntimeOrigin::signed(1)));
        // User tries to claim more than stored
        assert_noop!(
            LaunchClaim::claim(RuntimeOrigin::signed(10), 2_000 * XOR),
            crate::Error::<Test>::InsufficientClaim
        );
    })
}
