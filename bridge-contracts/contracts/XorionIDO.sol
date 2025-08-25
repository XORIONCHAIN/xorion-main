// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * USDT Locker for cross-chain claiming.
 *
 * Flow:
 * 1) User approves this contract to spend USDT.
 * 2) User calls deposit(amount, xorionRecipient).
 * 3) Contract pulls USDT and emits Locked(depositId, sender, amount, xorionRecipient).
 * 4) Relayers watch for Locked events and call the Xorion pallet accordingly.
 *
 * Notes:
 * - `xorionRecipient` is an opaque blob (e.g., SS58-decoded 32-byte AccountId, or any payload you standardize).
 * - Owner can pause/unpause, and (optionally) emergency withdraw.
 */

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";
import {Pausable} from "@openzeppelin/contracts/utils/Pausable.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

contract XorionIDO is Ownable, Pausable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    /// @dev USDT token contract (Ethereum mainnet address may be passed in).
    IERC20 public immutable usdt;

    /// @dev Auto-incrementing id for each deposit; starts at 1 for readability.
    uint256 public nextDepositId = 1;

    /// @dev Accounting (per user total locked). Useful for audits/refunds.
    mapping(address => uint256) public lockedOf;
    uint256 public totalLocked;

    /// @dev Emitted on every successful lock.
    event Locked(
        uint256 indexed depositId,
        address indexed sender,
        uint256 amount,
        bytes xorionRecipient
    );

    /// @dev Emitted if owner performs a withdrawal.
    event Withdrawal(address indexed to, uint256 amount);

    /// @param usdtAddress USDT token contract address (mainnet: 0xdAC17F958D2ee523a2206206994597C13D831ec7)
    /// @param initialOwner contract owner (can pause/unpause and withdraw if needed)
    constructor(address usdtAddress, address initialOwner) Ownable(initialOwner) {
        require(usdtAddress != address(0), "usdt addr zero");
        usdt = IERC20(usdtAddress);
    }

    /**
     * @notice Lock USDT for a Xorion recipient.
     * @param amount Amount of USDT in smallest units (USDT has 6 decimals).
     * @param xorionRecipient Opaque bytes identifying recipient on Xorion (e.g., 32-byte AccountId).
     */
    function deposit(uint256 amount, bytes calldata xorionRecipient) external nonReentrant whenNotPaused {
        require(amount > 0, "amount=0");
        // Pull USDT from sender; SafeERC20 handles non-standard returns (USDT-safe).
        usdt.safeTransferFrom(msg.sender, address(this), amount);

        // Accounting
        lockedOf[msg.sender] += amount;
        totalLocked += amount;

        // Emit event for relayers
        emit Locked(nextDepositId, msg.sender, amount, xorionRecipient);
        unchecked {
            ++nextDepositId;
        }
    }

    /* ------------------------- Admin controls ------------------------- */

    function pause() external onlyOwner {_pause();}

    function unpause() external onlyOwner {_unpause();}

    /**
     * @notice Withdraw funds to address
     */
    function withdraw(address to, uint256 amount) external onlyOwner nonReentrant {
        require(to != address(0), "to=0");
        usdt.safeTransfer(to, amount);
        emit Withdrawal(to, amount);
    }

    /* ---------------------------- Views ------------------------------- */

    function usdtBalance() external view returns (uint256) {
        return usdt.balanceOf(address(this));
    }
}
