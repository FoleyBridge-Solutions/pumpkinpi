use pumpkinpi_protocol::{
    DivergenceId, DivergenceRecord, DivergenceState, DivergenceTransitionSummary, ProjectId,
    RequirementId, RequirementIndex, RequirementNode, ReviewFindingProposal, SourceOfIntentRecord,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

fn digest(value: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(value.as_ref()))
}

fn normalized_tokens(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|token| token.len() > 2)
        .collect()
}

fn requirement_kind(heading: &str, text: &str) -> String {
    let value = format!("{heading} {text}").to_ascii_lowercase();
    if value.contains("security") || value.contains("trust") || value.contains("must not") {
        "constraint"
    } else if value.contains("acceptance") || value.contains("scenario") {
        "acceptance"
    } else if value.contains("non-goal") || value.contains("prohibited") {
        "non_goal"
    } else {
        "behavior"
    }
    .into()
}

fn acceptance_criteria(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| {
            let line = line.to_ascii_lowercase();
            line.contains(" must ")
                || line.starts_with("must ")
                || line.contains(" requires ")
                || line.contains(" only when ")
        })
        .map(str::to_string)
        .collect()
}

fn affected_components(finding: &ReviewFindingProposal) -> Vec<String> {
    let mut components = BTreeSet::new();
    for value in std::iter::once(&finding.fault).chain(finding.evidence.iter()) {
        for token in value.split_whitespace() {
            let token = token.trim_matches(|character: char| {
                matches!(character, ',' | '.' | ':' | ';' | '(' | ')' | '`')
            });
            if token.contains('/') || token.ends_with(".rs") || token.ends_with(".toml") {
                components.insert(token.to_string());
            }
        }
    }
    components.into_iter().collect()
}

fn similarity(left: &str, right: &str) -> f64 {
    let left = normalized_tokens(left);
    let right = normalized_tokens(right);
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(&right).count() as f64;
    let union = left.union(&right).count() as f64;
    intersection / union
}

/// Compile a disposable graph from exact canonical bytes. Nodes deliberately retain their source
/// span and hash so this projection can accelerate orchestration without becoming authority.
pub(crate) fn compile_requirement_index(
    source: &SourceOfIntentRecord,
    generated_at: u64,
) -> RequirementIndex {
    let mut nodes = Vec::new();
    if let Some(bundle) = &source.authoritative_bundle {
        for document in &bundle.documents {
            let lines = document.content.lines().collect::<Vec<_>>();
            let headings = lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.trim_start().starts_with('#'))
                .map(|(index, line)| (index, line.trim_start_matches('#').trim().to_string()))
                .collect::<Vec<_>>();
            for (position, (start, heading)) in headings.iter().enumerate() {
                let end = headings
                    .get(position + 1)
                    .map_or(lines.len(), |(next, _)| *next);
                let text = lines[*start..end].join("\n");
                if text.trim().is_empty() {
                    continue;
                }
                let identity = format!(
                    "{}\0{}\0{}\0{}",
                    document.path,
                    start + 1,
                    end,
                    digest(text.as_bytes())
                );
                nodes.push(RequirementNode {
                    requirement_id: RequirementId(format!("req_{}", &digest(identity)[..24])),
                    document_path: document.path.clone(),
                    heading: heading.clone(),
                    start_line: (*start + 1) as u64,
                    end_line: end as u64,
                    source_hash: digest(text.as_bytes()),
                    kind: requirement_kind(heading, &text),
                    dependencies: vec![],
                    acceptance_criteria: acceptance_criteria(&text),
                    text,
                });
            }
        }
    }
    // Generated canonical payload supplements the exact bundle and must participate in coverage.
    if !source.canonical_payload.trim().is_empty() {
        let text = source.canonical_payload.clone();
        let hash = digest(text.as_bytes());
        nodes.push(RequirementNode {
            requirement_id: RequirementId(format!(
                "req_{}",
                &digest(format!("canonical\0{hash}"))[..24]
            )),
            document_path: "pumpkinpi:canonical_payload".into(),
            heading: "Canonical payload".into(),
            start_line: 1,
            end_line: text.lines().count().max(1) as u64,
            source_hash: hash,
            kind: "generated_context".into(),
            dependencies: vec![],
            acceptance_criteria: acceptance_criteria(&text),
            text,
        });
    }
    RequirementIndex {
        source_of_intent_revision: source.revision,
        source_content_hash: source.content_hash.clone(),
        nodes,
        generated_at,
    }
}

pub(crate) fn bind_requirements(
    finding: &ReviewFindingProposal,
    index: &RequirementIndex,
) -> Vec<RequirementId> {
    if !finding.requirement_ids.is_empty() {
        let valid = index
            .nodes
            .iter()
            .map(|node| node.requirement_id.clone())
            .collect::<BTreeSet<_>>();
        return finding
            .requirement_ids
            .iter()
            .filter(|id| valid.contains(*id))
            .cloned()
            .collect();
    }
    let mut scored = index
        .nodes
        .iter()
        .map(|node| {
            (
                similarity(
                    &finding.requirement,
                    &format!("{} {}", node.heading, node.text),
                ),
                node.requirement_id.clone(),
            )
        })
        .filter(|(score, _)| *score >= 0.12)
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .0
            .total_cmp(&left.0)
            .then_with(|| left.1.cmp(&right.1))
    });
    scored.into_iter().take(3).map(|(_, id)| id).collect()
}

fn finding_fingerprint(finding: &ReviewFindingProposal, requirements: &[RequirementId]) -> String {
    let requirements = requirements
        .iter()
        .map(|id| id.0.as_str())
        .collect::<Vec<_>>()
        .join("\0");
    let normalized = normalized_tokens(&format!("{} {}", finding.requirement, finding.fault))
        .into_iter()
        .collect::<Vec<_>>()
        .join(" ");
    digest(format!("{requirements}\0{normalized}"))
}

fn matches_existing(
    finding: &ReviewFindingProposal,
    requirement_ids: &[RequirementId],
    existing: &DivergenceRecord,
) -> bool {
    let shared_requirement = !requirement_ids.is_empty()
        && requirement_ids
            .iter()
            .any(|id| existing.requirement_ids.contains(id));
    let requirement_score = similarity(&finding.requirement, &existing.requirement);
    let fault_score = similarity(&finding.fault, &existing.fault);
    (shared_requirement && fault_score >= 0.35)
        || (requirement_score >= 0.55 && fault_score >= 0.45)
}

/// Reconcile a complete review into durable identities. Existing findings absent from the complete
/// review become verified, but that transition alone never implies Project approval.
pub(crate) fn reconcile(
    project_id: &ProjectId,
    revision: u64,
    reality: &str,
    findings: &[ReviewFindingProposal],
    index: &RequirementIndex,
    ledger: &mut BTreeMap<DivergenceId, DivergenceRecord>,
    observed_at: u64,
) -> DivergenceTransitionSummary {
    let mut summary = DivergenceTransitionSummary::default();
    let mut matched = BTreeSet::new();

    for finding in findings {
        let requirement_ids = bind_requirements(finding, index);
        let existing_id = ledger
            .iter()
            .filter(|(id, item)| {
                item.project_id == *project_id
                    && item.source_of_intent_revision == revision
                    && !matched.contains(*id)
                    && matches_existing(finding, &requirement_ids, item)
            })
            .max_by(|(_, left), (_, right)| {
                let left_score = similarity(&finding.fault, &left.fault);
                let right_score = similarity(&finding.fault, &right.fault);
                left_score.total_cmp(&right_score)
            })
            .map(|(id, _)| id.clone());

        if let Some(id) = existing_id {
            let record = ledger.get_mut(&id).expect("matched divergence disappeared");
            matched.insert(id);
            if matches!(
                record.state,
                DivergenceState::Verified | DivergenceState::Addressed
            ) {
                record.state = DivergenceState::Reopened;
                record.reopen_count += 1;
                summary.reopened += 1;
            } else {
                record.state = DivergenceState::Open;
                summary.still_open += 1;
            }
            record.requirement_ids = requirement_ids;
            record.requirement = finding.requirement.clone();
            record.fault = finding.fault.clone();
            record.affected_components = affected_components(finding);
            record.evidence = finding.evidence.clone();
            record.verification_criteria =
                finding.suggested_next_objective.iter().cloned().collect();
            record.suggested_next_objective = finding.suggested_next_objective.clone();
            record.last_observed_reality = reality.into();
            record.attempt_count += 1;
            record.updated_at = observed_at;
        } else {
            let fingerprint = finding_fingerprint(finding, &requirement_ids);
            let id = DivergenceId(format!("div_{}", &fingerprint[..24]));
            // Two duplicate findings in one review resolve to one identity.
            if matched.contains(&id) {
                continue;
            }
            matched.insert(id.clone());
            ledger.insert(
                id.clone(),
                DivergenceRecord {
                    divergence_id: id,
                    fingerprint,
                    project_id: project_id.clone(),
                    source_of_intent_revision: revision,
                    requirement_ids,
                    requirement: finding.requirement.clone(),
                    fault: finding.fault.clone(),
                    state: DivergenceState::Open,
                    affected_components: affected_components(finding),
                    evidence: finding.evidence.clone(),
                    verification_criteria: finding
                        .suggested_next_objective
                        .iter()
                        .cloned()
                        .collect(),
                    suggested_next_objective: finding.suggested_next_objective.clone(),
                    first_observed_reality: reality.into(),
                    last_observed_reality: reality.into(),
                    attempt_count: 1,
                    reopen_count: 0,
                    created_at: observed_at,
                    updated_at: observed_at,
                },
            );
            summary.opened += 1;
        }
    }

    for (id, record) in ledger.iter_mut().filter(|(_, item)| {
        item.project_id == *project_id && item.source_of_intent_revision == revision
    }) {
        if !matched.contains(id)
            && matches!(
                record.state,
                DivergenceState::Open | DivergenceState::Reopened
            )
        {
            record.state = DivergenceState::Verified;
            record.last_observed_reality = reality.into();
            record.updated_at = observed_at;
            summary.verified += 1;
        }
    }
    summary
}

pub(crate) fn open_for_prompt(
    project_id: &ProjectId,
    revision: u64,
    ledger: &BTreeMap<DivergenceId, DivergenceRecord>,
) -> Vec<DivergenceRecord> {
    let mut records = ledger
        .values()
        .filter(|item| {
            item.project_id == *project_id
                && item.source_of_intent_revision == revision
                && matches!(
                    item.state,
                    DivergenceState::Open | DivergenceState::Reopened
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    records.sort_by_key(|item| {
        (
            std::cmp::Reverse(item.reopen_count),
            std::cmp::Reverse(item.attempt_count),
            item.divergence_id.clone(),
        )
    });
    records
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkinpi_protocol::{SourceDocument, SourceOfIntentBundle, SourceStatus};

    fn source() -> SourceOfIntentRecord {
        SourceOfIntentRecord {
            source_of_intent_id: "source".into(),
            spoke_id: "spoke".into(),
            project_id: "project".into(),
            format: "markdown".into(),
            revision: 2,
            canonical_payload: "Keep exact intent.".into(),
            authoritative_bundle: Some(SourceOfIntentBundle {
                manifest_path: "design.md".into(),
                bundle_hash: "bundle".into(),
                documents: vec![SourceDocument {
                    path: "design.md".into(),
                    content_hash: "document".into(),
                    byte_len: 48,
                    content: "# Security\nReview is read-only.\n\n# Recovery\nResume safely.\n"
                        .into(),
                }],
            }),
            content_hash: "source-hash".into(),
            status: SourceStatus::Active,
            created_at: 1,
            updated_at: 1,
        }
    }

    fn finding(fault: &str) -> ReviewFindingProposal {
        ReviewFindingProposal {
            requirement_ids: vec![],
            requirement: "Review must be read-only".into(),
            fault: fault.into(),
            evidence: vec!["observed".into()],
            suggested_next_objective: Some("isolate review".into()),
        }
    }

    #[test]
    fn requirement_nodes_are_stable_and_traceable() {
        let source = source();
        let first = compile_requirement_index(&source, 10);
        let second = compile_requirement_index(&source, 20);
        assert_eq!(first.nodes.len(), 3);
        assert_eq!(
            first.nodes[0].requirement_id,
            second.nodes[0].requirement_id
        );
        assert_eq!(first.nodes[0].document_path, "design.md");
        assert_eq!(first.nodes[0].start_line, 1);
    }

    #[test]
    fn complete_reviews_reconcile_stable_divergence_history() {
        let project = ProjectId("project".into());
        let index = compile_requirement_index(&source(), 1);
        let mut ledger = BTreeMap::new();
        let first = reconcile(
            &project,
            2,
            "reality-1",
            &[finding("Reviewer can modify the worktree")],
            &index,
            &mut ledger,
            1,
        );
        assert_eq!(first.opened, 1);
        let second = reconcile(
            &project,
            2,
            "reality-2",
            &[finding("The reviewer can modify its worktree")],
            &index,
            &mut ledger,
            2,
        );
        assert_eq!(second.still_open, 1);
        assert_eq!(ledger.len(), 1);
        let verified = reconcile(&project, 2, "reality-3", &[], &index, &mut ledger, 3);
        assert_eq!(verified.verified, 1);
        let reopened = reconcile(
            &project,
            2,
            "reality-4",
            &[finding("Reviewer can modify the worktree")],
            &index,
            &mut ledger,
            4,
        );
        assert_eq!(reopened.reopened, 1);
    }
}
