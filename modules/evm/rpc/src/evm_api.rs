//! EVM rpc interface.

use crate::call_request::{CallRequest, EstimateResourcesResponse};
use ethereum_types::{H160, U256};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use sp_core::Bytes;

/// EVM rpc interface.
#[rpc(client, server)]
pub trait EVMApi<BlockHash> {
    /// Call contract, returning the output data.
    #[method(name = "evm_call")]
    fn call(&self, call_request: CallRequest, at: Option<BlockHash>) -> RpcResult<Bytes>;

    /// Estimate gas needed for execution of given contract.
    #[method(name = "evm_estimateGas")]
    fn estimate_gas(&self, call_request: CallRequest, at: Option<BlockHash>) -> RpcResult<U256>;

    /// Estimate resources needed for execution of given contract.
    #[method(name = "evm_estimateResources")]
    fn estimate_resources(
        &self,
        from: H160,
        unsigned_extrinsic: Bytes,
        at: Option<BlockHash>,
    ) -> RpcResult<EstimateResourcesResponse>;
}
