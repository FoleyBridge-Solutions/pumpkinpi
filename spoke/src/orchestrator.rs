use crate::ObservedReviewEvidence;
use anyhow::{Result, anyhow};
use pumpkinpi_protocol::{
    ImplementationRunResult, IntentTurnProposal, ReviewFindingProposal, ReviewObligation,
    ReviewRunResult, ReviewVerdict,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RealizationPhase {
    Implementing,
    Reviewing,
    WaitingForUser,
    Satisfied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RealizationMachine {
    pub revision: u64,
    pub iteration: u64,
    pub phase: RealizationPhase,
    pub findings: Vec<ReviewFindingProposal>,
}

impl RealizationMachine {
    pub fn start(revision: u64) -> Self {
        Self {
            revision,
            iteration: 1,
            phase: RealizationPhase::Implementing,
            findings: Vec::new(),
        }
    }

    pub fn implementation_completed(&mut self, result: &ImplementationRunResult) -> Result<()> {
        if self.phase != RealizationPhase::Implementing {
            return Err(anyhow!(
                "implementation result is invalid in {:?}",
                self.phase
            ));
        }
        if result.question.is_some() {
            self.phase = RealizationPhase::WaitingForUser;
        } else {
            self.phase = RealizationPhase::Reviewing;
        }
        Ok(())
    }

    pub fn review_completed(&mut self, result: ReviewRunResult) -> Result<()> {
        if self.phase != RealizationPhase::Reviewing {
            return Err(anyhow!("review result is invalid in {:?}", self.phase));
        }
        result.validate().map_err(anyhow::Error::msg)?;
        match result.verdict {
            ReviewVerdict::Approved => {
                self.findings.clear();
                self.phase = RealizationPhase::Satisfied;
            }
            ReviewVerdict::Findings => {
                self.findings = result.findings;
                self.iteration += 1;
                self.phase = RealizationPhase::Implementing;
            }
        }
        Ok(())
    }
}

pub(crate) fn parse_intent_proposal(
    raw: &str,
    current_revision: u64,
) -> Result<IntentTurnProposal> {
    let proposal: IntentTurnProposal = serde_json::from_str(raw.trim())
        .map_err(|e| anyhow!("Intent Agent violated its typed contract: {e}"))?;
    if proposal.projection.trim().is_empty() {
        return Err(anyhow!("Intent Agent projection cannot be empty"));
    }
    if let Some(update) = &proposal.source_update {
        if update.activate && proposal.question.is_some() {
            return Err(anyhow!(
                "Intent Agent cannot activate intent while asking a consequential question"
            ));
        }
        if update.base_revision != current_revision {
            return Err(anyhow!(
                "Intent Agent proposed revision {} from stale base {}; current revision is {}",
                current_revision + 1,
                update.base_revision,
                current_revision
            ));
        }
        if update.canonical_payload.trim().is_empty() {
            return Err(anyhow!("Source of Intent proposal cannot be empty"));
        }
    }
    Ok(proposal)
}

pub(crate) fn intent_prompt(
    revision: u64,
    source: &str,
    authoritative_manifest: &str,
    message: &str,
    assembling: bool,
) -> String {
    format!(
        r#"You are PumpkinPi's Project Intent Agent. You maintain the complete canonical Source of Intent; you do not implement it in this turn.

You may inspect the current project with read-only tools. Never edit files, run mutating commands, or claim a canonical revision in prose. When the owner references local documents, read all relevant documents before proposing intent. Preserve their breadth, specificity, constraints, non-goals, decisions, and acceptance criteria; do not reduce broad project intent to a task.

The authoritative manifest below is part of canonical intent. Read every listed document from the Project and verify its SHA-256 hash before responding. Return exact path/hash pairs in source_coverage. Generated canonical_payload supplements this bundle and must never summarize it away.

Return ONLY one JSON object with this exact schema:
{{"acts":["clarify"|"correct"|"decide"|"reference_context"|"request_projection"|"prioritize"|"pause"|"resume"|"cancel"],"source_coverage":[{{"path":string,"content_hash":string}}],"projection":string,"question":string|null,"source_update":{{"base_revision":number,"canonical_payload":string,"refresh_authoritative_bundle":boolean,"activate":boolean}}|null,"assumptions":[string]}}

A source_update must contain complete conversational amendments and operational context, never a diff or replacement for the authoritative bundle. Set refresh_authoritative_bundle=true only when the owner explicitly directs adoption of changed/current Project documents; include reference_context in acts and cover the replacement manifest exactly. Set activate=true when intent is sufficiently established to govern autonomous realization. Active intent is standing authorization for PumpkinPi to iterate implementation and independent review until no reviewer findings remain. A question keeps intent from activation only when the missing answer is consequential.

INITIALIZATION IS ASSEMBLING: {assembling}
CURRENT SOURCE OF INTENT revision {revision}:
{source}

AUTHORITATIVE DOCUMENT BUNDLE:
{authoritative_manifest}

OWNER MESSAGE:
{message}"#
    )
}

pub(crate) fn implementation_prompt(
    revision: u64,
    source: &str,
    iteration: u64,
    findings: &[ReviewFindingProposal],
    authoritative_manifest: &str,
) -> String {
    let findings = serde_json::to_string_pretty(findings).unwrap_or_else(|_| "[]".into());
    format!(
        r#"You are an internal PumpkinPi implementation Run. Implement the active Source of Intent in the current project.

This is iteration {iteration}. Inspect current reality, select the highest-value bounded objective, make concrete changes, and validate them. If reviewer findings are supplied, address them without weakening or rewriting intent. Do not modify PumpkinPi's external Source of Intent storage. Do not merely describe code that should be written.

Read and hash every document in the authoritative bundle before changing files. Never modify those documents. Return exact path/hash pairs in source_coverage.

Return ONLY one JSON object after tool work with this schema:
{{"source_coverage":[{{"path":string,"content_hash":string}}],"objective":string,"summary":string,"observations":[string],"changes":[string],"validation":[string],"evidence":[string],"residual_divergence":[string],"question":string|null}}

Every change and validation claim must cite concrete evidence. Ask a question only when a consequential owner decision is genuinely required.

SOURCE OF INTENT revision {revision}:
{source}

AUTHORITATIVE DOCUMENT BUNDLE:
{authoritative_manifest}

CURRENT REVIEW FINDINGS:
{findings}"#
    )
}

pub(crate) fn review_prompt(
    revision: u64,
    observed_reality_version: &str,
    source: &str,
    implementation: &ImplementationRunResult,
    authoritative_manifest: &str,
    obligations: &[ReviewObligation],
) -> String {
    let implementation =
        serde_json::to_string_pretty(implementation).unwrap_or_else(|_| "{}".into());
    let obligations = serde_json::to_string_pretty(obligations).unwrap_or_else(|_| "[]".into());
    format!(
        r#"You are PumpkinPi's independent whole-Project reviewer. You did not implement this iteration. Inspect the complete current project using read-only tools and assess it against every applicable part of the complete Source of Intent, not only the latest diff.

Find every fault you can: omissions, incorrect behavior, architecture violations, regressions, unsupported claims, missing tests, weak recovery, security failures, and unfulfilled requirements. Run non-mutating validation where useful. Do not approve merely because the implementation Run says it succeeded. Do not weaken or rewrite intent.

Read and hash every document in the authoritative bundle before review. Return exact path/hash pairs in source_coverage. Missing or changed coverage prohibits approval.

This is the evidence-capture phase. Perform all inspection and validation tool work, then end your turn without a final assessment. The Spoke will durably record the observed tool results and send their generated evidence IDs back in a second, Spoke-controlled assessment prompt. Only that second turn may return the final JSON below. Never guess or precompute evidence IDs.

On the second turn, return ONLY one JSON object with this schema:
{{"source_coverage":[{{"path":string,"content_hash":string}}],"target_revision":number,"observed_reality_version":string,"scope":"whole_project","reviewed_scope":[string],"checks":[string],"evidence":[string],"obligation_observations":[{{"obligation_id":string,"evidence_id":string}}],"findings":[{{"requirement":string,"fault":string,"evidence":[string],"suggested_next_objective":string|null}}],"unreviewed_required_scope":[string],"verdict":"findings"|"approved"}}

The Spoke issued the obligations below before this Run. For approval, perform every exact required read or command. A file obligation requires an unbounded read from line 1. Put every obligation_id exactly once in reviewed_scope; put every exact obligation subject in checks; put every captured durable sha256 evidence ID in evidence; and map each obligation to its own successful event in obligation_observations. One event cannot satisfy multiple obligations. Output text, copied coverage, arbitrary scope prose, partial reads, and unissued checks cannot establish approval. Findings may continue to cite descriptive evidence.

Echo exactly target_revision {revision} and observed_reality_version "{observed_reality_version}". The Spoke independently verifies both bindings, exact canonical source coverage, unchanged Project reality, and complete obligation evidence. Use verdict "approved" only when every issued obligation is satisfied, findings and unreviewed_required_scope are empty, and the corroborated evidence supports finding no fault in Project reality against this entire intent revision.

SPOKE-ISSUED REVIEW OBLIGATIONS:
{obligations}

SOURCE OF INTENT revision {revision}:
{source}

AUTHORITATIVE DOCUMENT BUNDLE:
{authoritative_manifest}

IMPLEMENTATION RUN CLAIMS (claims are not evidence):
{implementation}"#
    )
}

pub(crate) fn review_assessment_prompt(observations: &[ObservedReviewEvidence]) -> String {
    let observations = serde_json::to_string_pretty(observations).unwrap_or_else(|_| "[]".into());
    format!(
        r#"The Spoke has durably recorded the tool observations from your evidence-capture turn. Their generated IDs and independently captured metadata are below.

Now make the final assessment. Return ONLY the typed review JSON requested in the original review instructions. Bind obligations only to applicable successful observations from this manifest. Copy evidence_id values exactly; never invent, derive, or substitute an ID. Approval is prohibited unless this manifest independently and completely supports every issued obligation. Findings remain required for every fault, and any incomplete scope must be reported.

SPOKE-RECORDED OBSERVATION MANIFEST:
{observations}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn implementation() -> ImplementationRunResult {
        ImplementationRunResult {
            source_coverage: vec![],
            objective: "typed contract".into(),
            summary: "implemented".into(),
            observations: vec![],
            changes: vec!["protocol contract".into()],
            validation: vec!["cargo test".into()],
            evidence: vec!["tests pass".into()],
            residual_divergence: vec![],
            question: None,
        }
    }

    #[test]
    fn findings_always_drive_another_iteration() {
        let mut machine = RealizationMachine::start(4);
        machine.implementation_completed(&implementation()).unwrap();
        machine
            .review_completed(ReviewRunResult {
                source_coverage: vec![],
                target_revision: 4,
                observed_reality_version: "reality-1".into(),
                scope: pumpkinpi_protocol::ReviewScope::WholeProject,
                reviewed_scope: vec!["workspace".into()],
                checks: vec![],
                evidence: vec![],
                obligation_observations: vec![],
                findings: vec![ReviewFindingProposal {
                    requirement: "durable replay".into(),
                    fault: "restart loses cursor".into(),
                    evidence: vec!["recovery test failed".into()],
                    suggested_next_objective: Some("persist cursor before ack".into()),
                }],
                unreviewed_required_scope: vec![],
                verdict: ReviewVerdict::Findings,
            })
            .unwrap();
        assert_eq!(machine.phase, RealizationPhase::Implementing);
        assert_eq!(machine.iteration, 2);
        assert_eq!(machine.findings.len(), 1);
    }

    #[test]
    fn only_complete_zero_finding_review_satisfies() {
        let mut machine = RealizationMachine::start(4);
        machine.implementation_completed(&implementation()).unwrap();
        machine
            .review_completed(ReviewRunResult {
                source_coverage: vec![],
                target_revision: 4,
                observed_reality_version: "reality-1".into(),
                scope: pumpkinpi_protocol::ReviewScope::WholeProject,
                reviewed_scope: vec!["workspace".into()],
                checks: vec!["cargo test --workspace".into()],
                evidence: vec!["workspace test output: passed".into()],
                obligation_observations: vec![pumpkinpi_protocol::ReviewObligationObservation {
                    obligation_id: "workspace".into(),
                    evidence_id: "workspace test output: passed".into(),
                }],
                findings: vec![],
                unreviewed_required_scope: vec![],
                verdict: ReviewVerdict::Approved,
            })
            .unwrap();
        assert_eq!(machine.phase, RealizationPhase::Satisfied);
    }

    #[test]
    fn iteration_count_never_becomes_success() {
        let mut machine = RealizationMachine::start(9);
        for expected_iteration in 1..=100 {
            machine.implementation_completed(&implementation()).unwrap();
            machine
                .review_completed(ReviewRunResult {
                    source_coverage: vec![],
                    target_revision: 9,
                    observed_reality_version: "reality-1".into(),
                    scope: pumpkinpi_protocol::ReviewScope::WholeProject,
                    reviewed_scope: vec!["complete project".into()],
                    checks: vec![],
                    evidence: vec![],
                    obligation_observations: vec![],
                    findings: vec![ReviewFindingProposal {
                        requirement: "still exact".into(),
                        fault: format!("fault {expected_iteration}"),
                        evidence: vec!["review evidence".into()],
                        suggested_next_objective: None,
                    }],
                    unreviewed_required_scope: vec![],
                    verdict: ReviewVerdict::Findings,
                })
                .unwrap();
            assert_eq!(machine.phase, RealizationPhase::Implementing);
        }
        assert_eq!(machine.iteration, 101);
    }

    #[test]
    fn activation_with_a_question_is_rejected() {
        let invalid = r##"{"acts":["reference_context"],"source_coverage":[],"projection":"maybe","question":"Which design?","source_update":{"base_revision":0,"canonical_payload":"# Intent","activate":true},"assumptions":[]}"##;
        assert!(parse_intent_proposal(invalid, 0).is_err());
    }

    #[test]
    fn old_execute_contract_is_rejected() {
        let old = r#"{"response":"done","execute":true,"work_request":"everything"}"#;
        assert!(parse_intent_proposal(old, 1).is_err());
    }
}
