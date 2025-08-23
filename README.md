# Xorion Network Connection Guide

This guide explains how to connect to the Xorion network using different node configurations. Whether you want to run a
full node, validator, light client, or RPC node, this documentation will help you get connected to the live network.

## Prerequisites

Ensure you have the required dependencies installed for your platform. Check
the [Polkadot installation guide](https://docs.polkadot.com/develop/parachains/install-polkadot-sdk/) for
platform-specific requirements.

## Getting Started

Clone the Xorion node repository:

```sh
git clone https://github.com/Kofituo/xorion-node.git
cd xorion-node
```

Build the node:

```sh
cargo build --release
```

## Network Connection Types

### 1. Full Node (Archive)

A full node stores the complete blockchain history and can serve historical data to other nodes and applications.

```sh
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-data \
  --blocks-pruning archive \
  --rpc-external \
  --rpc-methods safe \
  --rpc-port 9944 \
  --rpc-cors all
```

**Key Parameters:**

- `--chain xorion-testnet-spec.json`: Connects to the Xorion testnet
- `--blocks-pruning archive`: Keeps full blockchain history
- `--base-path ./xorion-data`: Data directory for blockchain storage
- `--rpc-external`: Allows external RPC connections
- `--rpc-cors all`: Allows cross-origin requests

### 2. Light Client

For light client functionality, use fast sync mode with minimal storage requirements:

```sh
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-light \
  --sync fast \
  --in-peers 16 \
  --out-peers 8 \
  --rpc-external \
  --rpc-methods safe \
  --rpc-port 9944
```

**Benefits:**

- Fast initial sync with `--sync fast`
- Lower resource requirements
- Reduced peer connections for efficiency

### 3. Validator Node

Validators participate in consensus and block production. **Note:** You need sufficient stake and nomination to become
an active validator.

**Validator Setup Process:**

1. **Generate Session Keys Locally (Recommended):**

   **Important:** The Xorion network requires three session keys in this specific order:
    - **BABE** (Sr25519) - for block authoring
    - **GRANDPA** (Ed25519) - for finality gadget
    - **Authority Discovery** (Sr25519) - for peer discovery

   ```sh
   # Generate keys locally in the required order
   # 1. BABE key (Sr25519)
   ./target/release/xorion-node key generate --scheme Sr25519 --keystore-path ./validator-keystore --key-type babe

   # 2. GRANDPA key (Ed25519)
   ./target/release/xorion-node key generate --scheme Ed25519 --keystore-path ./validator-keystore --key-type gran

   # 3. Authority Discovery key (Sr25519)
   ./target/release/xorion-node key generate --scheme Sr25519 --keystore-path ./validator-keystore --key-type audi
   ```

   Alternative: Generate via RPC:
   ```sh
   curl -H "Content-Type: application/json" \
        -d '{"id":1, "jsonrpc":"2.0", "method": "author_rotateKeys", "params":[]}' \
        http://localhost:9944
   ```

   **Note:** When using RPC generation, the returned hex string contains all three keys concatenated in the correct
   order (BABE + GRANDPA + Authority Discovery).


2. **Generate Node Key**

    ```sh
    ./target/release/xorion-node key generate-node-key --file node-key
    ```

   Use the generated `node-key` file and `validator-keystore` to start the validator node.

    ```sh
    ./target/release/xorion-node \
      --chain xorion-testnet-spec.json \
      --base-path ./xorion-validator \
      --validator \
      --name "YourValidatorName" \
      --blocks-pruning 10000 \
      --state-pruning 10000 \
      --node-key-file ./node-key \
      --keystore-path ./validator-keystore \
      --public-addr /ip4/YOUR_PUBLIC_IP/tcp/30334
    ```

3. **Bond Your Tokens:**
    - Go to Network > Staking > Account Actions in Polkadot-JS Apps
    - Click "+ Stash" to start bonding process
    - Choose your **Stash account** (holds the funds to be bonded)
    - Select your **Controller account** (can be same as stash or different)
    - Choose **Reward destination**:
        - **Stash account (increase amount at stake)**: Rewards compound automatically
        - **Controller account**: Rewards go to controller, not staked automatically
        - **Specified payment account**: Send rewards to any account you choose
    - Enter the amount to bond (leave some for transaction fees)
    - Submit the bonding transaction

2. **Set Session Keys:**
    - After bonding, you'll see options to either "Session Key" or "Nominate"
    - Click **"Session Key"**
    - Enter the session keys (the hex string from step 1)
    - Submit the transaction and wait for confirmation

3. **Start Validating:**
    - Once session keys are set correctly, the **"Validate"** option will appear
    - Click "Validate"
    - Set your **validator preferences**:
        - **Reward commission**: Percentage you keep from nominator rewards (0-100%)
        - **Blocked nominators**: Option to block specific nominators
    - Submit the validate transaction
    - Your validator will be active in the next era if you have enough stake

**Important Notes:**

- Your bonded funds will be **locked** for the unbonding period (typically 28 days on most networks)
- You need minimum stake to be in the active validator set
- Session keys should be kept secure and backed up safely

**Security Setup for Validators:**

```sh
# Validator node (private, no RPC)
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-validator \
  --validator \
  --name "YourValidatorName" \
  --node-key-file ./node-key \
  --keystore-path ./validator-keystore \ 
  --reserved-only \
  --reserved-nodes /ip4/SENTRY_IP/tcp/30333/p2p/SENTRY_PEER_ID
```

### 4. RPC Node

Optimized for serving RPC requests to applications and wallets.

```sh
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-rpc \
  --rpc-external \
  --unsafe-rpc-external \
  --rpc-methods unsafe \
  --rpc-port 9944 \
  --rpc-max-connections 1000 \
  --rpc-max-request-size 25 \
  --rpc-max-response-size 25 \
  --rpc-max-subscriptions-per-connection 4096 \
  --rpc-cors all
```

**Security Note:** Only use `--rpc-methods unsafe` and `--unsafe-rpc-external` on trusted networks with proper access
controls.

### 5. Archive Node

For applications requiring full historical data access:

```sh
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-archive \
  --blocks-pruning archive \
  --state-pruning archive \
  --rpc-external \
  --rpc-methods safe \
  --rpc-port 9944 \
  --db-cache 4096
```

### 6. Sentry Node

Sentry nodes protect validators by acting as intermediaries:

```sh
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-sentry \
  --name "SentryNode" \
  --rpc-external \
  --rpc-methods safe \
  --public-addr /ip4/YOUR_PUBLIC_IP/tcp/30333 \
  --reserved-nodes /ip4/VALIDATOR_PRIVATE_IP/tcp/30333/p2p/VALIDATOR_PEER_ID
```

## Network Configuration Options

### Boot Nodes

Connect to specific boot nodes for network discovery:

```sh
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-data \
  --bootnodes /ip4/1.2.3.4/tcp/30333/p2p/12D3KooW... \
  --bootnodes /ip4/5.6.7.8/tcp/30333/p2p/12D3KooW...
```

### Reserved Peers

For guaranteed connections to specific trusted nodes:

```sh
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-data \
  --reserved-nodes /ip4/1.2.3.4/tcp/30333/p2p/12D3KooW... \
  --reserved-only
```

### Network Ports and Addressing

Configure custom ports and addresses:

```sh
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-data \
  --port 30334 \
  --rpc-port 9945 \
  --listen-addr /ip4/0.0.0.0/tcp/30334 \
  --public-addr /ip4/YOUR_PUBLIC_IP/tcp/30334
```

## Advanced Configuration

### Sync Modes

Choose appropriate sync strategy:

```sh
# Full sync (default)
--sync full

# Fast sync (recommended for most users)
--sync fast

# Warp sync (fastest initial sync)
--sync warp
```

### Database Backend

Select database backend for optimal performance:

```sh
# RocksDB (default)
--database rocksdb

# ParityDB (experimental, better for SSDs)
--database paritydb
```

### Memory and Performance Tuning

```sh
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-data \
  --db-cache 2048 \
  --trie-cache-size 1073741824 \
  --max-runtime-instances 16 \
  --pool-limit 16384 \
  --pool-kbytes 40960
```

### Peer Connection Limits

```sh
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-data \
  --in-peers 64 \
  --out-peers 16 \
  --in-peers-light 200 \
  --max-parallel-downloads 10
```

## Monitoring and Telemetry

### Enable Telemetry

Send node metrics to telemetry servers:

```sh
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-data \
  --telemetry-url 'wss://telemetry.polkadot.io/submit/ 0' \
  --name "YourNodeName"
```

### Prometheus Metrics

Enable Prometheus endpoint for monitoring:

```sh
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-data \
  --prometheus-external \
  --prometheus-port 9615
```

### Detailed Logging

Configure logging for debugging:

```sh
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-data \
  --log runtime=debug,babe=trace \
  --detailed-log-output
```

## Connecting Applications

### Polkadot-JS Apps

1. Open [Polkadot-JS Apps](https://polkadot.js.org/apps/)
2. Click the network dropdown (top-left)
3. Select "Development" > "Local Node"
4. Change endpoint to `ws://localhost:9944` (or your custom port)
5. Click "Switch"

## Troubleshooting

### Common Issues and Solutions

1. **Port conflicts**: Ensure ports aren't already in use
   ```sh
   netstat -tulpn | grep :9944
   ```

2. **Permission issues**: Ensure proper directory permissions
   ```sh
   chmod -R 755 ./xorion-data
   ```

3. **Sync problems**: Clear database and resync
   ```sh
   ./target/release/xorion-node purge-chain \
     --chain xorion-testnet-spec.json \
     --base-path ./xorion-data
   ```

### Debug Mode

Run with debug logging for troubleshooting:

```sh
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --base-path ./xorion-data \
  --log debug \
  --detailed-log-output
```

## Security Best Practices

### Validator Security

1. **Firewall configuration**: Only allow necessary ports
2. **Key management**: Store session keys securely
3. **Monitoring**: Set up alerts for validator performance

### RPC Security

1. Use `--rpc-methods safe` for public endpoints
2. Implement rate limiting: `--rpc-rate-limit 100`
3. Whitelist trusted IPs: `--rpc-rate-limit-whitelisted-ips 192.168.1.0/24`
4. Use reverse proxy with authentication for sensitive endpoints

### Network Security

```sh
# Secure validator setup
./target/release/xorion-node \
  --chain xorion-testnet-spec.json \
  --validator \
  --base-path ./xorion-validator \
  --reserved-only \
  --reserved-nodes /ip4/SENTRY_IP/tcp/30333/p2p/SENTRY_PEER_ID \
  --no-private-ip
```

## Development and Testing

### Local Development Chain

```sh
./target/release/xorion-node \
  --dev \
  --tmp \
  --alice
```

### Multi-Node Local Testnet

```sh
# Node 1 (Alice)
./target/release/xorion-node \
  --chain=local \
  --alice \
  --tmp \
  --port 30333

# Node 2 (Bob)  
./target/release/xorion-node \
  --chain=local \
  --bob \
  --tmp \
  --port 30334 \
  --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/NODE1_PEER_ID
```

## Support and Resources

- **GitHub Issues**: [xorion-node/issues](https://github.com/Kofituo/xorion-node/issues)
- **Community**: [Polkadot Discord](https://discord.gg/c4VVaRVdKq)

For additional help with specific command options, use:

```sh
./target/release/xorion-node --help
./target/release/xorion-node <command> --help
```
