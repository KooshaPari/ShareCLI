//! Compile-time guarantee that the 11 re-exports in `sharecli-core-pure` resolve to the
//! **same** type definitions as `sharecli_core::transaction_contract::*`.
//!
//! If anyone ever renames a `transaction_contract` item, this example breaks at
//! `cargo check -p sharecli-core-pure --examples`. That is the contract.

use sharecli_core::transaction_contract as src;
use sharecli_core_pure as pure;

/// Generic type-equality witness: `Same<A>` is implemented for `A` and only `A`.
/// If we can put both `src::X` and `pure::X` into the same `Same<_>` slot, they are the
/// same type.
trait Same<T: ?Sized> {}
impl<T: ?Sized> Same<T> for T {}

/// Each line asserts `pure::X` and `src::X` are the SAME type (by trying to assign one to
/// the other). All 11 PURE items must be present.
#[allow(dead_code, unused_variables, unused_must_use)]
fn _type_identity() {
    // 9 type-level equality checks via Same<src::X>:
    fn _a<T: Same<src::Actor>>() {}
    fn _b<T: Same<src::ActorKind>>() {}
    fn _c<T: Same<src::EffectOutcome>>() {}
    fn _d<T: Same<src::Grant>>() {}
    fn _e<T: Same<src::GuardError>>() {}
    fn _f<T: Same<src::ParentMembership>>() {}
    fn _g<T: Same<src::ProducerState>>() {}
    fn _h<T: Same<src::ReservationPool>>() {}
    fn _i<T: Same<src::SharedProducer>>() {}
    _a::<src::Actor>();
    _b::<src::ActorKind>();
    _c::<src::EffectOutcome>();
    _d::<src::Grant>();
    _e::<src::GuardError>();
    _f::<src::ParentMembership>();
    _g::<src::ProducerState>();
    _h::<src::ReservationPool>();
    _i::<src::SharedProducer>();
    // And prove the pure crate's items satisfy the same bounds:
    _a::<pure::Actor>();
    _b::<pure::ActorKind>();
    _c::<pure::EffectOutcome>();
    _d::<pure::Grant>();
    _e::<pure::GuardError>();
    _f::<pure::ParentMembership>();
    _g::<pure::ProducerState>();
    _h::<pure::ReservationPool>();
    _i::<pure::SharedProducer>();

    // 2 free functions: identity via fn-pointer equality.
    let _: fn(&src::Grant, &src::Grant, bool) -> bool = pure::validate_delegation;
    let _: fn(
        src::EffectOutcome,
        bool,
        bool,
        bool,
        bool,
    ) -> bool = pure::can_retry_effect;
}

fn main() {
    _type_identity();
    println!("OK: all 11 sharecli-core-pure re-exports point at the same definitions as sharecli_core::transaction_contract");
}
