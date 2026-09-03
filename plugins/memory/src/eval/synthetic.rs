//! Synthetic write-time-gating accuracy corpus, adapted from the Write-Time
//! Gating §4 methodology (arXiv 2603.15994) to this crate's `DefaultSalienceGate`.
//!
//! The paper's experiment concerns a reputation-scored knowledge store; this gate
//! has no reputation concept, so the adaptation maps "high-reputation correct
//! fact" to a `UserStated`, high-importance candidate (always accepted on first
//! write) and "low-reputation distractor" to an `AgentInferred`, low-importance
//! candidate (archived by the salience-threshold check). This is a local
//! adaptation, not a reproduction of the paper's exact experiment — see the
//! deviation recorded in `docs/specs/eval-harness-v1/spec.md`.
//!
//! Distractor embeddings are spread deterministically across `[0.0, 0.75)` cosine
//! similarity to their target, deliberately staying below this gate's
//! `novelty_doubt_threshold` (default 0.80): above that floor, `DefaultSalienceGate`
//! correctly prioritizes near-duplicate merging over salience filtering, which is a
//! different (and equally correct) code path, not a gate-accuracy failure. Keeping
//! the spread in the unambiguous low-similarity band isolates exactly the property
//! under test — that low-value candidates get archived regardless of topical
//! similarity — while trivially avoiding the merge/contradiction branch entirely.

use crate::eval::corpus::{
    CorpusMemory, CorpusScope, ExpectedGateOutcome, GateCase, GateCorpus, MemoryAnchor,
};
use crate::{MemoryType, ProvenanceLevel};

/// Number of targets generated per ratio. The spec's methodology calls for a
/// minimum of 100; Phase A uses a smaller count so `eval run --ci` stays within
/// the CI job's time budget. See the documented deviation in the spec.
pub(crate) const SYNTHETIC_TARGET_COUNT: usize = 20;

/// Distractor-to-target ratio, matching the spec's Synthetic Distractor Corpus section.
// The shared "ToOne" suffix names the ratio family (N:1); it is not accidental
// repetition, so the enum_variant_names lint doesn't apply here.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DistractorRatio {
    OneToOne,
    FourToOne,
    EightToOne,
}

impl DistractorRatio {
    pub const fn distractors_per_target(self) -> usize {
        match self {
            Self::OneToOne => 1,
            Self::FourToOne => 4,
            Self::EightToOne => 8,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::OneToOne => "1to1",
            Self::FourToOne => "4to1",
            Self::EightToOne => "8to1",
        }
    }
}

/// Generates a synthetic gate-decision corpus at the given ratio. Each target
/// contributes one "accept" case (the target written into an empty store) and
/// `ratio` "archive" cases (a low-importance distractor written against a store
/// containing only that target).
pub(crate) fn generate_gate_corpus(ratio: DistractorRatio) -> GateCorpus {
    let distractor_count = ratio.distractors_per_target();
    let mut cases = Vec::with_capacity(SYNTHETIC_TARGET_COUNT * (1 + distractor_count));

    for target_index in 0..SYNTHETIC_TARGET_COUNT {
        let target_key = format!("synthetic-{}-target-{target_index}", ratio.label());
        let target = CorpusMemory {
            key: target_key.clone(),
            content: format!(
                "synthetic target fact number {target_index} for ratio {}",
                ratio.label()
            ),
            memory_type: MemoryType::Fact,
            provenance: ProvenanceLevel::UserStated,
            importance: 0.8,
            anchor: None,
            scope: CorpusScope::default(),
        };

        cases.push(GateCase {
            key: format!("{target_key}-accept"),
            existing: Vec::new(),
            candidate: target.clone(),
            expected: ExpectedGateOutcome::Accept,
        });

        for distractor_index in 0..distractor_count {
            // Deterministic spread across [0.0, 0.75) — see module docs for why 0.75
            // is the deliberate ceiling.
            let target_cosine = (distractor_index as f32 / distractor_count as f32) * 0.75;
            let distractor_key = format!("{target_key}-distractor-{distractor_index}");
            let distractor = CorpusMemory {
                key: distractor_key.clone(),
                content: format!(
                    "synthetic low value distractor entry {distractor_index} near target {target_index}"
                ),
                memory_type: MemoryType::Observation,
                provenance: ProvenanceLevel::AgentInferred,
                importance: 0.05,
                anchor: Some(MemoryAnchor {
                    key: target_key.clone(),
                    target_cosine,
                }),
                scope: CorpusScope::default(),
            };

            cases.push(GateCase {
                key: distractor_key,
                existing: vec![target.clone()],
                candidate: distractor,
                expected: ExpectedGateOutcome::Archive,
            });
        }
    }

    GateCorpus {
        version: format!("synthetic-{}-v1", ratio.label()),
        cases,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_one_accept_case_and_ratio_archive_cases_per_target() {
        for ratio in [
            DistractorRatio::OneToOne,
            DistractorRatio::FourToOne,
            DistractorRatio::EightToOne,
        ] {
            let corpus = generate_gate_corpus(ratio);
            let expected_len = SYNTHETIC_TARGET_COUNT * (1 + ratio.distractors_per_target());
            assert_eq!(corpus.cases.len(), expected_len, "ratio {}", ratio.label());

            let accept_count = corpus
                .cases
                .iter()
                .filter(|case| case.expected == ExpectedGateOutcome::Accept)
                .count();
            assert_eq!(accept_count, SYNTHETIC_TARGET_COUNT);

            let archive_count = corpus
                .cases
                .iter()
                .filter(|case| case.expected == ExpectedGateOutcome::Archive)
                .count();
            assert_eq!(
                archive_count,
                SYNTHETIC_TARGET_COUNT * ratio.distractors_per_target()
            );
        }
    }

    #[test]
    fn distractor_cosine_spread_never_reaches_the_merge_similarity_band() {
        let corpus = generate_gate_corpus(DistractorRatio::EightToOne);
        for case in &corpus.cases {
            if let Some(anchor) = &case.candidate.anchor {
                assert!(
                    anchor.target_cosine < 0.80,
                    "distractor cosine {} for case {} must stay below the novelty floor",
                    anchor.target_cosine,
                    case.key
                );
            }
        }
    }

    #[test]
    fn each_case_key_is_unique_within_a_ratio() {
        let corpus = generate_gate_corpus(DistractorRatio::FourToOne);
        let mut keys: Vec<&str> = corpus.cases.iter().map(|case| case.key.as_str()).collect();
        let original_len = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), original_len, "case keys must be unique");
    }
}
