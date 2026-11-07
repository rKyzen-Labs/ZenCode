export type ProjectType =
  | 'RustCargo'
  | 'NodeTypeScript'
  | 'NodeJavaScript'
  | 'Python'
  | 'Go'
  | 'Generic';

export interface ProjectState {
  path: string;
  name: string;
  project_type: ProjectType;
  has_git?: boolean;
  is_git_repo?: boolean;
  current_branch: string | null;
  git_status_summary: string;
  is_clean?: boolean;
}

export type ChangeCategory =
  | 'Modified'
  | 'Added'
  | 'Untracked'
  | 'Deleted'
  | 'Renamed'
  | 'Copied'
  | 'Binary'
  | 'ModeChange'
  | 'Symlink';

export interface FileChange {
  path: string;
  original_path?: string | null;
  category: ChangeCategory;
  is_binary: boolean;
  is_symlink: boolean;
  mode_change?: string | null;
  additions: number;
  deletions: number;
  diff_content?: string | null;
}

export interface ChangeInventory {
  worktree_path: string;
  branch_name: string;
  base_commit: string;
  files: FileChange[];
  total_additions: number;
  total_deletions: number;
  modified_count: number;
  added_count: number;
  untracked_count: number;
  deleted_count: number;
  renamed_count: number;
  binary_count: number;
}

export interface UncommittedFile {
  path: string;
  status_code: string;
  description: string;
}

export interface PrimaryTreeStatus {
  is_clean: boolean;
  uncommitted_files: UncommittedFile[];
}

export interface OrphanWorktree {
  task_id: string;
  branch_name: string;
  worktree_path: string;
  head_commit: string;
  age_seconds: number;
}

export type MergeResult =
  | { status: 'Success'; commit_sha: string }
  | { status: 'Conflict'; conflicting_files: string[] };

export interface WorkerStartError {
  error_type: string;
  message: string;
  uncommitted_files?: UncommittedFile[] | null;
}

export type TaskState =
  | 'Idle'
  | 'Planning'
  | 'WorkerRunning'
  | 'AwaitingReview'
  | 'Complete'
  | 'Failed'
  | { WorkerRunning: { session_id: string } }
  | { Failed: string }
  | Record<string, any>;

export type EngineHealth =
  | 'NotStarted'
  | 'Stopped'
  | 'Starting'
  | 'Ready'
  | 'Busy'
  | 'Degraded'
  | 'Failed'
  | { Ready: { port: number; version: string } }
  | { Failed: string }
  | Record<string, any>;

export function formatEngineHealth(health?: EngineHealth | null): string {
  if (!health) return 'stopped';
  if (typeof health === 'string') return health.toLowerCase();
  if (typeof health === 'object') {
    const key = Object.keys(health)[0];
    if (key) return key.toLowerCase();
  }
  return String(health).toLowerCase();
}

export function isEngineReady(health?: EngineHealth | null): boolean {
  if (!health) return false;
  if (health === 'Ready') return true;
  if (typeof health === 'object' && 'Ready' in health) return true;
  return false;
}

export function formatTaskState(state?: TaskState | null): string {
  if (!state) return 'idle';
  if (typeof state === 'string') return state.toLowerCase();
  if (typeof state === 'object') {
    const key = Object.keys(state)[0];
    if (key) return key.toLowerCase();
  }
  return String(state).toLowerCase();
}

export function isTaskState(state: TaskState | undefined | null, target: string): boolean {
  if (!state) return target.toLowerCase() === 'idle';
  if (typeof state === 'string') return state.toLowerCase() === target.toLowerCase();
  if (typeof state === 'object') {
    const key = Object.keys(state)[0];
    return key ? key.toLowerCase() === target.toLowerCase() : false;
  }
  return false;
}

export interface SystemStatus {
  gemini_ready: boolean;
  gemini_status_text: string;
  ollama_ready: boolean;
  ollama_status_text: string;
  opencode_health: EngineHealth;
  task_state: TaskState;
  request_count: number;
}

export interface ContextPreview {
  preview_id: string;
  task_description: string;
  project_type: string;
  shallow_tree: string[];
  manifest_summaries: string[];
  git_branch_and_status: string;
  preview_text: string;
}

export interface PlanStep {
  id?: string;
  goal?: string;
  likely_files?: string[];
  instructions?: string[];
  depends_on?: string[];
  acceptance_checks?: string[];
  // Compatibility fields
  step_number?: number;
  description?: string;
  target_files?: string[];
  risk_level?: 'Low' | 'Medium' | 'High' | string;
}

export type ZencodePlanStep = PlanStep;

export interface ZencodePlan {
  task_summary?: string;
  assumptions?: string[];
  questions?: string[];
  project_findings?: string[];
  steps: PlanStep[];
  risks?: string[];
  verification_suggestions?: string[];
  // Compatibility fields
  goal?: string;
  architecture_notes?: string;
  acceptance_criteria?: string[];
}

export interface PlanSubmissionResult {
  plan: ZencodePlan;
  request_count?: number;
  active_model?: string;
  switch_reason?: string | null;
}

export interface ModelQuotaStatus {
  model_id: string;
  display_name: string;
  rpm_limit: number;
  rpm_consumed: number;
  rpm_remaining: number;
  rpm_reset_seconds: number;

  tpm_limit: number;
  tpm_consumed: number;
  tpm_remaining: number;
  tpm_reset_seconds: number;

  rpd_limit: number;
  rpd_consumed: number;
  rpd_remaining: number;
  rpd_reset_formatted: string;
  rpd_reset_seconds: number;

  is_available: boolean;
  threshold_80_percent_exceeded: boolean;
  unavailability_reason?: string | null;
}

export interface QuotaUpdateEvent {
  active_model: string;
  pinned_model: string | null;
  last_switch_reason: string | null;
  models: ModelQuotaStatus[];
  timestamp_ms: number;
}

export interface WorkerEventPayload {
  session_id: string;
  event_type: string;
  content: string;
  timestamp_ms: number;
}

export type BuildSystemType =
  | 'Cargo'
  | 'Pnpm'
  | 'Npm'
  | 'Yarn'
  | 'Go'
  | 'Python'
  | 'Unknown';

export interface RecommendedCommand {
  name: string;
  program: string;
  args: string[];
  description: string;
}

export interface BuildCommandsInfo {
  build_system: BuildSystemType;
  build_system_name: string;
  recommended_commands: RecommendedCommand[];
}

export interface CommandRequest {
  id: string;
  program: string;
  args: string[];
  cwd: string;
  description: string;
  timeout_seconds?: number | null;
}

export interface ProcessOutputEvent {
  command_id: string;
  stream: 'stdout' | 'stderr';
  line: string;
  timestamp_ms: number;
}

export interface ExecutionResult {
  command_id: string;
  program: string;
  args: string[];
  cwd: string;
  exit_code: number | null;
  success: boolean;
  duration_ms: number;
  stdout_tail: string[];
  stderr_tail: string[];
  cancelled: boolean;
  error_message?: string | null;
}

export interface DiagnosticBundle {
  task_id: string;
  exported_at: string;
  is_clean: boolean;
  primary_tree_status?: any;
  worktree_branch?: string | null;
  files_changed: number;
  logs: string[];
  execution_history: ExecutionResult[];
  raw_json: string;
}

