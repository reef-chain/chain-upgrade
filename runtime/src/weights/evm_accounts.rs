//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> module_evm_accounts::WeightInfo for WeightInfo<T> {
    fn claim_account() -> Weight {
        Weight::from_parts(1_119_200_000, 0)
            .saturating_add(DbWeight::get().reads(3 as u64))
            .saturating_add(DbWeight::get().writes(2 as u64))
    }
    fn claim_default_account() -> Weight {
        Weight::from_parts(304_000_000, 0)
            .saturating_add(DbWeight::get().reads(1 as u64))
            .saturating_add(DbWeight::get().writes(2 as u64))
    }
}
