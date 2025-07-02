//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(clippy::unnecessary_cast)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

impl crate::WeightInfo for () {
    fn claim_account() -> Weight {
        Weight::from_parts(1_253_760_000, 0)
            .saturating_add(DbWeight::get().reads(3 as u64))
            .saturating_add(DbWeight::get().writes(4 as u64))
    }

    fn claim_default_account() -> Weight {
        Weight::from_parts(304_000_000, 0)
            .saturating_add(DbWeight::get().reads(1 as u64))
            .saturating_add(DbWeight::get().writes(2 as u64))
    }
}
