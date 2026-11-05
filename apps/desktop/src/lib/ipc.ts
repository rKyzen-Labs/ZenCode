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

export async function startWorkerSession(plan: ZencodePlan): Promise<string> {
  return await invoke<string>('start_worker_session', { plan });
}

export async function stopWorkerSession(sessionId: string): Promise<void> {
  return await invoke<void>('stop_worker_session', { sessionId });
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
