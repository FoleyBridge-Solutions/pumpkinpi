use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

pub const PROTOCOL_VERSION: u32 = 3;
pub fn protocol_version() -> u32 {
    PROTOCOL_VERSION
}

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
        impl From<String> for $name {
            fn from(v: String) -> Self {
                Self(v)
            }
        }
        impl From<&str> for $name {
            fn from(v: &str) -> Self {
                Self(v.into())
            }
        }
    };
}
id_type!(SpokeId);
id_type!(ProjectId);
id_type!(SourceOfIntentId);
id_type!(IntentChatId);
id_type!(OperationId);
id_type!(TimelineItemId);
id_type!(SessionId);
id_type!(RunId);
id_type!(ReviewId);
id_type!(EvidenceId);
id_type!(FindingId);
id_type!(RequirementId);
id_type!(DivergenceId);
id_type!(RequestId);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProjectKey {
    pub spoke_id: SpokeId,
    pub project_id: ProjectId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpokeStatus {
    Offline,
    Online,
    Disabled,
    Revoked,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Active,
    Missing,
    Stale,
    Removed,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InitializationStatus {
    Uninitialized,
    Inspecting,
    Clarifying,
    Ready,
    Failed,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentStatus {
    Ready,
    WaitingForUser,
    UpdatingIntent,
    Working,
    Blocked,
    Stale,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    Absent,
    Assembling,
    Active,
    Updating,
    Conflicted,
    Unavailable,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Queued,
    Accepted,
    Running,
    Blocked,
    Completed,
    Failed,
    Cancelled,
    Rejected,
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionPurpose {
    Intent,
    Inspection,
    Implementation,
    Validation,
    Review,
    ApprovalReview,
    Recovery,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Starting,
    Idle,
    Running,
    Blocked,
    Stopped,
    Crashed,
    Missing,
    Stale,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimelineKind {
    UserIntent,
    AssistantProjection,
    Question,
    Decision,
    IntentUpdate,
    Progress,
    Outcome,
    Evidence,
    ConsequentialPrompt,
    Lifecycle,
    Error,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Primary,
    Detail,
    Diagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RealizationStatus {
    #[default]
    Inactive,
    Reconciling,
    Reviewing,
    WaitingForUser,
    Paused,
    Satisfied,
    Blocked,
    Stale,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    Findings,
    Approved,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewScope {
    BoundedObjective,
    RequestedOutcome,
    WholeProject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpokeRecord {
    pub spoke_id: SpokeId,
    pub name: String,
    pub hostname: String,
    pub version: String,
    pub status: SpokeStatus,
    pub created_at: u64,
    pub enrolled_at: Option<u64>,
    pub last_seen_at: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub project_id: ProjectId,
    pub spoke_id: SpokeId,
    pub name: String,
    pub cwd: String,
    pub source_of_intent_id: SourceOfIntentId,
    pub intent_chat_id: IntentChatId,
    pub initialization_status: InitializationStatus,
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub run_as_user: Option<String>,
    pub allow_root_sessions: bool,
    pub status: ProjectStatus,
    pub trusted: bool,
    #[serde(default)]
    pub realization_status: RealizationStatus,
    pub created_at: u64,
    pub updated_at: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDocument {
    pub path: String,
    pub content_hash: String,
    pub byte_len: u64,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceOfIntentBundle {
    pub manifest_path: String,
    pub bundle_hash: String,
    pub documents: Vec<SourceDocument>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentCoverage {
    pub path: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceOfIntentRecord {
    pub source_of_intent_id: SourceOfIntentId,
    pub spoke_id: SpokeId,
    pub project_id: ProjectId,
    pub format: String,
    pub revision: u64,
    pub canonical_payload: String,
    /// Exact authoritative source material. Generated payloads supplement, never replace, it.
    #[serde(default)]
    pub authoritative_bundle: Option<SourceOfIntentBundle>,
    pub content_hash: String,
    pub status: SourceStatus,
    pub created_at: u64,
    pub updated_at: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceOfIntentMetadata {
    pub source_of_intent_id: SourceOfIntentId,
    pub spoke_id: SpokeId,
    pub project_id: ProjectId,
    pub format: String,
    pub revision: u64,
    pub content_hash: String,
    #[serde(default)]
    pub authoritative_bundle_hash: Option<String>,
    #[serde(default)]
    pub authoritative_document_count: u64,
    pub status: SourceStatus,
    pub created_at: u64,
    pub updated_at: u64,
}
impl From<&SourceOfIntentRecord> for SourceOfIntentMetadata {
    fn from(source: &SourceOfIntentRecord) -> Self {
        Self {
            source_of_intent_id: source.source_of_intent_id.clone(),
            spoke_id: source.spoke_id.clone(),
            project_id: source.project_id.clone(),
            format: source.format.clone(),
            revision: source.revision,
            content_hash: source.content_hash.clone(),
            authoritative_bundle_hash: source
                .authoritative_bundle
                .as_ref()
                .map(|bundle| bundle.bundle_hash.clone()),
            authoritative_document_count: source
                .authoritative_bundle
                .as_ref()
                .map_or(0, |bundle| bundle.documents.len() as u64),
            status: source.status.clone(),
            created_at: source.created_at,
            updated_at: source.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentChatRecord {
    pub intent_chat_id: IntentChatId,
    pub spoke_id: SpokeId,
    pub project_id: ProjectId,
    pub source_of_intent_revision: u64,
    pub status: IntentStatus,
    pub next_cursor: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub last_active_at: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationRecord {
    pub operation_id: OperationId,
    pub request_id: Option<RequestId>,
    pub spoke_id: SpokeId,
    pub project_id: ProjectId,
    pub intent_chat_id: IntentChatId,
    pub source_of_intent_revision: Option<u64>,
    pub kind: String,
    pub status: OperationStatus,
    pub error: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub completed_at: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineItem {
    pub timeline_item_id: TimelineItemId,
    pub spoke_id: SpokeId,
    pub project_id: ProjectId,
    pub intent_chat_id: IntentChatId,
    pub operation_id: Option<OperationId>,
    pub session_id: Option<SessionId>,
    pub run_id: Option<RunId>,
    pub source_of_intent_revision: Option<u64>,
    pub kind: TimelineKind,
    pub visibility: Visibility,
    pub status: Option<OperationStatus>,
    pub summary: Option<String>,
    pub content: Option<String>,
    pub cursor: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub completed_at: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: SessionId,
    pub run_id: Option<RunId>,
    pub spoke_id: SpokeId,
    pub project_id: ProjectId,
    pub purpose: SessionPurpose,
    pub source_of_intent_revision: Option<u64>,
    pub parent_operation_id: Option<OperationId>,
    pub status: SessionStatus,
    pub run_as_user: Option<String>,
    pub run_as_root: bool,
    pub pi_session_file: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DivergenceState {
    Open,
    Addressed,
    Verified,
    Reopened,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementNode {
    pub requirement_id: RequirementId,
    pub document_path: String,
    pub heading: String,
    pub start_line: u64,
    pub end_line: u64,
    pub source_hash: String,
    pub kind: String,
    pub dependencies: Vec<RequirementId>,
    pub acceptance_criteria: Vec<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementIndex {
    pub source_of_intent_revision: u64,
    pub source_content_hash: String,
    pub nodes: Vec<RequirementNode>,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DivergenceRecord {
    pub divergence_id: DivergenceId,
    pub fingerprint: String,
    pub project_id: ProjectId,
    pub source_of_intent_revision: u64,
    pub requirement_ids: Vec<RequirementId>,
    pub requirement: String,
    pub fault: String,
    pub state: DivergenceState,
    pub affected_components: Vec<String>,
    pub evidence: Vec<String>,
    pub verification_criteria: Vec<String>,
    pub suggested_next_objective: Option<String>,
    pub first_observed_reality: String,
    pub last_observed_reality: String,
    pub attempt_count: u64,
    pub reopen_count: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DivergenceTransitionSummary {
    pub opened: u64,
    pub still_open: u64,
    pub verified: u64,
    pub reopened: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationTelemetry {
    pub project_id: ProjectId,
    pub operation_id: OperationId,
    pub source_of_intent_revision: u64,
    pub iteration: u64,
    pub implementation_ms: u64,
    pub validation_ms: u64,
    pub review_ms: u64,
    pub checkpoint_ms: u64,
    pub total_ms: u64,
    pub implementation_prompt_bytes: u64,
    pub review_prompt_bytes: u64,
    pub changed: bool,
    pub divergence_transitions: DivergenceTransitionSummary,
    pub recorded_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    pub finding_id: FindingId,
    pub requirement: String,
    pub fault: String,
    pub evidence: Vec<String>,
    pub suggested_next_objective: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRecord {
    pub review_id: ReviewId,
    pub spoke_id: SpokeId,
    pub project_id: ProjectId,
    pub run_id: RunId,
    pub source_of_intent_revision: u64,
    pub observed_content_hash: String,
    pub scope: ReviewScope,
    pub reviewed_scope: Vec<String>,
    pub checks: Vec<String>,
    #[serde(default, alias = "supporting_evidence")]
    pub evidence: Vec<String>,
    /// Scope requirements issued by the Spoke before the independent Run began.
    #[serde(default)]
    pub obligations: Vec<ReviewObligation>,
    /// One independently captured tool observation satisfying each issued obligation.
    #[serde(default)]
    pub obligation_observations: Vec<ReviewObligationObservation>,
    pub findings: Vec<ReviewFinding>,
    pub unreviewed_required_scope: Vec<String>,
    pub verdict: ReviewVerdict,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAccount {
    pub provider_account_id: String,
    pub provider_id: String,
    pub label: String,
    pub created_at: u64,
    pub revoked_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub project: ProjectRecord,
    pub source: SourceOfIntentMetadata,
    pub chat: IntentChatRecord,
    pub timeline: Vec<TimelineItem>,
    pub operations: Vec<OperationRecord>,
    #[serde(default)]
    pub reviews: Vec<ReviewRecord>,
    #[serde(default)]
    pub divergences: Vec<DivergenceRecord>,
    #[serde(default)]
    pub requirement_index: Option<RequirementIndex>,
    #[serde(default)]
    pub iteration_telemetry: Vec<IterationTelemetry>,
    pub gap_before: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientCommand {
    HubStatus,
    SpokeList,
    ProjectList {
        spoke_id: Option<SpokeId>,
    },
    ProjectGet {
        spoke_id: SpokeId,
        project_id: ProjectId,
    },
    ProjectInitialize {
        spoke_id: SpokeId,
        cwd: String,
        name: Option<String>,
    },
    ProjectPathList {
        spoke_id: SpokeId,
        path: String,
    },
    ProjectRemove {
        spoke_id: SpokeId,
        project_id: ProjectId,
    },
    ProjectModelSet {
        spoke_id: SpokeId,
        project_id: ProjectId,
        provider: String,
        model: String,
    },
    ProviderList,
    ProviderSet {
        provider_id: String,
        label: String,
        api_key: String,
    },
    ProviderRevoke {
        provider_account_id: String,
    },
    IntentSubscribe {
        spoke_id: SpokeId,
        project_id: ProjectId,
        cursor: Option<u64>,
    },
    IntentSend {
        spoke_id: SpokeId,
        project_id: ProjectId,
        message: String,
        expected_revision: Option<u64>,
    },
    IntentCancel {
        spoke_id: SpokeId,
        project_id: ProjectId,
        operation_id: OperationId,
    },
    IntentAnswer {
        spoke_id: SpokeId,
        project_id: ProjectId,
        operation_id: OperationId,
        request_id: String,
        response: Value,
    },
    IntentGetProjection {
        spoke_id: SpokeId,
        project_id: ProjectId,
    },
}
impl ClientCommand {
    pub fn spoke_id(&self) -> Option<&SpokeId> {
        match self {
            Self::ProjectList { spoke_id: Some(v) }
            | Self::ProjectGet { spoke_id: v, .. }
            | Self::ProjectInitialize { spoke_id: v, .. }
            | Self::ProjectPathList { spoke_id: v, .. }
            | Self::ProjectRemove { spoke_id: v, .. }
            | Self::ProjectModelSet { spoke_id: v, .. }
            | Self::IntentSubscribe { spoke_id: v, .. }
            | Self::IntentSend { spoke_id: v, .. }
            | Self::IntentCancel { spoke_id: v, .. }
            | Self::IntentAnswer { spoke_id: v, .. }
            | Self::IntentGetProjection { spoke_id: v, .. } => Some(v),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRequest {
    #[serde(default = "protocol_version")]
    pub protocol_version: u32,
    pub id: RequestId,
    #[serde(flatten)]
    pub command: ClientCommand,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientHello {
    Auth {
        #[serde(default = "protocol_version")]
        protocol_version: u32,
        token: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientPayload {
    Authenticated,
    HubStatus {
        version: String,
    },
    SpokeList {
        spokes: Vec<SpokeRecord>,
    },
    ProjectList {
        projects: Vec<ProjectRecord>,
    },
    ProviderList {
        accounts: Vec<ProviderAccount>,
    },
    ProjectSnapshot {
        snapshot: Box<ProjectSnapshot>,
    },
    ProjectPathList {
        spoke_id: SpokeId,
        parent: String,
        directories: Vec<String>,
    },
    Projection {
        spoke_id: SpokeId,
        project_id: ProjectId,
        revision: u64,
        content: String,
    },
    Accepted {
        operation: OperationRecord,
    },
    Timeline {
        item: TimelineItem,
    },
    Operation {
        operation: OperationRecord,
    },
    Interaction {
        spoke_id: SpokeId,
        project_id: ProjectId,
        operation_id: OperationId,
        request_id: String,
        method: String,
        payload: Value,
    },
    ProjectUpdated {
        project: ProjectRecord,
    },
    SpokeUpdated {
        spoke: SpokeRecord,
    },
    ReplayGap {
        spoke_id: SpokeId,
        project_id: ProjectId,
        requested: u64,
        available: u64,
    },
    Error {
        code: String,
        message: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEvent {
    #[serde(default = "protocol_version")]
    pub protocol_version: u32,
    pub id: Option<RequestId>,
    /// Creation time at the event's authoritative origin, as Unix seconds.
    pub created_at: u64,
    #[serde(flatten)]
    pub payload: ClientPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HubToSpoke {
    Command {
        request: ClientRequest,
        #[serde(default)]
        provider_env: BTreeMap<String, String>,
    },
    Shutdown,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpokeToHub {
    Hello {
        #[serde(default = "protocol_version")]
        protocol_version: u32,
        spoke_id: SpokeId,
        version: String,
    },
    Auth {
        #[serde(default = "protocol_version")]
        protocol_version: u32,
        spoke_id: SpokeId,
        signature: String,
    },
    Inventory {
        #[serde(default = "protocol_version")]
        protocol_version: u32,
        complete: bool,
        revision: u64,
        projects: Vec<ProjectRecord>,
    },
    ClientEvent {
        #[serde(default = "protocol_version")]
        protocol_version: u32,
        event: Box<ClientEvent>,
    },
    Heartbeat {
        #[serde(default = "protocol_version")]
        protocol_version: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollRequest {
    pub setup_key: String,
    pub hostname: String,
    pub version: String,
    pub public_key: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollResponse {
    pub ok: bool,
    pub spoke_id: Option<SpokeId>,
    pub hub_url: Option<String>,
    pub error: Option<String>,
}

/// Typed proposals emitted by agents. The Spoke validates every transition and remains authoritative.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentAct {
    Clarify,
    Correct,
    Decide,
    ReferenceContext,
    RequestProjection,
    Prioritize,
    Pause,
    Resume,
    Cancel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceUpdateProposal {
    pub base_revision: u64,
    pub canonical_payload: String,
    /// Replace the canonical bundle with the currently closed on-disk manifest. Valid only for an
    /// explicit owner-directed context adoption and complete coverage of the replacement bundle.
    #[serde(default)]
    pub refresh_authoritative_bundle: bool,
    /// Active intent is standing authorization for iterative realization.
    pub activate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentTurnProposal {
    pub acts: Vec<IntentAct>,
    pub source_coverage: Vec<DocumentCoverage>,
    pub projection: String,
    pub question: Option<String>,
    pub source_update: Option<SourceUpdateProposal>,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationRunResult {
    pub source_coverage: Vec<DocumentCoverage>,
    pub objective: String,
    pub summary: String,
    pub observations: Vec<String>,
    pub changes: Vec<String>,
    pub validation: Vec<String>,
    pub evidence: Vec<String>,
    pub residual_divergence: Vec<String>,
    pub question: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewObligationKind {
    AuthoritativeDocument,
    ProjectFile,
    ValidationCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewObligation {
    pub obligation_id: String,
    pub kind: ReviewObligationKind,
    /// Exact read path or bash command the Spoke requires the reviewer to execute.
    pub subject: String,
    /// Content hash fixed before review for file obligations.
    pub expected_content_hash: Option<String>,
    pub validation_area: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewObligationObservation {
    pub obligation_id: String,
    /// Durable ID of the successful tool event captured independently by the Spoke.
    pub evidence_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRunResult {
    pub source_coverage: Vec<DocumentCoverage>,
    pub target_revision: u64,
    pub observed_reality_version: String,
    pub scope: ReviewScope,
    pub reviewed_scope: Vec<String>,
    pub checks: Vec<String>,
    pub evidence: Vec<String>,
    pub obligation_observations: Vec<ReviewObligationObservation>,
    pub findings: Vec<ReviewFindingProposal>,
    pub unreviewed_required_scope: Vec<String>,
    pub verdict: ReviewVerdict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewFindingProposal {
    #[serde(default)]
    pub requirement_ids: Vec<RequirementId>,
    pub requirement: String,
    pub fault: String,
    pub evidence: Vec<String>,
    pub suggested_next_objective: Option<String>,
}

impl ReviewRunResult {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.observed_reality_version.trim().is_empty() {
            return Err("review requires an observed reality version");
        }
        if self
            .reviewed_scope
            .iter()
            .any(|item| item.trim().is_empty())
            || self.checks.iter().any(|item| item.trim().is_empty())
            || self.evidence.iter().any(|item| item.trim().is_empty())
        {
            return Err("review scope, checks, and evidence cannot contain empty entries");
        }
        match self.verdict {
            ReviewVerdict::Approved
                if !self.findings.is_empty() || !self.unreviewed_required_scope.is_empty() =>
            {
                Err("approval requires zero findings and no unreviewed required scope")
            }
            ReviewVerdict::Approved if self.scope != ReviewScope::WholeProject => {
                Err("approval requires whole_project scope")
            }
            ReviewVerdict::Approved if self.reviewed_scope.is_empty() => {
                Err("approval requires explicit reviewed scope")
            }
            ReviewVerdict::Approved
                if self.checks.is_empty()
                    || self.evidence.is_empty()
                    || self.obligation_observations.is_empty() =>
            {
                Err("approval requires checks and Spoke-bound supporting observations")
            }
            ReviewVerdict::Approved => {
                let scope = self.reviewed_scope.iter().collect::<BTreeSet<_>>();
                let obligation_ids = self
                    .obligation_observations
                    .iter()
                    .map(|item| &item.obligation_id)
                    .collect::<BTreeSet<_>>();
                let evidence = self.evidence.iter().collect::<BTreeSet<_>>();
                let observation_evidence = self
                    .obligation_observations
                    .iter()
                    .map(|item| &item.evidence_id)
                    .collect::<BTreeSet<_>>();
                if scope != obligation_ids
                    || evidence != observation_evidence
                    || scope.len() != self.obligation_observations.len()
                    || evidence.len() != self.obligation_observations.len()
                    || self.checks.len() != self.obligation_observations.len()
                {
                    Err("approval scope, checks, and evidence must map one-to-one to obligations")
                } else {
                    Ok(())
                }
            }
            ReviewVerdict::Findings if self.findings.is_empty() => {
                Err("findings verdict requires at least one finding")
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiagnosticEnvelope {
    pub fields: BTreeMap<String, Value>,
}
