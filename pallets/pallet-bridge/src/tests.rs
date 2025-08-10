use crate::{Error, Event, LockedInfo, MAX_RELAYERS, mock::*};
use frame_support::{assert_noop, assert_ok};
use sp_core::H160;

fn last_bridge_event() -> RuntimeEvent {
    System::events().pop().expect("expected at least one event").event
}

#[test]
fn root_can_set_relayers_and_too_many_relayers_errors() {
    new_test_ext().execute_with(|| {
        // normal case
        let relayers = vec![H160::repeat_byte(0x11), H160::repeat_byte(0x22)];
        assert_ok!(Bridge::set_relayers(RuntimeOrigin::root(), relayers.clone()));

        // event emitted
        let ev = last_bridge_event();
        assert_eq!(ev, RuntimeEvent::Bridge(Event::RelayersUpdated(relayers.clone())));

        // error case: too many relayers (build a vec longer than MAX_RELAYERS)
        // MAX_RELAYERS constant in pallet is 100; create 101 items to trigger error
        let too_many: Vec<H160> = (0..(MAX_RELAYERS + 1))
            .map(|i| {
                let mut b = [0u8; 20];
                b[0] = (i & 0xff) as u8;
                H160::from(b)
            })
            .collect();

        assert_noop!(
            Bridge::set_relayers(RuntimeOrigin::root(), too_many),
            Error::<Test>::TooManyRelayers
        );
    });
}

#[test]
fn lock_creates_locked_message_and_emits_event() {
    new_test_ext().execute_with(|| {
        let sender: u64 = 1;
        let amount: u128 = 300;
        let fee: u128 = 10;
        let eth_recipient = H160::repeat_byte(0xAA);
        let nonce: u64 = 7;

        // call lock
        assert_ok!(Bridge::lock(RuntimeOrigin::signed(sender), amount, fee, eth_recipient, nonce));

        // event captured
        let ev = last_bridge_event();
        match ev {
            RuntimeEvent::Bridge(Event::Locked(
                who,
                stored_amount,
                stored_fee,
                stored_eth,
                stored_nonce,
                message_id,
            )) => {
                assert_eq!(who, sender);
                assert_eq!(stored_amount, amount);
                assert_eq!(stored_fee, fee);
                assert_eq!(stored_eth, eth_recipient);
                assert_eq!(stored_nonce, nonce);

                // storage must contain LockedMessages for that id
                let maybe = Bridge::locked(message_id);
                assert!(maybe.is_some());
                let info: LockedInfo<u64, u128> = maybe.unwrap();
                assert_eq!(info.owner, sender);
                assert_eq!(info.amount, amount);
                assert_eq!(info.relayer_fee, fee);
                assert_eq!(info.eth_recipient, eth_recipient);
                assert_eq!(info.nonce, nonce);

                // pallet account should have received total = amount + fee
                let pallet_acct = Bridge::account_id();
                let pallet_bal = Balances::free_balance(pallet_acct);
                assert_eq!(pallet_bal, amount + fee);
            },
            other => panic!("unexpected event: {other:?}"),
        }
    });
}

#[test]
fn cannot_lock_if_insufficient_balance() {
    new_test_ext().execute_with(|| {
        let sender: u64 = 99; // this account has 0 balance in genesis
        let amount: u128 = 10;
        let fee: u128 = 0;
        let eth_recipient = H160::repeat_byte(0xBB);
        let nonce: u64 = 1;

        assert_noop!(
            Bridge::lock(RuntimeOrigin::signed(sender), amount, fee, eth_recipient, nonce),
            Error::<Test>::InsufficientBalance
        );
    });
}

#[test]
fn release_transfers_and_reimburses_relayer_and_prevents_replay() {
    new_test_ext().execute_with(|| {
        let locker: u64 = 1;
        let relayer_submitter: u64 = 2;
        let amount: u128 = 200;
        let fee: u128 = 20;
        let eth_recipient = H160::repeat_byte(0xCC);
        let nonce: u64 = 9;

        // initial balances
        let before_relayer = Balances::free_balance(relayer_submitter);

        // lock
        assert_ok!(Bridge::lock(RuntimeOrigin::signed(locker), amount, fee, eth_recipient, nonce));

        // capture Locked event and message_id
        let ev = last_bridge_event();
        let message_id = match ev {
            RuntimeEvent::Bridge(Event::Locked(_, _, _, _, _, id)) => id,
            other => panic!("expected Locked event, got {other:?}"),
        };

        // release: since RelayerThreshold=0 (in mock), signatures vec can be empty
        assert_ok!(Bridge::release(
            RuntimeOrigin::signed(relayer_submitter),
            message_id,
            locker, // recipient is locker in this test for simplicity
            amount,
            vec![],
            None
        ));

        // Released event emitted
        let ev2 = last_bridge_event();
        assert_eq!(ev2, RuntimeEvent::Bridge(Event::Released(locker, amount, message_id)));

        // Locked entry should be removed
        assert!(Bridge::locked(message_id).is_none());

        // pallet balance should be 0 now
        let pallet_acct = Bridge::account_id();
        assert_eq!(Balances::free_balance(pallet_acct), 0u128);

        // relayer was reimbursed by fee
        // relayer had before_relayer, now should be before_relayer + fee
        assert_eq!(Balances::free_balance(relayer_submitter), before_relayer + fee);

        // replay: calling release again must error with MessageAlreadyProcessed
        assert_noop!(
            Bridge::release(
                RuntimeOrigin::signed(relayer_submitter),
                message_id,
                locker,
                amount,
                vec![],
                None
            ),
            Error::<Test>::MessageAlreadyProcessed
        );
    });
}

#[test]
fn top_up_relayer_fund_and_emergency_withdraw_works_and_pause_blocks_ops() {
    new_test_ext().execute_with(|| {
        let admin: u64 = 1;
        let depositor: u64 = 2;
        let amount: u128 = 50;

        // top up RelayerFund by depositor (transfer into pallet)
        assert_ok!(Bridge::top_up_relayer_fund(RuntimeOrigin::signed(depositor), amount));
        let fund = Bridge::relayer_fund();
        assert_eq!(fund, amount);

        // emergency withdraw by root
        let before_admin = Balances::free_balance(admin);
        assert_ok!(Bridge::emergency_withdraw(RuntimeOrigin::root(), admin, 10));
        // admin should have received 10
        assert_eq!(Balances::free_balance(admin), before_admin + 10);

        // set paused true
        assert_ok!(Bridge::set_paused(RuntimeOrigin::root(), true));
        assert!(Bridge::is_paused());
        // operations blocked: lock should fail
        let eth_recipient = H160::repeat_byte(0xDE);
        assert_noop!(
            Bridge::lock(RuntimeOrigin::signed(depositor), 5u128, 0u128, eth_recipient, 0u64),
            Error::<Test>::Paused
        );
        // unpause
        assert_ok!(Bridge::set_paused(RuntimeOrigin::root(), false));
        assert!(!Bridge::is_paused());
    });
}
