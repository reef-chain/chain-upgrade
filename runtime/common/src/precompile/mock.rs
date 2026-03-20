#![cfg(test)]

use crate::{AllPrecompiles, BlockWeights, SystemContractsFilter};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
    assert_ok, derive_impl, ord_parameter_types, parameter_types,
    traits::InstanceFilter,
    weights::IdentityFee,
    RuntimeDebug,
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use orml_traits::parameter_type_with_key;
pub use primitives::{
    evm::AddressMapping, mocks::MockAddressMapping, Amount, CurrencyId,
    TokenSymbol,
};
use sp_core::{bytes::from_hex, crypto::AccountId32, Bytes, H160, H256};
use sp_runtime::{
    traits::{BlakeTwo256, Convert, IdentityLookup},
    Perbill,
};
use frame_support::weights::Weight;
use sp_std::{collections::btree_map::BTreeMap, str::FromStr};

pub type AccountId = AccountId32;
type Balance = u128;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

type Block = frame_system::mocking::MockBlock<Runtime>;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Runtime {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = BlockWeights;
    type BlockLength = ();
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type Block = Block;
}

impl pallet_timestamp::Config for Runtime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ();
    type WeightInfo = ();
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
    type ReserveIdentifier = [u8; 8];
}

parameter_types! {
    pub const ExistentialDeposit: Balance = 1;
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
    type Balance = Balance;
    type DustRemoval = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxFreezes = frame_support::traits::VariantCountOf<RuntimeFreezeReason>;
    type DoneSlashHandler = ();
}

pub const REEF: CurrencyId = CurrencyId::Token(TokenSymbol::REEF);
pub const RUSD: CurrencyId = CurrencyId::Token(TokenSymbol::RUSD);

parameter_types! {
    pub const GetNativeCurrencyId: CurrencyId = REEF;
}

impl module_currencies::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MultiCurrency = Tokens;
    type NativeCurrency = AdaptedBasicCurrency;
    type WeightInfo = ();
    type AddressMapping = MockAddressMapping;
    type EVMBridge = EVMBridge;
}

impl module_evm_bridge::Config for Runtime {
    type EVM = ModuleEVM;
}

parameter_types! {
    pub const TransactionByteFee: Balance = 10;
    pub const GetStableCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::RUSD);
    pub AllNonNativeCurrencyIds: Vec<CurrencyId> = vec![CurrencyId::Token(TokenSymbol::RUSD)];
}

impl module_transaction_payment::Config for Runtime {
    type AllNonNativeCurrencyIds = AllNonNativeCurrencyIds;
    type NativeCurrencyId = GetNativeCurrencyId;
    type StableCurrencyId = GetStableCurrencyId;
    type Currency = Balances;
    type MultiCurrency = Currencies;
    type OnTransactionPayment = ();
    type TransactionByteFee = TransactionByteFee;
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = IdentityFee<Balance>;
    type FeeMultiplierUpdate = ();
    type WeightInfo = ();
}
pub type ChargeTransactionPayment = module_transaction_payment::ChargeTransactionPayment<Runtime>;

parameter_types! {
    pub const ProxyDepositBase: u64 = 1;
    pub const ProxyDepositFactor: u64 = 1;
    pub const MaxProxies: u16 = 4;
    pub const MaxPending: u32 = 2;
    pub const AnnouncementDepositBase: u64 = 1;
    pub const AnnouncementDepositFactor: u64 = 1;
}

#[derive(
    Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug, MaxEncodedLen,
    scale_info::TypeInfo,
)]
pub enum ProxyType {
    Any,
    JustTransfer,
    JustUtility,
}
impl Default for ProxyType {
    fn default() -> Self {
        Self::Any
    }
}
impl InstanceFilter<RuntimeCall> for ProxyType {
    fn filter(&self, c: &RuntimeCall) -> bool {
        match self {
            ProxyType::Any => true,
            ProxyType::JustTransfer => {
                matches!(c, RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death { .. }))
            }
            ProxyType::JustUtility => matches!(c, RuntimeCall::Utility(..)),
        }
    }
    fn is_superset(&self, o: &Self) -> bool {
        self == &ProxyType::Any || self == o
    }
}

impl pallet_proxy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type ProxyType = ProxyType;
    type ProxyDepositBase = ProxyDepositBase;
    type ProxyDepositFactor = ProxyDepositFactor;
    type MaxProxies = MaxProxies;
    type WeightInfo = ();
    type CallHasher = BlakeTwo256;
    type MaxPending = MaxPending;
    type AnnouncementDepositBase = AnnouncementDepositBase;
    type AnnouncementDepositFactor = AnnouncementDepositFactor;
}

impl pallet_utility::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type PalletsOrigin = OriginCaller;
    type WeightInfo = ();
}

parameter_types! {
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(10) * BlockWeights::get().max_block;
    pub const MaxScheduledPerBlock: u32 = 50;
}

impl pallet_scheduler::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type BlockNumberProvider = frame_system::Pallet<Runtime>;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = MaxScheduledPerBlock;
    type WeightInfo = ();
    type OriginPrivilegeCmp = frame_support::traits::EqualPrivilegeOnly;
    type Preimages = ();
}

pub type AdaptedBasicCurrency =
    module_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, u64>;

pub type MultiCurrencyPrecompile =
    crate::MultiCurrencyPrecompile<AccountId, MockAddressMapping, Currencies>;

pub type StateRentPrecompile = crate::StateRentPrecompile<AccountId, MockAddressMapping, ModuleEVM>;
pub type ScheduleCallPrecompile = crate::ScheduleCallPrecompile<
    AccountId,
    MockAddressMapping,
    Scheduler,
    ChargeTransactionPayment,
    RuntimeCall,
    RuntimeOrigin,
    OriginCaller,
    Runtime,
>;

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
    pub const MaxCodeSize: u32 = 60 * 1024;
    pub const ChainId: u64 = 1;
}

pub struct GasToWeight;
impl Convert<u64, Weight> for GasToWeight {
    fn convert(a: u64) -> Weight {
        Weight::from_parts(a, 0)
    }
}

impl module_evm::Config for Runtime {
    type AddressMapping = MockAddressMapping;
    type Currency = Balances;
    type TransferAll = Currencies;
    type NewContractExtraBytes = NewContractExtraBytes;
    type StorageDepositPerByte = StorageDepositPerByte;
    type MaxCodeSize = MaxCodeSize;
    type RuntimeEvent = RuntimeEvent;
    type Precompiles = AllPrecompiles<
        SystemContractsFilter,
        MultiCurrencyPrecompile,
        StateRentPrecompile,
        ScheduleCallPrecompile,
    >;
    type ChainId = ChainId;
    type GasToWeight = GasToWeight;
    type ChargeTransactionPayment = ChargeTransactionPayment;
    type NetworkContractOrigin = EnsureSignedBy<NetworkContractAccount, AccountId>;
    type NetworkContractSource = NetworkContractSource;
    type DeveloperDeposit = DeveloperDeposit;
    type DeploymentFee = DeploymentFee;
    type FreeDeploymentOrigin = EnsureSignedBy<CouncilAccount, AccountId>;
    type WeightInfo = ();
}

pub const ALICE: AccountId = AccountId::new([1u8; 32]);

pub fn alice() -> H160 {
    H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1])
}

pub fn bob() -> H160 {
    H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2])
}

pub fn evm_genesis() -> BTreeMap<H160, module_evm::GenesisAccount<Balance, u64>> {
    let contracts_json = &include_bytes!("../../../../assets/bytecodes.json")[..];
    let contracts: Vec<(String, String, String)> = serde_json::from_slice(contracts_json).unwrap();
    let mut accounts = BTreeMap::new();
    for (_, address, code_string) in contracts {
        let account = module_evm::GenesisAccount {
            nonce: 0,
            balance: 0u128,
            storage: Default::default(),
            code: Bytes::from_str(&code_string).unwrap().0,
        };
        let addr = H160::from_slice(
            from_hex(address.as_str())
                .expect("predeploy-contracts must specify address")
                .as_slice(),
        );
        accounts.insert(addr, account);
    }
    accounts
}

pub const INITIAL_BALANCE: Balance = 1_000_000_000_000;
pub const REEF_ERC20_ADDRESS: &str = "0x0000000000000000000000000000000001000000";

frame_support::construct_runtime!(
    pub enum Runtime {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Tokens: orml_tokens,
        Balances: pallet_balances,
        Currencies: module_currencies,
        EVMBridge: module_evm_bridge,
        TransactionPayment: module_transaction_payment,
        Proxy: pallet_proxy,
        Utility: pallet_utility,
        Scheduler: pallet_scheduler,
        ModuleEVM: module_evm,
    }
);

// This function basically just builds a genesis storage key/value store
// according to our desired mockup.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Runtime>::default()
        .build_storage()
        .unwrap();

    let mut accounts = BTreeMap::new();
    let mut evm_genesis_accounts = evm_genesis();
    accounts.append(&mut evm_genesis_accounts);

    accounts.insert(
        alice(),
        module_evm::GenesisAccount {
            nonce: 1,
            balance: INITIAL_BALANCE,
            storage: Default::default(),
            code: Default::default(),
        },
    );
    accounts.insert(
        bob(),
        module_evm::GenesisAccount {
            nonce: 1,
            balance: INITIAL_BALANCE,
            storage: Default::default(),
            code: Default::default(),
        },
    );

    pallet_balances::GenesisConfig::<Runtime>::default()
        .assimilate_storage(&mut storage)
        .unwrap();
    module_evm::GenesisConfig::<Runtime> { accounts }
        .assimilate_storage(&mut storage)
        .unwrap();

    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| {
        System::set_block_number(1);
        Timestamp::set_timestamp(1);

        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            ALICE,
            REEF,
            1_000_000_000_000
        ));
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            ALICE,
            RUSD,
            1_000_000_000
        ));

        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            MockAddressMapping::get_account_id(&alice()),
            RUSD,
            1_000
        ));
    });
    ext
}

pub fn run_to_block(n: u32) {
    use frame_support::traits::{OnFinalize, OnInitialize};
    while System::block_number() < n {
        Scheduler::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        Scheduler::on_initialize(System::block_number());
    }
}
pub fn get_task_id(output: Vec<u8>) -> Vec<u8> {
    let mut num = [0u8; 4];
    num[..].copy_from_slice(&output[32 - 4..32]);
    let task_id_len: u32 = u32::from_be_bytes(num);
    return output[32..32 + task_id_len as usize].to_vec();
}
