import { useState, useEffect, useRef, type FC } from 'react';
import type {
  ContextPreview,
  ZencodePlan,
  WorkerEventPayload,
  TaskState,
  ChangeInventory as ChangeInventoryType,
  MergeResult,
  ExecutionResult,
} from '../types';
import { isTaskState } from '../types';
import { TaskSummary } from './TaskSummary';
import { ReviewPanel } from '../features/review/ReviewPanel';

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
  verificationHistory?: ExecutionResult[];
  onConfirmPlanRequest: (previewId: string) => Promise<void>;
  onStartWorkerSession: (plan: ZencodePlan) => Promise<void>;
  onStopWorkerSession?: (sessionId: string) => Promise<void>;
  onAcceptTask?: (taskId: string) => Promise<MergeResult>;
  onRejectTask?: (taskId: string) => Promise<void>;
  onResetTask?: () => void;
  onOpenReviewPane?: () => void;
  onTriggerCommand?: (program: string, args: string[], cwd?: string, description?: string) => void;
  onExportDiagnostics?: () => void;
  onTriggerAdvisory?: (errorText: string, filePath?: string) => void;
  advisorySuggestion?: string | null;
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
  verificationHistory = [],
  onConfirmPlanRequest,
  onStartWorkerSession,
  onStopWorkerSession: _onStopWorkerSession,
  onAcceptTask,
  onRejectTask,
  onResetTask: _onResetTask,
  onOpenReviewPane: _onOpenReviewPane,
  onTriggerCommand,
  onExportDiagnostics,
  onTriggerAdvisory,
  advisorySuggestion,
  diagnosticNotice,
  isLoading,
  error,
}) => {
  const [showFullPrompt, setShowFullPrompt] = useState(false);
  const [showPlanDetails, setShowPlanDetails] = useState(true);
  const [showRawEvents, setShowRawEvents] = useState(false);
  const [showAgentNotes, setShowAgentNotes] = useState(true);
  const [activeTab, setActiveTab] = useState<'review' | 'log' | 'verify'>('review');
  const eventsEndRef = useRef<HTMLDivElement>(null);

  // When inventory is received, ensure review is the active tab by default (Step 2)
  useEffect(() => {
    if (inventory && inventory.files.length > 0) {
      setActiveTab('review');
    }
  }, [inventory]);

  useEffect(() => {
    if (workerEvents.length > 0 && activeTab === 'log') {
      eventsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [workerEvents, activeTab]);

  const isPlanning = isTaskState(taskState, 'Planning') || isLoading;
  const isWorkerRunning = isTaskState(taskState, 'WorkerRunning');
  const isAwaitingReview = isTaskState(taskState, 'AwaitingReview');
  const isFailed = isTaskState(taskState, 'Failed');

  // Extract worker's own final natural language summary (Step 3)
  const workerSummary = (() => {
    for (let i = workerEvents.length - 1; i >= 0; i--) {
      const ev = workerEvents[i];
      if (!ev || typeof ev.content !== 'string') continue;
      try {
        const parsed = JSON.parse(ev.content);
        const text =
          parsed?.properties?.part?.text ||
          parsed?.properties?.text ||
          parsed?.text;
        if (typeof text === 'string' && text.trim().length > 0) {
          return text.trim();
        }
      } catch {
        if (
          ev.content.trim().length > 0 &&
          !ev.content.startsWith('{') &&
          ev.event_type !== 'session.status' &&
          ev.event_type !== 'session.idle'
        ) {
          return ev.content.trim();
        }
      }
    }
    return null;
  })();

  // Step 4: Extract distinct reasoning or step commentary
  const agentNotes = (() => {
    const notes: string[] = [];
    for (const ev of workerEvents) {
      if (!ev || typeof ev.content !== 'string') continue;
      try {
        const parsed = JSON.parse(ev.content);
        const reasoning =
          parsed?.properties?.part?.reasoning ||
          parsed?.properties?.reasoning ||
          (parsed?.type === 'reasoning' ? parsed?.text : null);
        if (typeof reasoning === 'string' && reasoning.trim().length > 0) {
          notes.push(reasoning.trim());
        } else if (typeof parsed?.properties?.part?.text === 'string') {
          const text = parsed.properties.part.text;
          const thinkMatch = text.match(/<think>([\s\S]*?)<\/think>/);
          if (thinkMatch && thinkMatch[1].trim()) {
            notes.push(thinkMatch[1].trim());
          }
        }
      } catch {
        const thinkMatch = ev.content.match(/<think>([\s\S]*?)<\/think>/);
        if (thinkMatch && thinkMatch[1].trim()) {
          notes.push(thinkMatch[1].trim());
        }
      }
    }
    return notes;
  })();

  // Step 6: Detect consecutive error attempts capped at 5
  const failedRuns = verificationHistory.filter(
    (r) => !r.success || (r.exit_code !== null && r.exit_code !== 0)
  );
  const consecutiveFailures = failedRuns.length;
  const lastFailedRun = failedRuns.length > 0 ? failedRuns[failedRuns.length - 1] : null;
  const isCappedAt5Failures = consecutiveFailures >= 5;

  const isTaskFinished = isAwaitingReview || Boolean(inventory) || (!isWorkerRunning && workerEvents.length > 0);

  return (
    <div className="flex-1 flex flex-col h-full overflow-y-auto p-4 max-w-6xl mx-auto w-full space-y-4 font-mono text-xs">
      {error && (
        <div className="p-3 bg-[#18181b] border border-rose-900/60 text-rose-300 text-xs">
          [error]: {error}
        </div>
      )}

      {/* PHASE 1: CONTEXT PREVIEW (Pre-confirmation) */}
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

      {/* PHASE 2: TASK SUMMARY COMPONENT (Step 3: Shown at top when session completes) */}
      {plan && isTaskFinished && (
        <TaskSummary
          inventory={inventory}
          verificationRuns={verificationHistory}
          workerSummary={workerSummary}
          isFailed={Boolean(isFailed)}
          onTriggerCommand={onTriggerCommand}
          onViewDiff={() => setActiveTab('review')}
          onViewLogs={() => setActiveTab('log')}
        />
      )}

      {/* PHASE 3: STEP 6 CAPPED LOCAL REPAIR & ESCALATION CARD */}
      {isCappedAt5Failures && lastFailedRun && (
        <div className="bg-amber-950/20 border border-amber-600/70 rounded p-3.5 space-y-2">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <span className="text-amber-400 font-bold">[!]</span>
              <span className="text-amber-200 font-semibold">
                Local repair paused: {consecutiveFailures} consecutive test/compiler failures
              </span>
            </div>
            <span className="text-amber-500 text-[10px] uppercase border border-amber-800 px-1.5 py-0.5 rounded">
              cap reached
            </span>
          </div>

          <p className="text-zinc-300 text-[11px] leading-relaxed">
            The local worker has reached the 5-attempt retry limit on the current error signature.
            You can request advisory guidance from Gemini on this specific error.
          </p>

          <div className="p-2 bg-black/60 border border-zinc-800 text-zinc-400 text-[10px] max-h-20 overflow-y-auto font-mono">
            {lastFailedRun.error_message ||
              lastFailedRun.stderr_tail.slice(-3).join('\n') ||
              'Exit code non-zero'}
          </div>

          <div className="flex items-center justify-between pt-1">
            <span className="text-zinc-500 text-[10px]">
              * Opt-in only. Gemini provides advice; local worker tests changes.
            </span>
            <button
              type="button"
              onClick={() =>
                onTriggerAdvisory?.(
                  lastFailedRun.error_message || lastFailedRun.stderr_tail.join('\n'),
                  inventory?.files[0]?.path
                )
              }
              className="px-3 py-1 rounded bg-amber-400 hover:bg-amber-300 text-black font-semibold text-xs transition-colors"
            >
              [ask gemini for advice on this specific error &gt;]
            </button>
          </div>
        </div>
      )}

      {/* PHASE 4: ADVISORY SUGGESTION CARD (If Gemini returned advice) */}
      {advisorySuggestion && (
        <div className="bg-[#121214] border border-blue-900/60 rounded p-3.5 space-y-2">
          <div className="flex items-center justify-between text-blue-300 border-b border-zinc-800 pb-1.5">
            <div className="flex items-center gap-2">
              <span className="text-blue-400 font-bold">[advisory_guidance]</span>
              <span className="text-zinc-400 text-[11px]">from Gemini (store=false)</span>
            </div>
            <span className="text-[10px] text-zinc-500">for local worker implementation</span>
          </div>
          <pre className="p-2.5 bg-[#09090b] border border-zinc-800 text-zinc-200 text-[11px] whitespace-pre-wrap leading-relaxed rounded font-mono">
            {advisorySuggestion}
          </pre>
        </div>
      )}

      {/* PHASE 5: STEP 4 AGENT NOTES SECTION (Real-time reasoning / commentary) */}
      {(agentNotes.length > 0 || isWorkerRunning) && (
        <div className="bg-[#121214] border border-[#27272a] rounded p-3 space-y-2">
          <div className="flex items-center justify-between border-b border-[#1f1f23] pb-1.5 text-zinc-400 text-[11px]">
            <div className="flex items-center gap-2">
              <span className="text-zinc-200 font-semibold">[agent_notes]</span>
              <span className="text-zinc-500">
                {agentNotes.length > 0
                  ? `(${agentNotes.length} reasoning steps recorded)`
                  : '(tool execution stream active)'}
              </span>
            </div>
            <button
              type="button"
              onClick={() => setShowAgentNotes(!showAgentNotes)}
              className="text-zinc-500 hover:text-zinc-200"
            >
              {showAgentNotes ? '[-] collapse notes' : '[+] view notes'}
            </button>
          </div>

          {showAgentNotes && (
            <div className="space-y-1.5 max-h-48 overflow-y-auto">
              {agentNotes.length === 0 ? (
                <div className="text-zinc-500 text-[10px] italic p-1">
                  [note: OpenCode exposes tool calls and output stream directly. Step-level reasoning commentary will appear here whenever emitted by the local model.]
                </div>
              ) : (
                agentNotes.map((note, i) => (
                  <div
                    key={i}
                    className="p-2 bg-[#09090b] border border-[#1f1f23] text-zinc-300 text-[11px] whitespace-pre-wrap rounded leading-relaxed"
                  >
                    <span className="text-zinc-500 font-bold mr-2">[{i + 1}]</span>
                    {note}
                  </div>
                ))
              )}
            </div>
          )}
        </div>
      )}

      {/* PHASE 6: IMPLEMENTATION PLAN CARD */}
      {plan && (
        <div className="bg-[#121214] border border-[#27272a] rounded p-3 space-y-2">
          <div className="flex items-center justify-between text-zinc-300 border-b border-[#1f1f23] pb-1.5">
            <div className="flex items-center gap-2">
              <span className="text-zinc-500 font-bold">[plan]:</span>
              <span className="text-zinc-200 font-semibold truncate max-w-xl">
                {plan.task_summary || plan.goal}
              </span>
            </div>
            <button
              type="button"
              onClick={() => setShowPlanDetails(!showPlanDetails)}
              className="text-zinc-500 hover:text-zinc-200 text-[10px]"
            >
              {showPlanDetails ? '[-] collapse plan' : '[+] view plan steps'}
            </button>
          </div>

          {showPlanDetails && (
            <div className="space-y-2 pt-1 text-[11px]">
              <div className="text-zinc-500 font-bold">Steps:</div>
              {plan.steps.map((step, idx) => (
                <div key={idx} className="pl-3 border-l border-zinc-700 space-y-0.5">
                  <div className="text-zinc-200 font-medium">
                    {idx + 1}. {step.goal || step.description}
                  </div>
                  {step.likely_files && step.likely_files.length > 0 && (
                    <div className="text-zinc-500 text-[10px]">
                      Likely files: [{step.likely_files.join(', ')}]
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}

          {/* If worker hasn't started yet */}
          {!isWorkerRunning && workerEvents.length === 0 && !inventory && (
            <div className="pt-2 border-t border-[#27272a] flex items-center justify-between">
              <span className="text-[10px] text-zinc-500">
                * executes inside dedicated git worktree branch
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
      )}

      {/* PHASE 7: TABS & PRIMARY CHANGE REVIEW (Step 2: ReviewPanel as primary view) */}
      {(isWorkerRunning || workerEvents.length > 0 || inventory) && (
        <div className="space-y-3 flex-1 flex flex-col min-h-0">
          {/* Main Tab Bar */}
          <div className="flex items-center justify-between border-b border-[#27272a] bg-[#09090b] px-1 pb-1">
            <div className="flex items-center gap-1.5">
              {/* Tab 1: ReviewPanel (Default when files changed) */}
              <button
                type="button"
                onClick={() => setActiveTab('review')}
                className={`px-3 py-1.5 rounded-t text-xs font-semibold transition-colors flex items-center gap-2 ${
                  activeTab === 'review'
                    ? 'bg-[#18181b] text-white border-t border-x border-[#27272a]'
                    : 'text-zinc-400 hover:text-zinc-200 hover:bg-[#121214]'
                }`}
              >
                <span>[review_changes]</span>
                {inventory && (
                  <span className="text-[10px] text-emerald-400">
                    ({inventory.files.length} files: +{inventory.total_additions}/-{inventory.total_deletions})
                  </span>
                )}
              </button>

              {/* Tab 2: Worker Log (Secondary, collapsed by default) */}
              <button
                type="button"
                onClick={() => setActiveTab('log')}
                className={`px-3 py-1.5 rounded-t text-xs font-semibold transition-colors flex items-center gap-2 ${
                  activeTab === 'log'
                    ? 'bg-[#18181b] text-white border-t border-x border-[#27272a]'
                    : 'text-zinc-400 hover:text-zinc-200 hover:bg-[#121214]'
                }`}
              >
                <span>[worker_log]</span>
                <span className="text-[10px] text-zinc-500">
                  ({workerEvents.length} events)
                </span>
              </button>

              {/* Tab 3: Verification & Checks */}
              <button
                type="button"
                onClick={() => setActiveTab('verify')}
                className={`px-3 py-1.5 rounded-t text-xs font-semibold transition-colors flex items-center gap-2 ${
                  activeTab === 'verify'
                    ? 'bg-[#18181b] text-white border-t border-x border-[#27272a]'
                    : 'text-zinc-400 hover:text-zinc-200 hover:bg-[#121214]'
                }`}
              >
                <span>[verification_and_checks]</span>
                {verificationHistory.length > 0 && (
                  <span className="text-[10px] text-zinc-400">
                    ({verificationHistory.length} run)
                  </span>
                )}
              </button>
            </div>

            <div className="text-[10px] text-zinc-500 pr-2">
              {isWorkerRunning ? (
                <span className="text-amber-400 font-bold animate-pulse">
                  [worker_running...]
                </span>
              ) : isAwaitingReview ? (
                <span className="text-emerald-400 font-bold">[review_ready]</span>
              ) : (
                <span>[isolated worktree]</span>
              )}
            </div>
          </div>

          {/* TAB 1: REVIEW PANEL AS PRIMARY VIEW (Step 2) */}
          {activeTab === 'review' && (
            <div className="flex-1 min-h-[520px] flex flex-col bg-[#0d0d0f]">
              {inventory && inventory.files.length > 0 ? (
                <ReviewPanel
                  inventory={inventory}
                  taskId={activeTaskId || 'active-task'}
                  onAccept={onAcceptTask || (async () => ({ status: 'Success' as const, commit_sha: 'sha' }))}
                  onReject={onRejectTask || (async () => {})}
                  isLoading={isLoading}
                />
              ) : (
                <div className="p-8 border border-[#27272a] rounded bg-[#121214] text-center space-y-3">
                  <div className="text-zinc-400 font-semibold">
                    {isWorkerRunning
                      ? '[worker currently modifying files in isolated worktree...]'
                      : '[no file modifications recorded yet]'}
                  </div>
                  <p className="text-zinc-500 text-[11px] max-w-md mx-auto">
                    {isWorkerRunning
                      ? 'The change inventory will automatically generate and display here as soon as the worker completes.'
                      : 'Changes will appear here once the local worker runs or modifies source code.'}
                  </p>
                  <button
                    type="button"
                    onClick={() => setActiveTab('log')}
                    className="px-3 py-1 bg-[#18181b] hover:bg-[#27272a] border border-[#27272a] text-zinc-300 rounded text-xs transition-colors"
                  >
                    [inspect worker log &gt;]
                  </button>
                </div>
              )}
            </div>
          )}

          {/* TAB 2: WORKER LOG (Collapsible / secondary) */}
          {activeTab === 'log' && (
            <div className="bg-[#121214] border border-[#27272a] rounded p-3 space-y-2">
              <div className="flex items-center justify-between border-b border-[#1f1f23] pb-2 text-zinc-400 text-[11px]">
                <div className="flex items-center gap-2">
                  <span className="text-zinc-200 font-semibold">[worker_log: ollama qwen3.5:9b]</span>
                  <span className="text-zinc-500">({workerEvents.length} stream messages)</span>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => setShowRawEvents(!showRawEvents)}
                    className="text-zinc-500 hover:text-zinc-300 text-[11px]"
                  >
                    {showRawEvents ? '[clean]' : '[raw_sse]'}
                  </button>
                  <span className="text-zinc-300 text-[11px]">
                    {isWorkerRunning ? '[running]' : '[finished]'}
                  </span>
                </div>
              </div>

              <div className="p-2.5 bg-[#09090b] border border-[#1f1f23] font-mono text-[11px] space-y-1.5 max-h-96 overflow-y-auto">
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

          {/* TAB 3: VERIFICATION & CHECKS */}
          {activeTab === 'verify' && (
            <div className="bg-[#121214] border border-[#27272a] rounded p-4 space-y-3">
              <div className="flex items-center justify-between border-b border-zinc-800 pb-2 text-zinc-300">
                <span className="font-semibold">[verification_and_checks]</span>
                <span className="text-zinc-500 text-[10px]">
                  supervised via windows job objects
                </span>
              </div>

              <div className="flex flex-wrap items-center gap-2">
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
                  className="px-3 py-1.5 border border-zinc-700 bg-zinc-900 hover:bg-zinc-800 text-zinc-200 hover:text-white rounded text-xs transition-colors"
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
                  className="px-3 py-1.5 border border-zinc-700 bg-zinc-900 hover:bg-zinc-800 text-zinc-200 hover:text-white rounded text-xs transition-colors"
                >
                  [cargo test --workspace]
                </button>

                <button
                  type="button"
                  onClick={onExportDiagnostics}
                  className="px-3 py-1.5 border border-zinc-700 bg-zinc-900 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 rounded text-xs transition-colors ml-auto"
                >
                  [export_diagnostic_bundle]
                </button>
              </div>

              {diagnosticNotice && (
                <div className="p-2.5 border border-emerald-900/60 bg-emerald-950/20 text-emerald-300 text-[11px] break-all rounded">
                  {diagnosticNotice}
                </div>
              )}

              {/* History of verification runs */}
              {verificationHistory.length > 0 && (
                <div className="space-y-1.5 pt-2 border-t border-[#1f1f23]">
                  <div className="text-[10px] text-zinc-500 uppercase tracking-wider font-semibold">
                    verification history
                  </div>
                  {verificationHistory.map((run, i) => (
                    <div
                      key={i}
                      className="p-2 bg-[#09090b] border border-[#1f1f23] rounded flex items-center justify-between text-[11px]"
                    >
                      <div className="flex items-center gap-2">
                        <span className="text-zinc-500">$</span>
                        <span className="text-zinc-300 font-semibold">{run.program}</span>
                        <span className="text-zinc-500">[{run.args.join(' ')}]</span>
                        <span className="text-zinc-600 text-[10px]">({run.duration_ms}ms)</span>
                      </div>
                      <div>
                        {run.cancelled ? (
                          <span className="text-amber-500 font-bold">[cancelled]</span>
                        ) : run.success || run.exit_code === 0 ? (
                          <span className="text-emerald-400 font-bold">[exit: 0 ok]</span>
                        ) : (
                          <span className="text-rose-400 font-bold">
                            [exit: {run.exit_code ?? 'err'} fail]
                          </span>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
};
