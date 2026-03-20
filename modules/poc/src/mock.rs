#![cfg(test)]

use crate as module_poc;
use frame_support::{construct_runtime, derive_impl, parameter_types};
pub use primitives::{currency::*, time::*, BlockNumber};
use sp_runtime::Perbill;

type Balance = u64;
type TechCouncilInstance = pallet_collective::Instance1;

parameter_types!(
    pub const BlockHashCount: u32 = 250;
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Runtime {
    type BaseCallFilter = frame_support::traits::Everything;
    type Hash = sp_runtime::testing::H256;
    type Hashing = sp_runtime::traits::BlakeTwo256;
    type AccountId = u64;
    type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
    type BlockHashCount = BlockHashCount;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type Block = frame_system::mocking::MockBlock<Runtime>;
}

parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}
impl pallet_balances::Config for Runtime {
    type MaxLocks = MaxLocks;
    type Balance = u64;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxFreezes = frame_support::traits::VariantCountOf<RuntimeFreezeReason>;
    type DoneSlashHandler = ();
}

parameter_types! {
    pub const TechCouncilMotionDuration: BlockNumber = 7 * HOURS;
    pub const TechCouncilMaxProposals: u32 = 100;
    pub const TechCouncilMaxMembers: u32 = 3;
    pub const TechCouncilMaxCandidates: u32 = 100;
    pub MaxCollectivesProposalWeight: frame_support::weights::Weight = frame_support::weights::Weight::from_parts(
        frame_support::weights::constants::WEIGHT_REF_TIME_PER_SECOND,
        u64::MAX,
    );
}

impl pallet_collective::Config<TechCouncilInstance> for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = TechCouncilMotionDuration;
    type MaxProposals = TechCouncilMaxProposals;
    type MaxMembers = TechCouncilMaxMembers;
    type DefaultVote = pallet_collective::MoreThanMajorityThenPrimeDefaultVote;
    type WeightInfo = ();
    type SetMembersOrigin = frame_system::EnsureRoot<u64>;
    type MaxProposalWeight = MaxCollectivesProposalWeight;
    type DisapproveOrigin = frame_system::EnsureRoot<u64>;
    type KillOrigin = frame_system::EnsureRoot<u64>;
    type Consideration = ();
}

parameter_types! {
    pub const EraDuration: BlockNumber = 7 * HOURS;
    pub const NominatorAPY: Perbill = Perbill::from_percent(10);
    pub const CouncilInflation: Perbill = Perbill::from_percent(1);
    pub const CandidacyDeposit: Balance = 250_000;
    pub const MinLockAmount: Balance = 100;
    pub const TotalLockedCap: Balance = 10_000_000;
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

// type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
    pub enum Runtime {
        System: frame_system,
        Balances: pallet_balances,
        TechCouncil: pallet_collective::<Instance1>,
        Poc: module_poc,
    }
);

pub type Origin = RuntimeOrigin;
pub type Call = RuntimeCall;

pub fn new_test_ext() -> sp_io::TestExternalities {
    use sp_runtime::BuildStorage;
    let mut t = frame_system::GenesisConfig::<Runtime>::default()
        .build_storage()
        .unwrap();

    // inject test balances
    let balances_config = pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![
            (0, 1_000_000), // alice
            (1, 1_000_000), // bob
            (2, 1_000_000), // charlie
            (3, 1_000_000), // eve
        ],
        dev_accounts: None,
    };
    balances_config.assimilate_storage(&mut t).unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));

    ext
}

pub fn tech_council_members() -> Vec<u64> {
    pallet_collective::Members::<Runtime, TechCouncilInstance>::get()
}
