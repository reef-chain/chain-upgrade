//! Currencies module.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

use codec::Codec;
use frame_support::{
    pallet_prelude::*,
    traits::{
        tokens::{
            fungible, fungibles, DepositConsequence, Fortitude, Precision, Preservation,
            Provenance, Restriction, WithdrawConsequence,
        },
        Currency as PalletCurrency, ExistenceRequirement, Get, Imbalance,
        LockableCurrency as PalletLockableCurrency, ReservableCurrency as PalletReservableCurrency,
        WithdrawReasons,
    },
    transactional,
};
use frame_system::pallet_prelude::*;
use orml_traits::currency::OnDust;
use orml_traits::{
    arithmetic::{Signed, SimpleArithmetic},
    currency::TransferAll,
    BalanceStatus, BasicCurrency, BasicCurrencyExtended, BasicLockableCurrency,
    BasicReservableCurrency, LockIdentifier, MultiCurrency, MultiCurrencyExtended,
    MultiLockableCurrency, MultiReservableCurrency,
};
use primitives::{
    evm::{AddressMapping, EvmAddress},
    CurrencyId, TokenSymbol,
};
use sp_io::hashing::blake2_256;
use sp_runtime::{
    traits::{CheckedSub, MaybeSerializeDeserialize, Saturating, StaticLookup, Zero},
    DispatchError, DispatchResult,
};
use sp_std::{
    convert::{TryFrom, TryInto},
    fmt::Debug,
    marker, result,
};
use support::{EVMBridge, InvokeContext};

mod default_weight;
mod mock;
mod tests;

pub use module::*;

pub trait WeightInfo {
    fn transfer_non_native_currency() -> Weight;
    fn transfer_native_currency() -> Weight;
    fn update_balance_non_native_currency() -> Weight;
    fn update_balance_native_currency_creating() -> Weight;
    fn update_balance_native_currency_killing() -> Weight;
}

type BalanceOf<T> = <<T as Config>::MultiCurrency as MultiCurrency<
    <T as frame_system::Config>::AccountId,
>>::Balance;
type CurrencyIdOf<T> = <<T as Config>::MultiCurrency as MultiCurrency<
    <T as frame_system::Config>::AccountId,
>>::CurrencyId;

type AmountOf<T> = <<T as Config>::MultiCurrency as MultiCurrencyExtended<
    <T as frame_system::Config>::AccountId,
>>::Amount;

#[frame_support::pallet]
pub mod module {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        #[allow(deprecated)]
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type MultiCurrency: TransferAll<Self::AccountId>
            + MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyId>
            + MultiLockableCurrency<Self::AccountId, CurrencyId = CurrencyId>
            + MultiReservableCurrency<Self::AccountId, CurrencyId = CurrencyId>
            + fungibles::Inspect<Self::AccountId, AssetId = CurrencyId, Balance = BalanceOf<Self>>
            + fungibles::Mutate<Self::AccountId, AssetId = CurrencyId, Balance = BalanceOf<Self>>
            + fungibles::Unbalanced<Self::AccountId, AssetId = CurrencyId, Balance = BalanceOf<Self>>
            + fungibles::InspectHold<
                Self::AccountId,
                AssetId = CurrencyId,
                Balance = BalanceOf<Self>,
                Reason = (),
            > + fungibles::MutateHold<Self::AccountId, AssetId = CurrencyId, Balance = BalanceOf<Self>>
            + fungibles::UnbalancedHold<
                Self::AccountId,
                AssetId = CurrencyId,
                Balance = BalanceOf<Self>,
            >;
        type NativeCurrency: BasicCurrencyExtended<
                Self::AccountId,
                Balance = BalanceOf<Self>,
                Amount = AmountOf<Self>,
            > + BasicLockableCurrency<Self::AccountId, Balance = BalanceOf<Self>>
            + BasicReservableCurrency<Self::AccountId, Balance = BalanceOf<Self>>
            + fungible::Inspect<Self::AccountId, Balance = BalanceOf<Self>>
            + fungible::Mutate<Self::AccountId, Balance = BalanceOf<Self>>
            + fungible::Unbalanced<Self::AccountId, Balance = BalanceOf<Self>>
            + fungible::InspectHold<Self::AccountId, Balance = BalanceOf<Self>>
            + fungible::MutateHold<Self::AccountId, Balance = BalanceOf<Self>>
            + fungible::UnbalancedHold<Self::AccountId, Balance = BalanceOf<Self>>;

        /// Weight information for extrinsics in this module.
        type WeightInfo: WeightInfo;

        /// Mapping from address to account id.
        type AddressMapping: AddressMapping<Self::AccountId>;
        type EVMBridge: EVMBridge<Self::AccountId, BalanceOf<Self>>;
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Unable to convert the Amount type into Balance.
        AmountIntoBalanceFailed,
        /// Balance is too low.
        BalanceTooLow,
        /// ERC20 invalid operation
        ERC20InvalidOperation,
        /// EVM account not found
        EvmAccountNotFound,
        RealOriginNotFound,
        /// Deposit result is not expected
        DepositFailed,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(crate) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Currency transfer success. [currency_id, from, to, amount]
        Transferred(CurrencyIdOf<T>, T::AccountId, T::AccountId, BalanceOf<T>),
        /// Update balance success. [currency_id, who, amount]
        BalanceUpdated(CurrencyIdOf<T>, T::AccountId, AmountOf<T>),
        /// Deposit success. [currency_id, who, amount]
        Deposited(CurrencyIdOf<T>, T::AccountId, BalanceOf<T>),
        /// Withdraw success. [currency_id, who, amount]
        Withdrawn(CurrencyIdOf<T>, T::AccountId, BalanceOf<T>),
    }

    #[pallet::pallet]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Transfer some balance to another account under `currency_id`.
        ///
        /// The dispatch origin for this call must be `Signed` by the
        /// transactor.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::transfer_non_native_currency())]
        pub fn transfer(
            origin: OriginFor<T>,
            dest: <<T as frame_system::Config>::Lookup as StaticLookup>::Source,
            currency_id: CurrencyIdOf<T>,
            #[pallet::compact] amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let from = ensure_signed(origin)?;
            let to = T::Lookup::lookup(dest)?;
            <Self as MultiCurrency<T::AccountId>>::transfer(
                currency_id,
                &from,
                &to,
                amount,
                ExistenceRequirement::AllowDeath,
            )?;
            Ok(().into())
        }

        /// Transfer some native currency to another account.
        ///
        /// The dispatch origin for this call must be `Signed` by the
        /// transactor.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::transfer_native_currency())]
        pub fn transfer_native_currency(
            origin: OriginFor<T>,
            dest: <T::Lookup as StaticLookup>::Source,
            #[pallet::compact] amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let from = ensure_signed(origin)?;
            let to = T::Lookup::lookup(dest)?;
            T::NativeCurrency::transfer(&from, &to, amount, ExistenceRequirement::AllowDeath)?;

            Self::deposit_event(Event::Transferred(
                CurrencyId::Token(TokenSymbol::REEF),
                from,
                to,
                amount,
            ));
            Ok(().into())
        }

        /// update amount of account `who` under `currency_id`.
        ///
        /// The dispatch origin of this call must be _Root_.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::update_balance_non_native_currency())]
        pub fn update_balance(
            origin: OriginFor<T>,
            who: <T::Lookup as StaticLookup>::Source,
            currency_id: CurrencyIdOf<T>,
            amount: AmountOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let dest = T::Lookup::lookup(who)?;
            <Self as MultiCurrencyExtended<T::AccountId>>::update_balance(
                currency_id,
                &dest,
                amount,
            )?;
            Ok(().into())
        }
    }
}

impl<T: Config> MultiCurrency<T::AccountId> for Pallet<T> {
    type CurrencyId = CurrencyIdOf<T>;
    type Balance = BalanceOf<T>;

    fn minimum_balance(currency_id: Self::CurrencyId) -> Self::Balance {
        match currency_id {
            CurrencyId::ERC20(_) => Default::default(),
            CurrencyId::Token(TokenSymbol::REEF) => T::NativeCurrency::minimum_balance(),
            _ => T::MultiCurrency::minimum_balance(currency_id),
        }
    }

    fn total_issuance(currency_id: Self::CurrencyId) -> Self::Balance {
        match currency_id {
            CurrencyId::ERC20(contract) => T::EVMBridge::total_supply(InvokeContext {
                contract,
                sender: Default::default(),
                origin: Default::default(),
            })
            .unwrap_or_default(),
            CurrencyId::Token(TokenSymbol::REEF) => T::NativeCurrency::total_issuance(),
            _ => T::MultiCurrency::total_issuance(currency_id),
        }
    }

    fn total_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
        match currency_id {
            CurrencyId::ERC20(contract) => {
                if let Some(address) = T::AddressMapping::get_evm_address(who) {
                    let context = InvokeContext {
                        contract,
                        sender: Default::default(),
                        origin: Default::default(),
                    };
                    return T::EVMBridge::balance_of(context, address).unwrap_or_default();
                }
                Default::default()
            }
            CurrencyId::Token(TokenSymbol::REEF) => T::NativeCurrency::total_balance(who),
            _ => T::MultiCurrency::total_balance(currency_id, who),
        }
    }

    fn free_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
        match currency_id {
            CurrencyId::ERC20(contract) => {
                if let Some(address) = T::AddressMapping::get_evm_address(who) {
                    let context = InvokeContext {
                        contract,
                        sender: Default::default(),
                        origin: Default::default(),
                    };
                    return T::EVMBridge::balance_of(context, address).unwrap_or_default();
                }
                Default::default()
            }
            CurrencyId::Token(TokenSymbol::REEF) => T::NativeCurrency::free_balance(who),
            _ => T::MultiCurrency::free_balance(currency_id, who),
        }
    }

    fn ensure_can_withdraw(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        match currency_id {
            CurrencyId::ERC20(contract) => {
                let address = T::AddressMapping::get_evm_address(who)
                    .ok_or(Error::<T>::EvmAccountNotFound)?;
                let balance = T::EVMBridge::balance_of(
                    InvokeContext {
                        contract,
                        sender: Default::default(),
                        origin: Default::default(),
                    },
                    address,
                )
                .unwrap_or_default();
                ensure!(balance >= amount, Error::<T>::BalanceTooLow);
                Ok(())
            }
            CurrencyId::Token(TokenSymbol::REEF) => {
                T::NativeCurrency::ensure_can_withdraw(who, amount)
            }
            _ => T::MultiCurrency::ensure_can_withdraw(currency_id, who, amount),
        }
    }

    fn transfer(
        currency_id: Self::CurrencyId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: Self::Balance,
        existence_requirement: ExistenceRequirement,
    ) -> DispatchResult {
        if amount.is_zero() || from == to {
            return Ok(());
        }

        match currency_id {
            CurrencyId::ERC20(contract) => {
                let sender = T::AddressMapping::get_evm_address(from)
                    .ok_or(Error::<T>::EvmAccountNotFound)?;
                let origin = T::EVMBridge::get_origin().ok_or(Error::<T>::RealOriginNotFound)?;
                let origin_address = T::AddressMapping::get_or_create_evm_address(&origin);
                let address = T::AddressMapping::get_or_create_evm_address(to);
                T::EVMBridge::transfer(
                    InvokeContext {
                        contract,
                        sender,
                        origin: origin_address,
                    },
                    address,
                    amount,
                )?;
            }
            CurrencyId::Token(TokenSymbol::REEF) => {
                T::NativeCurrency::transfer(from, to, amount, existence_requirement)?
            }
            _ => T::MultiCurrency::transfer(currency_id, from, to, amount, existence_requirement)?,
        }

        Self::deposit_event(Event::Transferred(
            currency_id,
            from.clone(),
            to.clone(),
            amount,
        ));
        Ok(())
    }

    fn deposit(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        if amount.is_zero() {
            return Ok(());
        }
        match currency_id {
            CurrencyId::ERC20(_) => return Err(Error::<T>::ERC20InvalidOperation.into()),
            CurrencyId::Token(TokenSymbol::REEF) => T::NativeCurrency::deposit(who, amount)?,
            _ => T::MultiCurrency::deposit(currency_id, who, amount)?,
        }
        Self::deposit_event(Event::Deposited(currency_id, who.clone(), amount));
        Ok(())
    }

    fn withdraw(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
        existence_requirement: ExistenceRequirement,
    ) -> DispatchResult {
        if amount.is_zero() {
            return Ok(());
        }
        match currency_id {
            CurrencyId::ERC20(_) => return Err(Error::<T>::ERC20InvalidOperation.into()),
            CurrencyId::Token(TokenSymbol::REEF) => {
                T::NativeCurrency::withdraw(who, amount, existence_requirement)?
            }
            _ => T::MultiCurrency::withdraw(currency_id, who, amount, existence_requirement)?,
        }
        Self::deposit_event(Event::Withdrawn(currency_id, who.clone(), amount));
        Ok(())
    }

    fn can_slash(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> bool {
        match currency_id {
            CurrencyId::ERC20(_) => false,
            CurrencyId::Token(TokenSymbol::REEF) => T::NativeCurrency::can_slash(who, amount),
            _ => T::MultiCurrency::can_slash(currency_id, who, amount),
        }
    }

    fn slash(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> Self::Balance {
        match currency_id {
            CurrencyId::ERC20(_) => Default::default(),
            CurrencyId::Token(TokenSymbol::REEF) => T::NativeCurrency::slash(who, amount),
            _ => T::MultiCurrency::slash(currency_id, who, amount),
        }
    }
}

impl<T: Config> MultiCurrencyExtended<T::AccountId> for Pallet<T> {
    type Amount = AmountOf<T>;

    fn update_balance(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        by_amount: Self::Amount,
    ) -> DispatchResult {
        match currency_id {
            CurrencyId::ERC20(_) => return Err(Error::<T>::ERC20InvalidOperation.into()),
            CurrencyId::Token(TokenSymbol::REEF) => {
                T::NativeCurrency::update_balance(who, by_amount)?
            }
            _ => T::MultiCurrency::update_balance(currency_id, who, by_amount)?,
        }
        Self::deposit_event(Event::BalanceUpdated(currency_id, who.clone(), by_amount));
        Ok(())
    }
}

impl<T: Config> MultiLockableCurrency<T::AccountId> for Pallet<T> {
    type Moment = BlockNumberFor<T>;

    fn set_lock(
        lock_id: LockIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        match currency_id {
            CurrencyId::ERC20(_) => Err(Error::<T>::ERC20InvalidOperation.into()),
            CurrencyId::Token(TokenSymbol::REEF) => {
                T::NativeCurrency::set_lock(lock_id, who, amount)
            }
            _ => T::MultiCurrency::set_lock(lock_id, currency_id, who, amount),
        }
    }

    fn extend_lock(
        lock_id: LockIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        match currency_id {
            CurrencyId::ERC20(_) => Err(Error::<T>::ERC20InvalidOperation.into()),
            CurrencyId::Token(TokenSymbol::REEF) => {
                T::NativeCurrency::extend_lock(lock_id, who, amount)
            }
            _ => T::MultiCurrency::extend_lock(lock_id, currency_id, who, amount),
        }
    }

    fn remove_lock(
        lock_id: LockIdentifier,
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
    ) -> DispatchResult {
        match currency_id {
            CurrencyId::ERC20(_) => Err(Error::<T>::ERC20InvalidOperation.into()),
            CurrencyId::Token(TokenSymbol::REEF) => T::NativeCurrency::remove_lock(lock_id, who),
            _ => T::MultiCurrency::remove_lock(lock_id, currency_id, who),
        }
    }
}

impl<T: Config> MultiReservableCurrency<T::AccountId> for Pallet<T> {
    fn can_reserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> bool {
        match currency_id {
            CurrencyId::ERC20(_) => Self::ensure_can_withdraw(currency_id, who, value).is_ok(),
            CurrencyId::Token(TokenSymbol::REEF) => T::NativeCurrency::can_reserve(who, value),
            _ => T::MultiCurrency::can_reserve(currency_id, who, value),
        }
    }

    fn slash_reserved(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        match currency_id {
            CurrencyId::ERC20(contract) => {
                if let Some(address) = T::AddressMapping::get_evm_address(who) {
                    let account_balance = T::EVMBridge::balance_of(
                        InvokeContext {
                            contract,
                            sender: Default::default(),
                            origin: Default::default(),
                        },
                        address,
                    )
                    .unwrap_or_default();
                    return if value < account_balance {
                        value
                    } else {
                        account_balance
                    };
                }
                value
            }
            CurrencyId::Token(TokenSymbol::REEF) => T::NativeCurrency::slash_reserved(who, value),
            _ => T::MultiCurrency::slash_reserved(currency_id, who, value),
        }
    }

    fn reserved_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
        match currency_id {
            CurrencyId::ERC20(contract) => {
                if let Some(address) = T::AddressMapping::get_evm_address(who) {
                    return T::EVMBridge::balance_of(
                        InvokeContext {
                            contract,
                            sender: Default::default(),
                            origin: Default::default(),
                        },
                        reserve_address(address),
                    )
                    .unwrap_or_default();
                }
                Default::default()
            }
            CurrencyId::Token(TokenSymbol::REEF) => T::NativeCurrency::reserved_balance(who),
            _ => T::MultiCurrency::reserved_balance(currency_id, who),
        }
    }

    fn reserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> DispatchResult {
        match currency_id {
            CurrencyId::ERC20(contract) => {
                if value.is_zero() {
                    return Ok(());
                }
                let address = T::AddressMapping::get_evm_address(who)
                    .ok_or(Error::<T>::EvmAccountNotFound)?;
                T::EVMBridge::transfer(
                    InvokeContext {
                        contract,
                        sender: address,
                        origin: address,
                    },
                    reserve_address(address),
                    value,
                )
            }
            CurrencyId::Token(TokenSymbol::REEF) => T::NativeCurrency::reserve(who, value),
            _ => T::MultiCurrency::reserve(currency_id, who, value),
        }
    }

    fn unreserve(
        currency_id: Self::CurrencyId,
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Self::Balance {
        match currency_id {
            CurrencyId::ERC20(contract) => {
                if value.is_zero() {
                    return value;
                }
                if let Some(address) = T::AddressMapping::get_evm_address(who) {
                    let sender = reserve_address(address);
                    let reserved_balance = T::EVMBridge::balance_of(
                        InvokeContext {
                            contract,
                            sender: Default::default(),
                            origin: Default::default(),
                        },
                        sender,
                    )
                    .unwrap_or_default();
                    let actual = reserved_balance.min(value);
                    return match T::EVMBridge::transfer(
                        InvokeContext {
                            contract,
                            sender,
                            origin: address,
                        },
                        address,
                        actual,
                    ) {
                        Ok(_) => value - actual,
                        Err(_) => value,
                    };
                }
                value
            }
            CurrencyId::Token(TokenSymbol::REEF) => T::NativeCurrency::unreserve(who, value),
            _ => T::MultiCurrency::unreserve(currency_id, who, value),
        }
    }

    fn repatriate_reserved(
        currency_id: Self::CurrencyId,
        slashed: &T::AccountId,
        beneficiary: &T::AccountId,
        value: Self::Balance,
        status: BalanceStatus,
    ) -> result::Result<Self::Balance, DispatchError> {
        match currency_id {
            CurrencyId::ERC20(contract) => {
                if value.is_zero() {
                    return Ok(value);
                }
                if slashed == beneficiary {
                    return match status {
                        BalanceStatus::Free => Ok(Self::unreserve(currency_id, slashed, value)),
                        BalanceStatus::Reserved => {
                            Ok(value.saturating_sub(Self::reserved_balance(currency_id, slashed)))
                        }
                    };
                }

                let slashed_address = T::AddressMapping::get_evm_address(slashed)
                    .ok_or(Error::<T>::EvmAccountNotFound)?;
                let beneficiary_address = T::AddressMapping::get_or_create_evm_address(beneficiary);

                let slashed_reserve_address = reserve_address(slashed_address);
                let beneficiary_reserve_address = reserve_address(beneficiary_address);

                let slashed_reserved_balance = T::EVMBridge::balance_of(
                    InvokeContext {
                        contract,
                        sender: Default::default(),
                        origin: Default::default(),
                    },
                    slashed_reserve_address,
                )
                .unwrap_or_default();
                let actual = slashed_reserved_balance.min(value);
                match status {
                    BalanceStatus::Free => T::EVMBridge::transfer(
                        InvokeContext {
                            contract,
                            sender: slashed_reserve_address,
                            origin: slashed_address,
                        },
                        beneficiary_address,
                        actual,
                    ),
                    BalanceStatus::Reserved => T::EVMBridge::transfer(
                        InvokeContext {
                            contract,
                            sender: slashed_reserve_address,
                            origin: slashed_address,
                        },
                        beneficiary_reserve_address,
                        actual,
                    ),
                }
                .map(|_| value - actual)
            }
            CurrencyId::Token(TokenSymbol::REEF) => {
                T::NativeCurrency::repatriate_reserved(slashed, beneficiary, value, status)
            }
            _ => T::MultiCurrency::repatriate_reserved(
                currency_id,
                slashed,
                beneficiary,
                value,
                status,
            ),
        }
    }
}

pub struct Currency<T, GetCurrencyId>(marker::PhantomData<T>, marker::PhantomData<GetCurrencyId>);

impl<T, GetCurrencyId> BasicCurrency<T::AccountId> for Currency<T, GetCurrencyId>
where
    T: Config,
    GetCurrencyId: Get<CurrencyIdOf<T>>,
{
    type Balance = BalanceOf<T>;

    fn minimum_balance() -> Self::Balance {
        <Pallet<T>>::minimum_balance(GetCurrencyId::get())
    }

    fn total_issuance() -> Self::Balance {
        <Pallet<T>>::total_issuance(GetCurrencyId::get())
    }

    fn total_balance(who: &T::AccountId) -> Self::Balance {
        <Pallet<T>>::total_balance(GetCurrencyId::get(), who)
    }

    fn free_balance(who: &T::AccountId) -> Self::Balance {
        <Pallet<T>>::free_balance(GetCurrencyId::get(), who)
    }

    fn ensure_can_withdraw(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
        <Pallet<T>>::ensure_can_withdraw(GetCurrencyId::get(), who, amount)
    }

    fn transfer(
        from: &T::AccountId,
        to: &T::AccountId,
        amount: Self::Balance,
        existence_requirement: ExistenceRequirement,
    ) -> DispatchResult {
        <Pallet<T> as MultiCurrency<T::AccountId>>::transfer(
            GetCurrencyId::get(),
            from,
            to,
            amount,
            existence_requirement,
        )
    }

    fn deposit(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
        <Pallet<T>>::deposit(GetCurrencyId::get(), who, amount)
    }

    fn withdraw(
        who: &T::AccountId,
        amount: Self::Balance,
        existence_requirement: ExistenceRequirement,
    ) -> DispatchResult {
        <Pallet<T>>::withdraw(GetCurrencyId::get(), who, amount, existence_requirement)
    }

    fn can_slash(who: &T::AccountId, amount: Self::Balance) -> bool {
        <Pallet<T>>::can_slash(GetCurrencyId::get(), who, amount)
    }

    fn slash(who: &T::AccountId, amount: Self::Balance) -> Self::Balance {
        <Pallet<T>>::slash(GetCurrencyId::get(), who, amount)
    }
}

impl<T, GetCurrencyId> BasicCurrencyExtended<T::AccountId> for Currency<T, GetCurrencyId>
where
    T: Config,
    GetCurrencyId: Get<CurrencyIdOf<T>>,
{
    type Amount = AmountOf<T>;

    fn update_balance(who: &T::AccountId, by_amount: Self::Amount) -> DispatchResult {
        <Pallet<T> as MultiCurrencyExtended<T::AccountId>>::update_balance(
            GetCurrencyId::get(),
            who,
            by_amount,
        )
    }
}

impl<T, GetCurrencyId> BasicLockableCurrency<T::AccountId> for Currency<T, GetCurrencyId>
where
    T: Config,
    GetCurrencyId: Get<CurrencyIdOf<T>>,
{
    type Moment = BlockNumberFor<T>;

    fn set_lock(
        lock_id: LockIdentifier,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        <Pallet<T> as MultiLockableCurrency<T::AccountId>>::set_lock(
            lock_id,
            GetCurrencyId::get(),
            who,
            amount,
        )
    }

    fn extend_lock(
        lock_id: LockIdentifier,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        <Pallet<T> as MultiLockableCurrency<T::AccountId>>::extend_lock(
            lock_id,
            GetCurrencyId::get(),
            who,
            amount,
        )
    }

    fn remove_lock(lock_id: LockIdentifier, who: &T::AccountId) -> DispatchResult {
        <Pallet<T> as MultiLockableCurrency<T::AccountId>>::remove_lock(
            lock_id,
            GetCurrencyId::get(),
            who,
        )
    }
}

impl<T, GetCurrencyId> BasicReservableCurrency<T::AccountId> for Currency<T, GetCurrencyId>
where
    T: Config,
    GetCurrencyId: Get<CurrencyIdOf<T>>,
{
    fn can_reserve(who: &T::AccountId, value: Self::Balance) -> bool {
        <Pallet<T> as MultiReservableCurrency<T::AccountId>>::can_reserve(
            GetCurrencyId::get(),
            who,
            value,
        )
    }

    fn slash_reserved(who: &T::AccountId, value: Self::Balance) -> Self::Balance {
        <Pallet<T> as MultiReservableCurrency<T::AccountId>>::slash_reserved(
            GetCurrencyId::get(),
            who,
            value,
        )
    }

    fn reserved_balance(who: &T::AccountId) -> Self::Balance {
        <Pallet<T> as MultiReservableCurrency<T::AccountId>>::reserved_balance(
            GetCurrencyId::get(),
            who,
        )
    }

    fn reserve(who: &T::AccountId, value: Self::Balance) -> DispatchResult {
        <Pallet<T> as MultiReservableCurrency<T::AccountId>>::reserve(
            GetCurrencyId::get(),
            who,
            value,
        )
    }

    fn unreserve(who: &T::AccountId, value: Self::Balance) -> Self::Balance {
        <Pallet<T> as MultiReservableCurrency<T::AccountId>>::unreserve(
            GetCurrencyId::get(),
            who,
            value,
        )
    }

    fn repatriate_reserved(
        slashed: &T::AccountId,
        beneficiary: &T::AccountId,
        value: Self::Balance,
        status: BalanceStatus,
    ) -> result::Result<Self::Balance, DispatchError> {
        <Pallet<T> as MultiReservableCurrency<T::AccountId>>::repatriate_reserved(
            GetCurrencyId::get(),
            slashed,
            beneficiary,
            value,
            status,
        )
    }
}

/// Adapt other currency traits implementation to `BasicCurrency`.

/// Adapt other currency traits implementation to `BasicCurrency`.
pub struct BasicCurrencyAdapter<T, Currency, Amount, Moment>(
    marker::PhantomData<(T, Currency, Amount, Moment)>,
);

type PalletBalanceOf<A, Currency> = <Currency as PalletCurrency<A>>::Balance;

// Adapt `frame_support::traits::Currency`
impl<T, AccountId, Currency, Amount, Moment> BasicCurrency<AccountId>
    for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
    Currency: PalletCurrency<AccountId>,
    T: Config,
{
    type Balance = PalletBalanceOf<AccountId, Currency>;

    fn minimum_balance() -> Self::Balance {
        <Currency as PalletCurrency<_>>::minimum_balance()
    }

    fn total_issuance() -> Self::Balance {
        <Currency as PalletCurrency<_>>::total_issuance()
    }

    fn total_balance(who: &AccountId) -> Self::Balance {
        <Currency as PalletCurrency<_>>::total_balance(who)
    }

    fn free_balance(who: &AccountId) -> Self::Balance {
        <Currency as PalletCurrency<_>>::free_balance(who)
    }

    fn ensure_can_withdraw(who: &AccountId, amount: Self::Balance) -> DispatchResult {
        let new_balance = Self::free_balance(who)
            .checked_sub(&amount)
            .ok_or(Error::<T>::BalanceTooLow)?;

        <Currency as PalletCurrency<_>>::ensure_can_withdraw(
            who,
            amount,
            WithdrawReasons::all(),
            new_balance,
        )
    }

    fn transfer(
        from: &AccountId,
        to: &AccountId,
        amount: Self::Balance,
        existence_requirement: ExistenceRequirement,
    ) -> DispatchResult {
        <Currency as PalletCurrency<_>>::transfer(from, to, amount, existence_requirement)
    }

    fn deposit(who: &AccountId, amount: Self::Balance) -> DispatchResult {
        if !amount.is_zero() {
            let deposit_result = <Currency as PalletCurrency<_>>::deposit_creating(who, amount);
            let actual_deposit = deposit_result.peek();
            ensure!(actual_deposit == amount, Error::<T>::DepositFailed);
        }

        Ok(())
    }

    fn withdraw(
        who: &AccountId,
        amount: Self::Balance,
        existence_requirement: ExistenceRequirement,
    ) -> DispatchResult {
        <Currency as PalletCurrency<_>>::withdraw(
            who,
            amount,
            WithdrawReasons::all(),
            existence_requirement,
        )
        .map(|_| ())
    }

    fn can_slash(who: &AccountId, amount: Self::Balance) -> bool {
        <Currency as PalletCurrency<_>>::can_slash(who, amount)
    }

    fn slash(who: &AccountId, amount: Self::Balance) -> Self::Balance {
        let (_, gap) = <Currency as PalletCurrency<_>>::slash(who, amount);
        gap
    }
}

// Adapt `frame_support::traits::Currency`
impl<T, AccountId, Currency, Amount, Moment> BasicCurrencyExtended<AccountId>
    for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
    Amount: Signed
        + TryInto<PalletBalanceOf<AccountId, Currency>>
        + TryFrom<PalletBalanceOf<AccountId, Currency>>
        + SimpleArithmetic
        + Codec
        + Copy
        + MaybeSerializeDeserialize
        + Debug
        + Default
        + MaxEncodedLen,
    Currency: PalletCurrency<AccountId>,
    T: Config,
{
    type Amount = Amount;

    fn update_balance(who: &AccountId, by_amount: Self::Amount) -> DispatchResult {
        let by_balance = by_amount
            .abs()
            .try_into()
            .map_err(|_| Error::<T>::AmountIntoBalanceFailed)?;
        if by_amount.is_positive() {
            Self::deposit(who, by_balance)
        } else {
            Self::withdraw(who, by_balance, ExistenceRequirement::AllowDeath)
        }
    }
}

// Adapt `frame_support::traits::LockableCurrency`
impl<T, AccountId, Currency, Amount, Moment> BasicLockableCurrency<AccountId>
    for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
    Currency: PalletLockableCurrency<AccountId>,
    T: Config,
{
    type Moment = Moment;

    fn set_lock(lock_id: LockIdentifier, who: &AccountId, amount: Self::Balance) -> DispatchResult {
        <Currency as PalletLockableCurrency<_>>::set_lock(
            lock_id,
            who,
            amount,
            WithdrawReasons::all(),
        );
        Ok(())
    }

    fn extend_lock(
        lock_id: LockIdentifier,
        who: &AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        <Currency as PalletLockableCurrency<_>>::extend_lock(
            lock_id,
            who,
            amount,
            WithdrawReasons::all(),
        );
        Ok(())
    }

    fn remove_lock(lock_id: LockIdentifier, who: &AccountId) -> DispatchResult {
        <Currency as PalletLockableCurrency<_>>::remove_lock(lock_id, who);
        Ok(())
    }
}

// Adapt `frame_support::traits::ReservableCurrency`
impl<T, AccountId, Currency, Amount, Moment> BasicReservableCurrency<AccountId>
    for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
    Currency: PalletReservableCurrency<AccountId>,
    T: Config,
{
    fn can_reserve(who: &AccountId, value: Self::Balance) -> bool {
        <Currency as PalletReservableCurrency<_>>::can_reserve(who, value)
    }

    fn slash_reserved(who: &AccountId, value: Self::Balance) -> Self::Balance {
        let (_, gap) = <Currency as PalletReservableCurrency<_>>::slash_reserved(who, value);
        gap
    }

    fn reserved_balance(who: &AccountId) -> Self::Balance {
        <Currency as PalletReservableCurrency<_>>::reserved_balance(who)
    }

    fn reserve(who: &AccountId, value: Self::Balance) -> DispatchResult {
        <Currency as PalletReservableCurrency<_>>::reserve(who, value)
    }

    fn unreserve(who: &AccountId, value: Self::Balance) -> Self::Balance {
        <Currency as PalletReservableCurrency<_>>::unreserve(who, value)
    }

    fn repatriate_reserved(
        slashed: &AccountId,
        beneficiary: &AccountId,
        value: Self::Balance,
        status: BalanceStatus,
    ) -> result::Result<Self::Balance, DispatchError> {
        <Currency as PalletReservableCurrency<_>>::repatriate_reserved(
            slashed,
            beneficiary,
            value,
            status,
        )
    }
}

/// impl fungile for Currency<T, GetCurrencyId>
type FungibleBalanceOf<A, Currency> = <Currency as fungible::Inspect<A>>::Balance;
impl<T, Currency, Amount, Moment> fungible::Inspect<T::AccountId>
    for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
    Currency: fungible::Inspect<T::AccountId>,
    T: Config,
{
    type Balance = FungibleBalanceOf<T::AccountId, Currency>;

    fn total_issuance() -> Self::Balance {
        <Currency as fungible::Inspect<_>>::total_issuance()
    }
    fn minimum_balance() -> Self::Balance {
        <Currency as fungible::Inspect<_>>::minimum_balance()
    }
    fn balance(who: &T::AccountId) -> Self::Balance {
        <Currency as fungible::Inspect<_>>::balance(who)
    }
    fn total_balance(who: &T::AccountId) -> Self::Balance {
        <Currency as fungible::Inspect<_>>::total_balance(who)
    }
    fn reducible_balance(
        who: &T::AccountId,
        preservation: Preservation,
        force: Fortitude,
    ) -> Self::Balance {
        <Currency as fungible::Inspect<_>>::reducible_balance(who, preservation, force)
    }
    fn can_deposit(
        who: &T::AccountId,
        amount: Self::Balance,
        provenance: Provenance,
    ) -> DepositConsequence {
        <Currency as fungible::Inspect<_>>::can_deposit(who, amount, provenance)
    }
    fn can_withdraw(
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> WithdrawConsequence<Self::Balance> {
        <Currency as fungible::Inspect<_>>::can_withdraw(who, amount)
    }
}

impl<T, Currency, Amount, Moment> fungible::Unbalanced<T::AccountId>
    for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
    Currency: fungible::Unbalanced<T::AccountId>,
    T: Config,
{
    fn handle_dust(_dust: fungible::Dust<T::AccountId, Self>) {
        // https://github.com/paritytech/substrate/blob/569aae5341ea0c1d10426fa1ec13a36c0b64393b/frame/support/src/traits/tokens/fungibles/regular.rs#L124
        // Note: currently the field of Dust type is private and there is no constructor for it, so
        // we can't construct a Dust value and pass it.
        // `BasicCurrencyAdapter` overwrites these functions which can be called as user-level
        // operation of fungible traits when calling these functions, it will not actually reach
        // `Unbalanced::handle_dust`.
    }

    fn write_balance(
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> Result<Option<Self::Balance>, DispatchError> {
        <Currency as fungible::Unbalanced<_>>::write_balance(who, amount)
    }

    fn set_total_issuance(amount: Self::Balance) {
        <Currency as fungible::Unbalanced<_>>::set_total_issuance(amount)
    }
}

impl<T, Currency, Amount, Moment> fungible::Mutate<T::AccountId>
    for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
    Currency: fungible::Mutate<T::AccountId>,
    T: Config,
{
    fn mint_into(
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> Result<Self::Balance, DispatchError> {
        <Currency as fungible::Mutate<_>>::mint_into(who, amount)
    }

    fn burn_from(
        who: &T::AccountId,
        amount: Self::Balance,
        preservation: Preservation,
        precision: Precision,
        fortitude: Fortitude,
    ) -> Result<Self::Balance, DispatchError> {
        <Currency as fungible::Mutate<_>>::burn_from(
            who,
            amount,
            preservation,
            precision,
            fortitude,
        )
    }

    fn transfer(
        source: &T::AccountId,
        dest: &T::AccountId,
        amount: Self::Balance,
        preservation: Preservation,
    ) -> Result<Self::Balance, DispatchError> {
        <Currency as fungible::Mutate<_>>::transfer(source, dest, amount, preservation)
    }
}

impl<T, Currency, Amount, Moment> fungible::InspectHold<T::AccountId>
    for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
    Currency: fungible::InspectHold<T::AccountId>,
    T: Config,
{
    type Reason = <Currency as fungible::InspectHold<T::AccountId>>::Reason;

    fn balance_on_hold(reason: &Self::Reason, who: &T::AccountId) -> Self::Balance {
        <Currency as fungible::InspectHold<_>>::balance_on_hold(reason, who)
    }
    fn total_balance_on_hold(who: &T::AccountId) -> Self::Balance {
        <Currency as fungible::InspectHold<_>>::total_balance_on_hold(who)
    }
    fn reducible_total_balance_on_hold(who: &T::AccountId, force: Fortitude) -> Self::Balance {
        <Currency as fungible::InspectHold<_>>::reducible_total_balance_on_hold(who, force)
    }
    fn hold_available(reason: &Self::Reason, who: &T::AccountId) -> bool {
        <Currency as fungible::InspectHold<_>>::hold_available(reason, who)
    }
    fn can_hold(reason: &Self::Reason, who: &T::AccountId, amount: Self::Balance) -> bool {
        <Currency as fungible::InspectHold<_>>::can_hold(reason, who, amount)
    }
}

type ReasonOfFungible<P, T> =
    <P as fungible::InspectHold<<T as frame_system::Config>::AccountId>>::Reason;
impl<T, Currency, Amount, Moment> fungible::UnbalancedHold<T::AccountId>
    for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
    Currency: fungible::UnbalancedHold<T::AccountId>,
    T: Config,
{
    fn set_balance_on_hold(
        reason: &ReasonOfFungible<Self, T>,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        <Currency as fungible::UnbalancedHold<_>>::set_balance_on_hold(reason, who, amount)
    }
}

impl<T, Currency, Amount, Moment> fungible::MutateHold<T::AccountId>
    for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
    Currency: fungible::MutateHold<T::AccountId>,
    T: Config,
{
    fn hold(
        reason: &ReasonOfFungible<Self, T>,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> DispatchResult {
        <Currency as fungible::MutateHold<_>>::hold(reason, who, amount)
    }

    fn release(
        reason: &ReasonOfFungible<Self, T>,
        who: &T::AccountId,
        amount: Self::Balance,
        precision: Precision,
    ) -> Result<Self::Balance, DispatchError> {
        <Currency as fungible::MutateHold<_>>::release(reason, who, amount, precision)
    }

    fn transfer_on_hold(
        reason: &ReasonOfFungible<Self, T>,
        source: &T::AccountId,
        dest: &T::AccountId,
        amount: Self::Balance,
        precision: Precision,
        restriction: Restriction,
        fortitude: Fortitude,
    ) -> Result<Self::Balance, DispatchError> {
        <Currency as fungible::MutateHold<_>>::transfer_on_hold(
            reason,
            source,
            dest,
            amount,
            precision,
            restriction,
            fortitude,
        )
    }
}

impl<T: Config> TransferAll<T::AccountId> for Pallet<T> {
    #[transactional]
    fn transfer_all(source: &T::AccountId, dest: &T::AccountId) -> DispatchResult {
        // transfer non-native free to dest
        <T::MultiCurrency as TransferAll<_>>::transfer_all(source, dest)?;

        // transfer all free to dest
        <T::NativeCurrency as BasicCurrency<_>>::transfer(
            source,
            dest,
            <T::NativeCurrency as BasicCurrency<_>>::free_balance(source),
            ExistenceRequirement::AllowDeath,
        )
    }
}

fn reserve_address(address: EvmAddress) -> EvmAddress {
    let payload = (b"erc20:", address);
    EvmAddress::from_slice(&payload.using_encoded(blake2_256)[0..20])
}

pub struct TransferDust<T, GetAccountId>(marker::PhantomData<(T, GetAccountId)>);
impl<T: Config, GetAccountId> OnDust<T::AccountId, CurrencyId, BalanceOf<T>>
    for TransferDust<T, GetAccountId>
where
    T: Config,
    GetAccountId: Get<T::AccountId>,
{
    fn on_dust(who: &T::AccountId, currency_id: CurrencyId, amount: BalanceOf<T>) {
        // transfer the dust to treasury account, ignore the result,
        // if failed will leave some dust which still could be recycled.
        let _ = match currency_id {
            CurrencyId::ERC20(_) => Ok(()),
            CurrencyId::Token(TokenSymbol::REEF) => {
                <T::NativeCurrency as BasicCurrency<_>>::transfer(
                    who,
                    &GetAccountId::get(),
                    amount,
                    ExistenceRequirement::AllowDeath,
                )
            }
            _ => <T::MultiCurrency as MultiCurrency<_>>::transfer(
                currency_id,
                who,
                &GetAccountId::get(),
                amount,
                ExistenceRequirement::AllowDeath,
            ),
        };
    }
}
