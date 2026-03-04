// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Active learning for Alternating Real-Time Automata (ARTA).
//!
//! This crate contains the observation-table machinery, cohesion repairs,
//! evidence-AFA construction, and hypothesis conversion used by LearnARTA's
//! end-to-end learner.
//!
//! # Example
//!
//! ```
//! use learn_arta::ActiveArtaLearner;
//! use learn_arta_core::time::interval::Interval;
//! use learn_arta_core::{
//!     ArtaBuilder, DagStateFormula, DagStateFormulaManager, DelayRep, LocationId, StateFormula,
//!     TimedWord,
//! };
//! use learn_arta_traits::MembershipOracle;
//! use std::convert::Infallible;
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
//! #[derive(Clone)]
//! struct ExactMq(learn_arta_core::Arta<char>);
//!
//! impl MembershipOracle for ExactMq {
//!     type Symbol = char;
//!     type Error = Infallible;
//!
//!     fn query(&mut self, w: &TimedWord<Self::Symbol>) -> Result<bool, Self::Error> {
//!         Ok(self.0.accepts(w))
//!     }
//! }
//!
//! let mut learner = ActiveArtaLearner::<char>::new();
//! let mut mq = ExactMq(target.clone());
//! let hypothesis = learner.build_hypothesis(&mut mq)?;
//!
//! let word = TimedWord::from_vec(vec![('a', DelayRep::from_integer(0))]);
//! assert!(!hypothesis.locations().is_empty());
//! assert!(target.accepts(&word));
//! # Ok(())
//! # }
//! ```

pub mod basis;
pub mod cohesion;
pub mod decomposition;
pub mod error;
pub mod evidence_afa;
pub mod hypothesis_arta;
pub mod learner;
pub mod observation_table;
pub mod rowvec;

pub use basis::{
    ApproxMilpConfig, BasisMinimization, BasisMinimizationError, BasisMinimizer,
    BasisReductionPhase,
};
pub use cohesion::{
    BasisWords, CohesionCheckError, CohesionFix, CohesionStepError, apply_fix,
    find_not_basis_closed, find_not_distinct, find_not_evidence_closed, find_redundant_basis_word,
    make_cohesive_step, next_cohesion_fix,
};
pub use decomposition::{BasisDecomposer, BasisFormula, BasisVar, DecompositionError};
pub use error::LearnError;
pub use evidence_afa::{AfaStateId, EvidenceAfa, EvidenceAfaError, build_from_cohesive_table};
pub use hypothesis_arta::{
    HypothesisArtaError, convert_basis_formula_to_dag_state_formula, evidence_state_to_location_id,
};
pub use learner::{ActiveArtaLearner, ActiveArtaLearnerState};
pub use observation_table::{ObservationTable, TableError, TableQueryError};
pub use rowvec::{RowVec, RowVecError};
