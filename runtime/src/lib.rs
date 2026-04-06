#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "1024"]
extern crate alloc;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));
// Standard prelude
use sp_std::{borrow::Cow, prelude::*};

// Codec and Encoding
use codec::{Decode, Encode, MaxEncodedLen};

// FRAME Support
use frame_support::{
    derive_impl,
    dynamic_params::{dynamic_pallet_params, dynamic_params},
    genesis_builder_helper::{build_state, get_preset},
    instances::{Instance1, Instance2},
    ord_parameter_types,
    pallet_prelude::{ConstU32, DispatchClass, Get},
    parameter_types,
    traits::{
        fungible::{HoldConsideration, NativeFromLeft, NativeOrWithId, UnionOf},
        schedule::Priority,
        tokens::{imbalance::ResolveAssetTo, pay::PayAssetFromAccount, GetSalary, PayFromAccount},
        AsEnsureOriginWithArg, ConstBool, ConstU128, ConstU16, ConstU64, ConstantStoragePrice,
        EitherOfDiverse, EnsureOrigin, EqualPrivilegeOnly, KeyOwnerProofSystem, LinearStoragePrice,
        Nothing, OnRuntimeUpgrade, OriginTrait, VariantCountOf, WithdrawReasons,
    },
    weights::{
        constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
        ConstantMultiplier, Weight,
    },
    BoundedVec, PalletId,
};
use pallet_transaction_payment::FeeDetails;
use pallet_transaction_payment::RuntimeDispatchInfo;

// FRAME System
use frame_support::pallet_prelude::StorageVersion;
use frame_support::weights::constants::WEIGHT_REF_TIME_PER_SECOND;
use frame_system::{
    ensure_root, EnsureRoot, EnsureRootWithSuccess, EnsureSigned, EnsureWithSuccess,
};

// Substrate Transaction Payment
#[allow(deprecated)]
pub use pallet_transaction_payment::{
    CurrencyAdapter, TargetedFeeAdjustment as SubstrateTargetedFeeAdjustment,
};

// Election Support
use frame_election_provider_support::bounds::ElectionBounds;
use frame_election_provider_support::bounds::ElectionBoundsBuilder;
use frame_election_provider_support::{
    onchain, BalancingConfig, ElectionDataProvider, SequentialPhragmen, VoteWeight,
};

// Assets
use pallet_assets_precompiles::{InlineIdConfig, NativeERC20, ERC20};

// Assets Conversion
use pallet_asset_conversion::{AccountIdConverter, Ascending, Chain, WithFirstAsset};

// NominationPools
use pallet_nomination_pools::PoolId;

// Election Provider Multi-phase
use pallet_election_provider_multi_phase::{GeometricDepositBase, SolutionAccuracyOf};

// Grandpa
use pallet_grandpa::{
    fg_primitives, AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList,
};

// Asset Conversion

//Revive
use pallet_revive::evm::runtime::EthExtra;

// Im Online
pub use pallet_im_online::sr25519::AuthorityId as ImOnlineId;

// Timestamp (only for std/test)
#[cfg(any(feature = "std", test))]
pub use pallet_timestamp::Call as TimestampCall;

// Balances
pub use pallet_balances::Call as BalancesCall;

use pallet_asset_conversion_tx_payment::SwapAssetAdapter;

// Identity
use pallet_identity::legacy::IdentityInfo;

// Authority Discovery
pub use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;

// Runtime APIs
use sp_api::impl_runtime_apis;

mod voter_bags;
pub use frame_system::Call as SystemCall;
use sp_staking::currency_to_vote::U128CurrencyToVote;
// Runtime Versioning
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use scale_info::TypeInfo;

// sp_core and crypto
use sp_core::{crypto::KeyTypeId, OpaqueMetadata, H160};

// sp_runtime Core Traits and Types
use sp_runtime::{
    curve::PiecewiseLinear,
    generic, impl_opaque_keys, str_array as s,
    traits::{
        self, AccountIdConversion, BadOrigin, BlakeTwo256, Block as BlockT, Bounded, NumberFor,
        OpaqueKeys, SaturatedConversion, StaticLookup, Zero,
    },
    transaction_validity::{TransactionPriority, TransactionSource, TransactionValidity},
    ApplyExtrinsicResult, DispatchResult, FixedPointNumber, FixedU128, Perbill, Percent, Permill,
    Perquintill,
};

// ORML Support
use orml_authority::EnsureDelayed;
use orml_traits::parameter_type_with_key;

// Custom Modules
use module_currencies::BasicCurrencyAdapter;
use module_evm::{CallInfo, CreateInfo};
use module_evm_accounts::EvmAddressMapping;
use module_transaction_payment::{Multiplier, TargetedFeeAdjustment};

// re-exports

use frame_system::limits::{BlockLength, BlockWeights};
pub use pallet_staking::StakerStatus;
pub use primitives::{
    evm::EstimateResourcesRequest, AccountId, AccountIndex, Amount, AuthoritysOriginId, Balance,
    BlockNumber, CurrencyId, EraIndex, Hash, Moment, Nonce, ReserveIdentifier, Signature,
    TokenSymbol,
};
pub use runtime_common::{
    GasToWeight, OffchainSolutionWeightLimit, Price, Rate, Ratio, SystemContractsFilter,
};

pub use primitives::{currency::*, evm, time::*};

mod assets_api;
mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod revive_migration;

//
// formerly authority.rs
//
parameter_types! {
    pub BurnAccount: AccountId = AccountId::from([0u8; 32]);
    pub const SevenDays: BlockNumber = 7 * DAYS;
}

pub fn get_all_module_accounts() -> Vec<AccountId> {
    vec![BurnAccount::get()]
}

pub struct AuthorityConfigImpl;
impl orml_authority::AuthorityConfig<RuntimeOrigin, OriginCaller, BlockNumber>
    for AuthorityConfigImpl
{
    fn check_schedule_dispatch(origin: RuntimeOrigin, _priority: Priority) -> DispatchResult {
        EnsureRoot::<AccountId>::try_origin(origin)
            .map_or_else(|_| Err(BadOrigin.into()), |_| Ok(()))
    }

    fn check_fast_track_schedule(
        origin: RuntimeOrigin,
        _initial_origin: &OriginCaller,
        _new_delay: BlockNumber,
    ) -> DispatchResult {
        ensure_root(origin).map_err(|_| BadOrigin.into())
    }

    fn check_delay_schedule(
        origin: RuntimeOrigin,
        _initial_origin: &OriginCaller,
    ) -> DispatchResult {
        ensure_root(origin).map_err(|_| BadOrigin.into())
    }

    fn check_cancel_schedule(
        origin: RuntimeOrigin,
        initial_origin: &OriginCaller,
    ) -> DispatchResult {
        ensure_root(origin.clone()).or_else(|_| {
            if origin.caller() == initial_origin {
                Ok(())
            } else {
                Err(BadOrigin.into())
            }
        })
    }
}

impl orml_authority::AsOriginId<RuntimeOrigin, OriginCaller> for AuthoritysOriginId {
    fn into_origin(self) -> OriginCaller {
        match self {
            AuthoritysOriginId::Root => RuntimeOrigin::root().caller().clone(),
        }
    }

    fn check_dispatch_from(&self, origin: RuntimeOrigin) -> DispatchResult {
        ensure_root(origin.clone()).or_else(|_| match self {
            AuthoritysOriginId::Root => <EnsureDelayed<
                SevenDays,
                EnsureRoot<AccountId>,
                BlockNumber,
                OriginCaller,
            > as EnsureOrigin<RuntimeOrigin>>::ensure_origin(
                origin
            )
            .map_or_else(|_| Err(BadOrigin.into()), |_| Ok(())),
        })
    }
}

// end authority.rs

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
    use super::*;

    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;

    impl_opaque_keys! {
        pub struct SessionKeys {
            pub babe: Babe,
            pub grandpa: Grandpa,
            pub im_online: ImOnline,
            pub authority_discovery: AuthorityDiscovery,
        }
    }
}

/// Fee-related
pub mod fee {
    use super::{Balance, MILLI_REEF};
    use frame_support::weights::{
        constants::ExtrinsicBaseWeight, WeightToFeeCoefficient, WeightToFeeCoefficients,
        WeightToFeePolynomial,
    };
    use smallvec::smallvec;
    use sp_runtime::PerThing;

    /// Handles converting a weight scalar to a fee value, based on the scale
    /// and granularity of the node's balance type.
    ///
    /// This should typically create a mapping between the following ranges:
    ///   - [0, system::MaximumBlockWeight]
    ///   - [Balance::min, Balance::max]
    ///
    /// Yet, it can be used for any other sort of change to weight-fee. Some
    /// examples being:
    ///   - Setting it to `0` will essentially disable the weight fee.
    ///   - Setting it to `1` will cause the literal `#[weight = x]` values to
    ///     be charged.
    pub struct WeightToFee;
    impl WeightToFeePolynomial for WeightToFee {
        type Balance = Balance;
        fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
            let p = MILLI_REEF;
            let q = Balance::from(ExtrinsicBaseWeight::get().ref_time()); // 125_000_000
            smallvec![WeightToFeeCoefficient {
                degree: 1,
                negative: false,
                coeff_frac: PerThing::from_rational(p % q, q),
                coeff_integer: p / q,
            }]
        }
    }
}

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: alloc::borrow::Cow::Borrowed("reef"),
    impl_name: alloc::borrow::Cow::Borrowed("reef"),
    authoring_version: 1,
    spec_version: 15,
    impl_version: 11,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 2,
    system_version: 1,
};

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

/// The BABE epoch configuration at genesis.
pub const BABE_GENESIS_EPOCH_CONFIG: sp_consensus_babe::BabeEpochConfiguration =
    sp_consensus_babe::BabeEpochConfiguration {
        c: PRIMARY_PROBABILITY, // 1 in 4 blocks will be BABE
        allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryPlainSlots,
    };

parameter_types! {
    pub const Version: RuntimeVersion = VERSION;
    pub const BlockHashCount: BlockNumber = 2400;
    pub const SS58Prefix: u8 = 42;
}

#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
    /// The basic call filter to use in dispatchable.
    type BaseCallFilter = frame_support::traits::Everything;
    /// Block & extrinsics weights: base values and limits.
    type BlockWeights = RuntimeBlockWeights;
    /// The maximum length of a block (in bytes).
    type BlockLength = RuntimeBlockLength;
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The aggregated dispatch type that is available for extrinsics.
    type RuntimeCall = RuntimeCall;
    /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
    type Lookup = (Indices, EvmAccounts);
    /// The index type for storing how many extrinsics an account has signed.
    type Nonce = Nonce;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// The hashing algorithm used.
    type Hashing = BlakeTwo256;
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    /// The ubiquitous origin type.
    type RuntimeOrigin = RuntimeOrigin;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// Maximum weight of each block.
    type DbWeight = RocksDbWeight;
    /// Version of the runtime.
    type Version = Version;
    /// This type is being generated by `construct_runtime!`.
    type PalletInfo = PalletInfo;
    /// What to do if a new account is created.
    type OnNewAccount = ();
    /// What to do if an account is fully reaped from the system.
    type OnKilledAccount = (
        module_evm::CallKillAccount<Runtime>,
        module_evm_accounts::CallKillAccount<Runtime>,
    );
    /// The data to be stored in an account.
    type AccountData = pallet_balances::AccountData<Balance>;
    /// Weight information for the extrinsics of this pallet.
    type SystemWeightInfo = frame_system::weights::SubstrateWeight<Runtime>;
    /// This is used as an identifier of the chain. 42 is the generic substrate prefix.
    type SS58Prefix = SS58Prefix;
    /// This is a hook that is use when setCode is called - not require unless using cumulus.
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
    type Block = Block;
}

pub type Migrations = migrations::Unreleased;

pub mod migrations {
    /// Unreleased migrations. Add new ones here:
    pub type Unreleased = (
        crate::MigrateBalancesTo12Decimals<crate::Runtime>,
        crate::revive_migration::ReviveMigrations,
    );
}

/// Storage version before this migration
const STORAGE_VERSION_PRE: u16 = 0;
/// Storage version after this migration
const STORAGE_VERSION_POST: u16 = 1;

pub struct MigrateBalancesTo12Decimals<T>(frame_support::pallet_prelude::PhantomData<T>);
impl<T: frame_system::Config> OnRuntimeUpgrade for MigrateBalancesTo12Decimals<T>
where
    T::AccountData:
        From<pallet_balances::AccountData<u128>> + Into<pallet_balances::AccountData<u128>>,
    T: pallet_staking::Config + pallet_balances::Config,
{
    fn on_runtime_upgrade() -> Weight {
        const DECIMAL_CONVERSION: u128 = 1_000_000;

        let onchain_version = StorageVersion::get::<frame_system::Pallet<T>>();
        if onchain_version != STORAGE_VERSION_PRE {
            log::warn!(
                target: "runtime::migration",
                "Skipping MigrateBalancesTo12Decimals: already at storage version {:?}. Expected {:?}.",
                onchain_version,
                STORAGE_VERSION_PRE,
            );
            // Charge only for the version read
            return T::DbWeight::get().reads(1);
        }
        log::info!("Starting balance . from 18 to 12 decimals");

        let mut freezes_migrated = 0u64;
        let mut locks_migrated = 0u64;
        let mut ledgers_migrated = 0u64;
        let mut holds_migrated = 0u64;
        let mut accounts_migrated = 0u64;
        let mut eras_overview_migrated = 0u64;
        let mut eras_rewards_migrated = 0u64;
        let mut eras_total_stake_migrated = 0u64;
        let mut eras_stakers_paged_migrated = 0u64;

        let balance_conversion: <T as pallet_balances::Config>::Balance =
            DECIMAL_CONVERSION.saturated_into();
        let staking_conversion: pallet_staking::BalanceOf<T> = DECIMAL_CONVERSION.saturated_into();

        frame_system::Account::<T>::translate::<
            frame_system::AccountInfo<T::Nonce, pallet_balances::AccountData<u128>>,
            _,
        >(|_key, mut old_info| {
            old_info.data.free /= DECIMAL_CONVERSION;
            old_info.data.reserved /= DECIMAL_CONVERSION;
            old_info.data.frozen /= DECIMAL_CONVERSION;

            accounts_migrated += 1;

            Some(frame_system::AccountInfo {
                nonce: old_info.nonce,
                consumers: old_info.consumers,
                providers: old_info.providers,
                sufficients: old_info.sufficients,
                data: old_info.data.into(),
            })
        });

        pallet_staking::Ledger::<T>::translate::<pallet_staking::StakingLedger<T>, _>(
            |_key, mut old_info| {
                old_info.total /= staking_conversion;
                old_info.active /= staking_conversion;
                for chunk in old_info.unlocking.iter_mut() {
                    chunk.value /= staking_conversion;
                }
                ledgers_migrated += 1;
                Some(old_info)
            },
        );

        pallet_balances::Locks::<T>::translate::<
            frame_support::WeakBoundedVec<
                pallet_balances::BalanceLock<<T as pallet_balances::Config>::Balance>,
                T::MaxLocks,
            >,
            _,
        >(|_key, locks| {
            let mut inner = locks.into_inner();
            for lock in inner.iter_mut() {
                lock.amount /= balance_conversion;
            }
            locks_migrated += 1;
            Some(frame_support::WeakBoundedVec::force_from(inner, None))
        });

        pallet_balances::Holds::<T>::translate::<
            frame_support::BoundedVec<
                frame_support::traits::tokens::IdAmount<
                    <T as pallet_balances::Config>::RuntimeHoldReason,
                    <T as pallet_balances::Config>::Balance,
                >,
                frame_support::traits::VariantCountOf<
                    <T as pallet_balances::Config>::RuntimeHoldReason,
                >,
            >,
            _,
        >(|_key, mut holds| {
            for hold in holds.iter_mut() {
                hold.amount /= balance_conversion;
            }
            holds_migrated += 1;
            Some(holds)
        });

        pallet_balances::Freezes::<T>::translate::<
            frame_support::BoundedVec<
                frame_support::traits::tokens::IdAmount<
                    <T as pallet_balances::Config>::FreezeIdentifier,
                    <T as pallet_balances::Config>::Balance,
                >,
                <T as pallet_balances::Config>::MaxFreezes,
            >,
            _,
        >(|_key, mut freezes| {
            for freeze in freezes.iter_mut() {
                freeze.amount /= balance_conversion;
            }
            freezes_migrated += 1;
            Some(freezes)
        });

        pallet_staking::ErasStakersOverview::<T>::translate::<
            sp_staking::PagedExposureMetadata<pallet_staking::BalanceOf<T>>,
            _,
        >(|_era, _validator, mut meta| {
            meta.own /= staking_conversion;
            meta.total /= staking_conversion;
            // page_count and nominator_count are counts, not balances — do NOT divide
            eras_overview_migrated += 1;
            Some(meta)
        });

        pallet_staking::ErasStakersPaged::<T>::translate::<
            sp_staking::ExposurePage<
                <T as frame_system::Config>::AccountId,
                pallet_staking::BalanceOf<T>,
            >,
            _,
        >(|(_era, _validator, _page), mut page| {
            page.page_total /= staking_conversion;
            for nominator in page.others.iter_mut() {
                nominator.value /= staking_conversion;
            }
            eras_stakers_paged_migrated += 1;
            Some(page)
        });

        pallet_staking::ErasValidatorReward::<T>::translate::<pallet_staking::BalanceOf<T>, _>(
            |_key, mut reward| {
                reward /= staking_conversion;
                eras_rewards_migrated += 1;
                Some(reward)
            },
        );

        // Migrate total Issuance
        pallet_balances::TotalIssuance::<Runtime>::mutate(|issuance| {
            *issuance /= DECIMAL_CONVERSION;
        });

        // Migrate Inactive Issuance
        pallet_balances::InactiveIssuance::<Runtime>::mutate(|issuance| {
            *issuance /= DECIMAL_CONVERSION;
        });

        pallet_staking::ErasTotalStake::<T>::translate::<pallet_staking::BalanceOf<T>, _>(
            |_era, mut total| {
                total /= staking_conversion;
                eras_total_stake_migrated += 1;
                Some(total)
            },
        );

        pallet_staking::MinNominatorBond::<T>::mutate(|v| *v /= staking_conversion);
        pallet_staking::MinValidatorBond::<T>::mutate(|v| *v /= staking_conversion);

        StorageVersion::new(STORAGE_VERSION_POST).put::<frame_system::Pallet<T>>();

        log::info!("Migrated {} accounts", accounts_migrated);

        let rw = accounts_migrated
            + locks_migrated
            + holds_migrated
            + freezes_migrated
            + ledgers_migrated
            + eras_rewards_migrated
            + eras_overview_migrated
            // + (eras_stakers_migrated * 2) // ErasStakers + ErasStakersClipped
            + 4  // TotalIssuance, InactiveIssuance, MinNominatorBond, MinValidatorBond
            + 1; // StorageVersion write

        T::DbWeight::get().reads_writes(rw + 1, rw)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
        use codec::Encode;

        // Guard check
        let onchain_version = StorageVersion::get::<pallet_balances::Pallet<T>>();
        ensure!(
            onchain_version == STORAGE_VERSION_PRE,
            "pre_upgrade: storage version is not at expected pre-migration version"
        );

        // Snapshot values we want to verify after migration
        let total_issuance = pallet_balances::TotalIssuance::<T>::get();
        let inactive_issuance = pallet_balances::InactiveIssuance::<T>::get();
        let account_count = frame_system::Account::<T>::iter().count() as u64;

        log::info!(
            target: "runtime::migration",
            "pre_upgrade: total_issuance={:?}, inactive={:?}, accounts={}",
            total_issuance, inactive_issuance, account_count,
        );

        Ok((total_issuance, inactive_issuance, account_count).encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
        use codec::Decode;

        type Balance = <T as pallet_balances::Config>::Balance;

        let (pre_total, pre_inactive, pre_account_count): (Balance, Balance, u64) =
            Decode::decode(&mut &state[..])
                .map_err(|_| "post_upgrade: failed to decode pre-upgrade state")?;

        let balance_conversion: Balance = DECIMAL_CONVERSION.saturated_into();

        // ---- Total issuance check ----
        let post_total = pallet_balances::TotalIssuance::<T>::get();
        let expected = pre_total / balance_conversion;
        // Allow up to `account_count` dust difference from integer division
        ensure!(
            post_total <= expected && expected - post_total <= pre_account_count.saturated_into(),
            "post_upgrade: TotalIssuance mismatch after migration"
        );

        // ---- Inactive issuance check ----
        let post_inactive = pallet_balances::InactiveIssuance::<T>::get();
        let expected_inactive = pre_inactive / balance_conversion;
        ensure!(
            post_inactive <= expected_inactive,
            "post_upgrade: InactiveIssuance mismatch after migration"
        );

        // ---- Account count must be unchanged ----
        let post_account_count = frame_system::Account::<T>::iter().count() as u64;
        ensure!(
            pre_account_count == post_account_count,
            "post_upgrade: account count changed during migration"
        );

        // ---- Storage version must have bumped ----
        let onchain_version = StorageVersion::get::<pallet_balances::Pallet<T>>();
        ensure!(
            onchain_version == STORAGE_VERSION_POST,
            "post_upgrade: storage version was not bumped"
        );

        log::info!(
            target: "runtime::migration",
            "post_upgrade: all checks passed. \
             total_issuance={:?}, inactive={:?}, accounts={}",
            post_total, post_inactive, post_account_count,
        );

        Ok(())
    }
}

parameter_types! {
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
}

impl pallet_session::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = <Self as frame_system::Config>::AccountId;
    type ValidatorIdOf = sp_runtime::traits::ConvertInto;
    type ShouldEndSession = Babe;
    type NextSessionRotation = Babe;
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
    type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
    type Keys = opaque::SessionKeys;
    type WeightInfo = ();
    type Currency = Balances;
    type KeyDeposit = ();
    type DisablingStrategy = pallet_session::disabling::UpToLimitWithReEnablingDisablingStrategy;
}

parameter_types! {
    pub const DepositPerItem: Balance = deposit(1, 0);
    pub const DepositPerByte: Balance = deposit(0, 1);
    pub const DefaultDepositLimit: Balance = deposit(1024, 1024 * 1024);
    pub CodeHashLockupDepositPercent: Perbill = Perbill::from_percent(30);
    pub const DepositPerChildTrieItem: Balance = deposit(1, 0) / 100;
    pub const MaxEthExtrinsicWeight: FixedU128 = FixedU128::from_rational(9, 10);
}

impl pallet_revive::Config for Runtime {
    type Time = Timestamp;
    type Balance = Balance;
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type RuntimeOrigin = RuntimeOrigin;
    type DepositPerItem = DepositPerItem;
    type DepositPerChildTrieItem = DepositPerChildTrieItem;
    type DepositPerByte = DepositPerByte;
    type WeightInfo = pallet_revive::weights::SubstrateWeight<Self>;
    type Precompiles = (
        NativeERC20<Self>, // 0x0000000000000000000000000000000001000000
        ERC20<Self, InlineIdConfig<0x1>, Instance1>,
        ERC20<Self, InlineIdConfig<0x2>, Instance2>,
    );
    type AddressMapper = pallet_revive::AccountId32Mapper<Self>;
    type RuntimeMemory = ConstU32<{ 128 * 1024 * 1024 }>;
    type PVFMemory = ConstU32<{ 512 * 1024 * 1024 }>;
    type UnsafeUnstableInterface = ConstBool<false>;
    type UploadOrigin = EnsureSigned<Self::AccountId>;
    type InstantiateOrigin = EnsureSigned<Self::AccountId>;
    type RuntimeHoldReason = RuntimeHoldReason;
    type CodeHashLockupDepositPercent = CodeHashLockupDepositPercent;
    type ChainId = ConstU64<13939>;
    type NativeToEthRatio = ConstU32<1_000_000>; // 10^(18 - 12) Eth is 10^18, Native is 10^12.
    type FindAuthor = <Runtime as pallet_authorship::Config>::FindAuthor;
    type AllowEVMBytecode = ConstBool<true>;
    type FeeInfo = pallet_revive::evm::fees::Info<Address, Signature, EthExtraImpl>;
    type MaxEthExtrinsicWeight = MaxEthExtrinsicWeight;
    type DebugEnabled = ConstBool<false>;
    type GasScale = ConstU32<1000>;
    type Issuance = Balances;
}

parameter_types! {
    pub const EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS;
    pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
    pub const ReportLongevity: u64 =
        BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
}

impl pallet_session::historical::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type FullIdentification = ();
    type FullIdentificationOf = pallet_staking::UnitIdentificationOf<Self>;
}

pallet_staking_reward_curve::build! {
    // 4.5% min, 27.5% max, 50% ideal stake
    const REWARD_CURVE: PiecewiseLinear<'static> = curve!(
        min_inflation: 0_045_000,
        max_inflation: 0_275_000,
        ideal_stake: 0_500_000,
        falloff: 0_050_000,
        max_piece_count: 40,
        test_precision: 0_005_500,
    );
}

pub struct OnChainSeqPhragmen;
impl onchain::Config for OnChainSeqPhragmen {
    type System = Runtime;
    type Solver = SequentialPhragmen<
        AccountId,
        pallet_election_provider_multi_phase::SolutionAccuracyOf<Runtime>,
    >;
    type DataProvider = Staking;
    type WeightInfo = frame_election_provider_support::weights::SubstrateWeight<Runtime>;
    type Bounds = ElectionBoundsOnChain;
    type Sort = ConstBool<true>;
    type MaxBackersPerWinner = MaxElectingVotersSolution;
    type MaxWinnersPerPage = MaxActiveValidators;
}

parameter_types! {
    pub const PostUnbondPoolsWindow: u32 = 4;
    pub const NominationPoolsPalletId: PalletId = PalletId(*b"py/nopls");
    pub const MaxPointsToBalance: u8 = 10;
}

use sp_runtime::traits::Convert;
pub struct BalanceToU256;
impl Convert<Balance, sp_core::U256> for BalanceToU256 {
    fn convert(balance: Balance) -> sp_core::U256 {
        sp_core::U256::from(balance)
    }
}
pub struct U256ToBalance;
impl Convert<sp_core::U256, Balance> for U256ToBalance {
    fn convert(n: sp_core::U256) -> Balance {
        n.try_into().unwrap_or(Balance::max_value())
    }
}

impl pallet_nomination_pools::Config for Runtime {
    type WeightInfo = ();
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type RewardCounter = FixedU128;
    type BalanceToU256 = BalanceToU256;
    type U256ToBalance = U256ToBalance;
    type PostUnbondingPoolsWindow = PostUnbondPoolsWindow;
    type MaxMetadataLen = ConstU32<256>;
    type BlockNumberProvider = System;
    type Filter = Nothing;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type StakeAdapter =
        pallet_nomination_pools::adapter::DelegateStake<Self, Staking, DelegatedStaking>;
    type AdminOrigin = EitherOfDiverse<
        EnsureRoot<AccountId>,
        pallet_collective::EnsureProportionAtLeast<AccountId, TechCouncilInstance, 3, 4>,
    >;
    type MaxUnbonding = ConstU32<8>;
    type PalletId = NominationPoolsPalletId;
    type MaxPointsToBalance = MaxPointsToBalance;
}

parameter_types! {
    pub const BagThresholds: &'static [u64] = &voter_bags::THRESHOLDS;
    pub const AutoRebagNumber: u32 = 10;
}

type VoterBagsListInstance = pallet_bags_list::Instance1;
impl pallet_bags_list::Config<VoterBagsListInstance> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    /// The voter bags-list is loosely kept up to date, and the real source of truth for the score
    /// of each node is the staking pallet.
    type ScoreProvider = Staking;
    type BagThresholds = BagThresholds;
    type Score = VoteWeight;
    type MaxAutoRebagPerBlock = AutoRebagNumber;
    type WeightInfo = pallet_bags_list::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const SessionsPerEra: sp_staking::SessionIndex = 24; // 24 hours
    pub const BondingDuration: sp_staking::EraIndex = 28; // 28 days
    pub const SlashDeferDuration: sp_staking::EraIndex = 27; // 27 days
    pub const RewardCurve: &'static PiecewiseLinear<'static> = &REWARD_CURVE;
    pub const MaxNominatorRewardedPerValidator: u32 = 64;
    pub const MaxAuthorities: u32 = 100;
    pub OffchainRepeat: BlockNumber = 5;
    pub HistoryDepth: u32 = 84;
    pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(17);
    pub const MaxControllersInDeprecationBatch: u32 = 5900;
}

pub struct StakingBenchmarkingConfig;
impl pallet_staking::BenchmarkingConfig for StakingBenchmarkingConfig {
    type MaxNominators = ConstU32<1000>;
    type MaxValidators = ConstU32<1000>;
}

/// Upper limit on the number of NPOS nominations.
const MAX_QUOTA_NOMINATIONS: u32 = 16;

impl pallet_staking::Config for Runtime {
    type OldCurrency = Balances;
    type RuntimeHoldReason = RuntimeHoldReason;
    type Currency = Balances;
    type MaxExposurePageSize = ConstU32<256>;
    type MaxValidatorSet = ConstU32<1000>;
    type Filter = Nothing;
    type MaxControllersInDeprecationBatch = MaxControllersInDeprecationBatch;
    type UnixTime = Timestamp;
    type CurrencyToVote = U128CurrencyToVote;
    type RewardRemainder = (); // burn
    type RuntimeEvent = RuntimeEvent;
    type Slash = (); // burn slashed rewards
    type Reward = (); // rewards are minted from the void
    type SessionsPerEra = SessionsPerEra;
    type BondingDuration = BondingDuration;
    type SlashDeferDuration = SlashDeferDuration;
    type SessionInterface = Self;
    type NextNewSession = Session;
    type WeightInfo = ();
    type ElectionProvider = ElectionProviderMultiPhase;
    type EraPayout = pallet_staking::ConvertCurve<RewardCurve>;
    type GenesisElectionProvider = onchain::OnChainExecution<OnChainSeqPhragmen>;
    type TargetList = pallet_staking::UseValidatorsMap<Self>;
    type MaxUnlockingChunks = ConstU32<32>;
    type HistoryDepth = HistoryDepth;
    type BenchmarkingConfig = StakingBenchmarkingConfig;
    type AdminOrigin = EitherOfDiverse<
        EnsureRoot<AccountId>,
        pallet_collective::EnsureProportionAtLeast<AccountId, TechCouncilInstance, 3, 4>,
    >;
    type CurrencyBalance = Balance;
    type VoterList = VoterList;
    type NominationsQuota = pallet_staking::FixedNominationsQuota<MAX_QUOTA_NOMINATIONS>;
    type EventListeners = NominationPools;
}

/// We assume that ~10% of the block weight is consumed by `on_initialize` handlers.
/// This is used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(10);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2 seconds of compute with a 6 second average block time, with maximum proof size.
const MAXIMUM_BLOCK_WEIGHT: Weight =
    Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND.saturating_mul(2), u64::MAX);

parameter_types! {
    // phase durations. 1/4 of the last session for each.
    pub const SignedPhase: u32 = EPOCH_DURATION_IN_BLOCKS / 4;
    pub const UnsignedPhase: u32 = EPOCH_DURATION_IN_BLOCKS / 4;

    // signed config
    pub const SignedRewardBase: Balance = 1 * DOLLARS;
    pub const SignedDepositBase: Balance = 1 * DOLLARS;
    pub const SignedDepositByte: Balance = 1 * CENTS;

    pub BetterUnsignedThreshold: Perbill = Perbill::from_rational(1u32, 10_000);

    // miner configs
    pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
        .base_block(BlockExecutionWeight::get())
        .for_class(DispatchClass::all(), |weights| {
            weights.base_extrinsic = ExtrinsicBaseWeight::get();
        })
        .for_class(DispatchClass::Normal, |weights| {
            weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
        })
        .for_class(DispatchClass::Operational, |weights| {
            weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
            // Operational transactions have some extra reserved space, so that they
            // are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
            weights.reserved = Some(
                MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
            );
        })
        .avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
        .build_or_panic();
    pub const MultiPhaseUnsignedPriority: TransactionPriority = StakingUnsignedPriority::get() - 1u64;
    pub RuntimeBlockLength: BlockLength =
        BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
    pub MinerMaxWeight: Weight = RuntimeBlockWeights::get()
        .get(DispatchClass::Normal)
        .max_extrinsic.expect("Normal extrinsics have a weight limit configured; qed")
        .saturating_sub(BlockExecutionWeight::get());
    // Solution can occupy 90% of normal block size
    pub MinerMaxLength: u32 = Perbill::from_rational(9u32, 10) *
        *RuntimeBlockLength::get()
        .max
        .get(DispatchClass::Normal);
}

frame_election_provider_support::generate_solution_type!(
    #[compact]
    pub struct NposSolution16::<
        VoterIndex = u32,
        TargetIndex = u16,
        Accuracy = sp_runtime::PerU16,
        MaxVoters = MaxElectingVoters,
    >(16)
);

parameter_types! {
    pub MaxNominations: u32 = <NposSolution16 as frame_election_provider_support::NposSolution>::LIMIT as u32;
    pub MaxElectingVoters: u32 = 40_000;
    pub MaxElectableTargets: u16 = 10_000;
    // OnChain values are lower.
    pub MaxOnChainElectingVoters: u32 = 5000;
    pub MaxOnChainElectableTargets: u16 = 1250;
    pub ElectionBoundsMultiPhase: ElectionBounds = ElectionBoundsBuilder::default()
        .voters_count(10_000.into()).targets_count(1_500.into()).build();
    pub ElectionBoundsOnChain: ElectionBounds = ElectionBoundsBuilder::default()
        .voters_count(5_000.into()).targets_count(1_250.into()).build();
    pub MaxElectingVotersSolution: u32 = 40_000;
    pub const SignedFixedDeposit: Balance = 1 * DOLLARS;
    pub const SignedDepositIncreaseFactor: Percent = Percent::from_percent(10);
    // The maximum winners that can be elected by the Election pallet which is equivalent to the
    // maximum active validators the staking pallet can have.
    pub MaxActiveValidators: u32 = 1000;
}

impl pallet_election_provider_multi_phase::MinerConfig for Runtime {
    type AccountId = AccountId;
    type MaxLength = MinerMaxLength;
    type MaxWeight = MinerMaxWeight;
    type Solution = NposSolution16;
    type MaxVotesPerVoter =
	<<Self as pallet_election_provider_multi_phase::Config>::DataProvider as ElectionDataProvider>::MaxVotesPerVoter;
    type MaxWinners = MaxActiveValidators;
    type MaxBackersPerWinner = MaxElectingVotersSolution;

    // The unsigned submissions have to respect the weight of the submit_unsigned call, thus their
    // weight estimate function is wired to this call's weight.
    fn solution_weight(v: u32, t: u32, a: u32, d: u32) -> Weight {
        <
			<Self as pallet_election_provider_multi_phase::Config>::WeightInfo
			as
			pallet_election_provider_multi_phase::WeightInfo
		>::submit_unsigned(v, t, a, d)
    }
}

/// Maximum number of iterations for balancing that will be executed in the embedded OCW
/// miner of election provider multi phase.
pub const MINER_MAX_ITERATIONS: u32 = 10;

/// A source of random balance for NposSolver, which is meant to be run by the OCW election miner.
pub struct OffchainRandomBalancing;
impl Get<Option<BalancingConfig>> for OffchainRandomBalancing {
    fn get() -> Option<BalancingConfig> {
        use sp_runtime::traits::TrailingZeroInput;
        let iterations = match MINER_MAX_ITERATIONS {
            0 => 0,
            max => {
                let seed = sp_io::offchain::random_seed();
                let random = <u32>::decode(&mut TrailingZeroInput::new(&seed))
                    .expect("input is padded with zeroes; qed")
                    % max.saturating_add(1);
                random as usize
            }
        };

        let config = BalancingConfig {
            iterations,
            tolerance: 0,
        };
        Some(config)
    }
}

type EnsureRootOrHalfCouncil = EitherOfDiverse<
    EnsureRoot<AccountId>,
    pallet_collective::EnsureProportionMoreThan<AccountId, TechCouncilInstance, 1, 2>,
>;

/// The numbers configured here could always be more than the the maximum limits of staking pallet
/// to ensure election snapshot will not run out of memory. For now, we set them to smaller values
/// since the staking is bounded and the weight pipeline takes hours for this single pallet.
pub struct ElectionProviderBenchmarkConfig;
impl pallet_election_provider_multi_phase::BenchmarkingConfig for ElectionProviderBenchmarkConfig {
    const VOTERS: [u32; 2] = [1000, 2000];
    const TARGETS: [u32; 2] = [500, 1000];
    const ACTIVE_VOTERS: [u32; 2] = [500, 800];
    const DESIRED_TARGETS: [u32; 2] = [200, 400];
    const SNAPSHOT_MAXIMUM_VOTERS: u32 = 1000;
    const MINER_MAXIMUM_VOTERS: u32 = 1000;
    const MAXIMUM_TARGETS: u32 = 300;
}

impl pallet_election_provider_multi_phase::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type EstimateCallFee = TransactionPayment;
    type SignedPhase = SignedPhase;
    type UnsignedPhase = UnsignedPhase;
    type BetterSignedThreshold = ();
    type MaxBackersPerWinner = MaxElectingVotersSolution;
    type OffchainRepeat = OffchainRepeat;
    type MinerTxPriority = MultiPhaseUnsignedPriority;
    type MinerConfig = Self;
    type SignedMaxSubmissions = ConstU32<10>;
    type SignedRewardBase = SignedRewardBase;
    type SignedDepositBase =
        GeometricDepositBase<Balance, SignedFixedDeposit, SignedDepositIncreaseFactor>;
    type SignedDepositByte = SignedDepositByte;
    type SignedMaxRefunds = ConstU32<3>;
    type SignedDepositWeight = ();
    type SignedMaxWeight = MinerMaxWeight;
    type SlashHandler = (); // burn slashes
    type RewardHandler = (); // nothing to do upon rewards
    type DataProvider = Staking;
    type Fallback = onchain::OnChainExecution<OnChainSeqPhragmen>;
    type GovernanceFallback = onchain::OnChainExecution<OnChainSeqPhragmen>;
    type Solver = SequentialPhragmen<AccountId, SolutionAccuracyOf<Self>, OffchainRandomBalancing>;
    type ForceOrigin = EnsureRootOrHalfCouncil;
    type MaxWinners = MaxActiveValidators;
    type BenchmarkingConfig = ElectionProviderBenchmarkConfig;
    type WeightInfo = pallet_election_provider_multi_phase::weights::SubstrateWeight<Self>;
    type ElectionBounds = ElectionBoundsMultiPhase;
}

impl pallet_babe::Config for Runtime {
    type EpochDuration = EpochDuration;
    type ExpectedBlockTime = ExpectedBlockTime;
    type EpochChangeTrigger = pallet_babe::ExternalTrigger;
    type KeyOwnerProof =
        <Historical as KeyOwnerProofSystem<(KeyTypeId, pallet_babe::AuthorityId)>>::Proof;
    type WeightInfo = ();
    type DisabledValidators = Session;
    type MaxNominators = MaxNominatorRewardedPerValidator;
    type MaxAuthorities = MaxAuthorities;
    type EquivocationReportSystem =
        pallet_babe::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

parameter_types! {
    pub const MaxSetIdSessionEntries: u32 = BondingDuration::get() * SessionsPerEra::get();
}

impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MaxAuthorities = MaxAuthorities;
    type MaxSetIdSessionEntries = MaxSetIdSessionEntries;
    type MaxNominators = MaxNominatorRewardedPerValidator;
    type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;
    type EquivocationReportSystem =
        pallet_grandpa::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

parameter_types! {
    pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = u64;
    type OnTimestampSet = Babe;
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl pallet_authorship::Config for Runtime {
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
    type EventHandler = (Staking, ImOnline);
}

impl pallet_offences::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
    type OnOffenceHandler = Staking;
}

impl pallet_authority_discovery::Config for Runtime {
    type MaxAuthorities = MaxAuthorities;
}

parameter_types! {
    pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
    pub const MaxPeerInHeartbeats: u32 = 10_000;
    pub const MaxPeerDataEncodingSize: u32 = 1_000;
    pub const MaxKeys: u32 = 10_000;
    /// We prioritize im-online heartbeats over election solution submission.
    pub const StakingUnsignedPriority: TransactionPriority = TransactionPriority::max_value() / 2;
}

impl pallet_im_online::Config for Runtime {
    type AuthorityId = ImOnlineId;
    type RuntimeEvent = RuntimeEvent;
    type ValidatorSet = Historical;
    type NextSessionRotation = Babe;
    type ReportUnresponsiveness = Offences;
    type UnsignedPriority = ImOnlineUnsignedPriority;
    type WeightInfo = ();
    type MaxKeys = MaxKeys;
    type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
}

parameter_types! {
    pub const BasicDeposit: Balance =      100 * REEF;
    pub const FieldDeposit: Balance =        1 * REEF;
    pub const SubAccountDeposit: Balance =  20 * REEF;
    pub const MaxSubAccounts: u32 = 100;
    pub const MaxAdditionalFields: u32 = 100;
    pub const MaxRegistrars: u32 = 20;
    pub const ByteDeposit: Balance = deposit(0, 1);
    pub const UsernameDeposit: Balance = deposit(0, 32);
}

impl pallet_identity::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BasicDeposit = BasicDeposit;
    type SubAccountDeposit = SubAccountDeposit;
    type MaxSubAccounts = MaxSubAccounts;
    type SigningPublicKey = <Signature as traits::Verify>::Signer;
    type ByteDeposit = ByteDeposit;
    type UsernameDeposit = UsernameDeposit;
    type IdentityInformation = IdentityInfo<MaxAdditionalFields>;
    type OffchainSignature = Signature;
    type UsernameAuthorityOrigin = EnsureRoot<Self::AccountId>;
    type PendingUsernameExpiration = ConstU32<{ 7 * DAYS }>;
    type MaxRegistrars = MaxRegistrars;
    type UsernameGracePeriod = ConstU32<{ 30 * DAYS }>;
    type MaxSuffixLength = ConstU32<7>;
    type MaxUsernameLength = ConstU32<32>;
    type Slashed = ();
    type ForceOrigin = EnsureRootOrTwoThridsTechCouncil;
    type RegistrarOrigin = EnsureRootOrTwoThridsTechCouncil;
    type WeightInfo = ();
}

parameter_types! {
    pub const IndexDeposit: Balance = 1 * REEF;
}

impl pallet_indices::Config for Runtime {
    type AccountIndex = AccountIndex;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type Deposit = IndexDeposit;
    type WeightInfo = ();
}

impl module_currencies::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type WeightInfo = ();
    type AddressMapping = EvmAddressMapping<Runtime>;
    type EVMBridge = EVMBridge;
}

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
        Zero::zero()
    };
}

impl orml_tokens::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = CurrencyId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type DustRemovalWhitelist = ();
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type CurrencyHooks = ();
    type ReserveIdentifier = ReserveIdentifier;
}

parameter_types! {
    pub const GetNativeCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::REEF);
    pub const GetStableCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::RUSD);
    // All currency types except for native currency, Sort by fee charge order
    pub AllNonNativeCurrencyIds: Vec<CurrencyId> = vec![];

}

parameter_types! {
    pub const TransactionByteFee: Balance = 10 * MILLI_REEF;
    pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
    pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(1, 100_000);
    pub MinimumMultiplier:  Multiplier = Multiplier::saturating_from_rational(1, 10 as u128);
    pub MaximumMultiplier: Multiplier = Bounded::max_value();
    pub const OperationalFeeMultiplier: u8 = 5;
    pub TipPerWeightStep: Balance = 0;
}

#[allow(deprecated)]
impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
    type OperationalFeeMultiplier = OperationalFeeMultiplier;
    type WeightToFee = pallet_revive::evm::fees::BlockRatioFee<1, 1, Self, Balance>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type FeeMultiplierUpdate = SubstrateTargetedFeeAdjustment<
        Self,
        TargetBlockFullness,
        AdjustmentVariable,
        MinimumMultiplier,
        MaximumMultiplier,
    >;
    type WeightInfo = pallet_transaction_payment::weights::SubstrateWeight<Runtime>;
}

impl module_transaction_payment::Config for Runtime {
    type AllNonNativeCurrencyIds = AllNonNativeCurrencyIds;
    type NativeCurrencyId = GetNativeCurrencyId;
    type StableCurrencyId = GetStableCurrencyId;
    type Currency = Balances;
    type MultiCurrency = Currencies;
    type OnTransactionPayment = (); // fees get burned
    type TransactionByteFee = TransactionByteFee;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type WeightToFee = fee::WeightToFee;
    type FeeMultiplierUpdate =
        TargetedFeeAdjustment<Self, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier>;
    type WeightInfo = weights::transaction_payment::WeightInfo<Runtime>;
}

pub struct EvmAccountsOnClaimHandler;
impl module_evm_accounts::Handler<AccountId> for EvmAccountsOnClaimHandler {
    fn handle(who: &AccountId) -> DispatchResult {
        if System::providers(who) == 0 {
            // no provider. i.e. no native tokens
            // ensure there are some native tokens, which will add provider
            EvmTransactionPayment::ensure_can_charge_fee(
                who,
                NativeTokenExistentialDeposit::get(),
                WithdrawReasons::TRANSACTION_PAYMENT,
            );
        }
        Ok(())
    }
}

impl module_evm_accounts::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type AddressMapping = EvmAddressMapping<Runtime>;
    type TransferAll = Currencies;
    type OnClaim = EvmAccountsOnClaimHandler;
    type WeightInfo = weights::evm_accounts::WeightInfo<Runtime>;
}

#[cfg(feature = "with-ethereum-compatibility")]
static ISTANBUL_CONFIG: evm::Config = evm::Config::istanbul();

parameter_types! {
    //In [3]: random.randint(1000, 100_000)
    //Out[3]: 13939
    pub const ChainId: u64 = 13939;
    // 10 REEF minimum storage deposit
    pub const NewContractExtraBytes: u32 = 1_000;
    pub const StorageDepositPerByte: Balance = 10 * MILLI_REEF;
    pub const MaxCodeSize: u32 = 60 * 1024;
    pub NetworkContractSource: H160 = H160::from_low_u64_be(0);
    pub const DeveloperDeposit: Balance = 1_000 * REEF;
    pub const DeploymentFee: Balance    = 100 * REEF;
}

pub type MultiCurrencyPrecompile =
    runtime_common::MultiCurrencyPrecompile<AccountId, EvmAddressMapping<Runtime>, Currencies>;
pub type StateRentPrecompile =
    runtime_common::StateRentPrecompile<AccountId, EvmAddressMapping<Runtime>, EVM>;
pub type ScheduleCallPrecompile = runtime_common::ScheduleCallPrecompile<
    AccountId,
    EvmAddressMapping<Runtime>,
    Scheduler,
    module_transaction_payment::ChargeTransactionPayment<Runtime>,
    RuntimeCall,
    RuntimeOrigin,
    OriginCaller,
    Runtime,
>;

impl module_evm::Config for Runtime {
    type AddressMapping = EvmAddressMapping<Runtime>;
    type Currency = Balances;
    type TransferAll = Currencies;
    type NewContractExtraBytes = NewContractExtraBytes;
    type StorageDepositPerByte = StorageDepositPerByte;
    type MaxCodeSize = MaxCodeSize;
    type RuntimeEvent = RuntimeEvent;
    type Precompiles = runtime_common::AllPrecompiles<
        SystemContractsFilter,
        MultiCurrencyPrecompile,
        StateRentPrecompile,
        ScheduleCallPrecompile,
    >;
    type ChainId = ChainId;
    type GasToWeight = GasToWeight;
    type ChargeTransactionPayment = module_transaction_payment::ChargeTransactionPayment<Runtime>;
    type NetworkContractOrigin = EnsureRoot<AccountId>; // todo: EnsureRootOrTwoThridsTechCouncil
    type NetworkContractSource = NetworkContractSource;
    type DeveloperDeposit = DeveloperDeposit;
    type DeploymentFee = DeploymentFee;
    type FreeDeploymentOrigin = EnsureRoot<AccountId>; // todo: EnsureRootOrTwoThridsTechCouncil
    type WeightInfo = weights::evm::WeightInfo<Runtime>;

    #[cfg(feature = "with-ethereum-compatibility")]
    fn config() -> &'static evm::Config {
        &ISTANBUL_CONFIG
    }
}

impl module_evm_bridge::Config for Runtime {
    type EVM = EVM;
}

pub type AssetsFreezerInstance = pallet_assets_freezer::Instance1;
impl pallet_assets_freezer::Config<AssetsFreezerInstance> for Runtime {
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type RuntimeEvent = RuntimeEvent;
}

parameter_types! {
    // note: if we add other native tokens (RUSD) we have to set native
    // existential deposit to 0 or check for other tokens on account pruning
    pub const NativeTokenExistentialDeposit: Balance =       1;
    pub const MaxNativeTokenExistentialDeposit: Balance = 1000 * REEF;
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = ReserveIdentifier::Count as u32;
}

/// A reason for placing a hold on funds.
#[derive(
    Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, MaxEncodedLen, Debug, TypeInfo,
)]
pub enum HoldReason {
    /// The NIS Pallet has reserved it for a non-fungible receipt.
    Nis,
    /// Used by the NFT Fractionalization Pallet.
    NftFractionalization,
}

impl pallet_balances::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxLocks = MaxLocks;
    /// The type for recording an account's balance.
    type Balance = Balance;
    type DustRemoval = (); // burn
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type DoneSlashHandler = ();
    type ExistentialDeposit = NativeTokenExistentialDeposit;
    type AccountStore = frame_system::Pallet<Runtime>;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = ReserveIdentifier;
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
    type RuntimeHoldReason = RuntimeHoldReason;
}

parameter_types! {
    pub const PreimageMaxSize: u32 = 4096 * 1024;
    pub const PreimageBaseDeposit: Balance = 1 * DOLLARS;
    // One cent: $10,000 / MB
    pub const PreimageByteDeposit: Balance = 1 * CENTS;
}

/// Dynamic parameters that can be changed at runtime through the
/// `pallet_parameters::set_parameter`.
#[dynamic_params(RuntimeParameters, pallet_parameters::Parameters::<Runtime>)]
pub mod dynamic_params {
    use super::*;

    #[dynamic_pallet_params]
    #[codec(index = 0)]
    pub mod storage {
        /// Configures the base deposit of storing some data.
        #[codec(index = 0)]
        pub static BaseDeposit: Balance = 1 * DOLLARS;

        /// Configures the per-byte deposit of storing some data.
        #[codec(index = 1)]
        pub static ByteDeposit: Balance = 1 * CENTS;
    }

    #[dynamic_pallet_params]
    #[codec(index = 1)]
    pub mod referenda {
        /// The configuration for the tracks
        #[codec(index = 0)]
        pub static Tracks: BoundedVec<
            pallet_referenda::Track<u16, Balance, BlockNumber>,
            ConstU32<100>,
        > = BoundedVec::truncate_from(vec![pallet_referenda::Track {
            id: 0u16,
            info: pallet_referenda::TrackInfo {
                name: s("root"),
                max_deciding: 1,
                decision_deposit: 10,
                prepare_period: 4,
                decision_period: 4,
                confirm_period: 2,
                min_enactment_period: 4,
                min_approval: pallet_referenda::Curve::LinearDecreasing {
                    length: Perbill::from_percent(100),
                    floor: Perbill::from_percent(50),
                    ceil: Perbill::from_percent(100),
                },
                min_support: pallet_referenda::Curve::LinearDecreasing {
                    length: Perbill::from_percent(100),
                    floor: Perbill::from_percent(0),
                    ceil: Perbill::from_percent(100),
                },
            },
        }]);

        /// A list mapping every origin with a track Id
        #[codec(index = 1)]
        pub static Origins: BoundedVec<(OriginCaller, u16), ConstU32<100>> =
            BoundedVec::truncate_from(vec![(
                OriginCaller::system(frame_system::RawOrigin::Root),
                0,
            )]);
    }
}

parameter_types! {
    pub const PreimageHoldReason: RuntimeHoldReason =
        RuntimeHoldReason::Preimage(pallet_preimage::HoldReason::Preimage);
}

impl pallet_preimage::Config for Runtime {
    type WeightInfo = pallet_preimage::weights::SubstrateWeight<Runtime>;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type Consideration = HoldConsideration<
        AccountId,
        Balances,
        PreimageHoldReason,
        LinearStoragePrice<
            dynamic_params::storage::BaseDeposit,
            dynamic_params::storage::ByteDeposit,
            Balance,
        >,
    >;
}

parameter_types! {
    pub const VoteLockingPeriod: BlockNumber = 30 * DAYS;
}

impl pallet_conviction_voting::Config for Runtime {
    type WeightInfo = pallet_conviction_voting::weights::SubstrateWeight<Self>;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type VoteLockingPeriod = VoteLockingPeriod;
    type MaxVotes = ConstU32<512>;
    type MaxTurnout = frame_support::traits::TotalIssuanceOf<Balances, Self::AccountId>;
    type Polls = Referenda;
    type BlockNumberProvider = System;
    type VotingHooks = ();
}

parameter_types! {
    pub const AlarmInterval: BlockNumber = 1;
    pub const SubmissionDeposit: Balance = 100 * DOLLARS;
    pub const UndecidingTimeout: BlockNumber = 28 * DAYS;
}
pub struct TracksInfo;
impl pallet_referenda::TracksInfo<Balance, BlockNumber> for TracksInfo {
    type Id = u16;
    type RuntimeOrigin = <RuntimeOrigin as frame_support::traits::OriginTrait>::PalletsOrigin;

    fn tracks(
    ) -> impl Iterator<Item = Cow<'static, pallet_referenda::Track<Self::Id, Balance, BlockNumber>>>
    {
        dynamic_params::referenda::Tracks::get()
            .into_iter()
            .map(Cow::Owned)
    }
    fn track_for(id: &Self::RuntimeOrigin) -> Result<Self::Id, ()> {
        dynamic_params::referenda::Origins::get()
            .iter()
            .find(|(o, _)| id == o)
            .map(|(_, track_id)| *track_id)
            .ok_or(())
    }
}

// 28
//  6 13 28

impl pallet_referenda::Config for Runtime {
    type WeightInfo = pallet_referenda::weights::SubstrateWeight<Self>;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type Scheduler = Scheduler;
    type BlockNumberProvider = System;
    type Currency = pallet_balances::Pallet<Self>;
    type SubmitOrigin = EnsureSigned<AccountId>;
    type CancelOrigin = EnsureRoot<AccountId>;
    type KillOrigin = EnsureRoot<AccountId>;
    type Slash = ();
    type Votes = pallet_conviction_voting::VotesOf<Runtime>;
    type Tally = pallet_conviction_voting::TallyOf<Runtime>;
    type SubmissionDeposit = SubmissionDeposit;
    type MaxQueued = ConstU32<100>;
    type UndecidingTimeout = UndecidingTimeout;
    type AlarmInterval = AlarmInterval;
    type Tracks = TracksInfo;
    type Preimages = Preimage;
}

//

// 840 168 42 14 7 ?

impl pallet_referenda::Config<pallet_referenda::Instance2> for Runtime {
    type WeightInfo = pallet_referenda::weights::SubstrateWeight<Self>;
    type RuntimeCall = RuntimeCall;
    type BlockNumberProvider = System;
    type RuntimeEvent = RuntimeEvent;
    type Scheduler = Scheduler;
    type Currency = pallet_balances::Pallet<Self>;
    type SubmitOrigin = EnsureSigned<AccountId>;
    type CancelOrigin = EnsureRoot<AccountId>;
    type KillOrigin = EnsureRoot<AccountId>;
    type Slash = ();
    type Votes = pallet_ranked_collective::Votes;
    type Tally = pallet_ranked_collective::TallyOf<Runtime>;
    type SubmissionDeposit = SubmissionDeposit;
    type MaxQueued = ConstU32<100>;
    type UndecidingTimeout = UndecidingTimeout;
    type AlarmInterval = AlarmInterval;
    type Tracks = TracksInfo;
    type Preimages = Preimage;
}

parameter_types! {
    pub const ChildBountyValueMinimum: Balance = 1 * DOLLARS;
}

impl pallet_child_bounties::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxActiveChildBountyCount = ConstU32<5>;
    type ChildBountyValueMinimum = ChildBountyValueMinimum;
    type WeightInfo = pallet_child_bounties::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const BountyCuratorDeposit: Permill = Permill::from_percent(50);
    pub const BountyValueMinimum: Balance = 5 * DOLLARS;
    pub const BountyDepositBase: Balance = 1 * DOLLARS;
    pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
    pub const CuratorDepositMin: Balance = 1 * DOLLARS;
    pub const CuratorDepositMax: Balance = 100 * DOLLARS;
    pub const BountyDepositPayoutDelay: BlockNumber = 1 * DAYS;
    pub const BountyUpdatePeriod: BlockNumber = 14 * DAYS;
}

impl pallet_bounties::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type BountyDepositBase = BountyDepositBase;
    type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
    type BountyUpdatePeriod = BountyUpdatePeriod;
    type CuratorDepositMultiplier = CuratorDepositMultiplier;
    type CuratorDepositMin = CuratorDepositMin;
    type CuratorDepositMax = CuratorDepositMax;
    type BountyValueMinimum = BountyValueMinimum;
    type DataDepositPerByte = DataDepositPerByte;
    type MaximumReasonLength = MaximumReasonLength;
    type WeightInfo = pallet_bounties::weights::SubstrateWeight<Runtime>;
    type ChildBountyManager = ChildBounties;
    type OnSlash = Treasury;
}

impl pallet_asset_rate::Config for Runtime {
    type CreateOrigin = EnsureRoot<AccountId>;
    type RemoveOrigin = EnsureRoot<AccountId>;
    type UpdateOrigin = EnsureRoot<AccountId>;
    type Currency = Balances;
    type AssetKind = NativeOrWithId<u32>;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_asset_rate::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = AssetRateArguments;
}

parameter_types! {
    pub const SpendPeriod: BlockNumber = 1 * DAYS;
    pub const Burn: Permill = Permill::from_percent(50);
    pub const TipCountdown: BlockNumber = 1 * DAYS;
    pub const TipFindersFee: Percent = Percent::from_percent(20);
    pub const TipReportDepositBase: Balance = 1 * DOLLARS;
    pub const DataDepositPerByte: Balance = 1 * CENTS;
    pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
    pub const MaximumReasonLength: u32 = 300;
    pub const MaxApprovals: u32 = 100;
    pub const MaxBalance: Balance = Balance::max_value();
    pub const SpendPayoutPeriod: BlockNumber = 30 * DAYS;
}

impl pallet_treasury::Config for Runtime {
    type PalletId = TreasuryPalletId;
    type Currency = Balances;
    type RejectOrigin = EitherOfDiverse<
        EnsureRoot<AccountId>,
        pallet_collective::EnsureProportionMoreThan<AccountId, TechCouncilInstance, 1, 2>,
    >;
    type RuntimeEvent = RuntimeEvent;
    type SpendPeriod = SpendPeriod;
    type Burn = Burn;
    type BurnDestination = ();
    type SpendFunds = Bounties;
    type WeightInfo = pallet_treasury::weights::SubstrateWeight<Runtime>;
    type MaxApprovals = MaxApprovals;
    type SpendOrigin = EnsureWithSuccess<EnsureRoot<AccountId>, AccountId, MaxBalance>;
    type AssetKind = NativeOrWithId<u32>;
    type Beneficiary = AccountId;
    type BeneficiaryLookup = Indices;
    type Paymaster = PayAssetFromAccount<NativeAndAssets, TreasuryAccount>;
    type BalanceConverter = AssetRate;
    type PayoutPeriod = SpendPayoutPeriod;
    type BlockNumberProvider = System;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = PalletTreasuryArguments;
}

impl pallet_ranked_collective::Config for Runtime {
    type WeightInfo = pallet_ranked_collective::weights::SubstrateWeight<Self>;
    type RuntimeEvent = RuntimeEvent;
    type PromoteOrigin = EnsureRootWithSuccess<AccountId, ConstU16<65535>>;
    type DemoteOrigin = EnsureRootWithSuccess<AccountId, ConstU16<65535>>;
    type Polls = RankedPolls;
    type MinRankOfClass = traits::Identity;
    type VoteWeight = pallet_ranked_collective::Geometric;
    type AddOrigin = EnsureRoot<AccountId>;
    type RemoveOrigin = Self::DemoteOrigin;
    type ExchangeOrigin = EnsureRootWithSuccess<AccountId, ConstU16<65535>>;
    type MemberSwappedHandler = (CoreFellowship, Salary);
    type MaxMemberCount = ();
}

parameter_types! {
    pub const AssetDeposit: Balance = 100 * DOLLARS;
    pub const ApprovalDeposit: Balance = 1 * DOLLARS;
    pub const StringLimit: u32 = 50;
    pub const MetadataDepositBase: Balance = 10 * DOLLARS;
    pub const MetadataDepositPerByte: Balance = 1 * DOLLARS;
}

impl pallet_assets::Config<Instance1> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = u128;
    type AssetId = u32;
    type AssetIdParameter = codec::Compact<u32>;
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type ForceOrigin = EnsureRoot<AccountId>;
    type AssetDeposit = AssetDeposit;
    type AssetAccountDeposit = ConstU128<DOLLARS>;
    type MetadataDepositBase = MetadataDepositBase;
    type MetadataDepositPerByte = MetadataDepositPerByte;
    type ApprovalDeposit = ApprovalDeposit;
    type StringLimit = StringLimit;
    type Holder = ();
    type Freezer = ();
    type Extra = ();
    type CallbackHandle = ();
    type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
    type RemoveItemsLimit = ConstU32<1000>;
    type ReserveData = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

impl pallet_skip_feeless_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl pallet_asset_conversion_tx_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AssetId = NativeOrWithId<u32>;
    type OnChargeAssetTransaction = SwapAssetAdapter<
        Native,
        NativeAndAssets,
        AssetConversion,
        ResolveAssetTo<TreasuryAccount, NativeAndAssets>,
    >;
    type WeightInfo = pallet_asset_conversion_tx_payment::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = AssetConversionTxHelper;
}

ord_parameter_types! {
    pub const AssetConversionOrigin: AccountId = AccountIdConversion::<AccountId>::into_account_truncating(&AssetConversionPalletId::get());
}

impl pallet_assets::Config<Instance2> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = u128;
    type AssetId = u32;
    type AssetIdParameter = codec::Compact<u32>;
    type Currency = Balances;
    type CreateOrigin =
        AsEnsureOriginWithArg<frame_system::EnsureSignedBy<AssetConversionOrigin, AccountId>>;
    type ForceOrigin = EnsureRoot<AccountId>;
    type AssetDeposit = AssetDeposit;
    type AssetAccountDeposit = ConstU128<DOLLARS>;
    type MetadataDepositBase = MetadataDepositBase;
    type MetadataDepositPerByte = MetadataDepositPerByte;
    type ApprovalDeposit = ApprovalDeposit;
    type StringLimit = StringLimit;
    type Holder = ();
    type Freezer = ();
    type Extra = ();
    type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
    type RemoveItemsLimit = ConstU32<1000>;
    type CallbackHandle = ();
    type ReserveData = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

parameter_types! {
    pub const AssetConversionPalletId: PalletId = PalletId(*b"py/ascon");
    pub const PoolSetupFee: Balance = 1 * DOLLARS; // should be more or equal to the existential deposit
    pub const MintMinLiquidity: Balance = 100;  // 100 is good enough when the main currency has 10-12 decimals.
    pub const LiquidityWithdrawalFee: Permill = Permill::from_percent(0);
    pub const Native: NativeOrWithId<u32> = NativeOrWithId::Native;
}

pub type NativeAndAssets =
    UnionOf<Balances, Assets, NativeFromLeft, NativeOrWithId<u32>, AccountId>;

impl pallet_asset_conversion::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = u128;
    type HigherPrecisionBalance = sp_core::U256;
    type AssetKind = NativeOrWithId<u32>;
    type Assets = NativeAndAssets;
    type PoolId = (Self::AssetKind, Self::AssetKind);
    type PoolLocator = Chain<
        WithFirstAsset<
            Native,
            AccountId,
            NativeOrWithId<u32>,
            AccountIdConverter<AssetConversionPalletId, Self::PoolId>,
        >,
        Ascending<
            AccountId,
            NativeOrWithId<u32>,
            AccountIdConverter<AssetConversionPalletId, Self::PoolId>,
        >,
    >;
    type PoolAssetId = <Self as pallet_assets::Config<Instance2>>::AssetId;
    type PoolAssets = PoolAssets;
    type PoolSetupFee = PoolSetupFee;
    type PoolSetupFeeAsset = Native;
    type PoolSetupFeeTarget = ResolveAssetTo<AssetConversionOrigin, Self::Assets>;
    type PalletId = AssetConversionPalletId;
    type LPFee = ConstU32<3>; // means 0.3%
    type LiquidityWithdrawalFee = LiquidityWithdrawalFee;
    type WeightInfo = pallet_asset_conversion::weights::SubstrateWeight<Runtime>;
    type MaxSwapPathLength = ConstU32<4>;
    type MintMinLiquidity = MintMinLiquidity;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

pub type NativeAndAssetsFreezer =
    UnionOf<Balances, AssetsFreezer, NativeFromLeft, NativeOrWithId<u32>, AccountId>;

/// Benchmark Helper
#[cfg(feature = "runtime-benchmarks")]
pub struct AssetRewardsBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_asset_rewards::benchmarking::BenchmarkHelper<NativeOrWithId<u32>>
    for AssetRewardsBenchmarkHelper
{
    fn staked_asset() -> NativeOrWithId<u32> {
        NativeOrWithId::<u32>::WithId(100)
    }
    fn reward_asset() -> NativeOrWithId<u32> {
        NativeOrWithId::<u32>::WithId(101)
    }
}

parameter_types! {
    pub const StakingRewardsPalletId: PalletId = PalletId(*b"py/stkrd");
    pub const CreationHoldReason: RuntimeHoldReason =
        RuntimeHoldReason::AssetRewards(pallet_asset_rewards::HoldReason::PoolCreation);
    // 1 item, 135 bytes into the storage on pool creation.
    pub const StakePoolCreationDeposit: Balance = deposit(1, 135);
}

impl pallet_asset_rewards::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type AssetId = NativeOrWithId<u32>;
    type Balance = Balance;
    type Assets = NativeAndAssets;
    type PalletId = StakingRewardsPalletId;
    type BlockNumberProvider = frame_system::Pallet<Runtime>;
    type CreatePoolOrigin = EnsureSigned<AccountId>;
    type WeightInfo = ();
    type AssetsFreezer = NativeAndAssetsFreezer;
    type Consideration = HoldConsideration<
        AccountId,
        Balances,
        CreationHoldReason,
        ConstantStoragePrice<StakePoolCreationDeposit, Balance>,
    >;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = AssetRewardsBenchmarkHelper;
}

impl pallet_asset_conversion_ops::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type PriorAccountIdConverter = pallet_asset_conversion::AccountIdConverterNoSeed<(
        NativeOrWithId<u32>,
        NativeOrWithId<u32>,
    )>;
    type AssetsRefund = <Runtime as pallet_asset_conversion::Config>::Assets;
    type PoolAssetsRefund = <Runtime as pallet_asset_conversion::Config>::PoolAssets;
    type PoolAssetsTeam = <Runtime as pallet_asset_conversion::Config>::PoolAssets;
    type DepositAsset = Balances;
    type WeightInfo = pallet_asset_conversion_ops::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(10) * RuntimeBlockWeights::get().max_block;
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
    type OriginPrivilegeCmp = EqualPrivilegeOnly;
    type Preimages = Preimage;
}

pub struct DynamicParametersManagerOrigin;
impl frame_support::traits::EnsureOriginWithArg<RuntimeOrigin, RuntimeParametersKey>
    for DynamicParametersManagerOrigin
{
    type Success = ();

    fn try_origin(
        origin: RuntimeOrigin,
        key: &RuntimeParametersKey,
    ) -> Result<Self::Success, RuntimeOrigin> {
        match key {
            RuntimeParametersKey::Storage(_) => {
                frame_system::ensure_root(origin.clone()).map_err(|_| origin)?;
                return Ok(());
            }
            RuntimeParametersKey::Referenda(_) => {
                frame_system::ensure_root(origin.clone()).map_err(|_| origin)?;
                return Ok(());
            }
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin(_key: &RuntimeParametersKey) -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::root())
    }
}

impl pallet_parameters::Config for Runtime {
    type RuntimeParameters = RuntimeParameters;
    type RuntimeEvent = RuntimeEvent;
    type AdminOrigin = DynamicParametersManagerOrigin;
    type WeightInfo = ();
}

impl orml_authority::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type Scheduler = Scheduler;
    type AsOriginId = AuthoritysOriginId;
    type AuthorityConfig = AuthorityConfigImpl;
    type WeightInfo = ();
}

impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = ();
}

parameter_types! {
    pub const Budget: Balance = 10_000 * DOLLARS;
    pub TreasuryAccount: AccountId = Treasury::account_id();
}

pub struct SalaryForRank;
impl GetSalary<u16, AccountId, Balance> for SalaryForRank {
    fn get_salary(a: u16, _: &AccountId) -> Balance {
        Balance::from(a) * 1000 * DOLLARS
    }
}

impl pallet_salary::Config for Runtime {
    type WeightInfo = ();
    type RuntimeEvent = RuntimeEvent;
    type Paymaster = PayFromAccount<Balances, TreasuryAccount>;
    type Members = RankedCollective;
    type Salary = SalaryForRank;
    type RegistrationPeriod = ConstU32<200>;
    type PayoutPeriod = ConstU32<200>;
    type Budget = Budget;
}

impl pallet_core_fellowship::Config for Runtime {
    type WeightInfo = ();
    type RuntimeEvent = RuntimeEvent;
    type Members = RankedCollective;
    type Balance = Balance;
    type ParamsOrigin = frame_system::EnsureRoot<AccountId>;
    type InductOrigin = pallet_core_fellowship::EnsureInducted<Runtime, (), 1>;
    type ApproveOrigin = EnsureRootWithSuccess<AccountId, ConstU16<9>>;
    type PromoteOrigin = EnsureRootWithSuccess<AccountId, ConstU16<9>>;
    type FastPromoteOrigin = Self::PromoteOrigin;
    type EvidenceSize = ConstU32<16_384>;
    type MaxRank = ConstU16<9>;
}

parameter_types! {
    pub const DelegatedStakingPalletId: PalletId = PalletId(*b"py/dlstk");
    pub const SlashRewardFraction: Perbill = Perbill::from_percent(1);
}

impl pallet_delegated_staking::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type PalletId = DelegatedStakingPalletId;
    type Currency = Balances;
    type OnSlash = ();
    type SlashRewardFraction = SlashRewardFraction;
    type RuntimeHoldReason = RuntimeHoldReason;
    type CoreStaking = Staking;
}

type TechCouncilInstance = pallet_collective::Instance1;

type EnsureRootOrTwoThridsTechCouncil = EitherOfDiverse<
    EnsureRoot<AccountId>,
    pallet_collective::EnsureProportionAtLeast<AccountId, TechCouncilInstance, 2, 3>,
>;

parameter_types! {
    pub const EraDuration: BlockNumber = 7 * DAYS;
    pub const TechCouncilMotionDuration: BlockNumber = 7 * DAYS;

    pub const TechCouncilMaxMembers: u32 = 9; // 21 eventually
    pub const TechCouncilMaxCandidates: u32 = 100;
    pub const TechCouncilMaxProposals: u32 = 10;

    pub const NominatorAPY: Perbill =     Perbill::from_percent(10);
    pub const CouncilInflation: Perbill = Perbill::from_percent(1);
    pub const CandidacyDeposit: Balance =   1_000_000 * primitives::currency::REEF;
    pub const MinLockAmount: Balance =        100_000 * primitives::currency::REEF;
    pub const TotalLockedCap: Balance = 2_000_000_000 * primitives::currency::REEF;
    pub const ProposalDepositOffset: Balance = NativeTokenExistentialDeposit::get() + NativeTokenExistentialDeposit::get();
    pub const ProposalHoldReason: RuntimeHoldReason =
        RuntimeHoldReason::TechCouncil(pallet_collective::HoldReason::ProposalSubmission);
    pub MaxCollectivesProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;
}

impl pallet_collective::Config<TechCouncilInstance> for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = TechCouncilMotionDuration;
    type MaxProposals = TechCouncilMaxProposals;
    type MaxMembers = TechCouncilMaxMembers;
    type DisapproveOrigin = EnsureRoot<Self::AccountId>;
    type KillOrigin = EnsureRoot<Self::AccountId>;
    type DefaultVote = pallet_collective::MoreThanMajorityThenPrimeDefaultVote;
    type WeightInfo = ();
    type SetMembersOrigin = EnsureRoot<Self::AccountId>;
    type MaxProposalWeight = MaxCollectivesProposalWeight;
    type Consideration = HoldConsideration<
        AccountId,
        Balances,
        ProposalHoldReason,
        pallet_collective::deposit::Delayed<
            ConstU32<2>,
            pallet_collective::deposit::Linear<ConstU32<2>, ProposalDepositOffset>,
        >,
        u32,
    >;
}

impl module_poc::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type EraDuration = EraDuration;
    type NominatorAPY = NominatorAPY;
    type CouncilInflation = CouncilInflation;
    type CandidacyDeposit = CandidacyDeposit;
    type MinLockAmount = MinLockAmount;
    type TotalLockedCap = TotalLockedCap;
    type MaxCandidates = TechCouncilMaxCandidates;
    type MaxMembers = TechCouncilMaxMembers;
    type MembershipChanged = TechCouncil;
    type WeightInfo = ();
}

impl pallet_insecure_randomness_collective_flip::Config for Runtime {}

impl pallet_utility::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const DepositBase: Balance = deposit(1, 88);
    pub const DepositFactor: Balance = deposit(0, 32);
    pub const MaxSignatories: u16 = 100;
}

impl pallet_multisig::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = pallet_multisig::weights::SubstrateWeight<Runtime>;
    type BlockNumberProvider = frame_system::Pallet<Runtime>;
}

// Create the runtime by composing the FRAME pallets that were previously configured.

// workaround for a weird bug in macro
use pallet_session::historical as pallet_session_historical;

#[frame_support::runtime]
mod runtime {
    use super::*;

    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask,
        RuntimeViewFunction
    )]
    pub struct Runtime;

    // Core
    #[runtime::pallet_index(1)]
    pub type RandomnessCollectiveFlip = pallet_insecure_randomness_collective_flip::Pallet<Runtime>;

    #[runtime::pallet_index(2)]
    pub type Timestamp = pallet_timestamp::Pallet<Runtime>;

    #[runtime::pallet_index(3)]
    pub type Sudo = pallet_sudo::Pallet<Runtime>;

    #[runtime::pallet_index(4)]
    pub type Scheduler = pallet_scheduler::Pallet<Runtime>;

    // Account lookup
    #[runtime::pallet_index(5)]
    pub type Indices = pallet_indices::Pallet<Runtime>;

    // Tokens & Fees
    #[runtime::pallet_index(6)]
    pub type Balances = pallet_balances::Pallet<Runtime>;

    #[runtime::pallet_index(7)]
    pub type Currencies = module_currencies::Pallet<Runtime>;

    #[runtime::pallet_index(8)]
    pub type Tokens = orml_tokens::Pallet<Runtime>;

    #[runtime::pallet_index(9)]
    pub type EvmTransactionPayment = module_transaction_payment::Pallet<Runtime>;

    // Authorization + Utility
    #[runtime::pallet_index(10)]
    pub type Authority = orml_authority::Pallet<Runtime>;

    #[runtime::pallet_index(11)]
    pub type Utility = pallet_utility::Pallet<Runtime>;

    #[runtime::pallet_index(12)]
    pub type Multisig = pallet_multisig::Pallet<Runtime>;

    // Smart Contracts
    #[runtime::pallet_index(20)]
    pub type EvmAccounts = module_evm_accounts::Pallet<Runtime>;

    #[runtime::pallet_index(21)]
    pub type EVM = module_evm::Pallet<Runtime>;
    // type EVM = module_evm::{Pallet, Config<T>, Call, Storage, Event<T>};

    #[runtime::pallet_index(22)]
    pub type EVMBridge = module_evm_bridge::Pallet<Runtime>;

    // Consensus
    #[runtime::pallet_index(30)]
    pub type Authorship = pallet_authorship::Pallet<Runtime>;

    #[runtime::pallet_index(31)]
    pub type Babe = pallet_babe::Pallet<Runtime>;

    #[runtime::pallet_index(32)]
    pub type Grandpa = pallet_grandpa::Pallet<Runtime>;

    #[runtime::pallet_index(33)]
    pub type Staking = pallet_staking::Pallet<Runtime>;

    #[runtime::pallet_index(34)]
    pub type Session = pallet_session::Pallet<Runtime>;

    #[runtime::pallet_index(35)]
    pub type Historical = pallet_session_historical::Pallet<Runtime>;

    #[runtime::pallet_index(36)]
    pub type Offences = pallet_offences::Pallet<Runtime>;

    #[runtime::pallet_index(37)]
    pub type ImOnline = pallet_im_online::Pallet<Runtime>;

    #[runtime::pallet_index(38)]
    pub type AuthorityDiscovery = pallet_authority_discovery::Pallet<Runtime>;

    #[runtime::pallet_index(42)]
    pub type ElectionProviderMultiPhase = pallet_election_provider_multi_phase::Pallet<Runtime>;

    #[runtime::pallet_index(43)]
    pub type NominationPools = pallet_nomination_pools::Pallet<Runtime>;

    #[runtime::pallet_index(44)]
    pub type Preimage = pallet_preimage::Pallet<Runtime>;

    #[runtime::pallet_index(45)]
    pub type VoterList = pallet_bags_list::Pallet<Runtime, Instance1>;

    #[runtime::pallet_index(46)]
    pub type Referenda = pallet_referenda::Pallet<Runtime>;

    #[runtime::pallet_index(47)]
    pub type RankedPolls = pallet_referenda::Pallet<Runtime, Instance2>;

    #[runtime::pallet_index(48)]
    pub type ConvictionVoting = pallet_conviction_voting::Pallet<Runtime>;

    #[runtime::pallet_index(49)]
    pub type RankedCollective = pallet_ranked_collective::Pallet<Runtime>;

    // Identity
    #[runtime::pallet_index(51)]
    pub type Identity = pallet_identity::Pallet<Runtime>;

    // PoC & Governance
    #[runtime::pallet_index(52)]
    pub type TechCouncil = pallet_collective::Pallet<Runtime, Instance1>;

    #[runtime::pallet_index(53)]
    pub type Poc = module_poc::Pallet<Runtime>;

    #[runtime::pallet_index(54)]
    pub type Revive = pallet_revive::Pallet<Runtime>;

    #[runtime::pallet_index(55)]
    pub type DelegatedStaking = pallet_delegated_staking::Pallet<Runtime>;

    #[runtime::pallet_index(56)]
    pub type CoreFellowship = pallet_core_fellowship::Pallet<Runtime>;

    #[runtime::pallet_index(57)]
    pub type Salary = pallet_salary::Pallet<Runtime>;

    #[runtime::pallet_index(58)]
    pub type PoolAssets = pallet_assets::Pallet<Runtime, Instance2>;

    // Already Existing
    #[runtime::pallet_index(0)]
    pub type System = frame_system::Pallet<Runtime>;

    #[runtime::pallet_index(50)]
    pub type Assets = pallet_assets::Pallet<Runtime, Instance1>;

    #[runtime::pallet_index(60)]
    pub type AssetRewards = pallet_asset_rewards::Pallet<Runtime>;

    #[runtime::pallet_index(61)]
    pub type AssetsFreezer = pallet_assets_freezer::Pallet<Runtime, Instance1>;

    #[runtime::pallet_index(62)]
    pub type AssetConversion = pallet_asset_conversion::Pallet<Runtime>;

    #[runtime::pallet_index(63)]
    pub type AssetConversionMigration = pallet_asset_conversion_ops::Pallet<Runtime>;

    #[runtime::pallet_index(64)]
    pub type Parameters = pallet_parameters::Pallet<Runtime>;

    #[runtime::pallet_index(65)]
    pub type TransactionPayment = pallet_transaction_payment::Pallet<Runtime>;

    #[runtime::pallet_index(66)]
    pub type Treasury = pallet_treasury::Pallet<Runtime>;

    #[runtime::pallet_index(67)]
    pub type ChildBounties = pallet_child_bounties::Pallet<Runtime>;

    #[runtime::pallet_index(68)]
    pub type Bounties = pallet_bounties::Pallet<Runtime>;

    #[runtime::pallet_index(69)]
    pub type AssetRate = pallet_asset_rate::Pallet<Runtime>;

    #[runtime::pallet_index(70)]
    pub type SkipFeelessPayment = pallet_skip_feeless_payment::Pallet<Runtime>;

    #[runtime::pallet_index(71)]
    pub type AssetConversionTxPayment = pallet_asset_conversion_tx_payment::Pallet<Runtime>;
}

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, AccountIndex>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    module_transaction_payment::ChargeTransactionPayment<Runtime>,
    pallet_revive::evm::tx_extension::SetOrigin<Runtime>,
    frame_system::WeightReclaim<Runtime>,
    // module_evm::SetEvmOrigin<Runtime>,
);

pub type TxExtension = (
    frame_system::AuthorizeCall<Runtime>,
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_skip_feeless_payment::SkipCheckIfFeeless<
        Runtime,
        pallet_asset_conversion_tx_payment::ChargeAssetTxPayment<Runtime>,
    >,
    pallet_revive::evm::tx_extension::SetOrigin<Runtime>,
    frame_system::WeightReclaim<Runtime>,
    // module_evm::SetEvmOrigin<Runtime>,
);

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EthExtraImpl;

impl EthExtra for EthExtraImpl {
    type Config = Runtime;
    type Extension = TxExtension;

    fn get_eth_extension(nonce: u32, tip: Balance) -> Self::Extension {
        (
            frame_system::AuthorizeCall::<Runtime>::new(),
            frame_system::CheckNonZeroSender::<Runtime>::new(),
            frame_system::CheckSpecVersion::<Runtime>::new(),
            frame_system::CheckTxVersion::<Runtime>::new(),
            frame_system::CheckGenesis::<Runtime>::new(),
            frame_system::CheckEra::from(crate::generic::Era::Immortal),
            frame_system::CheckNonce::<Runtime>::from(nonce),
            frame_system::CheckWeight::<Runtime>::new(),
            pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(tip, None)
                .into(),
            pallet_revive::evm::tx_extension::SetOrigin::<Runtime>::new_from_eth_transaction(),
            frame_system::WeightReclaim::<Runtime>::new(),
            // module_evm::SetEvmOrigin::<Runtime>::new(),
        )
    }
}

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    pallet_revive::evm::runtime::UncheckedExtrinsic<Address, Signature, EthExtraImpl>;

pub type UncheckedExtrinsic2 =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, TxExtension>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, TxExtension>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
    Migrations,
>;

impl<C> frame_system::offchain::CreateTransactionBase<C> for Runtime
where
    RuntimeCall: From<C>,
{
    type Extrinsic = UncheckedExtrinsic;
    type RuntimeCall = RuntimeCall;
}

impl<LocalCall> frame_system::offchain::CreateBare<LocalCall> for Runtime
where
    RuntimeCall: From<LocalCall>,
{
    fn create_bare(call: RuntimeCall) -> UncheckedExtrinsic {
        generic::UncheckedExtrinsic::new_bare(call).into()
    }
}

impl<LocalCall> frame_system::offchain::CreateTransaction<LocalCall> for Runtime
where
    RuntimeCall: From<LocalCall>,
{
    type Extension = TxExtension;

    fn create_transaction(call: RuntimeCall, extension: TxExtension) -> UncheckedExtrinsic {
        generic::UncheckedExtrinsic::new_transaction(call, extension).into()
    }
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
    RuntimeCall: From<LocalCall>,
{
    fn create_signed_transaction<
        C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>,
    >(
        call: RuntimeCall,
        public: <Signature as sp_runtime::traits::Verify>::Signer,
        account: AccountId,
        nonce: Nonce,
    ) -> Option<UncheckedExtrinsic> {
        // take the biggest period possible.
        let period = BlockHashCount::get()
            .checked_next_power_of_two()
            .map(|c| c / 2)
            .unwrap_or(2) as u64;
        let current_block = System::block_number()
            .saturated_into::<u64>()
            // The `System::block_number` is initialized with `n+1`,
            // so the actual block number is `n`.
            .saturating_sub(1);
        let tip = 0;
        let tx_ext: TxExtension = (
            frame_system::AuthorizeCall::<Runtime>::new(),
            frame_system::CheckNonZeroSender::<Runtime>::new(),
            frame_system::CheckSpecVersion::<Runtime>::new(),
            frame_system::CheckTxVersion::<Runtime>::new(),
            frame_system::CheckGenesis::<Runtime>::new(),
            frame_system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
            frame_system::CheckNonce::<Runtime>::from(nonce),
            frame_system::CheckWeight::<Runtime>::new(),
            pallet_skip_feeless_payment::SkipCheckIfFeeless::from(
                pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(
                    tip, None,
                ),
            ),
            pallet_revive::evm::tx_extension::SetOrigin::<Runtime>::default(),
            frame_system::WeightReclaim::<Runtime>::new(),
            // module_evm::SetEvmOrigin::<Runtime>::new(),
        );
        let raw_payload = SignedPayload::new(call, tx_ext)
            .map_err(|e| {
                log::warn!("Unable to create signed payload: {:?}", e);
            })
            .ok()?;
        let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
        let address = Indices::unlookup(account);
        let (call, tx_ext, _) = raw_payload.deconstruct();
        let transaction =
            generic::UncheckedExtrinsic::new_signed(call, address, signature, tx_ext).into();
        Some(transaction)
    }
}

impl frame_system::offchain::SigningTypes for Runtime {
    type Public = <Signature as sp_runtime::traits::Verify>::Signer;
    type Signature = Signature;
}

pallet_revive::impl_runtime_apis_plus_revive_traits!(
    Runtime,
    Revive,
    Executive,
    EthExtraImpl,

    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: <Block as BlockT>::LazyBlock) {
            Executive::execute_block(block);
        }

        fn initialize_block(header: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }
        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }
        fn metadata_versions() -> sp_std::vec::Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: <Block as BlockT>::LazyBlock,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
        fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
            build_state::<RuntimeGenesisConfig>(config)
        }

        fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
            get_preset::<RuntimeGenesisConfig>(id, |_| None)
        }

        fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
            vec![]
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_consensus_babe::BabeApi<Block> for Runtime {
        fn configuration() -> sp_consensus_babe::BabeConfiguration {
            sp_consensus_babe::BabeConfiguration {
                slot_duration: Babe::slot_duration(),
                epoch_length: EpochDuration::get(),
                c: BABE_GENESIS_EPOCH_CONFIG.c,
                authorities: Babe::authorities().to_vec(),
                randomness: Babe::randomness(),
                allowed_slots: BABE_GENESIS_EPOCH_CONFIG.allowed_slots,
            }
        }

        fn current_epoch_start() -> sp_consensus_babe::Slot {
            Babe::current_epoch_start()
        }

        fn current_epoch() -> sp_consensus_babe::Epoch {
            Babe::current_epoch()
        }

        fn next_epoch() -> sp_consensus_babe::Epoch {
            Babe::next_epoch()
        }

        fn generate_key_ownership_proof(
            _slot_number: sp_consensus_babe::Slot,
            authority_id: sp_consensus_babe::AuthorityId,
            ) -> Option<sp_consensus_babe::OpaqueKeyOwnershipProof> {
            use codec::Encode;

            Historical::prove((sp_consensus_babe::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(sp_consensus_babe::OpaqueKeyOwnershipProof::new)
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: sp_consensus_babe::EquivocationProof<<Block as BlockT>::Header>,
            key_owner_proof: sp_consensus_babe::OpaqueKeyOwnershipProof,
            ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;

            Babe::submit_unsigned_equivocation_report(
                equivocation_proof,
                key_owner_proof,
                )
        }
    }

    impl sp_authority_discovery::AuthorityDiscoveryApi<Block> for Runtime {
        fn authorities() -> Vec<AuthorityDiscoveryId> {
            AuthorityDiscovery::authorities()
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
            opaque::SessionKeys::generate(seed)
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl fg_primitives::GrandpaApi<Block> for Runtime {
        fn grandpa_authorities() -> GrandpaAuthorityList {
            Grandpa::grandpa_authorities()
        }

        fn current_set_id() -> fg_primitives::SetId {
            Grandpa::current_set_id()
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            _equivocation_proof: fg_primitives::EquivocationProof<
                <Block as BlockT>::Hash,
                NumberFor<Block>,
            >,
            _key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            None
        }

        fn generate_key_ownership_proof(
            _set_id: fg_primitives::SetId,
            _authority_id: GrandpaId,
        ) -> Option<fg_primitives::OpaqueKeyOwnershipProof> {
            // NOTE: this is the only implementation possible since we've
            // defined our key owner proof type as a bottom type (i.e. a type
            // with no values).
            None
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
        fn account_nonce(account: AccountId) -> Nonce {
            System::account_nonce(account)
        }
    }

    impl assets_api::AssetsApi<
        Block,
        AccountId,
        Balance,
        u32,
    > for Runtime
    {
        fn account_balances(account: AccountId) -> Vec<(u32, Balance)> {
            Assets::account_balances(account)
        }
    }

    impl pallet_staking_runtime_api::StakingApi<Block, Balance,AccountId> for Runtime {
        fn nominations_quota(balance: Balance) -> u32 {
            Staking::api_nominations_quota(balance)
        }
        fn eras_stakers_page_count(era: sp_staking::EraIndex, account: AccountId) -> sp_staking::Page {
            Staking::api_eras_stakers_page_count(era, account)
        }

        fn pending_rewards(era: sp_staking::EraIndex, account: AccountId) -> bool {
            Staking::api_pending_rewards(era, account)
        }
    }

   impl pallet_nomination_pools_runtime_api::NominationPoolsApi<Block, AccountId, Balance> for Runtime {
        fn pending_rewards(who: AccountId) -> Balance {
            NominationPools::api_pending_rewards(who).unwrap_or_default()
        }

        fn points_to_balance(pool_id: PoolId, points: Balance) -> Balance {
            NominationPools::api_points_to_balance(pool_id, points)
        }

        fn balance_to_points(pool_id: PoolId, new_funds: Balance) -> Balance {
            NominationPools::api_balance_to_points(pool_id, new_funds)
        }

        fn pool_pending_slash(pool_id: PoolId) -> Balance {
            NominationPools::api_pool_pending_slash(pool_id)
        }

        fn member_pending_slash(member: AccountId) -> Balance {
            NominationPools::api_member_pending_slash(member)
        }

        fn pool_needs_delegate_migration(pool_id: PoolId) -> bool {
            NominationPools::api_pool_needs_delegate_migration(pool_id)
        }

        fn member_needs_delegate_migration(member: AccountId) -> bool {
            NominationPools::api_member_needs_delegate_migration(member)
        }

        fn member_total_balance(member: AccountId) -> Balance {
            NominationPools::api_member_total_balance(member)
        }

        fn pool_balance(pool_id: PoolId) -> Balance {
            NominationPools::api_pool_balance(pool_id)
        }

        fn pool_accounts(pool_id: PoolId) -> (AccountId, AccountId) {
            NominationPools::api_pool_accounts(pool_id)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
        for Runtime
    {
        fn query_call_info(call: RuntimeCall, len: u32) -> RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_call_info(call, len)
        }
        fn query_call_fee_details(call: RuntimeCall, len: u32) -> FeeDetails<Balance> {
            TransactionPayment::query_call_fee_details(call, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
         TransactionPayment::query_info(uxt, len)

        }
        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment_rpc_runtime_api::FeeDetails<Balance> {
            TransactionPayment::query_fee_details(uxt, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl module_evm_rpc_runtime_api::EVMRuntimeRPCApi<Block, Balance> for Runtime {
        fn call(
            from: H160,
            to: H160,
            data: Vec<u8>,
            value: Balance,
            gas_limit: u64,
            storage_limit: u32,
            estimate: bool,
        ) -> Result<CallInfo, sp_runtime::DispatchError> {
            let mut config = <Runtime as module_evm::Config>::config().clone();
            if estimate {
                config.estimate = true;
            }
            module_evm::Runner::<Runtime>::call(
                from,
                from,
                to,
                data,
                value,
                gas_limit,
                storage_limit,
                &config,
            )
        }

        fn create(
            from: H160,
            data: Vec<u8>,
            value: Balance,
            gas_limit: u64,
            storage_limit: u32,
            estimate: bool,
        ) -> Result<CreateInfo, sp_runtime::DispatchError> {
            let mut config = <Runtime as module_evm::Config>::config().clone();
            if estimate {
                config.estimate = true;
            }
            module_evm::Runner::<Runtime>::create(
                from,
                data,
                value,
                gas_limit,
                storage_limit,
                &config,
            )
        }

        fn get_estimate_resources_request(
            extrinsic: Vec<u8>,
        ) -> Result<EstimateResourcesRequest, sp_runtime::DispatchError> {

          let utx = UncheckedExtrinsic2::decode(&mut &*extrinsic)
                .map_err(|_| sp_runtime::DispatchError::Other("Invalid parameter extrinsic, decode failed"))?;

            let request = match utx.function {
                RuntimeCall::EVM(module_evm::Call::call{target, input, value, gas_limit, storage_limit}) => {
                    Some(EstimateResourcesRequest {
                        from: None,
                        to: Some(target),
                        gas_limit: Some(gas_limit),
                        storage_limit: Some(storage_limit),
                        value: Some(value),
                        data: Some(input),
                    })
                }
                RuntimeCall::EVM(module_evm::Call::create{init, value, gas_limit, storage_limit}) => {
                    Some(EstimateResourcesRequest {
                        from: None,
                        to: None,
                        gas_limit: Some(gas_limit),
                        storage_limit: Some(storage_limit),
                        value: Some(value),
                        data: Some(init),
                    })
                }
                _ => None,
            };

            request.ok_or(sp_runtime::DispatchError::Other("Invalid parameter extrinsic, not evm Call"))
        }

    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::{list_benchmark, Benchmarking, BenchmarkList};
            use frame_support::traits::StorageInfoTrait;
            use frame_system_benchmarking::Pallet as SystemBench;
            use orml_benchmarking::{list_benchmark as orml_list_benchmark};

            let mut list = Vec::<BenchmarkList>::new();

            list_benchmark!(list, extra, frame_system, SystemBench::<Runtime>);
            list_benchmark!(list, extra, pallet_balances, Balances);
            list_benchmark!(list, extra, pallet_timestamp, Timestamp);
            list_benchmark!(list, extra, module_poc, Poc);

            orml_list_benchmark!(list, extra, evm, benchmarking::evm);
            orml_list_benchmark!(list, extra, evm_accounts, benchmarking::evm_accounts);

            let storage_info = AllPalletsWithSystem::storage_info();

            return (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
            use frame_benchmarking::{Benchmarking, BenchmarkBatch, add_benchmark, TrackedStorageKey};
            use orml_benchmarking::{add_benchmark as orml_add_benchmark};

            use frame_system_benchmarking::Pallet as SystemBench;
            impl frame_system_benchmarking::Config for Runtime {}

            let whitelist: Vec<TrackedStorageKey> = vec![
                // Block Number
                hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
                // Total Issuance
                hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
                // Execution Phase
                hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
                // Event Count
                hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
                // System Events
                hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
            ];

            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);

            add_benchmark!(params, batches, frame_system, SystemBench::<Runtime>);
            add_benchmark!(params, batches, pallet_balances, Balances);
            add_benchmark!(params, batches, pallet_timestamp, Timestamp);
            add_benchmark!(params, batches, module_poc, Poc);

            orml_add_benchmark!(params, batches, evm, benchmarking::evm);
            orml_add_benchmark!(params, batches, evm_accounts, benchmarking::evm_accounts);

            if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
            Ok(batches)
        }
    }
);
