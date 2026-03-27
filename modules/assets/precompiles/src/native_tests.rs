// Test mock for NativeERC20 precompile.

#[cfg(test)]
pub mod mock {
	use frame_support::{derive_impl, traits::AsEnsureOriginWithArg};
	use sp_runtime::BuildStorage;

	type Block = frame_system::mocking::MockBlock<Test>;

	#[frame_support::runtime]
	mod runtime {
		#[runtime::runtime]
		#[runtime::derive(
			RuntimeCall,
			RuntimeEvent,
			RuntimeError,
			RuntimeOrigin,
			RuntimeTask,
			RuntimeHoldReason,
			RuntimeFreezeReason
		)]
		pub struct Test;

		#[runtime::pallet_index(0)]
		pub type System = frame_system;
		#[runtime::pallet_index(10)]
		pub type Balances = pallet_balances;
		#[runtime::pallet_index(20)]
		pub type Assets = pallet_assets;
		#[runtime::pallet_index(21)]
		pub type Revive = pallet_revive;
	}

	#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
	impl frame_system::Config for Test {
		type Block = Block;
		type AccountData = pallet_balances::AccountData<u64>;
	}

	#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
	impl pallet_balances::Config for Test {
		type AccountStore = System;
	}

	#[derive_impl(pallet_assets::config_preludes::TestDefaultConfig as pallet_assets::DefaultConfig)]
	impl pallet_assets::Config for Test {
		type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<u64>>;
		type ForceOrigin = frame_system::EnsureRoot<u64>;
		type Currency = Balances;
	}

	#[derive_impl(pallet_revive::config_preludes::TestDefaultConfig)]
	impl pallet_revive::Config for Test {
		type AddressMapper = pallet_revive::TestAccountMapper<Self>;
		type Balance = u64;
		type Currency = Balances;
		type Precompiles = (crate::NativeERC20<Self>,);
	}

	pub use runtime::*;

	pub fn new_test_ext() -> sp_io::TestExternalities {
		let t = RuntimeGenesisConfig {
			assets: pallet_assets::GenesisConfig {
				assets: vec![],
				metadata: vec![],
				accounts: vec![],
				next_asset_id: None,
				reserves: vec![],
			},
			system: Default::default(),
			balances: Default::default(),
			revive: Default::default(),
		}
		.build_storage()
		.unwrap();
		let mut ext: sp_io::TestExternalities = t.into();
		ext.execute_with(|| {
			System::set_block_number(1);
		});
		ext
	}
}
