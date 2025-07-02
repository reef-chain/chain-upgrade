use crate::Balance;
use codec::{Decode, Encode};
use module_evm_utility::evm::ExitReason;
use scale_info::TypeInfo;
use sp_core::{H160, U256};
use sp_runtime::RuntimeDebug;
use module_evm_utility::ethereum::AccessListItem;
use sp_std::vec::Vec;

pub use module_evm_utility::evm::backend::{Basic as Account, Log};
pub use module_evm_utility::evm::Config;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

// #[derive(
//     Eq, Serialize, Deserialize, PartialEq, PartialOrd, Default, Ord, Copy, Clone, Debug, TypeInfo,
// )]
// pub struct CodecH160(pub H160);

// impl Encode for CodecH160 {
//     fn encode(&self) -> Vec<u8> {
//         self.0.as_bytes().encode()
//     }
// }

// impl Decode for CodecH160 {
//     fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
//         let bytes: [u8; 20] = Decode::decode(input)?;
//         Ok(CodecH160(H160::from(bytes)))
//     }
// }

// #[derive(Eq, Serialize,Deserialize,PartialEq,PartialOrd,Default, Ord,Copy, Clone, Debug)]
// pub struct U256Wrapper(pub U256);

// impl Encode for U256Wrapper {
//     fn encode(&self) -> Vec<u8> {
//         let mut bytes = [0u8; 32];
//         self.0.to_big_endian(&mut bytes);
//         bytes.encode()
//     }
// }

// impl Decode for U256Wrapper {
//     fn decode<I: codec::Input>(
//         input: &mut I,
//     ) -> Result<Self, codec::Error> {
//         let bytes: [u8; 32] = Decode::decode(input)?;
//         Ok(U256Wrapper(U256::from_big_endian(&bytes)))
//     }
// }

/// Evm Address.
pub type EvmAddress = H160;

#[derive(Clone, Eq, PartialEq, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
/// External input from the transaction.
pub struct Vicinity {
    /// Current transaction gas price.
    pub gas_price: U256,
    /// Origin of the transaction.
    pub origin: EvmAddress,
    // Envirnomental base fee per gas
    pub block_base_fee_per_gas: Option<U256>,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CreateInfo {
    pub exit_reason: ExitReason,
    pub address: EvmAddress,
    pub output: Vec<u8>,
    pub used_gas: U256,
    pub used_storage: i32,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CallInfo {
    pub exit_reason: ExitReason,
    pub output: Vec<u8>,
    pub used_gas: U256,
    pub used_storage: i32,
}
/// A mapping between `AccountId` and `EvmAddress`.
pub trait AddressMapping<AccountId> {
    fn get_account_id(evm: &EvmAddress) -> AccountId;
    fn get_evm_address(account_id: &AccountId) -> Option<EvmAddress>;
    fn get_or_create_evm_address(account_id: &AccountId) -> EvmAddress;
    fn get_default_evm_address(account_id: &AccountId) -> EvmAddress;
    fn is_linked(account_id: &AccountId, evm: &EvmAddress) -> bool;
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct EstimateResourcesRequest {
    /// From
    pub from: Option<H160>,
    /// To
    pub to: Option<H160>,
    /// Gas Limit
    pub gas_limit: Option<u64>,
    /// Storage Limit
    pub storage_limit: Option<u32>,
    /// Value
    pub value: Option<Balance>,
    /// Data
    pub data: Option<Vec<u8>>,
    /// AccessList
	pub access_list: Option<Vec<AccessListItem>>,
}
