import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  ProjectState,
  SystemStatus,
  ContextPreview,
  PlanSubmissionResult,
  ZencodePlan,
  WorkerEventPayload,
  TaskState,
  EngineHealth,
  QuotaUpdateEvent,
  ChangeInventory,
  MergeResult,
  PrimaryTreeStatus,
  OrphanWorktree,
} from '../types';

export async function openProject(path: string): Promise<ProjectState> {
  return await invoke<ProjectState>('open_project', { path });
}

export async function getProjectState(): Promise<[ProjectState | null, SystemStatus]> {
  return await invoke<[ProjectState | null, SystemStatus]>('get_project_state');
}

export async function previewPlanningContext(taskDescription: string): Promise<ContextPreview> {
  return await invoke<ContextPreview>('preview_planning_context', {
    taskDescription,
  });
}

export async function submitPlanRequest(
  contextPreviewId: string,
  confirmed: boolean
): Promise<PlanSubmissionResult> {
  return await invoke<PlanSubmissionResult>('submit_plan_request', {
    contextPreviewId,
    confirmed,
  });
}

export async function startWorkerSession(
  plan: ZencodePlan,
  taskId?: string,
  acknowledgeDirty?: boolean
): Promise<string> {
  return await invoke<string>('start_worker_session', {
    plan,
    taskId: taskId ?? null,
    acknowledgeDirty: acknowledgeDirty ?? false,
  });
}

export async function stopWorkerSession(sessionId: string): Promise<void> {
  return await invoke<void>('stop_worker_session', { sessionId });
}

export async function getChangeInventory(taskId: string): Promise<ChangeInventory> {
  return await invoke<ChangeInventory>('get_change_inventory', { taskId });
}

export async function acceptTask(taskId: string): Promise<MergeResult> {
  return await invoke<MergeResult>('accept_task', { taskId });
}

export async function rejectTask(taskId: string): Promise<void> {
  return await invoke<void>('reject_task', { taskId });
}

export async function getPrimaryTreeStatus(): Promise<PrimaryTreeStatus> {
  return await invoke<PrimaryTreeStatus>('get_primary_tree_status');
}

export async function stashPrimaryChanges(message?: string): Promise<string> {
  return await invoke<string>('stash_primary_changes', { message: message ?? null });
}

export async function commitPrimaryChanges(message: string): Promise<string> {
  return await invoke<string>('commit_primary_changes', { message });
}

export async function listActiveWorktrees(): Promise<OrphanWorktree[]> {
  return await invoke<OrphanWorktree[]>('list_active_worktrees_command');
}

export async function recoverOrphanedWorktree(
  taskId: string,
  action: 'review' | 'reject' | 'ignore'
): Promise<ChangeInventory | null> {
  return await invoke<ChangeInventory | null>('recover_orphaned_worktree', {
    taskId,
    action,
  });
}

export async function subscribeToInventoryReady(
  callback: (inventory: ChangeInventory) => void
): Promise<UnlistenFn> {
  return await listen<ChangeInventory>('review:inventory_ready', (event) => {
    callback(event.payload);
  });
}

export async function subscribeToWorkerMessages(
  callback: (payload: WorkerEventPayload) => void
): Promise<UnlistenFn> {
  return await listen<WorkerEventPayload>('worker:message', (event) => {
    callback(event.payload);
  });
}

export async function subscribeToWorkerStatus(
  callback: (status: TaskState) => void
): Promise<UnlistenFn> {
  return await listen<TaskState>('worker:status', (event) => {
    callback(event.payload);
  });
}

export async function subscribeToEngineHealth(
  callback: (health: EngineHealth) => void
): Promise<UnlistenFn> {
  return await listen<EngineHealth>('engine:health', (event) => {
    callback(event.payload);
  });
}

export async function getQuotaState(): Promise<QuotaUpdateEvent> {
  return await invoke<QuotaUpdateEvent>('get_quota_state');
}

export async function setPinnedModel(model: string | null): Promise<QuotaUpdateEvent> {
  return await invoke<QuotaUpdateEvent>('set_pinned_model', { model });
}

export async function subscribeToQuotaUpdates(
  callback: (payload: QuotaUpdateEvent) => void
): Promise<UnlistenFn> {
  return await listen<QuotaUpdateEvent>('planner:quota_update', (event) => {
    callback(event.payload);
  });
}
