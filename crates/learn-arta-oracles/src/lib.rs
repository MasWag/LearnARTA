// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Oracle implementations for LearnARTA.
//!
//! The crate currently provides:
//! - exact membership against a concrete target ARTA,
//! - a caching membership wrapper,
//! - an exact white-box equivalence oracle over a known target ARTA.
//!
//! # Example
//!
//! ```
//! use learn_arta_core::time::interval::Interval;
//! use learn_arta_core::{
//!     ArtaBuilder, DagStateFormula, DagStateFormulaManager, DelayRep, LocationId, StateFormula,
//!     TimedWord,
//! };
//! use learn_arta_oracles::{ArtaMembershipOracle, CachingMembershipOracle};
//! use learn_arta_traits::MembershipOracle;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mgr = DagStateFormulaManager::new();
//! let q0 = LocationId::new("q0");
//! let init = DagStateFormula::var(&mgr, q0.clone());
//!
//! let mut builder = ArtaBuilder::new(init);
//! builder.add_location(q0.clone()).add_accepting(q0.clone());
//! builder.add_transition(
//!     q0.clone(),
//!     'a',
//!     Interval::closed(0, 0)?,
//!     DagStateFormula::var(&mgr, q0.clone()),
//! );
//! let target = builder.build()?;
//!
//! let mut mq = CachingMembershipOracle::new(ArtaMembershipOracle::new(target));
//! let word = TimedWord::from_vec(vec![('a', DelayRep::from_integer(0))]);
//!
//! assert!(mq.query(&word).expect("membership query should succeed"));
//! assert_eq!(mq.cache_hits(), 0);
//! assert!(mq.query(&word).expect("cached membership query should succeed"));
//! assert_eq!(mq.cache_hits(), 1);
//! # Ok(())
//! # }
//! ```

pub mod arta_membership;
pub mod caching;
pub mod whitebox;

pub use arta_membership::ArtaMembershipOracle;
pub use caching::CachingMembershipOracle;
pub use whitebox::{WhiteBoxEqOracle, WhiteBoxEqOracleError};
