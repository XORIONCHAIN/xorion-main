use crate::{Error, Pallet, mock::*};
use frame_support::{assert_noop, assert_ok};
use sp_core::H256;

#[test]
fn deposit_works() {
    new_test_ext().execute_with(|| {
        let depositor = 1;
        let amount = 100u128;
        let commitment_hash = H256::from_low_u64_be(123);
        let sovereign_account = Pallet::<Test>::sovereign_account_id();

        // The public inputs are the raw bytes of the data.
        let public_inputs =
            vec![amount.to_be_bytes().to_vec(), commitment_hash.as_bytes().to_vec()];

        // Check initial balances
        assert_eq!(Balances::free_balance(depositor), 1000);
        assert_eq!(Balances::free_balance(sovereign_account), 0);

        // Perform the deposit
        assert_ok!(ConfidentialTransactions::deposit(
            RuntimeOrigin::signed(depositor),
            create_dummy_proof(),
            public_inputs,
            amount
        ));

        // Check that funds were transferred to the sovereign account
        assert_eq!(Balances::free_balance(depositor), 900);
        assert_eq!(Balances::free_balance(sovereign_account), 100);

        // Check that the Merkle tree was updated
        assert_eq!(ConfidentialTransactions::next_leaf_index(), 1);
        assert_ne!(ConfidentialTransactions::merkle_root(), H256::default());
        // Verify that the correct leaf was inserted at the correct position
        assert_eq!(ConfidentialTransactions::tree_nodes((TreeDepth::get(), 0)), commitment_hash);
    });
}

#[test]
fn withdraw_works() {
    new_test_ext().execute_with(|| {
        let depositor = 1;
        let recipient = 2;
        let amount = 100u128;
        let commitment_hash = H256::from_low_u64_be(123);
        let sovereign_account = Pallet::<Test>::sovereign_account_id();
        let nullifier_hash = H256::from_low_u64_be(456);

        // First, deposit some funds to have something to withdraw
        let deposit_inputs =
            vec![amount.to_be_bytes().to_vec(), commitment_hash.as_bytes().to_vec()];
        assert_ok!(ConfidentialTransactions::deposit(
            RuntimeOrigin::signed(depositor),
            create_dummy_proof(),
            deposit_inputs,
            amount
        ));

        // Get the current merkle root to use in the withdrawal proof
        let merkle_root = ConfidentialTransactions::merkle_root();

        // The public inputs must be the raw bytes of the data, in the correct order.
        let withdraw_inputs = vec![
            merkle_root.as_bytes().to_vec(),
            nullifier_hash.as_bytes().to_vec(),
            H256::from_low_u64_be(recipient).as_bytes().to_vec(), // Mock recipient hash
            amount.to_be_bytes().to_vec(),
            (0u64).to_be_bytes().to_vec(), // Mock fee
        ];

        // Check balances before withdrawal
        assert_eq!(Balances::free_balance(sovereign_account), 100);
        assert_eq!(Balances::free_balance(recipient), 1000);

        // Perform the withdrawal
        assert_ok!(ConfidentialTransactions::withdraw(
            RuntimeOrigin::signed(depositor), // `depositor` pays the fee
            create_dummy_proof(),
            withdraw_inputs,
            recipient,
            amount
        ));

        // Check that funds were transferred from the sovereign account
        assert_eq!(Balances::free_balance(sovereign_account), 0);
        assert_eq!(Balances::free_balance(recipient), 1100);

        // Check that the nullifier was recorded
        assert!(ConfidentialTransactions::nullifiers(nullifier_hash));
    });
}
#[test]
fn withdraw_fails_on_used_nullifier() {
    new_test_ext().execute_with(|| {
        let depositor = 1;
        let recipient = 2;
        let amount = 100u128;
        let commitment_hash = H256::from_low_u64_be(123);
        let nullifier_hash = H256::from_low_u64_be(456);

        // Deposit
        let deposit_inputs =
            vec![amount.to_be_bytes().to_vec(), commitment_hash.as_bytes().to_vec()];
        assert_ok!(ConfidentialTransactions::deposit(
            RuntimeOrigin::signed(depositor),
            create_dummy_proof(),
            deposit_inputs,
            amount
        ));

        let merkle_root = ConfidentialTransactions::merkle_root();
        let withdraw_inputs = vec![
            merkle_root.as_bytes().to_vec(),
            nullifier_hash.as_bytes().to_vec(),
            H256::from_low_u64_be(recipient).as_bytes().to_vec(),
            amount.to_be_bytes().to_vec(),
            (0u64).to_be_bytes().to_vec(),
        ];

        // First withdrawal should work
        assert_ok!(ConfidentialTransactions::withdraw(
            RuntimeOrigin::signed(depositor),
            create_dummy_proof(),
            withdraw_inputs.clone(),
            recipient,
            amount
        ));

        // Second attempt with the same nullifier should fail
        assert_noop!(
            ConfidentialTransactions::withdraw(
                RuntimeOrigin::signed(depositor),
                create_dummy_proof(),
                withdraw_inputs,
                recipient,
                amount
            ),
            Error::<Test>::NullifierAlreadyUsed
        );
    });
}

#[test]
fn transact_works() {
    new_test_ext().execute_with(|| {
        // Deposit two notes to be used as inputs
        assert_ok!(ConfidentialTransactions::deposit(
            RuntimeOrigin::signed(1),
            create_dummy_proof(),
            vec![10u64.to_be_bytes().to_vec(), H256::from_low_u64_be(1).as_bytes().to_vec()],
            10
        ));
        assert_ok!(ConfidentialTransactions::deposit(
            RuntimeOrigin::signed(1),
            create_dummy_proof(),
            vec![5u64.to_be_bytes().to_vec(), H256::from_low_u64_be(2).as_bytes().to_vec()],
            5
        ));

        let merkle_root = ConfidentialTransactions::merkle_root();

        let nullifier1_hash = H256::from_low_u64_be(101); // Corresponds to note 1
        let nullifier2_hash = H256::from_low_u64_be(102); // Corresponds to note 2

        let commitment1_hash = H256::from_low_u64_be(201); // New note for recipient
        let commitment2_hash = H256::from_low_u64_be(202); // New change note

        let transact_inputs = vec![
            merkle_root.as_bytes().to_vec(),
            nullifier1_hash.as_bytes().to_vec(),
            nullifier2_hash.as_bytes().to_vec(),
            commitment1_hash.as_bytes().to_vec(),
            commitment2_hash.as_bytes().to_vec(),
        ];

        // Check state before transaction
        assert_eq!(ConfidentialTransactions::next_leaf_index(), 2);
        assert!(!ConfidentialTransactions::nullifiers(nullifier1_hash));
        assert!(!ConfidentialTransactions::nullifiers(nullifier2_hash));

        // Perform the transaction
        assert_ok!(ConfidentialTransactions::transact(
            RuntimeOrigin::signed(1),
            create_dummy_proof(),
            transact_inputs
        ));

        // Check state after transaction
        assert_eq!(ConfidentialTransactions::next_leaf_index(), 4); // Two new leaves
        assert!(ConfidentialTransactions::nullifiers(nullifier1_hash));
        assert!(ConfidentialTransactions::nullifiers(nullifier2_hash));
    });
}
