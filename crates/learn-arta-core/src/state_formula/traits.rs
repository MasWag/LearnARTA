// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::minimal_model::{MinimalModelKey, default_semantic_key};
use std::fmt::{Debug, Display};
use std::hash::Hash;

/// Positive Boolean formula abstraction over location variables.
///
/// The abstraction is manager-based so implementations that need contextual
/// allocation (e.g. DAG interning, BDD packages) can share memory efficiently.
///
/// `Eq` and `Hash` are representation-level operations for the concrete formula
/// type. Algorithms that need semantic equality should use
/// [`StateFormula::semantic_key`].
pub trait StateFormula: Clone + Eq + Hash + Debug + Display + Send + Sync + 'static {
    /// Variable type used by this formula representation.
    type Var: Eq + Hash + Clone + Debug + Send + Sync + 'static;

    /// Manager/context type for creating and normalizing formulas.
    type Manager: Clone + Send + Sync + 'static;

    /// Construct `⊤`.
    fn top(mgr: &Self::Manager) -> Self;

    /// Construct `⊥`.
    fn bot(mgr: &Self::Manager) -> Self;

    /// Construct a variable node.
    fn var(mgr: &Self::Manager, v: Self::Var) -> Self;

    /// Construct a normalized conjunction.
    fn and(mgr: &Self::Manager, terms: impl IntoIterator<Item = Self>) -> Self;

    /// Construct a normalized disjunction.
    fn or(mgr: &Self::Manager, terms: impl IntoIterator<Item = Self>) -> Self;

    /// Number of representation nodes reachable from this formula.
    fn size(&self) -> usize;

    /// Deterministic list of distinct variables appearing in this formula.
    fn vars(&self) -> Vec<Self::Var>;

    /// Return the manager that owns this formula.
    fn manager(&self) -> &Self::Manager;

    /// Substitute every variable in `f` using `sub`, preserving formula
    /// structure (`∧`/`∨`/`⊤`/`⊥`) and applying implementation normalization.
    fn substitute(mgr: &Self::Manager, f: &Self, sub: impl FnMut(Self::Var) -> Self) -> Self;

    /// Evaluate `f` under a Boolean valuation `val` for variables.
    fn eval_bool(f: &Self, val: impl FnMut(Self::Var) -> bool) -> bool;

    /// Canonical semantic key for algorithms that must quotient by formula semantics.
    ///
    /// This does not change the representation-level meaning of [`Eq`] or [`Hash`].
    /// Implementations may override this with a more efficient canonicalization.
    fn semantic_key(&self) -> MinimalModelKey<Self::Var>
    where
        Self::Var: Ord,
    {
        default_semantic_key(self)
    }
}
