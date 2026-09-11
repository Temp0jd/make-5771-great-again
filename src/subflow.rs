//! Portable subflow definitions and deterministic selection policy.
use crate::model::{SearchRegionSpec, WorkflowStep};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subflow {
    pub id: u64,
    pub name: String,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ChoicePolicy {
    #[default]
    Prefer,
    Allow,
    Forbid,
}
impl ChoicePolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::Prefer => "优先",
            Self::Allow => "允许随机",
            Self::Forbid => "禁止",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceRule {
    pub id: u64,
    pub name: String,
    pub template: Option<String>,
    pub policy: ChoicePolicy,
}

/// A user-declared occupied candidate area, in the profile reference resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceSlot {
    pub id: u64,
    pub name: String,
    pub region: SearchRegionSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PriorityChoice {
    /// Required stage marker prevents random clicks outside the intended screen.
    pub stage_template: Option<String>,
    pub rules: Vec<ChoiceRule>,
    pub slots: Vec<ChoiceSlot>,
    /// Explicitly unsafe fallback: an unrecognized candidate may be forbidden.
    pub random_unknown: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotEvidence {
    Preferred(usize),
    Allowed,
    Unknown,
    Blocked,
    Ambiguous,
}

/// Forbid dominates; conflicting positive identities never become random candidates.
pub fn classify_slot(hits: &[(usize, ChoicePolicy)], ambiguous: bool) -> SlotEvidence {
    if hits
        .iter()
        .any(|(_, policy)| *policy == ChoicePolicy::Forbid)
    {
        return SlotEvidence::Blocked;
    }
    if ambiguous || hits.len() > 1 {
        return SlotEvidence::Ambiguous;
    }
    match hits.first() {
        Some((rank, ChoicePolicy::Prefer)) => SlotEvidence::Preferred(*rank),
        Some((_, ChoicePolicy::Allow)) => SlotEvidence::Allowed,
        Some((_, ChoicePolicy::Forbid)) => SlotEvidence::Blocked,
        None => SlotEvidence::Unknown,
    }
}

/// Rules are ranked by list order, never by recognition score. Random selection
/// only operates on the declared slots, after all exclusion checks finish.
pub fn choose_slot(evidence: &[SlotEvidence], random_unknown: bool, random: u64) -> Option<usize> {
    let best_rank = evidence
        .iter()
        .filter_map(|e| match e {
            SlotEvidence::Preferred(rank) => Some(*rank),
            _ => None,
        })
        .min();
    let candidates: Vec<_> = evidence
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let eligible = if let Some(rank) = best_rank {
                *e == SlotEvidence::Preferred(rank)
            } else {
                *e == SlotEvidence::Allowed || (random_unknown && *e == SlotEvidence::Unknown)
            };
            eligible.then_some(i)
        })
        .collect();
    (!candidates.is_empty()).then(|| candidates[(random % candidates.len() as u64) as usize])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn priority_is_rule_order_not_slot_order() {
        let e = [
            SlotEvidence::Preferred(5),
            SlotEvidence::Unknown,
            SlotEvidence::Preferred(1),
        ];
        assert_eq!(choose_slot(&e, true, 0), Some(2));
    }
    #[test]
    fn random_never_uses_blocked_or_ambiguous_slots() {
        let e = [
            SlotEvidence::Blocked,
            SlotEvidence::Unknown,
            SlotEvidence::Ambiguous,
            SlotEvidence::Allowed,
        ];
        assert_eq!(choose_slot(&e, false, 2), Some(3));
        for seed in 0..20 {
            assert!([Some(1), Some(3)].contains(&choose_slot(&e, true, seed)));
        }
        assert_eq!(
            choose_slot(&[SlotEvidence::Blocked, SlotEvidence::Ambiguous], true, 0),
            None
        );
        assert_eq!(choose_slot(&[], true, 0), None);
        assert_eq!(choose_slot(&[SlotEvidence::Unknown], false, 0), None);
    }
    #[test]
    fn forbid_and_ambiguity_override_preference() {
        assert_eq!(
            classify_slot(
                &[(0, ChoicePolicy::Prefer), (1, ChoicePolicy::Forbid)],
                false
            ),
            SlotEvidence::Blocked
        );
        assert_eq!(
            classify_slot(&[(0, ChoicePolicy::Prefer)], true),
            SlotEvidence::Ambiguous
        );
        assert_eq!(
            classify_slot(
                &[(0, ChoicePolicy::Prefer), (1, ChoicePolicy::Allow)],
                false
            ),
            SlotEvidence::Ambiguous
        );
    }
}
