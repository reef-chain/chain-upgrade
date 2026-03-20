//! Mocks for the evm-accounts module.

#![cfg(test)]

use super::*;
use frame_support::{construct_runtime, derive_impl, parameter_types};
use orml_traits::parameter_type_with_key;
use primitives::{Amount, Balance, CurrencyId, TokenSymbol};
use sp_core::{crypto::AccountId32, H256};
use sp_io::hashing::keccak_256;
use sp_runtime::{
    traits::IdentityLookup,
    BuildStorage,
};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;

pub const ALICE: AccountId = AccountId32::new([0u8; 32]);
pub const BOB: AccountId = AccountId32::new([1u8; 32]);

mod evm_accounts {
    pub use super::super::*;
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

type Block = frame_system::mocking::MockBlock<Runtime>;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Hash = H256;
    type Hashing = ::sp_runtime::traits::BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type BlockHashCount = BlockHashCount;
    type BlockWeights = ();
    type BlockLength = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type DbWeight = ();
    type BaseCallFilter = frame_support::traits::Everything;
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type Block = Block;
}

parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = frame_system::Pallet<Runtime>;
    type MaxLocks = MaxLocks;
    type WeightInfo = ();
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = primitives::ReserveIdentifier;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxFreezes = frame_support::traits::VariantCountOf<RuntimeFreezeReason>;
    type DoneSlashHandler = ();
}

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
        Default::default()
    };
}

impl orml_tokens::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = CurrencyId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type CurrencyHooks = ();
    type DustRemovalWhitelist = ();
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = primitives::ReserveIdentifier;
}

parameter_types! {
    pub const GetNativeCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::REEF);
}

impl orml_currencies::Config for Runtime {
    type MultiCurrency = Tokens;
    type NativeCurrency = AdaptedBasicCurrency;
    type GetNativeCurrencyId = GetNativeCurrencyId;
    type WeightInfo = ();
}
pub type AdaptedBasicCurrency =
    orml_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

pub struct EvmAccountsOnClaimHandler;
impl evm_accounts::Handler<AccountId> for EvmAccountsOnClaimHandler {
    fn handle(_who: &AccountId) -> DispatchResult {
        Ok(())
    }
}

impl Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type AddressMapping = EvmAddressMapping<Runtime>;
    type TransferAll = Currencies;
    type OnClaim = EvmAccountsOnClaimHandler;
    type WeightInfo = ();
}

construct_runtime!(
    pub enum Runtime {
        System: frame_system,
        EvmAccountsModule: evm_accounts,
        Tokens: orml_tokens,
        Balances: pallet_balances,
        Currencies: orml_currencies,
    }
);

pub struct ExtBuilder();

impl Default for ExtBuilder {
    fn default() -> Self {
        Self()
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::<Runtime>::default()
            .build_storage()
            .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![(bob_account_id(), 100000)],
            dev_accounts: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| System::set_block_number(1));
        ext
    }
}

pub fn alice() -> secp256k1::SecretKey {
    secp256k1::SecretKey::parse(&keccak_256(b"Alice")).unwrap()
}

pub fn bob() -> secp256k1::SecretKey {
    secp256k1::SecretKey::parse(&keccak_256(b"Bob")).unwrap()
}

pub fn bob_account_id() -> AccountId {
    let address = EvmAccountsModule::eth_address(&bob());
    let mut data = [0u8; 32];
    data[0..4].copy_from_slice(b"evm:");
    data[4..24].copy_from_slice(&address[..]);
    AccountId32::from(Into::<[u8; 32]>::into(data))
}
