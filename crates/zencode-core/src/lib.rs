pub mod design_system;
pub mod planner;
pub mod routing;
pub mod security;
pub mod shredder;
pub mod skills;

pub use design_system::{
    anti_pattern_shield, detect_design_system_intent, format_design_system_guideline,
    reflection_checklist, DesignSystemArchetype, DesignSystemPreset, DARK_CYBER_PRESET,
    DEVELOPER_TOOL_PRESET, MINIMALIST_EDITORIAL_PRESET, MODERN_SAAS_PRESET,
};

pub use skills::{
    detect_stack_from_dir, detect_stack_from_files, discover_workspace_skills,
    format_dynamic_stack_context, get_stack_guideline, ProjectStack, WorkspaceSkill,
};

pub use planner::{
    GeminiPlannerClient, ImplementationPlan, MultimodalAttachment, PlanStep, PlannerError,
    PlanningContext, UsageMetadata, detect_mime_type, is_multimodal_file,
    verify_and_reconcile_summary,
};
pub use routing::{Operation, RouteTarget, RoutingError};
pub use security::{CredentialService, SecurityError, redact_secrets};
pub use shredder::shred_file_context;
