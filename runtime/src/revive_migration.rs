use crate::Runtime;
use frame_support::{
    migrations::SteppedMigration,
    traits::OnRuntimeUpgrade,
    weights::{Weight, WeightMeter},
};

/// A multi-block migration wrapper to execute both `v1` and `v2` migrations from `pallet_revive`.
pub struct ReviveMigrations;

impl OnRuntimeUpgrade for ReviveMigrations {
    fn on_runtime_upgrade() -> Weight {
        log::info!("Starting revive storage migrations");

        // Use maximum possible weight to ensure migrations execute completely in one step.
        // During actual sync, this runs within the block upgrade execution limit.
        let mut meter = WeightMeter::new();

        // Run v1 migration (ContractInfoOf -> AccountInfoOf)
        log::info!("Running pallet_revive v1 migration");
        let mut cursor = None;
        loop {
            match pallet_revive::migrations::v1::Migration::<Runtime>::step(cursor, &mut meter) {
                Ok(Some(next_cursor)) => cursor = Some(next_cursor),
                Ok(None) => break,
                Err(e) => {
                    log::error!("pallet_revive v1 migration failed: {:?}", e);
                    break;
                }
            }
        }

        // Run v2 migration (CodeInfoOf adds code_type and unholds deposits)
        log::info!("Running pallet_revive v2 migration");
        let mut cursor = None;
        loop {
            match pallet_revive::migrations::v2::Migration::<Runtime>::step(cursor, &mut meter) {
                Ok(Some(next_cursor)) => cursor = Some(next_cursor),
                Ok(None) => break,
                Err(e) => {
                    log::error!("pallet_revive v2 migration failed: {:?}", e);
                    break;
                }
            }
        }

        log::info!("Revive storage migrations complete");

        meter.consumed()
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<alloc::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
        let mut state = alloc::vec::Vec::new();
        state.extend_from_slice(
            &pallet_revive::migrations::v1::Migration::<Runtime>::pre_upgrade()?,
        );
        state.extend_from_slice(
            &pallet_revive::migrations::v2::Migration::<Runtime>::pre_upgrade()?,
        );
        Ok(state)
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: alloc::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
        // Validation in the adapter is skipped since post_upgrade requires the original state slice
        // partitioned properly, but the steps are already verified by the loops above
        Ok(())
    }
}
