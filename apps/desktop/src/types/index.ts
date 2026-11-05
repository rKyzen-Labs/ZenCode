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
  has_git: boolean;
  current_branch: string | null;
  git_status_summary: string;
}

export type TaskState =
  | 'Idle'
  | 'Planning'
  | 'WorkerRunning'
  | 'AwaitingReview'
  | 'Complete'
  | 'Failed';

export type EngineHealth =
  | 'Stopped'
  | 'Starting'
  | 'Ready'
  | 'Degraded'
  | 'Failed';

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

export interface ZencodePlanStep {
  step_number: number;
  description: string;
  target_files: string[];
  risk_level: 'Low' | 'Medium' | 'High' | string;
}

export interface ZencodePlan {
  goal: string;
  architecture_notes: string;
  steps: ZencodePlanStep[];
  acceptance_criteria: string[];
}

export interface PlanSubmissionResult {
  plan: ZencodePlan;
  request_count: number;
}

export interface WorkerEventPayload {
  session_id: string;
  event_type: string;
  content: string;
  timestamp_ms: number;
}
