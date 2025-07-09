//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(clippy::unnecessary_cast)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

impl crate::WeightInfo for () {
    fn on_finalize() -> Weight {
        Weight::from_parts(39_708_000, 0)
            .saturating_add(DbWeight::get().reads(2 as u64))
            .saturating_add(DbWeight::get().writes(1 as u64))
    }

    fn set_default_fee_token() -> Weight {
        Weight::from_parts(1_000_000, 0)
            .saturating_add(DbWeight::get().writes(1 as u64))
            .into()
    }
}
