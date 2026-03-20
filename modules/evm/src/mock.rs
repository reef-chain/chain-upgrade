#![cfg(test)]

use super::*;

use frame_support::{construct_runtime, derive_impl, ord_parameter_types, parameter_types};
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use primitives::mocks::MockAddressMapping;
use primitives::{Amount, CurrencyId, TokenSymbol};
use sp_core::{H160, H256};
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    AccountId32, BuildStorage,
};
use std::{collections::BTreeMap, str::FromStr};
use frame_support::weights::Weight;

mod evm_mod {
    pub use super::super::*;
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

type Block = frame_system::mocking::MockBlock<Runtime>;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Runtime {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId32;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u64>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
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
    type Balance = u64;
    type DustRemoval = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = primitives::ReserveIdentifier;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxFreezes = frame_support::traits::VariantCountOf<RuntimeFreezeReason>;
    type DoneSlashHandler = ();
}

parameter_types! {
    pub const MinimumPeriod: u64 = 1000;
}
impl pallet_timestamp::Config for Runtime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: CurrencyId| -> u64 {
        Default::default()
    };
}

impl orml_tokens::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = u64;
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

impl module_currencies::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MultiCurrency = Tokens;
    type NativeCurrency = AdaptedBasicCurrency;
    type WeightInfo = ();
    type AddressMapping = MockAddressMapping;
    type EVMBridge = ();
}
pub type AdaptedBasicCurrency =
    module_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, u64>;

pub struct GasToWeight;

impl Convert<u64, Weight> for GasToWeight {
    fn convert(a: u64) -> Weight {
        Weight::from_parts(a, 0)
    }
}

parameter_types! {
    pub NetworkContractSource: H160 = alice();
}

ord_parameter_types! {
    pub const CouncilAccount: AccountId32 = AccountId32::from([1u8; 32]);
    pub const NetworkContractAccount: AccountId32 = AccountId32::from([0u8; 32]);
    pub const NewContractExtraBytes: u32 = 100;
    pub const StorageDepositPerByte: u64 = 10;
    pub const DeveloperDeposit: u64 = 1000;
    pub const DeploymentFee: u64 = 200;
    pub const MaxCodeSize: u32 = 1000;
    pub const ChainId: u64 = 1;
}

impl Config for Runtime {
    type AddressMapping = MockAddressMapping;
    type Currency = Balances;
    type TransferAll = Currencies;
    type NewContractExtraBytes = NewContractExtraBytes;
    type StorageDepositPerByte = StorageDepositPerByte;
    type MaxCodeSize = MaxCodeSize;

    type RuntimeEvent = RuntimeEvent;
    type Precompiles = ();
    type ChainId = ChainId;
    type GasToWeight = GasToWeight;
    type ChargeTransactionPayment = ();

    type NetworkContractOrigin = EnsureSignedBy<NetworkContractAccount, AccountId32>;
    type NetworkContractSource = NetworkContractSource;
    type DeveloperDeposit = DeveloperDeposit;
    type DeploymentFee = DeploymentFee;
    type FreeDeploymentOrigin = EnsureSignedBy<CouncilAccount, AccountId32>;

    type WeightInfo = ();
}

construct_runtime!(
    pub enum Runtime {
        System: frame_system,
        EVM: evm_mod,
        Tokens: orml_tokens,
        Balances: pallet_balances,
        Currencies: module_currencies,
    }
);

pub const INITIAL_BALANCE: u64 = 1_000_000_000_000;

pub fn contract_a() -> H160 {
    H160::from_str("2000000000000000000000000000000000000001").unwrap()
}

pub fn contract_b() -> H160 {
    H160::from_str("2000000000000000000000000000000000000002").unwrap()
}

pub fn alice() -> H160 {
    H160::from_str("1000000000000000000000000000000000000001").unwrap()
}

pub fn bob() -> H160 {
    H160::from_str("1000000000000000000000000000000000000002").unwrap()
}

pub fn charlie() -> H160 {
    H160::from_str("1000000000000000000000000000000000000003").unwrap()
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Runtime>::default()
        .build_storage()
        .unwrap();

    let mut accounts = BTreeMap::new();

    accounts.insert(
        contract_a(),
        GenesisAccount {
            nonce: 1,
            balance: Default::default(),
            storage: Default::default(),
            code: vec![
                0x00, // STOP
            ],
        },
    );
    accounts.insert(
        contract_b(),
        GenesisAccount {
            nonce: 1,
            balance: Default::default(),
            storage: Default::default(),
            code: vec![
                0xff, // INVALID
            ],
        },
    );

    accounts.insert(
        alice(),
        GenesisAccount {
            nonce: 1,
            balance: INITIAL_BALANCE,
            storage: Default::default(),
            code: Default::default(),
        },
    );
    accounts.insert(
        bob(),
        GenesisAccount {
            nonce: 1,
            balance: INITIAL_BALANCE,
            storage: Default::default(),
            code: Default::default(),
        },
    );

    pallet_balances::GenesisConfig::<Runtime>::default()
        .assimilate_storage(&mut t)
        .unwrap();
    evm_mod::GenesisConfig::<Runtime> { accounts }
        .assimilate_storage(&mut t)
        .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

pub fn balance(address: H160) -> u64 {
    let account_id = <Runtime as Config>::AddressMapping::get_account_id(&address);
    Balances::free_balance(account_id)
}

pub fn reserved_balance(address: H160) -> u64 {
    let account_id = <Runtime as Config>::AddressMapping::get_account_id(&address);
    Balances::reserved_balance(account_id)
}

pub fn deploy_free(contract: H160) {
    let _ = EVM::deploy_free(RuntimeOrigin::signed(CouncilAccount::get()), contract);
}
