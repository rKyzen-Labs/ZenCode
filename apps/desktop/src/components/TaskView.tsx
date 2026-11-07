import { useState, useEffect, useRef, type FC } from 'react';
import type {
  ContextPreview,
  ZencodePlan,
  WorkerEventPayload,
  TaskState,
  ChangeInventory as ChangeInventoryType,
  MergeResult,
} from '../types';
import { isTaskState } from '../types';

interface TaskViewProps {
  contextPreview: ContextPreview | null;
  plan: ZencodePlan | null;
  taskState: TaskState;
  workerEvents: WorkerEventPayload[];
  workerSessionId?: string | null;
  activePlannerModel?: string;
  switchReason?: string | null;
  inventory: ChangeInventoryType | null;
  activeTaskId: string | null;
  onConfirmPlanRequest: (previewId: string) => Promise<void>;
  onStartWorkerSession: (plan: ZencodePlan) => Promise<void>;
  onStopWorkerSession?: (sessionId: string) => Promise<void>;
  onAcceptTask?: (taskId: string) => Promise<MergeResult>;
  onRejectTask?: (taskId: string) => Promise<void>;
  onResetTask?: () => void;
  onOpenReviewPane?: () => void;
  onTriggerCommand?: (program: string, args: string[], cwd?: string, description?: string) => void;
  onExportDiagnostics?: () => void;
  diagnosticNotice?: string | null;
  isLoading: boolean;
  error: string | null;
}

export const TaskView: FC<TaskViewProps> = ({
  contextPreview,
  plan,
  taskState,
  workerEvents,
  workerSessionId: _workerSessionId,
  activePlannerModel: _activePlannerModel,
  switchReason: _switchReason,
  inventory,
  activeTaskId,
  onConfirmPlanRequest,
  onStartWorkerSession,
  onStopWorkerSession: _onStopWorkerSession,
  onAcceptTask: _onAcceptTask,
  onRejectTask: _onRejectTask,
  onResetTask: _onResetTask,
  onOpenReviewPane,
  onTriggerCommand,
  onExportDiagnostics,
  diagnosticNotice,
  isLoading,
  error,
}) => {
  const [showFullPrompt, setShowFullPrompt] = useState(false);
  const [showRawEvents, setShowRawEvents] = useState(false);
  const eventsEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (workerEvents.length > 0) {
      eventsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [workerEvents]);

  const isPlanning = isTaskState(taskState, 'Planning') || isLoading;
  const isWorkerRunning = isTaskState(taskState, 'WorkerRunning');
  const isAwaitingReview = isTaskState(taskState, 'AwaitingReview');

  return (
    <div className="flex-1 flex flex-col h-full overflow-y-auto p-4 max-w-4xl mx-auto w-full space-y-4 font-mono text-xs">
      {error && (
        <div className="p-3 bg-[#18181b] border border-rose-900/60 text-rose-300 text-xs">
          [error]: {error}
        </div>
      )}

      {/* PHASE 1: CONTEXT PREVIEW */}
      {contextPreview && !plan && (
        <div className="bg-[#121214] border border-[#27272a] rounded p-4 space-y-3">
          <div className="flex items-center justify-between border-b border-[#27272a] pb-2 text-zinc-400">
            <span>[context_preview: 0 network calls]</span>
            <span className="text-zinc-600">[status: awaiting_confirmation]</span>
          </div>

          <div className="space-y-1">
            <span className="text-zinc-500">[target_task]:</span>
            <div className="p-2 bg-[#09090b] border border-[#1f1f23] text-zinc-200">
              {contextPreview.task_description}
            </div>
          </div>

          <div className="grid grid-cols-2 gap-2 text-[11px] text-zinc-400">
            <div className="p-2 bg-[#09090b] border border-[#1f1f23]">
              [manifests]: {contextPreview.manifest_summaries.length} manifest(s)
            </div>
            <div className="p-2 bg-[#09090b] border border-[#1f1f23] truncate">
              [git]: {contextPreview.git_branch_and_status}
            </div>
          </div>

          <button
            type="button"
            onClick={() => setShowFullPrompt(!showFullPrompt)}
            className="text-[11px] text-zinc-500 hover:text-zinc-300 transition-colors"
          >
            {showFullPrompt ? '[-] hide prompt payload' : '[+] inspect prompt payload'}
          </button>

          {showFullPrompt && (
            <pre className="p-2 bg-[#09090b] border border-[#1f1f23] text-[10px] text-zinc-400 max-h-48 overflow-y-auto whitespace-pre-wrap">
              {contextPreview.preview_text}
            </pre>
          )}

          <div className="pt-2 border-t border-[#1f1f23] flex items-center justify-between">
            <span className="text-[10px] text-zinc-600">
              * gemini plan call (store=false, single request)
            </span>

            <button
              type="button"
              onClick={() => onConfirmPlanRequest(contextPreview.preview_id)}
              disabled={isPlanning}
              className="px-3 py-1.5 bg-zinc-200 text-black hover:bg-white font-medium rounded text-xs transition-colors disabled:opacity-40"
            >
              {isPlanning ? '[requesting_plan...]' : '[confirm_and_request_plan]'}
            </button>
          </div>
        </div>
      )}

      {/* PHASE 2: IMPLEMENTATION PLAN CARD (Matches Example Image 2 pure monospace box) */}
      {plan && (
        <div className="space-y-3">
          <div className="bg-[#121214] border border-[#27272a] rounded p-4 space-y-3">
            <div className="text-zinc-400">
              Execute the following implementation plan using local tools only:
            </div>

            <div className="space-y-1">
              <span className="text-zinc-500 font-bold">Task: </span>
              <span className="text-zinc-200">{plan.task_summary || plan.goal}</span>
            </div>

            <div className="space-y-2 pt-1">
              <span className="text-zinc-500 font-bold">Steps:</span>
              {plan.steps.map((step, idx) => (
                <div key={idx} className="pl-3 border-l border-zinc-700 space-y-1">
                  <div className="text-zinc-200">
                    - [Goal]: {step.goal || step.description}
                  </div>
                  {step.instructions && step.instructions.length > 0 && (
                    <div className="text-zinc-500 text-[11px]">
                      Instructions: {JSON.stringify(step.instructions)}
                    </div>
                  )}
                  {step.likely_files && step.likely_files.length > 0 && (
                    <div className="text-zinc-500 text-[11px]">
                      Likely files: {JSON.stringify(step.likely_files)}
                    </div>
                  )}
                </div>
              ))}
            </div>

            {/* If worker hasn't started yet */}
            {!isWorkerRunning && workerEvents.length === 0 && !inventory && (
              <div className="pt-3 border-t border-[#27272a] flex items-center justify-between">
                <span className="text-[10px] text-zinc-500">
                  * executes inside isolated git worktree branch
                </span>

                <button
                  type="button"
                  onClick={() => onStartWorkerSession(plan)}
                  disabled={isLoading}
                  className="px-3 py-1.5 bg-zinc-200 text-black hover:bg-white font-medium rounded text-xs transition-colors disabled:opacity-40"
                >
                  [execute_local_worker (qwen3.5:9b)]
                </button>
              </div>
            )}
          </div>

          {/* Tool status line (Matches Image 2: "Used 2 Read, Write") */}
          {(isWorkerRunning || workerEvents.length > 0) && (
            <div className="text-zinc-500 text-xs px-1">
              Used 2 Read, Write
            </div>
          )}

          {/* Assistant completion note */}
          {(isAwaitingReview || inventory) && (
            <div className="text-zinc-400 text-xs px-1 space-y-1">
              <div>
                The changes have been applied to isolated worktree branch{' '}
                <span className="text-zinc-200 underline">
                  zencode/task-{activeTaskId || 'active'}
                </span>
                .
              </div>
              <div className="text-zinc-500 text-[11px]">
                Primary working tree remains clean and untouched.
              </div>
            </div>
          )}

          {/* Change Review Pill (Matches Image 1 style: "16 files changed +3228 -79 > [Review]") */}
          {inventory && (
            <div className="bg-[#121214] border border-[#27272a] hover:border-zinc-600 rounded p-3 flex items-center justify-between transition-colors">
              <div className="flex items-center gap-2 text-xs">
                <span className="text-zinc-200 font-semibold">
                  {inventory.files.length} file{inventory.files.length !== 1 ? 's' : ''} changed
                </span>
                <span className="text-zinc-400">+{inventory.total_additions}</span>
                <span className="text-zinc-500">-{inventory.total_deletions}</span>
                <span className="text-zinc-600">&gt;</span>
              </div>

              <button
                type="button"
                onClick={onOpenReviewPane}
                className="px-3 py-1 bg-zinc-200 text-black hover:bg-white font-medium rounded text-xs transition-colors cursor-pointer"
              >
                [review]
              </button>
            </div>
          )}

          {/* Verification & Build Commands Bar (Milestone 3) */}
          {(inventory || isAwaitingReview || (workerEvents.length > 0 && !isWorkerRunning)) && (
            <div className="bg-[#121214] border border-[#27272a] rounded p-3 space-y-2">
              <div className="flex items-center justify-between text-zinc-400 text-[11px] border-b border-zinc-800 pb-1.5">
                <span className="font-semibold text-zinc-300">[verification_and_checks]</span>
                <span className="text-zinc-500">process isolation: windows job object</span>
              </div>
              <div className="flex flex-wrap items-center gap-2 pt-1">
                <button
                  type="button"
                  onClick={() =>
                    onTriggerCommand?.(
                      'cargo',
                      ['check', '-p', 'zencode-core'],
                      inventory?.worktree_path,
                      'Fast compile-check on crate'
                    )
                  }
                  className="px-2.5 py-1 border border-zinc-700 bg-zinc-900 hover:bg-zinc-800 text-zinc-300 hover:text-white rounded text-[11px] transition-colors"
                >
                  [cargo check]
                </button>
                <button
                  type="button"
                  onClick={() =>
                    onTriggerCommand?.(
                      'cargo',
                      ['test', '--workspace'],
                      inventory?.worktree_path,
                      'Run workspace test suite'
                    )
                  }
                  className="px-2.5 py-1 border border-zinc-700 bg-zinc-900 hover:bg-zinc-800 text-zinc-300 hover:text-white rounded text-[11px] transition-colors"
                >
                  [cargo test --workspace]
                </button>
                <button
                  type="button"
                  onClick={onExportDiagnostics}
                  className="px-2.5 py-1 border border-zinc-700 bg-zinc-900 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 rounded text-[11px] transition-colors ml-auto"
                >
                  [export_diagnostic_bundle]
                </button>
              </div>
              {diagnosticNotice && (
                <div className="p-2 border border-emerald-900/60 bg-emerald-950/20 text-emerald-300 text-[11px] break-all">
                  {diagnosticNotice}
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {/* WORKER STREAMING LOG FEED (When worker running) */}
      {(isWorkerRunning || (workerEvents.length > 0 && !inventory)) && (
        <div className="bg-[#121214] border border-[#27272a] rounded p-3 space-y-2">
          <div className="flex items-center justify-between border-b border-[#1f1f23] pb-2 text-zinc-400 text-[11px]">
            <span>[worker_stream: ollama qwen3.5:9b]</span>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => setShowRawEvents(!showRawEvents)}
                className="text-zinc-500 hover:text-zinc-300"
              >
                {showRawEvents ? '[clean]' : '[raw_sse]'}
              </button>
              <span className="text-zinc-300">
                {isWorkerRunning ? '[running]' : '[finished]'}
              </span>
            </div>
          </div>

          <div className="p-2 bg-[#09090b] border border-[#1f1f23] font-mono text-[11px] space-y-1.5 max-h-64 overflow-y-auto">
            {workerEvents.length === 0 ? (
              <div className="text-zinc-600">waiting for local worker response...</div>
            ) : (
              workerEvents.map((event, idx) => {
                let parsed: any = null;
                if (typeof event?.content === 'string') {
                  try {
                    parsed = JSON.parse(event.content);
                  } catch {
                    // plain text
                  }
                }
                const cleanText = parsed?.properties?.part?.text;
                return (
                  <div key={idx} className="space-y-0.5">
                    <span className="text-zinc-600 text-[10px]">
                      [{event.event_type || 'msg'}]
                    </span>
                    <pre className="text-zinc-300 whitespace-pre-wrap leading-relaxed">
                      {!showRawEvents && cleanText
                        ? cleanText
                        : typeof event?.content === 'string'
                        ? event.content
                        : JSON.stringify(event?.content)}
                    </pre>
                  </div>
                );
              })
            )}
            <div ref={eventsEndRef} />
          </div>
        </div>
      )}
    </div>
  );
};
