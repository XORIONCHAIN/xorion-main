require("@nomicfoundation/hardhat-toolbox");
// Install dotenv: npm install dotenv
require('dotenv').config();

const SEPOLIA_RPC_URL = "process.env.SEPOLIA_RPC_URL";
const PRIVATE_KEY = process.env.PRIVATE_KEY || "your-wallet-private-key";
const ETHERSCAN_API_KEY = process.env.ETHERSCAN_API_KEY

module.exports = {
    solidity: "0.8.28",
    networks: {
        sepolia: {
            url: SEPOLIA_RPC_URL,
            accounts: [PRIVATE_KEY],
            chainId: 11155111,
        }
    },
    sourcify: {
        enabled: true
    },
    etherscan: {
        apiKey: ETHERSCAN_API_KEY,
    },
};
