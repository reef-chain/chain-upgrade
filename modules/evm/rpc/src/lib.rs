#![allow(clippy::upper_case_acronyms)]

pub use crate::evm_api::EVMApiServer;
use ethereum_types::{H160, U256};
use jsonrpsee::core::RpcResult;
use jsonrpsee::types::error::{ErrorCode, ErrorObjectOwned};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::{Bytes, Decode};
use sp_rpc::number::NumberOrHex;
use sp_runtime::{
    codec::Codec,
    traits::{self, Block as BlockT, MaybeDisplay, MaybeFromStr},
};
use std::convert::{TryFrom, TryInto};
use std::{marker::PhantomData, sync::Arc};

use call_request::{CallRequest, EstimateResourcesResponse};
pub use module_evm::{AddressMapping, ExitError, ExitReason};
pub use module_evm_rpc_runtime_api::EVMRuntimeRPCApi;

pub use pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi;

mod call_request;
pub mod evm_api;

// default gas and storage limits:
// limits only apply to call() API
// create() API takes values from the caller and is unlimited
pub const GAS_LIMIT: u64 = 100_000_000;
pub const STORAGE_LIMIT: u32 = 1_000_000;

pub fn err<T: ToString>(
    code: i32,
    message: T,
    data: Option<&[u8]>,
) -> jsonrpsee::types::error::ErrorObjectOwned {
    jsonrpsee::types::error::ErrorObject::owned(
        code,
        message.to_string(),
        data.map(|bytes| {
            jsonrpsee::core::to_json_raw_value(&format!("0x{}", hex::encode(bytes)))
                .expect("fail to serialize data")
        }),
    )
}

pub fn internal_err<T: ToString>(message: T) -> jsonrpsee::types::error::ErrorObjectOwned {
    err(jsonrpsee::types::error::INTERNAL_ERROR_CODE, message, None)
}

fn invalid_params<T: ToString>(message: T) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        ErrorCode::InvalidParams.code(),
        message.to_string(),
        None::<()>,
    )
}

pub fn internal_err_with_data<T: ToString>(
    message: T,
    data: &[u8],
) -> jsonrpsee::types::error::ErrorObjectOwned {
    err(
        jsonrpsee::types::error::INTERNAL_ERROR_CODE,
        message,
        Some(data),
    )
}

#[allow(dead_code)]
fn error_on_execution_failure(reason: &ExitReason, data: &[u8]) -> RpcResult<()> {
    match reason {
        ExitReason::Succeed(_) => Ok(()),
        ExitReason::Error(err) => {
            if *err == ExitError::OutOfGas {
                // `ServerError(0)` will be useful in estimate gas
                return Err(internal_err("out of gas"));
            } else {
                Err(internal_err_with_data(format!("evm error: {err:?}"), &[]))
            }
        }
        ExitReason::Revert(_) => {
            let message = "VM Exception while processing transaction: execution revert".to_string();
            Err(crate::internal_err_with_data(message, data))
        }
        ExitReason::Fatal(err) => Err(crate::internal_err_with_data(
            format!("evm fatal: {err:?}"),
            &[],
        )),
    }
}

pub struct EVM<B, C, Balance> {
    client: Arc<C>,
    _marker: PhantomData<(B, Balance)>,
}

impl<B, C, Balance> EVM<B, C, Balance> {
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

fn to_u128(val: NumberOrHex) -> std::result::Result<u128, ()> {
    val.into_u256().try_into().map_err(|_| ())
}

impl<B, C, Balance> EVMApiServer<<B as BlockT>::Hash> for EVM<B, C, Balance>
where
    B: BlockT,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: EVMRuntimeRPCApi<B, Balance>,
    C::Api: pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<B, Balance>,
    Balance: Codec
        + MaybeDisplay
        + MaybeFromStr
        + Default
        + Send
        + Sync
        + 'static
        + TryFrom<u128>
        + Into<U256>,
{
    fn call(&self, request: CallRequest, at: Option<<B as BlockT>::Hash>) -> RpcResult<Bytes> {
        let hash = match at {
            Some(hash) => hash,
            None => self.client.info().best_hash,
        };

        let CallRequest {
            from,
            to,
            gas_limit,
            storage_limit,
            value,
            data,
        } = request;

        let gas_limit = gas_limit.unwrap_or(GAS_LIMIT).min(GAS_LIMIT);
        let storage_limit = storage_limit.unwrap_or(STORAGE_LIMIT).min(STORAGE_LIMIT);
        let data = data.map(|d| d.0).unwrap_or_default();

        let api = self.client.runtime_api();

        let balance_value = if let Some(value) = value {
            to_u128(value).and_then(|v| TryInto::<Balance>::try_into(v).map_err(|_| ()))
        } else {
            Ok(Default::default())
        };

        let balance_value = balance_value
            .map_err(|_| invalid_params(format!("Invalid parameter value: {:?}", value)))?;

        match to {
            Some(to) => {
                let info = api
                    .call(
                        hash,
                        from.unwrap_or_default(),
                        to,
                        data,
                        balance_value,
                        gas_limit,
                        storage_limit,
                        false,
                    )
                    .map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
                    .map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

                error_on_execution_failure(&info.exit_reason, &info.output)?;

                Ok(Bytes(info.output))
            }
            None => {
                let info = api
                    .create(
                        hash,
                        from.unwrap_or_default(),
                        data,
                        balance_value,
                        gas_limit,
                        storage_limit,
                        false,
                    )
                    .map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
                    .map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

                error_on_execution_failure(&info.exit_reason, &info.output)?;

                Ok(Bytes(info.output[..].to_vec()))
            }
        }
    }

    fn estimate_gas(
        &self,
        request: CallRequest,
        at: Option<<B as BlockT>::Hash>,
    ) -> RpcResult<U256> {
        let hash = match at {
            Some(hash) => hash,
            None => self.client.info().best_hash,
        };

        let calculate_gas_used = |request| {
            let CallRequest {
                from,
                to,
                gas_limit,
                storage_limit,
                value,
                data,
            } = request;

            let gas_limit = gas_limit.unwrap_or(GAS_LIMIT).min(GAS_LIMIT);
            let storage_limit = storage_limit.unwrap_or(STORAGE_LIMIT).min(STORAGE_LIMIT);
            let data = data.map(|d| d.0).unwrap_or_default();

            let balance_value = if let Some(value) = value {
                to_u128(value).and_then(|v| TryInto::<Balance>::try_into(v).map_err(|_| ()))
            } else {
                Ok(Default::default())
            };

            let balance_value = balance_value
                .map_err(|_| invalid_params(format!("Invalid parameter value: {:?}", value)))?;

            let used_gas = match to {
                Some(to) => {
                    let info = self
                        .client
                        .runtime_api()
                        .call(
                            hash,
                            from.unwrap_or_default(),
                            to,
                            data,
                            balance_value,
                            gas_limit,
                            storage_limit,
                            true,
                        )
                        .map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
                        .map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

                    error_on_execution_failure(&info.exit_reason, &info.output)?;

                    info.used_gas
                }
                None => {
                    let info = self
                        .client
                        .runtime_api()
                        .create(
                            hash,
                            from.unwrap_or_default(),
                            data,
                            balance_value,
                            gas_limit,
                            storage_limit,
                            true,
                        )
                        .map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
                        .map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

                    error_on_execution_failure(&info.exit_reason, &[])?;

                    info.used_gas
                }
            };

            Ok(used_gas)
        };

        if cfg!(feature = "rpc_binary_search_estimate") {
            let mut lower = U256::from(21_000);
            // get a good upper limit, but below U64::max to operation overflow
            let mut upper = U256::from(GAS_LIMIT);
            let mut mid = upper;
            let mut best = mid;
            let mut old_best: U256;

            // if the gas estimation depends on the gas limit, then we want to binary
            // search until the change is under some threshold. but if not dependent,
            // we want to stop immediately.
            let mut change_pct = U256::from(100);
            let threshold_pct = U256::from(10);

            // invariant: lower <= mid <= upper
            while change_pct > threshold_pct {
                let mut test_request = request.clone();
                test_request.gas_limit = Some(mid.as_u64());
                match calculate_gas_used(test_request) {
                    // if Ok -- try to reduce the gas used
                    Ok(used_gas) => {
                        old_best = best;
                        best = used_gas;
                        change_pct = (U256::from(100) * (old_best - best))
                            .checked_div(old_best)
                            .unwrap_or_default();
                        upper = mid;
                        mid = (lower + upper + 1) / 2;
                    }

                    // if Err -- we need more gas
                    Err(_) => {
                        lower = mid;
                        mid = (lower + upper + 1) / 2;

                        // exit the loop
                        if mid == lower {
                            break;
                        }
                    }
                }
            }
            Ok(best)
        } else {
            calculate_gas_used(request)
        }
    }

    fn estimate_resources(
        &self,
        from: H160,
        unsigned_extrinsic: Bytes,
        at: Option<<B as BlockT>::Hash>,
    ) -> RpcResult<EstimateResourcesResponse> {
        let hash = match at {
            Some(hash) => hash,
            None => self.client.info().best_hash,
        };

        let request = self
            .client
            .runtime_api()
            .get_estimate_resources_request(hash, unsigned_extrinsic.to_vec())
            .map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
            .map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

        let request = CallRequest {
            from: Some(from),
            to: request.to,
            gas_limit: request.gas_limit,
            storage_limit: request.storage_limit,
            value: request.value.map(|v| NumberOrHex::Hex(U256::from(v))),
            data: request.data.map(Bytes),
        };

        let calculate_gas_used = |request| -> RpcResult<(U256, i32)> {
            let CallRequest {
                from,
                to,
                gas_limit,
                storage_limit,
                value,
                data,
            } = request;

            let gas_limit = gas_limit.unwrap_or(GAS_LIMIT).min(GAS_LIMIT);
            let storage_limit = storage_limit.unwrap_or(STORAGE_LIMIT).min(STORAGE_LIMIT);
            let data = data.map(|d| d.0).unwrap_or_default();

            let balance_value = if let Some(value) = value {
                to_u128(value).and_then(|v| TryInto::<Balance>::try_into(v).map_err(|_| ()))
            } else {
                Ok(Default::default())
            };

            let balance_value = balance_value
                .map_err(|_| invalid_params(format!("Invalid parameter value: {:?}", value)))?;

            let (used_gas, used_storage) = match to {
                Some(to) => {
                    let info = self
                        .client
                        .runtime_api()
                        .call(
                            hash,
                            from.unwrap_or_default(),
                            to,
                            data,
                            balance_value,
                            gas_limit,
                            storage_limit,
                            true,
                        )
                        .map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
                        .map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

                    error_on_execution_failure(&info.exit_reason, &info.output)?;

                    (info.used_gas, info.used_storage)
                }
                None => {
                    let info = self
                        .client
                        .runtime_api()
                        .create(
                            hash,
                            from.unwrap_or_default(),
                            data,
                            balance_value,
                            gas_limit,
                            storage_limit,
                            true,
                        )
                        .map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
                        .map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

                    error_on_execution_failure(&info.exit_reason, &[])?;

                    (info.used_gas, info.used_storage)
                }
            };

            Ok((used_gas, used_storage))
        };

        if cfg!(feature = "rpc_binary_search_estimate") {
            let mut lower = U256::from(21_000);
            // get a good upper limit, but below U64::max to operation overflow
            let mut upper = U256::from(GAS_LIMIT);
            let mut mid = upper;
            let mut best = mid;
            let mut old_best: U256;
            let mut storage: i32 = Default::default();

            // if the gas estimation depends on the gas limit, then we want to binary
            // search until the change is under some threshold. but if not dependent,
            // we want to stop immediately.
            let mut change_pct = U256::from(100);
            let threshold_pct = U256::from(10);

            // invariant: lower <= mid <= upper
            while change_pct > threshold_pct {
                let mut test_request = request.clone();
                test_request.gas_limit = Some(mid.as_u64());
                match calculate_gas_used(test_request) {
                    // if Ok -- try to reduce the gas used
                    Ok((used_gas, used_storage)) => {
                        log::debug!(
                            target: "evm",
                            "calculate_gas_used ok, used_gas: {:?}, used_storage: {:?}",
                            used_gas, used_storage,
                        );

                        old_best = best;
                        best = used_gas;
                        change_pct = (U256::from(100) * (old_best - best))
                            .checked_div(old_best)
                            .unwrap_or_default();
                        upper = mid;
                        mid = (lower + upper + 1) / 2;
                        storage = used_storage;
                    }

                    Err(err) => {
                        log::debug!(
                            target: "evm",
                            "calculate_gas_used err, lower: {:?}, upper: {:?}, mid: {:?}",
                            lower, upper, mid
                        );

                        // if Err == OutofGas or OutofFund, we need more gas
                        if err.code() == ErrorCode::ServerError(0).code() {
                            lower = mid;
                            mid = (lower + upper + 1) / 2;
                            if mid == lower {
                                break;
                            }
                        }

                        // Other errors, return directly
                        return Err(err);
                    }
                }
            }

            let uxt: <B as traits::Block>::Extrinsic = Decode::decode(&mut &*unsigned_extrinsic)
                .map_err(|e| {
                    internal_err(format!(
                        "execution error: Unable to dry run extrinsic {:?}",
                        e
                    ))
                })?;

            let fee = self
                .client
                .runtime_api()
                .query_fee_details(hash, uxt, unsigned_extrinsic.len() as u32)
                .map_err(|e| {
                    internal_err(format!(
                        "runtime error: Unable to query fee details {:?}",
                        e
                    ))
                })?;

            let adjusted_weight_fee = fee
                .inclusion_fee
                .map_or_else(Default::default, |inclusion| inclusion.adjusted_weight_fee);

            Ok(EstimateResourcesResponse {
                gas: best,
                storage,
                weight_fee: adjusted_weight_fee.into(),
            })
        } else {
            let (used_gas, used_storage) = calculate_gas_used(request)?;

            let uxt: <B as BlockT>::Extrinsic =
                Decode::decode(&mut &*unsigned_extrinsic).map_err(|e| {
                    internal_err(format!(
                        "execution error: Unable to dry run extrinsic {:?}",
                        e
                    ))
                })?;

            let fee = self
                .client
                .runtime_api()
                .query_fee_details(hash, uxt, unsigned_extrinsic.len() as u32)
                .map_err(|e| {
                    internal_err(format!(
                        "runtime error: Unable to query fee details {:?}",
                        e
                    ))
                })?;

            let adjusted_weight_fee = fee
                .inclusion_fee
                .map_or_else(Default::default, |inclusion| inclusion.adjusted_weight_fee);

            Ok(EstimateResourcesResponse {
                gas: used_gas,
                storage: used_storage,
                weight_fee: adjusted_weight_fee.into(),
            })
        }
    }
}

#[test]
fn decode_revert_message_should_work() {
    use sp_core::bytes::from_hex;
    assert_eq!(decode_revert_message(&vec![]), None);

    let data =
        from_hex("0x8c379a00000000000000000000000000000000000000000000000000000000000000020")
            .unwrap();
    assert_eq!(decode_revert_message(&data), None);

    let data = from_hex("0x8c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000d6572726f72206d65737361676").unwrap();
    assert_eq!(decode_revert_message(&data), None);

    let data = from_hex("0x8c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000d6572726f72206d65737361676500000000000000000000000000000000000000").unwrap();
    assert_eq!(decode_revert_message(&data), Some("error message".into()));
}
