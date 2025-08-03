# Confidential Transactions Pallet

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

## Overview

This pallet implements a confidential transaction layer for Substrate-based chains, enabling private transfers of assets
as described in the Xorion Chain whitepaper. It uses the Groth16 zk-SNARK proving system to allow users to convert
public assets into a private, "shielded" form and transact with them without revealing the sender, receiver, or amount.

The implementation is self-contained and uses a custom, gas-efficient Merkle tree for scalable commitment tracking, with
cryptographic logic powered by the `arkworks` ecosystem.

---

## Features

- **Shielded Pool**: Allows users to move funds between the public ledger and a private shielded pool using `deposit`
  and `withdraw` extrinsics.
- **Private Transfers**: Enables peer-to-peer transfers within the shielded pool using the `transact` extrinsic.
- **Double-Spend Protection**: Enforces uniqueness of spent notes through an on-chain nullifier set.
- **Scalable State**: Uses an integrated Merkle tree to manage commitments, ensuring the on-chain storage footprint
  remains constant and low, regardless of the number of private notes.
- **Cryptographically Secure**: Leverages the Groth16 proving system over the BLS12-381 curve for its zk-SNARK
  implementation.

---

## Dependencies

This pallet relies on the following key libraries:

- `frame-support` & `frame-system` for core Substrate logic.
- The `arkworks` ecosystem (`ark-groth16`, `ark-bls12-381`, `ark-crypto-primitives`) for all cryptographic operations.

---

## Pallet Configuration

To add this pallet to your runtime, you need to implement its `Config` trait in your `runtime/src/lib.rs` file.

**Example `runtime/src/lib.rs`:**

```rust
use frame_support::parameter_types;
use sp_runtime::PalletId;

parameter_types! {
    // A unique ID for the pallet, used to derive its sovereign account.
    pub const ConfidentialTransactionsPalletId: PalletId = PalletId(*b"xorionct");

    // The depth of the Merkle tree. A depth of 32 allows for over 4 billion commitments.
    pub const TreeDepth: u32 = 32;
}

impl pallet_confidential_transactions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type PalletId = ConfidentialTransactionsPalletId;
    type TreeDepth = TreeDepth;
}
````

## Extrinsics API

The pallet exposes three main extrinsics for user interaction.

**Important Note on `public_inputs`**: All `public_inputs` must be the raw byte representations of the data (e.g.,
`H256.as_bytes()`, `u128.to_be_bytes()`). The pallet is responsible for converting these bytes into field elements for
the SNARK verifier. The order is critical.

### `deposit(proof, public_inputs, amount)`

Moves public funds into the shielded pool, creating a new private commitment.

- **`proof`**: The serialized Groth16 proof from the `deposit` circuit.
- **`public_inputs`**: A vector of raw byte vectors:
    - `[0]`: The public `amount` being deposited (`u128.to_be_bytes()`).
    - `[1]`: The `commitment` hash of the new private note (`H256.as_bytes()`).
- **`amount`**: The public `Balance` to deposit.

### `withdraw(proof, public_inputs, recipient, amount)`

Moves funds from the shielded pool back to a public account.

- **`proof`**: The serialized Groth16 proof from the `transfer` circuit.
- **`public_inputs`**: A vector of raw byte vectors:
    - `[0]`: The `merkle_root` of the commitments tree (`H256.as_bytes()`).
    - `[1]`: The `nullifier` of the note being spent (`H256.as_bytes()`).
    - `[2]`: A hash of the public `recipient` account ID (`H256.as_bytes()`).
    - `[3]`: The `amount` being withdrawn (`u128.to_be_bytes()`).
    - `[4]`: The transaction `fee` (`u128.to_be_bytes()`).
- **`recipient`**: The public `T::AccountId` to receive the funds.
- **`amount`**: The public `Balance` to withdraw.

### `transact(proof, public_inputs)`

Performs a private transfer between parties within the shielded pool.

- **`proof`**: The serialized Groth16 proof from the `transfer` circuit.
- **`public_inputs`**: A vector of raw byte vectors:
    - `[0]`: The `merkle_root` of the commitments tree (`H256.as_bytes()`).
    - `[1]`: The `nullifier1` of the first input note (`H256.as_bytes()`).
    - `[2]`: The `nullifier2` of the second input note (`H256.as_bytes()`).
    - `[3]`: The `commitment1` of the first new output note (`H256.as_bytes()`).
    - `[4]`: The `commitment2` of the second new output note (`H256.as_bytes()`).

-----

## Genesis Configuration

You must provide the verification keys for the `deposit` and `transfer` circuits in your `chain_spec.rs` file. These
keys are generated off-chain from your compiled circuits.

**Example `chain_spec.rs`:**

```rust
// ...
use my_node_runtime::ConfidentialTransactionsConfig;

// ...
fn testnet_genesis(
    // ...
) -> GenesisConfig {
    GenesisConfig {
        // ...
        confidential_transactions: ConfidentialTransactionsConfig {
            deposit_vk: hex::decode("YOUR_DEPOSIT_VK_HEX_HERE").unwrap(),
            transfer_vk: hex::decode("YOUR_TRANSFER_VK_HEX_HERE").unwrap(),
            _phantom: Default::default(),
        },
        // ...
    }
}
```

-----

## Off-Chain Component

This pallet is the **on-chain verifier** and state manager for the confidential transaction system. It **requires a
corresponding off-chain component** (e.g., in a wallet or client-side application) that is responsible for:

1. Generating private keys for shielded notes.
2. Creating commitments and nullifiers.
3. Generating the zk-SNARK proofs for the `deposit`, `withdraw`, and `transact` operations.

The off-chain prover must use the exact same circuits that the on-chain verification keys were generated from.
