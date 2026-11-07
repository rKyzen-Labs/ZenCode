use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum RoutingError {
    #[error("Routing violation: Operation {operation:?} is not permitted to call Gemini. Must execute on {required_target:?}")]
    GeminiCallForbidden {
        operation: Operation,
        required_target: RouteTarget,
    },
    #[error("Re-planning or design analysis requires explicit user preview and confirmed consent")]
    MissingUserConsent { operation: Operation },
}

/// The operations defined in the locked Zencode V1 routing matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Operation {
    /// Create initial implementation plan
    CreateInitialPlan,
    /// Inspect project tree / filesystem
    InspectProjectTree,
    /// Search / retrieve source files
    SearchSourceFiles,
    /// Write or edit code
    WriteOrEditCode,
    /// Run build / test / lint
    RunBuildTestLint,
    /// Read compiler errors
    ReadCompilerErrors,
    /// Fix compiler / test failures
    FixFailures,
    /// Summarise task result
    SummariseTaskResult,
    /// Re-plan after major scope change (user-triggered only)
    ReplanAfterScopeChange,
    /// Screenshot / design analysis (post-V1 feature)
    ScreenshotDesignAnalysis,
    /// Advisory consultation on repeated compiler / test failures (opt-in, user-triggered only)
    AdvisoryErrorConsultation,
}

/// Who handles the operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RouteTarget {
    RustGeminiPlanner,
    RustLocalFs,
    LocalOpenCodeTools,
    LocalOllamaWorker,
    LocalApprovedProcess,
    LocalWorkerOrRust,
    PostV1GeminiWithConsent,
}

impl Operation {
    /// Returns the assigned execution route for this operation.
    pub const fn assigned_target(self) -> RouteTarget {
        match self {
            Self::CreateInitialPlan => RouteTarget::RustGeminiPlanner,
            Self::InspectProjectTree => RouteTarget::RustLocalFs,
            Self::SearchSourceFiles => RouteTarget::LocalOpenCodeTools,
            Self::WriteOrEditCode => RouteTarget::LocalOllamaWorker,
            Self::RunBuildTestLint => RouteTarget::LocalApprovedProcess,
            Self::ReadCompilerErrors => RouteTarget::LocalOllamaWorker,
            Self::FixFailures => RouteTarget::LocalOllamaWorker,
            Self::SummariseTaskResult => RouteTarget::LocalWorkerOrRust,
            Self::ReplanAfterScopeChange => RouteTarget::RustGeminiPlanner,
            Self::ScreenshotDesignAnalysis => RouteTarget::PostV1GeminiWithConsent,
            Self::AdvisoryErrorConsultation => RouteTarget::RustGeminiPlanner,
        }
    }

    /// Pure routing policy check: May this operation ever call Gemini?
    pub const fn may_call_gemini(self) -> bool {
        match self {
            Self::CreateInitialPlan => true,
            Self::ReplanAfterScopeChange => true,
            Self::ScreenshotDesignAnalysis => true,
            Self::AdvisoryErrorConsultation => true,
            Self::InspectProjectTree
            | Self::SearchSourceFiles
            | Self::WriteOrEditCode
            | Self::RunBuildTestLint
            | Self::ReadCompilerErrors
            | Self::FixFailures
            | Self::SummariseTaskResult => false,
        }
    }

    /// Enforces the routing policy at runtime with user consent verification.
    pub fn enforce_gemini_permission(self, consent_confirmed: bool) -> Result<(), RoutingError> {
        if !self.may_call_gemini() {
            return Err(RoutingError::GeminiCallForbidden {
                operation: self,
                required_target: self.assigned_target(),
            });
        }

        match self {
            Self::CreateInitialPlan => {
                // Must have user preview and confirmation before sending context
                if !consent_confirmed {
                    return Err(RoutingError::MissingUserConsent { operation: self });
                }
                Ok(())
            }
            Self::ReplanAfterScopeChange
            | Self::ScreenshotDesignAnalysis
            | Self::AdvisoryErrorConsultation => {
                if !consent_confirmed {
                    return Err(RoutingError::MissingUserConsent { operation: self });
                }
                Ok(())
            }
            _ => Err(RoutingError::GeminiCallForbidden {
                operation: self,
                required_target: self.assigned_target(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_matrix_enforcement() {
        // Forbidden operations must always fail even if consent is mistakenly passed true
        assert!(Operation::WriteOrEditCode.enforce_gemini_permission(true).is_err());
        assert!(Operation::RunBuildTestLint.enforce_gemini_permission(true).is_err());
        assert!(Operation::ReadCompilerErrors.enforce_gemini_permission(true).is_err());
        assert!(Operation::FixFailures.enforce_gemini_permission(true).is_err());
        assert!(Operation::InspectProjectTree.enforce_gemini_permission(true).is_err());
        assert!(Operation::SearchSourceFiles.enforce_gemini_permission(true).is_err());
        assert!(Operation::SummariseTaskResult.enforce_gemini_permission(true).is_err());

        // Allowed operations require explicit confirmation
        assert!(Operation::CreateInitialPlan.enforce_gemini_permission(false).is_err());
        assert!(Operation::CreateInitialPlan.enforce_gemini_permission(true).is_ok());

        assert!(Operation::ReplanAfterScopeChange.enforce_gemini_permission(false).is_err());
        assert!(Operation::ReplanAfterScopeChange.enforce_gemini_permission(true).is_ok());
    }
}
