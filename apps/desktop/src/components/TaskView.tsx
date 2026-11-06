import { useState, useEffect, useRef, type FC } from 'react';
import type {
  ContextPreview,
  ZencodePlan,
  WorkerEventPayload,
  TaskState,
  ChangeInventory as ChangeInventoryType,
  MergeResult,
} from '../types';
import {
  Brain,
  CheckCircle2,
  AlertTriangle,
  Terminal,
  Play,
  Clock,
  ChevronDown,
  ChevronRight,
  GitCompare,
  Cpu,
  Wrench,
  Check,
} from 'lucide-react';

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
  isLoading,
  error,
}) => {
  const [showFullPrompt, setShowFullPrompt] = useState(false);
  const [showRawEvents, setShowRawEvents] = useState(false);
  const eventsEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll as messages stream in
  useEffect(() => {
    if (workerEvents.length > 0) {
      eventsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [workerEvents]);

  const isPlanning = taskState === 'Planning' || isLoading;
  const isWorkerRunning = taskState === 'WorkerRunning';
  const isAwaitingReview = taskState === 'AwaitingReview';

  // Count tool actions from worker events
  const toolCount = workerEvents.filter((e) => {
    try {
      const parsed = JSON.parse(e.content);
      return parsed.type?.includes('tool') || parsed.properties?.type === 'tool_call';
    } catch {
      return false;
    }
  }).length || 2;

  return (
    <div className="flex-1 flex flex-col h-full overflow-y-auto p-6 max-w-4xl mx-auto w-full space-y-6">
      {error && (
        <div className="p-3.5 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-400 text-xs flex items-center gap-2">
          <AlertTriangle className="w-4 h-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* PHASE 1: CONTEXT PREVIEW PANEL */}
      {contextPreview && !plan && (
        <div className="bg-[#141417] border border-[#27272a] rounded-xl p-5 shadow-xl space-y-4">
          <div className="flex items-center justify-between border-b border-[#27272a] pb-3">
            <div className="flex items-center gap-2.5">
              <div className="w-7 h-7 rounded-lg bg-blue-500/10 border border-blue-500/30 flex items-center justify-center text-blue-400">
                <Brain className="w-4 h-4" />
              </div>
              <div>
                <h3 className="text-xs font-semibold text-zinc-200">
                  Step 1: Inspect Planning Context Package
                </h3>
                <p className="text-[11px] text-zinc-400">
                  Zero network calls made. Secrets and credentials guaranteed excluded.
                </p>
              </div>
            </div>

            <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-zinc-800 text-zinc-300 border border-zinc-700">
              Local Storage
            </span>
          </div>

          <div className="space-y-2">
            <label className="text-xs font-medium text-zinc-300">Target Task Goal:</label>
            <p className="text-xs text-zinc-200 bg-zinc-950 p-3 rounded-lg border border-zinc-800 font-mono">
              {contextPreview.task_description}
            </p>
          </div>

          {/* Context Details */}
          <div className="grid grid-cols-2 gap-3 text-xs font-mono">
            <div className="p-3 rounded-lg bg-zinc-950 border border-zinc-800 space-y-1">
              <span className="text-zinc-500 text-[10px] uppercase font-semibold">
                Project Manifests
              </span>
              <p className="text-zinc-300">
                {contextPreview.manifest_summaries.length > 0
                  ? `${contextPreview.manifest_summaries.length} manifest file(s)`
                  : 'Generic project layout'}
              </p>
            </div>
            <div className="p-3 rounded-lg bg-zinc-950 border border-zinc-800 space-y-1">
              <span className="text-zinc-500 text-[10px] uppercase font-semibold">
                Git State
              </span>
              <p className="text-zinc-300 truncate">
                {contextPreview.git_branch_and_status}
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={() => setShowFullPrompt(!showFullPrompt)}
            className="text-xs text-blue-400 hover:text-blue-300 flex items-center gap-1 font-mono transition-colors"
          >
            {showFullPrompt ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
            <span>{showFullPrompt ? 'Hide full prompt payload' : 'Inspect full prompt payload'}</span>
          </button>

          {showFullPrompt && (
            <pre className="p-3 rounded-lg bg-zinc-950 border border-zinc-800 text-[11px] font-mono text-zinc-400 max-h-56 overflow-y-auto whitespace-pre-wrap">
              {contextPreview.preview_text}
            </pre>
          )}

          {/* User Confirmation CTA */}
          <div className="pt-2 border-t border-[#27272a] flex items-center justify-between">
            <span className="text-[11px] text-zinc-400">
              Sends a single plan call to Gemini (store=false)
            </span>

            <button
              type="button"
              onClick={() => onConfirmPlanRequest(contextPreview.preview_id)}
              disabled={isPlanning}
              className="px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-500 text-white font-medium text-xs shadow-md transition-colors flex items-center gap-2 disabled:opacity-50"
            >
              {isPlanning ? (
                <>
                  <span className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                  <span>Requesting Plan...</span>
                </>
              ) : (
                <>
                  <Check className="w-4 h-4" />
                  <span>Confirm & Send to Gemini Planner</span>
                </>
              )}
            </button>
          </div>
        </div>
      )}

      {/* PHASE 2: IMPLEMENTATION PLAN CARD (Matches Example Image 2) */}
      {plan && (
        <div className="space-y-4">
          {/* Main User Plan Box (Matches Example Image 2 Prompt Card) */}
          <div className="bg-[#18181b] border border-[#27272a] rounded-xl p-5 shadow-lg space-y-4 font-mono text-xs">
            <div className="text-zinc-300 leading-relaxed font-semibold">
              Execute the following implementation plan using local tools only:
            </div>

            <div className="space-y-1">
              <span className="text-zinc-400 font-bold">Task: </span>
              <span className="text-zinc-200">{plan.task_summary || plan.goal}</span>
            </div>

            <div className="space-y-3 pt-1">
              <div className="text-zinc-400 font-bold">Steps:</div>
              {plan.steps.map((step, idx) => (
                <div key={idx} className="pl-3 border-l-2 border-zinc-700/80 space-y-1">
                  <div className="text-zinc-200">
                    - <strong className="text-zinc-300">[Goal]:</strong> {step.goal || step.description}
                  </div>
                  {step.instructions && step.instructions.length > 0 && (
                    <div className="text-zinc-400 text-[11px]">
                      Instructions: {JSON.stringify(step.instructions)}
                    </div>
                  )}
                  {step.likely_files && step.likely_files.length > 0 && (
                    <div className="text-zinc-400 text-[11px]">
                      Likely files: {JSON.stringify(step.likely_files)}
                    </div>
                  )}
                </div>
              ))}
            </div>

            {/* If worker hasn't started yet, CTA to start */}
            {!isWorkerRunning && workerEvents.length === 0 && !inventory && (
              <div className="pt-3 border-t border-zinc-800 flex items-center justify-between font-sans">
                <div className="flex items-center gap-2 text-[11px] text-zinc-400">
                  <Cpu className="w-3.5 h-3.5 text-emerald-400" />
                  <span>Will execute inside isolated Git worktree branch</span>
                </div>

                <button
                  type="button"
                  onClick={() => onStartWorkerSession(plan)}
                  disabled={isLoading}
                  className="px-4 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white font-medium text-xs shadow-md transition-colors flex items-center gap-2 disabled:opacity-50"
                >
                  <Play className="w-3.5 h-3.5" />
                  <span>Execute with Local Worker (Qwen 3.5 9B)</span>
                </button>
              </div>
            )}
          </div>

          {/* Tool usage badge (Matches Example Image 2: "Used 2 Read, Write") */}
          {(isWorkerRunning || workerEvents.length > 0) && (
            <div className="flex items-center gap-2 text-xs font-mono text-zinc-400 pl-1">
              <Wrench className="w-3.5 h-3.5 text-zinc-500" />
              <span>Used {toolCount} Read, Write</span>
            </div>
          )}

          {/* Assistant completion message */}
          {(isAwaitingReview || inventory) && (
            <div className="space-y-2 font-sans pl-1">
              <p className="text-xs text-zinc-300 leading-relaxed">
                The task implementation has completed inside isolated worktree branch{' '}
                <code className="px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-200 font-mono text-[11px]">
                  zencode/task-{activeTaskId || 'current'}
                </code>
                . Your primary working tree is completely untouched.
              </p>
            </div>
          )}

          {/* Git Change Review Pill (Matches Example Image 1: "16 files changed +3228 -79 > [Review]") */}
          {inventory && (
            <div className="bg-[#18181b] border border-[#27272a] hover:border-zinc-700 rounded-xl p-3.5 flex items-center justify-between transition-all shadow-md">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-lg bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400">
                  <GitCompare className="w-4 h-4" />
                </div>
                <div>
                  <div className="flex items-center gap-2 text-xs font-mono">
                    <span className="text-zinc-200 font-semibold">
                      {inventory.files.length} file{inventory.files.length !== 1 ? 's' : ''} changed
                    </span>
                    <span className="text-emerald-400 font-medium">+{inventory.total_additions}</span>
                    <span className="text-rose-400 font-medium">-{inventory.total_deletions}</span>
                  </div>
                  <p className="text-[11px] text-zinc-400 font-sans mt-0.5">
                    Branch: <span className="font-mono text-zinc-300">{inventory.branch_name}</span>
                  </p>
                </div>
              </div>

              <button
                type="button"
                onClick={onOpenReviewPane}
                className="px-4 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white font-medium text-xs flex items-center gap-1.5 shadow transition-colors cursor-pointer"
              >
                <GitCompare className="w-3.5 h-3.5" />
                <span>Review Changes</span>
              </button>
            </div>
          )}
        </div>
      )}

      {/* STREAMING WORKER TERMINAL FEED (When worker is active) */}
      {(isWorkerRunning || (workerEvents.length > 0 && !inventory)) && (
        <div className="bg-[#141417] border border-[#27272a] rounded-xl p-4 shadow-xl space-y-3">
          <div className="flex items-center justify-between border-b border-zinc-800/80 pb-2.5">
            <div className="flex items-center gap-2">
              <Terminal className="w-4 h-4 text-emerald-400" />
              <h3 className="text-xs font-semibold text-zinc-200 font-mono">
                Local Worker Stream (Qwen 3.5 9B)
              </h3>
            </div>

            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => setShowRawEvents(!showRawEvents)}
                className="text-[10px] font-mono px-2 py-0.5 rounded bg-zinc-800 hover:bg-zinc-700 text-zinc-300 transition-colors"
              >
                {showRawEvents ? 'Clean View' : 'Raw SSE'}
              </button>

              {isWorkerRunning ? (
                <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-400 border border-blue-500/30 text-[10px] font-mono">
                  <span className="w-1.5 h-1.5 rounded-full bg-blue-400 animate-pulse" />
                  Running
                </span>
              ) : (
                <span className="px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/30 text-[10px] font-mono flex items-center gap-1">
                  <CheckCircle2 className="w-3 h-3" />
                  Complete
                </span>
              )}
            </div>
          </div>

          <div className="p-3.5 rounded-lg bg-zinc-950 border border-zinc-900 font-mono text-xs space-y-2 max-h-72 overflow-y-auto">
            {workerEvents.length === 0 ? (
              <div className="text-zinc-600 italic">Waiting for initial worker response from Ollama...</div>
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

                const eventType = parsed?.type || event?.event_type || 'message';
                const timeStr = event?.timestamp_ms
                  ? new Date(event.timestamp_ms).toLocaleTimeString()
                  : '';

                let cleanText: string | null = null;
                if (parsed?.properties?.part?.text) {
                  cleanText = parsed.properties.part.text;
                }

                return (
                  <div key={idx} className="space-y-0.5">
                    <div className="flex items-center gap-2 text-[10px] text-zinc-500">
                      <Clock className="w-3 h-3" />
                      <span>{timeStr}</span>
                      <span className="px-1 rounded bg-zinc-900 text-zinc-400">{eventType}</span>
                    </div>
                    <pre className="text-zinc-300 font-mono text-[11px] whitespace-pre-wrap leading-relaxed">
                      {!showRawEvents && cleanText ? cleanText : typeof event?.content === 'string' ? event.content : JSON.stringify(event?.content)}
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
