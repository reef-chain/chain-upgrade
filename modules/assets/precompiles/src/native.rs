// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! ERC20 precompile for the native REEF token.
//!
//! Exposes the native [`pallet_revive::Config::Currency`] (i.e. `pallet_balances`) as an ERC20
//! contract at the fixed address `0x0000000000000000000000000000000001000000`.
//!
//! ## Allowances
//! `pallet_balances` has no built-in ERC20 allowance concept. Allowances are therefore stored
//! in on-chain storage under a deterministic prefix, using [`sp_io::storage`].

use alloc::vec::Vec;
use core::{marker::PhantomData, num::NonZero};
use ethereum_standards::IERC20::{self, IERC20Calls, IERC20Events};
use frame_support::traits::{
    fungible::{Inspect, Mutate},
    tokens::Preservation,
};
use pallet_revive::precompiles::{
    alloy::{
        primitives::{IntoLogData, U256 as AlloyU256},
        sol_types::{Revert, SolCall},
    },
    AddressMapper, AddressMatcher, Error, Ext, Precompile, RuntimeCosts, H160, H256,
};

/// The fixed 20-byte ERC20 address: `0x0000000000000000000000000000000001000000`.
///
/// Address layout (big-endian):
///  bytes 0-15 : 0x00…
///  bytes 16-19: 0x01 0x00 0x00 0x00
///
/// Derived from `AddressMatcher::Fixed(NonZero::new(0x0100))` which left-shifts 0x0100 by 16
/// bits into the u32 `0x01000000`, then places that as big-endian at bytes 16-19.
pub const NATIVE_ERC20_ADDRESS: [u8; 20] = {
    let mut addr = [0u8; 20];
    // 0x0100 << 16 = 0x01000000 → bytes [01, 00, 00, 00] at positions [16..=19]
    addr[16] = 0x01;
    addr[17] = 0x00;
    addr[18] = 0x00;
    addr[19] = 0x00;
    addr
};

/// An ERC20 precompile for the native REEF token.
///
/// Forwards ERC20 calls to `pallet_balances` (via `T::Currency`).
pub struct NativeERC20<T>(PhantomData<T>);

impl<T> Precompile for NativeERC20<T>
where
    T: pallet_revive::Config<Balance = u128>,
    u128: TryInto<T::Balance>,
    T::Balance: Into<u128>,
{
    type T = T;
    type Interface = IERC20::IERC20Calls;

    /// Fixed address: `0x0000000000000000000000000000000001000000`
    ///
    /// `NonZero::new(0x0100)` left-shifted by 16 = `0x01000000` as u32 suffix.
    const MATCHER: AddressMatcher =
        AddressMatcher::Fixed(NonZero::new(0x0100).expect("0x0100 is non-zero; qed"));

    const HAS_CONTRACT_INFO: bool = false;

    fn call(
        address: &[u8; 20],
        input: &Self::Interface,
        env: &mut impl Ext<T = Self::T>,
    ) -> Result<Vec<u8>, Error> {
        let _ = address; // fixed address, not used for routing

        match input {
            IERC20Calls::transfer(_) | IERC20Calls::approve(_) | IERC20Calls::transferFrom(_)
                if env.is_read_only() =>
            {
                Err(Error::Error(
                    pallet_revive::Error::<T>::StateChangeDenied.into(),
                ))
            }

            IERC20Calls::totalSupply(_) => Self::total_supply(env),
            IERC20Calls::balanceOf(call) => Self::balance_of(call, env),
            IERC20Calls::transfer(call) => Self::transfer(call, env),
            IERC20Calls::allowance(call) => Self::allowance(call, env),
            IERC20Calls::approve(call) => Self::approve(call, env),
            IERC20Calls::transferFrom(call) => Self::transfer_from(call, env),
        }
    }
}

const ERR_INVALID_CALLER: &str = "Invalid caller: origin must be a signed account";
const ERR_BALANCE_CONVERSION: &str = "Balance conversion failed";
const ERR_INSUFFICIENT_ALLOWANCE: &str = "Insufficient allowance";

impl<T> NativeERC20<T>
where
    T: pallet_revive::Config<Balance = u128>,
    u128: TryInto<T::Balance>,
    T::Balance: Into<u128>,
{
    /// Return the caller's Ethereum address.
    fn caller_address(env: &mut impl Ext<T = T>) -> Result<H160, Error> {
        env.caller()
            .account_id()
            .map(<T as pallet_revive::Config>::AddressMapper::to_address)
            .map_err(|_| {
                Error::Revert(Revert {
                    reason: ERR_INVALID_CALLER.into(),
                })
            })
    }

    /// Convert an alloy `U256` value to `T::Balance` (via u128).
    fn to_balance(value: AlloyU256) -> Result<T::Balance, Error> {
        // Clamp to u128::MAX and convert via u128 — T::Balance is u128 in Reef runtime.
        let as_u128 = u128::try_from(value).map_err(|_| {
            Error::Revert(Revert {
                reason: ERR_BALANCE_CONVERSION.into(),
            })
        })?;
        as_u128.try_into().map_err(|_| {
            Error::Revert(Revert {
                reason: ERR_BALANCE_CONVERSION.into(),
            })
        })
    }

    /// Convert `T::Balance` (u128) to alloy `U256`.
    fn to_alloy_u256(balance: T::Balance) -> AlloyU256 {
        let as_u128: u128 = balance.into();
        AlloyU256::from(as_u128)
    }

    /// Emit an ERC20 log event.
    fn deposit_event(env: &mut impl Ext<T = T>, event: IERC20Events) -> Result<(), Error> {
        let (topics, data) = event.into_log_data().split();
        let topics = topics.into_iter().map(|v| H256(v.0)).collect::<Vec<_>>();
        env.frame_meter_mut()
            .charge_weight_token(RuntimeCosts::DepositEvent {
                num_topic: topics.len() as u32,
                len: data.len() as u32,
            })?;
        env.deposit_event(topics, data.to_vec());
        Ok(())
    }

    // ── ERC20 handlers ───────────────────────────────────────────────────

    /// `totalSupply()` → `T::Currency::total_issuance()`.
    fn total_supply(env: &mut impl Ext<T = T>) -> Result<Vec<u8>, Error> {
        env.charge(frame_support::weights::Weight::from_parts(100_000, 0))?;
        let total = pallet_revive::TotalReviveIssuance::<T>::get();
        let value = Self::to_alloy_u256(total);
        Ok(IERC20::totalSupplyCall::abi_encode_returns(&value))
    }

    /// `balanceOf(address account)` → free balance of `account` in native token.
    fn balance_of(
        call: &IERC20::balanceOfCall,
        env: &mut impl Ext<T = T>,
    ) -> Result<Vec<u8>, Error> {
        env.charge(frame_support::weights::Weight::from_parts(100_000, 0))?;
        let address: H160 = call.account.into_array().into();
        let account = <T as pallet_revive::Config>::AddressMapper::to_account_id(&address);
        let balance = <T as pallet_revive::Config>::Currency::balance(&account);
        let value = Self::to_alloy_u256(balance);
        Ok(IERC20::balanceOfCall::abi_encode_returns(&value))
    }

    /// `transfer(address to, uint256 value)` — transfers native token from caller to `to`.
    fn transfer(call: &IERC20::transferCall, env: &mut impl Ext<T = T>) -> Result<Vec<u8>, Error> {
        env.charge(frame_support::weights::Weight::from_parts(500_000, 0))?;
        let from_addr = Self::caller_address(env)?;
        let from = <T as pallet_revive::Config>::AddressMapper::to_account_id(&from_addr);
        let to_addr: H160 = call.to.into_array().into();
        let to = <T as pallet_revive::Config>::AddressMapper::to_account_id(&to_addr);
        let amount = Self::to_balance(call.value)?;

        <T as pallet_revive::Config>::Currency::transfer(
            &from,
            &to,
            amount,
            Preservation::Preserve,
        )
        .map_err(|e| Error::Error(e.into()))?;

        Self::deposit_event(
            env,
            IERC20Events::Transfer(IERC20::Transfer {
                from: from_addr.0.into(),
                to: call.to,
                value: call.value,
            }),
        )?;

        Ok(IERC20::transferCall::abi_encode_returns(&true))
    }

    /// `allowance(address owner, address spender)` — reads the stored allowance.
    fn allowance(
        call: &IERC20::allowanceCall,
        env: &mut impl Ext<T = T>,
    ) -> Result<Vec<u8>, Error> {
        env.charge(frame_support::weights::Weight::from_parts(50_000, 0))?;
        let owner: H160 = call.owner.into_array().into();
        let spender: H160 = call.spender.into_array().into();
        let balance = pallet_revive::NativeERC20Allowances::<T>::get(owner, spender);
        let value = Self::to_alloy_u256(balance);
        Ok(IERC20::allowanceCall::abi_encode_returns(&value))
    }

    /// `approve(address spender, uint256 value)` — sets allowance for `spender`.
    fn approve(call: &IERC20::approveCall, env: &mut impl Ext<T = T>) -> Result<Vec<u8>, Error> {
        env.charge(frame_support::weights::Weight::from_parts(150_000, 0))?;
        let owner_addr = Self::caller_address(env)?;
        let spender: H160 = call.spender.into_array().into();
        let amount = Self::to_balance(call.value)?;
        pallet_revive::NativeERC20Allowances::<T>::insert(owner_addr, spender, amount);

        Self::deposit_event(
            env,
            IERC20Events::Approval(IERC20::Approval {
                owner: owner_addr.0.into(),
                spender: call.spender,
                value: call.value,
            }),
        )?;

        Ok(IERC20::approveCall::abi_encode_returns(&true))
    }

    /// `transferFrom(address from, address to, uint256 value)` — spends allowance and transfers.
    fn transfer_from(
        call: &IERC20::transferFromCall,
        env: &mut impl Ext<T = T>,
    ) -> Result<Vec<u8>, Error> {
        env.charge(frame_support::weights::Weight::from_parts(600_000, 0))?;
        let spender_addr = Self::caller_address(env)?;

        let from_addr: H160 = call.from.into_array().into();
        let to_h160: H160 = call.to.into_array().into();
        let amount = Self::to_balance(call.value)?;

        // Check and decrement allowance
        let current = pallet_revive::NativeERC20Allowances::<T>::get(from_addr, spender_addr);
        if current < amount {
            return Err(Error::Revert(Revert {
                reason: ERR_INSUFFICIENT_ALLOWANCE.into(),
            }));
        }
        pallet_revive::NativeERC20Allowances::<T>::insert(
            from_addr,
            spender_addr,
            current.saturating_sub(amount),
        );

        // Execute transfer
        let from_account = <T as pallet_revive::Config>::AddressMapper::to_account_id(&from_addr);
        let to_account = <T as pallet_revive::Config>::AddressMapper::to_account_id(&to_h160);

        <T as pallet_revive::Config>::Currency::transfer(
            &from_account,
            &to_account,
            amount,
            Preservation::Preserve,
        )
        .map_err(|e| Error::Error(e.into()))?;

        Self::deposit_event(
            env,
            IERC20Events::Transfer(IERC20::Transfer {
                from: call.from,
                to: call.to,
                value: call.value,
            }),
        )?;

        Ok(IERC20::transferFromCall::abi_encode_returns(&true))
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::alloy::hex;
    use alloy::primitives::U256 as AU256;
    use frame_support::{assert_ok, traits::Currency};
    use pallet_revive::{precompiles::TransactionLimits, ExecConfig};
    use sp_runtime::Weight;

    /// The precompile address in hex: `0000000000000000000000000000000001000000`
    const PRECOMPILE_ADDR_HEX: &[u8; 40] = b"0000000000000000000000000000000001000000";

    fn precompile_h160() -> H160 {
        H160::from(hex::const_decode_to_array::<20>(PRECOMPILE_ADDR_HEX).unwrap())
    }

    fn call_precompile<RT: pallet_revive::Config>(
        origin_account: RT::AccountId,
        data: Vec<u8>,
    ) -> pallet_revive::ContractResult<
        pallet_revive::ExecReturnValue,
        RT::Balance,
        pallet_revive::EventRecord<RT>,
    > {
        pallet_revive::Pallet::<RT>::bare_call(
            frame_system::RawOrigin::Signed(origin_account).into(),
            precompile_h160(),
            0u32.into(),
            TransactionLimits::WeightAndDeposit {
                weight_limit: Weight::MAX,
                deposit_limit: u64::MAX.into(),
            },
            data,
            ExecConfig::new_substrate_tx(),
        )
    }

    #[cfg(test)]
    mod with_mock {
        use super::*;
        use crate::native_tests::mock::{new_test_ext, Balances, RuntimeOrigin, System, Test};
        use frame_support::traits::Currency;

        #[test]
        fn address_matches() {
            assert!(
                <NativeERC20<Test> as Precompile>::MATCHER.matches(&NATIVE_ERC20_ADDRESS),
                "NativeERC20 MATCHER must match 0x0000000000000000000000000000000001000000"
            );
        }

        #[test]
        fn total_supply_works() {
            new_test_ext().execute_with(|| {
                let alice = 1u64;
                Balances::make_free_balance_be(&alice, 5_000_000);

                let data = IERC20::totalSupplyCall {}.abi_encode();
                let res = call_precompile::<Test>(alice, data);
                let ret_data = res.result.unwrap().data;
                let total = IERC20::totalSupplyCall::abi_decode_returns(&ret_data).unwrap();
                // Total issuance includes ED + alice's balance
                assert!(total >= AU256::from(5_000_000u64));
            });
        }

        #[test]
        fn balance_of_works() {
            new_test_ext().execute_with(|| {
                let alice = 1u64;
                Balances::make_free_balance_be(&alice, 1_000_000);

                let alice_addr = <Test as pallet_revive::Config>::AddressMapper::to_address(&alice);
                let data = IERC20::balanceOfCall {
                    account: alice_addr.0.into(),
                }
                .abi_encode();
                let res = call_precompile::<Test>(alice, data);
                let ret_data = res.result.unwrap().data;
                let balance = IERC20::balanceOfCall::abi_decode_returns(&ret_data).unwrap();
                assert_eq!(balance, AU256::from(1_000_000u64));
            });
        }

        #[test]
        fn transfer_works() {
            new_test_ext().execute_with(|| {
                let alice = 1u64;
                let bob = 2u64;
                Balances::make_free_balance_be(&alice, 1_000_000);
                Balances::make_free_balance_be(&bob, 1); // keep alive

                let bob_addr = <Test as pallet_revive::Config>::AddressMapper::to_address(&bob);
                let data = IERC20::transferCall {
                    to: bob_addr.0.into(),
                    value: AU256::from(100_000u64),
                }
                .abi_encode();

                let res = call_precompile::<Test>(alice, data);
                assert!(
                    !res.result.unwrap().did_revert(),
                    "transfer should not revert"
                );

                assert_eq!(Balances::free_balance(bob), 100_001);
            });
        }

        #[test]
        fn approve_and_allowance_works() {
            new_test_ext().execute_with(|| {
                let alice = 1u64;
                let bob = 2u64;
                Balances::make_free_balance_be(&alice, 500_000);

                let alice_addr = <Test as pallet_revive::Config>::AddressMapper::to_address(&alice);
                let bob_addr = <Test as pallet_revive::Config>::AddressMapper::to_address(&bob);

                // approve bob to spend 50_000 on behalf of alice
                let data = IERC20::approveCall {
                    spender: bob_addr.0.into(),
                    value: AU256::from(50_000u64),
                }
                .abi_encode();
                let res = call_precompile::<Test>(alice, data);
                assert!(!res.result.unwrap().did_revert());

                // check allowance
                let data = IERC20::allowanceCall {
                    owner: alice_addr.0.into(),
                    spender: bob_addr.0.into(),
                }
                .abi_encode();
                let res = call_precompile::<Test>(alice, data);
                let ret_data = res.result.unwrap().data;
                let allowance = IERC20::allowanceCall::abi_decode_returns(&ret_data).unwrap();
                assert_eq!(allowance, AU256::from(50_000u64));
            });
        }

        #[test]
        fn transfer_from_works() {
            new_test_ext().execute_with(|| {
                let alice = 1u64;
                let bob = 2u64;
                let charlie = 3u64;
                Balances::make_free_balance_be(&alice, 500_000);
                Balances::make_free_balance_be(&bob, 1);
                Balances::make_free_balance_be(&charlie, 1);

                let alice_addr = <Test as pallet_revive::Config>::AddressMapper::to_address(&alice);
                let bob_addr = <Test as pallet_revive::Config>::AddressMapper::to_address(&bob);
                let charlie_addr =
                    <Test as pallet_revive::Config>::AddressMapper::to_address(&charlie);

                // alice approves bob for 100_000
                let data = IERC20::approveCall {
                    spender: bob_addr.0.into(),
                    value: AU256::from(100_000u64),
                }
                .abi_encode();
                call_precompile::<Test>(alice, data);

                // bob transfers 40_000 from alice to charlie
                let data = IERC20::transferFromCall {
                    from: alice_addr.0.into(),
                    to: charlie_addr.0.into(),
                    value: AU256::from(40_000u64),
                }
                .abi_encode();
                let res = call_precompile::<Test>(bob, data);
                assert!(
                    !res.result.unwrap().did_revert(),
                    "transferFrom should not revert"
                );

                assert_eq!(Balances::free_balance(charlie), 40_001);

                // remaining allowance should be 60_000
                let data = IERC20::allowanceCall {
                    owner: alice_addr.0.into(),
                    spender: bob_addr.0.into(),
                }
                .abi_encode();
                let res = call_precompile::<Test>(alice, data);
                let ret_data = res.result.unwrap().data;
                let rem_allowance = IERC20::allowanceCall::abi_decode_returns(&ret_data).unwrap();
                assert_eq!(rem_allowance, AU256::from(60_000u64));
            });
        }
    }
}
