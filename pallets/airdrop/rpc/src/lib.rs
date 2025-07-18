use codec::Codec;
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::{ErrorCode, ErrorObject},
};
use pallet_airdrop_rpc_api::AirdropApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::{Block as BlockT, Zero};
use std::sync::Arc;

// Airdrop record structure for RPC responses
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AirdropRecord<BlockNumber, Balance> {
    pub claims_count: u32,
    pub last_claim_block: BlockNumber,
    pub total_received: Balance,
}

// RPC trait definition using jsonrpsee
#[rpc(client, server)]
pub trait AirdropRpc<BlockHash, AccountId, Balance, BlockNumber> {
    /// Check if an account is eligible for airdrop
    #[method(name = "airdrop_isEligibleForAirdrop")]
    async fn is_eligible_for_airdrop(
        &self,
        who: AccountId,
        at: Option<BlockHash>,
    ) -> RpcResult<bool>;

    /// Get the remaining cooldown blocks for an account
    #[method(name = "airdrop_getCooldownRemaining")]
    async fn get_cooldown_remaining(
        &self,
        who: AccountId,
        at: Option<BlockHash>,
    ) -> RpcResult<BlockNumber>;

    /// Get airdrop pool balance
    #[method(name = "airdrop_getAirdropPoolBalance")]
    async fn get_airdrop_pool_balance(&self, at: Option<BlockHash>) -> RpcResult<Balance>;
}

// Comprehensive airdrop status structure
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AirdropStatus<BlockNumber, Balance> {
    pub is_eligible: bool,
    pub cooldown_remaining: BlockNumber,
    pub record: Option<AirdropRecord<BlockNumber, Balance>>,
    pub pool_balance: Balance,
    pub airdrops_this_block: u32,
}

// RPC implementation
pub struct AirdropRpcImpl<C, Block> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<Block>,
}

impl<C, Block> AirdropRpcImpl<C, Block> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

#[async_trait]
impl<C, Block, AccountId, Balance, BlockNumber>
    AirdropRpcServer<Block::Hash, AccountId, Balance, BlockNumber> for AirdropRpcImpl<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: AirdropApi<Block, AccountId, Balance, BlockNumber>,
    AccountId: Clone + std::fmt::Display + Codec + Send + Sync + 'static,
    Balance: Clone + std::fmt::Display + Codec + Send + Sync + 'static + Zero,
    BlockNumber: Clone + std::fmt::Display + Codec + Send + Sync + 'static + Zero,
{
    async fn is_eligible_for_airdrop(
        &self,
        who: AccountId,
        at: Option<Block::Hash>,
    ) -> RpcResult<bool> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        api.is_eligible_for_airdrop(at_hash, who).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Failed to check airdrop eligibility",
                Some(e.to_string()),
            )
        })
    }

    async fn get_cooldown_remaining(
        &self,
        who: AccountId,
        at: Option<Block::Hash>,
    ) -> RpcResult<BlockNumber> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        api.get_cooldown_remaining(at_hash, who).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Failed to get cooldown remaining",
                Some(e.to_string()),
            )
        })
    }

    async fn get_airdrop_pool_balance(&self, at: Option<Block::Hash>) -> RpcResult<Balance> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        api.get_airdrop_pool_balance(at_hash).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Failed to get airdrop pool balance",
                Some(e.to_string()),
            )
        })
    }
}
