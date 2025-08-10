# Pallet-Bridge 🌉

A Substrate pallet that implements a bidirectional bridge for transferring assets between a Xorion chain and an
Ethereum-compatible chain. It uses a federated, multi-signature model where a set of trusted **relayers** validate
transfers between the two chains.

[](https://opensource.org/licenses/Apache-2.0)

## Overview

This pallet provides the fundamental on-chain logic for a "lock and mint" style bridge:

* **Xorion to Ethereum:** Users can **lock** native tokens on the Xorion chain. This action emits an event that relayers
  observe. The relayers then coordinate to mint or release corresponding tokens on the Ethereum side.
* **Ethereum to Xorion:** When tokens are locked on the Ethereum side, relayers sign the transaction details. A
  sufficient number of signatures allows a user to call **`release`** on this pallet, which transfers the equivalent
  native tokens from the bridge's account to the recipient.

The bridge is secured by a $K$-of-$N$ threshold of trusted relayers, where $K$ is the minimum number of signatures
required (`RelayerThreshold`) from a total of $N$ registered relayers.

-----

## ✨ Features

* **Token Locking:** Lock native Xorion tokens for transfer to an Ethereum recipient.
* **Token Releasing:** Release native tokens on Xorion after verifying an event from Ethereum.
* **Federated Security:** Utilizes a configurable set of trusted relayers with a multi-signature threshold ($K$-of-$N$)
  for approving releases.
* **Relayer Incentives:** Supports both per-transaction relayer fees and a global `RelayerFund` to compensate relayers
  for their operational costs.
* **Replay Protection:** Ensures that each cross-chain message can only be processed once.
* **Admin Controls:** Includes root-level controls to manage the relayer set, pause bridge operations in an emergency,
  and withdraw funds if necessary.

-----

## ⚙️ Workflow

The bridge facilitates transfers in two directions.

### 1\. Xorion ➡️ Ethereum

1. **Lock Funds:** A user calls the `lock()` extrinsic, specifying the `amount` of native tokens, their `eth_recipient`
   address on Ethereum, and an optional `relayer_fee`.
2. **Transfer & Event:** The pallet transfers the total amount from the user's account to the pallet's sovereign
   account. It then emits a `Locked` event containing all necessary details, including a unique `message_id`.
3. **Relayer Action:** Off-chain relayer nodes observe the `Locked` event. They use this information to submit a
   transaction to the corresponding smart contract on Ethereum, which then releases tokens to the `eth_recipient`.

### 2\. Ethereum ➡️ Xorion

1. **Lock on Ethereum:** A user interacts with a smart contract on Ethereum to lock tokens, specifying a recipient
   address on the Xorion chain. The Ethereum contract emits an event with a unique `message_id`.
2. **Relayers Sign:** Off-chain relayers observe this Ethereum event. Each relayer signs the `message_id` with their
   Ethereum private key.
3. **Gather Signatures:** One entity (usually one of the relayers, known as the "submitter") gathers at least
   `RelayerThreshold` valid signatures.
4. **Release Funds:** The submitter calls the `release()` extrinsic on this pallet, providing the `message_id`,
   `recipient`, `amount`, and the collected `signatures`.
5. **Verification & Payout:** The pallet performs the following checks:
    * Verifies that the message has not been processed before.
    * Recovers the signer's Ethereum address from each signature and confirms they are in the trusted `Relayers` list.
    * Ensures the number of valid, unique signatures meets the `RelayerThreshold`.
    * If all checks pass, it transfers the `amount` from its sovereign account to the `recipient`.
6. **Reimburse Relayer:** The pallet pays a reward to the `submitter`. It first checks if the original lock
   transaction (from Xorion -\> ETH) had a `relayer_fee`. If not, it pays out from the global `RelayerFund`.

-----

## pallet Components

### Configurable Parameters

* `Currency`: The currency type for handling balances (e.g., `pallet-balances`).
* `BridgePalletId`: A `PalletId` used to derive the sovereign account that holds all locked funds.
* `RelayerThreshold`: The minimum number of relayer signatures ($K$) required to approve a `release` transaction.
* `MaxSignatures`: The maximum number of signatures that can be included in a `release` call, used to bound transaction
  weight.

### Dispatchable Functions

#### User Functions

* `lock(amount, relayer_fee, eth_recipient, nonce)`: Locks native tokens to be bridged to Ethereum.

#### Relayer Functions

* `release(message_id, recipient, amount, signatures, max_relayer_reward)`: Releases tokens on Xorion after verifying
  relayer signatures for a message from Ethereum.

#### Admin (Root) Functions

* `set_relayers(relayers)`: Sets or updates the list of trusted relayer Ethereum addresses (`H160`).
* `set_paused(paused)`: Pauses or unpauses all bridge operations.
* `emergency_withdraw(to, amount)`: Withdraws funds from the pallet's sovereign account to a specified address. Useful
  for emergencies or upgrades.
* `top_up_relayer_fund(amount)`: Allows anyone (but typically an admin) to add funds to the global relayer incentive
  pool.

### Storage

* `Relayers`: `BoundedVec<H160, ...>` - The list of trusted relayer Ethereum addresses.
* `LockedMessages`: `StorageMap<[u8; 32], LockedInfo>` - Stores details of funds locked on Xorion that are pending
  release on Ethereum.
* `ProcessedMessages`: `StorageMap<[u8; 32], bool>` - A record of processed message IDs from Ethereum to prevent replay
  attacks.
* `RelayerFund`: `BalanceOf<T>` - A global fund to reward relayers when a specific transaction does not include a fee.
* `Paused`: `bool` - A flag to halt all bridge activity.

### Events

* `Locked`: Emitted when a user successfully locks funds.
* `Released`: Emitted when funds are successfully released to a recipient on Xorion.
* `RelayerReimbursed`: Emitted when a relayer is paid for submitting a successful `release` transaction.
* `RelayersUpdated`: Emitted when the admin changes the set of relayers.
* `PausedSet`: Emitted when the bridge is paused or unpaused.