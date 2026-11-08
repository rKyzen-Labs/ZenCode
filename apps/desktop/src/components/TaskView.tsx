import { useState, useEffect, useRef, type FC } from 'react';
import type {
  ContextPreview,
  ZencodePlan,
  WorkerEventPayload,
  TaskState,
  ChangeInventory as ChangeInventoryType,
  MergeResult,
  ExecutionResult,
  CommandRequest,
  ProjectState,
  QuotaUpdateEvent,
} from '../types';
import { isTaskState } from '../types';
import { TaskSummary } from './TaskSummary';

interface TaskViewProps {
  project?: ProjectState | null;
  onInitGitRepo?: () => Promise<void>;
  onCreateGitignore?: () => Promise<void>;
  gitignoreAddedNotice?: string | null;
  isGitInitDeclined?: boolean;
  onDeclineGitInit?: () => void;
  isGitignoreDeclined?: boolean;
  onDeclineGitignore?: () => void;
  isUnbornHeadDeclined?: boolean;
  onDeclineInitialCommit?: () => void;
  onCreateInitialCommit?: () => Promise<void> | void;
  initialCommitNotice?: string | null;
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
  pendingCommandApproval?: CommandRequest | null;
  onApproveCommand?: (req: CommandRequest) => void;
  onDenyCommand?: (req: CommandRequest) => void;
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
  onApplyAdvisoryToWorker?: (suggestion: string) => Promise<void> | void;
  quotaState?: QuotaUpdateEvent | null;
  diagnosticNotice?: string | null;
  isLoading: boolean;
  error: string | null;
}

export const TaskView: FC<TaskViewProps> = ({
  project,
  onInitGitRepo,
  onCreateGitignore,
  gitignoreAddedNotice,
  isGitInitDeclined = false,
  onDeclineGitInit,
  isGitignoreDeclined = false,
  onDeclineGitignore,
  isUnbornHeadDeclined = false,
  onDeclineInitialCommit,
  onCreateInitialCommit,
  initialCommitNotice,
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
  pendingCommandApproval,
  onApproveCommand,
  onDenyCommand,
  onConfirmPlanRequest,
  onStartWorkerSession,
  onStopWorkerSession,
  onAcceptTask,
  onRejectTask,
  onResetTask: _onResetTask,
  onOpenReviewPane,
  onTriggerCommand,
  onExportDiagnostics: _onExportDiagnostics,
  onTriggerAdvisory,
  advisorySuggestion,
  onApplyAdvisoryToWorker,
  quotaState,
  diagnosticNotice,
  isLoading,
  error,
}) => {
  const [showFullPrompt, setShowFullPrompt] = useState(false);
  const [showAgentNotes, setShowAgentNotes] = useState(true);
  const [showRawEvents, setShowRawEvents] = useState(false);
  const scrollEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll as new messages and events stream in
  useEffect(() => {
    scrollEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [contextPreview, plan, workerEvents.length, inventory, pendingCommandApproval]);

  const isPlanning = isTaskState(taskState, 'Planning') || (isLoading && Boolean(contextPreview) && !plan);
  const isWorkerRunning = isTaskState(taskState, 'WorkerRunning');
  const isAwaitingReview = isTaskState(taskState, 'AwaitingReview');
  const isFailed = isTaskState(taskState, 'Failed');

  // Extract worker's final natural language report
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

  // Extract step commentary, reasoning, and <think> tokens
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

  // Detect 5 consecutive verification failures for advisory loop
  const failedRuns = verificationHistory.filter(
    (r) => !r.success || (r.exit_code !== null && r.exit_code !== 0)
  );
  const isCappedAt5Failures = failedRuns.length >= 5;
  const lastFailedRun = failedRuns.length > 0 ? failedRuns[failedRuns.length - 1] : null;

  return (
    <div className="flex-1 flex flex-col p-4 max-w-4xl mx-auto w-full space-y-6 font-sans text-xs select-text">
      {/* Top Diagnostic Notice (if exported) */}
      {diagnosticNotice && (
        <div className="p-2.5 rounded-lg bg-zinc-900 border border-zinc-800 text-zinc-300 font-mono text-[11px] flex items-center justify-between">
          <span>{diagnosticNotice}</span>
        </div>
      )}

      {/* Global Error Banner */}
      {error && (
        <div className="p-3 rounded-lg bg-[#181818] border border-danger/50 text-danger font-mono text-xs">
          [error]: {error}
        </div>
      )}

      {/* Step 3: Offer-Based Git Init (Non-Git Folder) */}
      {project && !project.is_git_repo && (
        !isGitInitDeclined ? (
          <div className="w-full max-w-2xl rounded-xl border border-zinc-700 bg-surface-raised p-4 space-y-3 font-sans shadow-md">
            <div className="flex items-center justify-between border-b border-border pb-2">
              <span className="font-semibold text-zinc-100 text-xs">Git Repository Required</span>
              <span className="text-[10px] font-mono text-text-muted border border-border px-2 py-0.5 rounded">isolation safety</span>
            </div>
            <p className="text-zinc-300 text-xs leading-relaxed">
              This folder isn't a Git repository yet. Zencode needs Git to safely isolate and review AI changes on a unique worktree branch. Initialize Git here?
            </p>
            <div className="flex items-center justify-end gap-2 pt-1 border-t border-border">
              <button
                type="button"
                onClick={onDeclineGitInit}
                className="px-3 py-1.5 rounded-lg border border-border hover:bg-surface-hover text-text-muted font-medium text-xs transition-colors"
              >
                No, Keep Read-Only
              </button>
              <button
                type="button"
                onClick={onInitGitRepo}
                className="px-4 py-1.5 rounded-lg bg-zinc-100 hover:bg-white text-zinc-950 font-medium text-xs transition-colors shadow-sm"
              >
                Yes, Initialize Git
              </button>
            </div>
          </div>
        ) : (
          <div className="w-full max-w-2xl rounded-xl border border-border bg-surface p-3 space-y-1 font-sans text-xs">
            <div className="flex items-center gap-1.5 text-text-muted font-semibold">
              <span>[i]</span>
              <span>Read-Only Exploration Mode</span>
            </div>
            <p className="text-text-muted text-[11px] leading-relaxed">
              Git initialization was declined. Task creation and worker execution are blocked because Git worktree isolation is required to protect your primary files.
            </p>
          </div>
        )
      )}

      {/* Offer-Based Initial Commit (Empty / Unborn Repo) */}
      {project && project.is_git_repo && project.has_commits === false && (
        initialCommitNotice ? (
          <div className="w-full max-w-2xl rounded-xl border border-border bg-surface p-3 font-sans text-xs flex items-center justify-between text-zinc-300">
            <div className="flex items-center gap-2">
              <span className="text-success font-bold">[ok]</span>
              <span className="text-[11px] whitespace-pre-wrap">{initialCommitNotice}</span>
            </div>
          </div>
        ) : !isUnbornHeadDeclined ? (
          <div className="w-full max-w-2xl rounded-xl border border-zinc-700 bg-surface-raised p-4 space-y-3 font-sans shadow-md">
            <div className="flex items-center justify-between border-b border-border pb-2">
              <span className="font-semibold text-zinc-100 text-xs">Initial Commit Required</span>
              <span className="text-[10px] font-mono text-text-muted border border-border px-2 py-0.5 rounded">isolation safety</span>
            </div>
            <p className="text-zinc-300 text-xs leading-relaxed">
              This repository has no commits yet. Zencode needs at least one commit to create an isolated task branch. Create an initial empty commit?
            </p>
            <div className="flex items-center justify-end gap-2 pt-1 border-t border-border">
              <button
                type="button"
                onClick={onDeclineInitialCommit}
                className="px-3 py-1.5 rounded-lg border border-border hover:bg-surface-hover text-text-muted font-medium text-xs transition-colors"
              >
                No, Cancel
              </button>
              <button
                type="button"
                onClick={onCreateInitialCommit}
                className="px-4 py-1.5 rounded-lg bg-zinc-100 hover:bg-white text-zinc-950 font-medium text-xs transition-colors shadow-sm"
              >
                Yes, Create Initial Commit
              </button>
            </div>
          </div>
        ) : (
          <div className="w-full max-w-2xl rounded-xl border border-border bg-surface p-3 space-y-1 font-sans text-xs">
            <div className="flex items-center gap-1.5 text-text-muted font-semibold">
              <span>[i]</span>
              <span>Task Creation Blocked</span>
            </div>
            <p className="text-text-muted text-[11px] leading-relaxed">
              Creation of an initial commit was declined. Task creation and worker execution are blocked because Git worktree isolation requires at least one commit in the repository.
            </p>
          </div>
        )
      )}

      {/* Step 3: Offer-Based .gitignore Suggestion */}
      {project && project.is_git_repo && project.has_gitignore === false && (
        gitignoreAddedNotice ? (
          <div className="w-full max-w-2xl rounded-xl border border-border bg-surface p-3 font-sans text-xs flex items-center justify-between text-zinc-300">
            <div className="flex items-center gap-2">
              <span className="text-success font-bold">[ok]</span>
              <span className="text-[11px] whitespace-pre-wrap">{gitignoreAddedNotice}</span>
            </div>
          </div>
        ) : !isGitignoreDeclined ? (
          <div className="w-full max-w-2xl rounded-xl border border-border bg-surface-raised p-3.5 space-y-2.5 font-sans">
            <div className="flex items-center justify-between border-b border-border pb-1.5">
              <span className="text-zinc-200 text-xs font-semibold">Suggestion: .gitignore</span>
              <span className="text-[10px] font-mono text-text-muted">informational offer</span>
            </div>
            <p className="text-text-muted text-xs leading-relaxed">
              No .gitignore found. Add a standard one for {project.project_type}?
            </p>
            <div className="flex items-center justify-end gap-2 pt-1 border-t border-border">
              <button
                type="button"
                onClick={onDeclineGitignore}
                className="px-2.5 py-1 rounded border border-border hover:bg-surface-hover text-text-muted text-xs transition-colors"
              >
                No, Skip
              </button>
              <button
                type="button"
                onClick={onCreateGitignore}
                className="px-3 py-1 rounded bg-zinc-200 hover:bg-white text-zinc-950 font-medium text-xs transition-colors"
              >
                Yes, Add .gitignore
              </button>
            </div>
          </div>
        ) : null
      )}

      {/* 1. USER MESSAGE BUBBLE */}
      {contextPreview && (
        <div className="flex flex-col items-end space-y-1">
          <div className="text-[10px] text-zinc-500 font-medium px-1">You</div>
          <div className="max-w-2xl bg-zinc-800 text-zinc-100 rounded-2xl rounded-tr-sm px-4 py-2.5 shadow-sm text-xs leading-relaxed font-sans">
            {contextPreview.task_description}
          </div>
        </div>
      )}

      {/* 2. INLINE CONTEXT PREVIEW CARD (Pre-confirmation, No Network Call) */}
      {contextPreview && !plan && (
        <div className="flex flex-col items-start space-y-1">
          <div className="text-[10px] text-zinc-500 font-medium px-1 flex items-center gap-1.5">
            <span className="w-1.5 h-1.5 rounded-full bg-accent" />
            <span>Zencode Context Inspector</span>
          </div>

          <div className="w-full max-w-2xl rounded-xl border border-zinc-800 bg-[#121214] p-4 space-y-3 shadow-md font-sans">
            <div className="flex items-center justify-between border-b border-zinc-800/80 pb-2">
              <span className="font-semibold text-zinc-200">
                Context Package Assembled
              </span>
              <span className="text-[10px] font-mono text-zinc-300 bg-zinc-800/80 px-2 py-0.5 rounded border border-zinc-700">
                0 network calls made
              </span>
            </div>

            <p className="text-zinc-400 text-xs leading-relaxed">
              Before calling Gemini to create the implementation plan, you can review the exact context payload gathered from your workspace:
            </p>

            <div className="grid grid-cols-2 gap-2 text-[11px] font-mono">
              <div className="p-2 bg-zinc-950 border border-zinc-800/80 rounded text-zinc-400 truncate">
                <span className="text-zinc-500">manifests: </span>
                <span className="text-zinc-200">{contextPreview.manifest_summaries.length} found</span>
              </div>
              <div className="p-2 bg-zinc-950 border border-zinc-800/80 rounded text-zinc-400 truncate">
                <span className="text-zinc-500">git: </span>
                <span className="text-zinc-200">{contextPreview.git_branch_and_status}</span>
              </div>
            </div>

            {/* Toggle Full Raw Prompt Payload */}
            <div className="pt-1">
              <button
                type="button"
                onClick={() => setShowFullPrompt(!showFullPrompt)}
                className="text-[11px] text-zinc-500 hover:text-zinc-300 font-mono transition-colors"
              >
                {showFullPrompt ? '[-] Hide prompt preview' : '[+] Inspect full prompt preview'}
              </button>

              {showFullPrompt && (
                <pre className="mt-2 p-3 bg-zinc-950 border border-zinc-800 rounded font-mono text-[10px] text-zinc-300 whitespace-pre-wrap max-h-56 overflow-y-auto">
                  {contextPreview.preview_text}
                </pre>
              )}
            </div>

            {/* Confirm & Plan Primary Button */}
            <div className="pt-2 border-t border-zinc-800/80 flex items-center justify-between">
              <span className="text-[11px] text-zinc-500">
                Routes to Gemini 3.5 Flash-Lite (Standard Free Tier)
              </span>

              <button
                type="button"
                onClick={() => onConfirmPlanRequest(contextPreview.preview_id)}
                disabled={isPlanning}
                className="px-4 py-2 rounded-lg bg-zinc-100 hover:bg-white text-zinc-950 font-medium text-xs transition-colors shadow-sm disabled:opacity-40 flex items-center gap-1.5"
              >
                {isPlanning ? (
                  <span>Formulating Plan...</span>
                ) : (
                  <>
                    <span>Confirm &amp; Plan</span>
                    <span className="font-mono text-xs">&rarr;</span>
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 3. INLINE PLAN CARD */}
      {plan && (
        <div className="flex flex-col items-start space-y-1">
          <div className="text-[10px] text-zinc-500 font-medium px-1 flex items-center gap-1.5">
            <span className="w-1.5 h-1.5 rounded-full bg-accent" />
            <span>Implementation Plan</span>
          </div>

          <div className="w-full max-w-2xl rounded-xl border border-zinc-800 bg-[#121214] p-4 space-y-4 shadow-md font-sans">
            <div className="flex items-center justify-between border-b border-zinc-800/80 pb-2">
              <div>
                <h3 className="font-semibold text-zinc-200 text-xs">
                  {plan.task_summary || plan.goal || 'Plan of Action'}
                </h3>
                <div className="text-[10px] text-zinc-500 font-mono mt-0.5">
                  model: gemini-3.5-flash-lite | steps: {plan.steps.length}
                </div>
              </div>
              <span className="text-[10px] font-mono text-zinc-300 bg-zinc-800/80 px-2 py-0.5 rounded border border-zinc-700">
                plan formulated
              </span>
            </div>

            {/* Steps List */}
            <div className="space-y-2">
              {plan.steps.map((step, idx) => (
                <div
                  key={idx}
                  className="p-2.5 bg-zinc-950/80 border border-zinc-800/80 rounded-lg space-y-1 text-xs"
                >
                  <div className="flex items-center gap-2">
                    <span className="w-4 h-4 rounded-full bg-zinc-800 text-zinc-300 font-mono text-[10px] flex items-center justify-center font-bold">
                      {idx + 1}
                    </span>
                    <span className="font-medium text-zinc-200">
                      {step.goal || step.description || `Step ${idx + 1}`}
                    </span>
                  </div>
                  {step.instructions && step.instructions.length > 0 && (
                    <div className="text-zinc-400 text-[11px] leading-relaxed pl-6 space-y-0.5">
                      {step.instructions.map((inst, i) => (
                        <div key={i}>&bull; {inst}</div>
                      ))}
                    </div>
                  )}
                  {step.description && !step.instructions && (
                    <p className="text-zinc-400 text-[11px] leading-relaxed pl-6">
                      {step.description}
                    </p>
                  )}
                  {((step.target_files && step.target_files.length > 0) || (step.likely_files && step.likely_files.length > 0)) && (
                    <div className="pl-6 flex items-center gap-1 text-[10px] font-mono text-zinc-500">
                      <span>targets:</span>
                      <span className="text-zinc-300 truncate">
                        {(step.target_files || step.likely_files || []).join(', ')}
                      </span>
                    </div>
                  )}
                </div>
              ))}
            </div>

            {/* Run Locally Primary Button */}
            {!isWorkerRunning && !inventory && workerEvents.length === 0 && (
              <div className="pt-2 border-t border-zinc-800/80 flex items-center justify-between">
                <span className="text-[11px] text-zinc-500 font-sans">
                  Runs on local Ollama (Qwen 3.5 9B) in isolated Git worktree
                </span>

                <button
                  type="button"
                  onClick={() => onStartWorkerSession(plan)}
                  disabled={isLoading}
                  className="px-4 py-2 rounded-lg bg-zinc-100 hover:bg-white text-zinc-950 font-medium text-xs transition-colors shadow-sm disabled:opacity-40 flex items-center gap-1.5"
                >
                  <span>Run Locally</span>
                  <span className="font-mono text-xs">&rarr;</span>
                </button>
              </div>
            )}
          </div>
        </div>
      )}

      {/* 4. WORKER ACTIVITY & AGENT NOTES STREAM */}
      {(isWorkerRunning || workerEvents.length > 0) && (
        <div className="flex flex-col items-start space-y-1 w-full max-w-2xl">
          <div className="text-[10px] text-zinc-500 font-medium px-1 flex items-center gap-1.5">
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                isWorkerRunning ? 'bg-accent animate-pulse' : 'bg-success'
              }`}
            />
            <span>Local Worker Execution (Qwen 3.5 9B)</span>
          </div>

          <div className="w-full rounded-xl border border-zinc-800 bg-[#121214] p-4 space-y-3 shadow-md font-sans">
            {/* Status Header */}
            <div className="flex items-center justify-between border-b border-zinc-800/80 pb-2">
              <span className="font-semibold text-zinc-200">
                {isWorkerRunning ? 'Worker is executing tasks...' : 'Worker execution finished'}
              </span>
              <div className="flex items-center gap-2">
                {isWorkerRunning && onStopWorkerSession && (
                  <button
                    type="button"
                    onClick={() => onStopWorkerSession(activeTaskId || '')}
                    className="px-2 py-0.5 rounded border border-danger/60 hover:bg-danger/20 text-danger text-[10px] font-mono transition-colors"
                  >
                    Stop
                  </button>
                )}
                <span
                  className={`text-[10px] font-mono px-2 py-0.5 rounded border ${
                    isWorkerRunning
                      ? 'text-zinc-200 bg-zinc-800 border-zinc-700'
                      : 'text-zinc-300 bg-zinc-900 border-zinc-800'
                  }`}
                >
                  {isWorkerRunning ? 'worktree active' : 'completed'}
                </span>
              </div>
            </div>

            {/* Collapsible Agent Notes */}
            {agentNotes.length > 0 && (
              <div className="space-y-1.5">
                <button
                  type="button"
                  onClick={() => setShowAgentNotes(!showAgentNotes)}
                  className="w-full flex items-center justify-between text-[11px] text-zinc-400 hover:text-zinc-200 font-medium"
                >
                  <span>Agent Notes &amp; Reasoning ({agentNotes.length})</span>
                  <span className="text-[10px] font-mono">{showAgentNotes ? '[-]' : '[+]'}</span>
                </button>

                {showAgentNotes && (
                  <div className="p-3 bg-zinc-950 border border-zinc-800/80 rounded-lg space-y-2 max-h-60 overflow-y-auto font-mono text-[11px] text-zinc-300 leading-relaxed">
                    {agentNotes.map((note, idx) => (
                      <div key={idx} className="pb-2 border-b border-zinc-900 last:border-b-0">
                        {note}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}

            {/* Raw Event Stream Inspector */}
            <div className="pt-1">
              <button
                type="button"
                onClick={() => setShowRawEvents(!showRawEvents)}
                className="text-[10px] text-zinc-500 hover:text-zinc-400 font-mono"
              >
                {showRawEvents
                  ? '[-] Hide raw worker event stream'
                  : `[+] View raw worker events (${workerEvents.length})`}
              </button>

              {showRawEvents && (
                <div className="mt-2 p-2.5 bg-zinc-950 border border-zinc-800 rounded font-mono text-[10px] text-zinc-400 max-h-48 overflow-y-auto space-y-1">
                  {workerEvents.map((ev, i) => (
                    <div key={i} className="truncate">
                      <span className="text-zinc-600">[{ev.event_type}]: </span>
                      <span className="text-zinc-300">{ev.content}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* 5. INLINE COMMAND APPROVAL CARD */}
      {pendingCommandApproval && onApproveCommand && onDenyCommand && (
        <div className="flex flex-col items-start space-y-1 w-full max-w-2xl">
          <div className="text-[10px] text-zinc-300 font-semibold px-1 flex items-center gap-1.5">
            <span className="w-1.5 h-1.5 rounded-full bg-accent animate-pulse" />
            <span>Command Approval Required</span>
          </div>

          <div className="w-full rounded-xl border border-zinc-700 bg-[#121214] p-4 space-y-3 shadow-lg font-sans">
            <div className="flex items-center justify-between border-b border-zinc-800 pb-2">
              <span className="font-semibold text-zinc-100 text-xs">
                Process Execution Gate
              </span>
              <span className="text-[10px] font-mono text-zinc-400 border border-zinc-700 px-2 py-0.5 rounded">
                security checkpoint
              </span>
            </div>

            <p className="text-zinc-300 text-xs leading-relaxed">
              The agent wants to execute this command inside the task worktree:
            </p>

            <div className="p-3 bg-zinc-950 border border-zinc-800 rounded-lg font-mono text-xs space-y-1">
              <div className="flex items-center gap-2">
                <span className="text-zinc-500">$</span>
                <span className="text-zinc-100 font-bold">{pendingCommandApproval.program}</span>
                <span className="text-zinc-400">{pendingCommandApproval.args.join(' ')}</span>
              </div>
              <div className="text-[10px] text-zinc-500 truncate">
                cwd: {pendingCommandApproval.cwd}
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-2 border-t border-zinc-800">
              <button
                type="button"
                onClick={() => onDenyCommand(pendingCommandApproval)}
                className="px-3 py-1.5 rounded-lg border border-zinc-700 hover:bg-zinc-800 text-zinc-300 font-medium text-xs transition-colors"
              >
                Deny
              </button>
              <button
                type="button"
                onClick={() => onApproveCommand(pendingCommandApproval)}
                className="px-4 py-1.5 rounded-lg bg-zinc-100 hover:bg-white text-zinc-950 font-medium text-xs transition-colors shadow-sm"
              >
                Approve &amp; Run
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 6. TASK SUMMARY & DIFF REVIEW CARD (When complete) */}
      {(inventory || isAwaitingReview || (!isWorkerRunning && workerEvents.length > 0)) && (
        <div className="w-full max-w-2xl space-y-3">
          <TaskSummary
            inventory={inventory}
            verificationRuns={verificationHistory}
            workerSummary={workerSummary}
            isFailed={isFailed}
            isStuck={isCappedAt5Failures}
            onTriggerCommand={onTriggerCommand}
            onViewDiff={onOpenReviewPane}
          />

          {/* Accept / Reject Action Bar Right in Thread */}
          {inventory && onAcceptTask && onRejectTask && (
            <div className="p-3 bg-[#121214] border border-zinc-800 rounded-xl flex items-center justify-between shadow-md">
              <div className="text-xs text-zinc-400 font-sans">
                Ready to review and integrate into <span className="text-zinc-200 font-mono">main</span>?
              </div>

              <div className="flex items-center gap-2">
                {onOpenReviewPane && (
                  <button
                    type="button"
                    onClick={onOpenReviewPane}
                    className="px-3 py-1.5 rounded-lg border border-zinc-700 hover:bg-zinc-800 text-zinc-300 font-medium text-xs transition-colors"
                  >
                    View Diff &rarr;
                  </button>
                )}

                <button
                  type="button"
                  onClick={() => onRejectTask(activeTaskId || 'active-task')}
                  disabled={isLoading}
                  className="px-3 py-1.5 rounded-lg border border-zinc-700 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 font-medium text-xs transition-colors disabled:opacity-40"
                >
                  Reject
                </button>

                <button
                  type="button"
                  onClick={() => onAcceptTask(activeTaskId || 'active-task')}
                  disabled={isLoading}
                  className="px-4 py-1.5 rounded-lg bg-zinc-100 hover:bg-white text-zinc-950 font-medium text-xs transition-colors shadow-sm disabled:opacity-40"
                >
                  Accept All
                </button>
              </div>
            </div>
          )}
        </div>
      )}

      {/* 7. ADVISORY 5TH-FAILURE CARD (Opt-In Escalation) */}
      {isCappedAt5Failures && onTriggerAdvisory && lastFailedRun && (() => {
        const isPinned = Boolean(quotaState?.pinned_model);
        const relevantModels = isPinned
          ? quotaState?.models.filter(m => m.model_id === quotaState.pinned_model)
          : quotaState?.models;
        const isQuotaExhausted = Boolean(
          relevantModels && relevantModels.length > 0 && relevantModels.every(m => !m.is_available)
        );
        const exhaustionReason = relevantModels?.find(m => !m.is_available)?.unavailability_reason || 'Rate limit capacity reached';

        return (
          <div className="w-full max-w-2xl rounded-xl border border-zinc-700 bg-[#121214] p-4 space-y-3 font-sans shadow-md">
            <div className="flex items-center justify-between border-b border-zinc-800 pb-2">
              <span className="font-semibold text-zinc-200 text-xs">
                Local Repair Loop Capped (5 Failures)
              </span>
              <span className="text-[10px] font-mono text-zinc-400 bg-zinc-900 px-2 py-0.5 rounded border border-zinc-700">
                user opt-in required
              </span>
            </div>

            <p className="text-zinc-300 text-xs leading-relaxed">
              The local worker has attempted 5 consecutive automatic repairs on this build/test error and paused to prevent loop exhaustion.
            </p>

            <pre className="p-2.5 bg-zinc-950 border border-zinc-800 text-danger font-mono text-[10px] rounded max-h-28 overflow-y-auto">
              {lastFailedRun.stderr_tail.join('\n') || lastFailedRun.stdout_tail.join('\n') || 'Command exit failure'}
            </pre>

            <div className="flex items-center justify-between pt-1">
              <span className="text-[10px] text-zinc-400">
                Advisory only &bull; counts against Gemini quota
              </span>
              {isQuotaExhausted ? (
                <div className="flex flex-col items-end gap-1">
                  <button
                    type="button"
                    disabled
                    className="px-3 py-1.5 rounded-lg border border-border bg-surface text-text-muted font-medium text-xs cursor-not-allowed opacity-50"
                  >
                    Ask Gemini for Advice (Quota Exhausted)
                  </button>
                  <span className="text-[10px] text-danger font-mono">
                    Gemini quota exhausted ({exhaustionReason}). Advisory consultation unavailable until reset.
                  </span>
                </div>
              ) : (
                <button
                  type="button"
                  onClick={() =>
                    onTriggerAdvisory(
                      lastFailedRun.stderr_tail.join('\n') || lastFailedRun.stdout_tail.join('\n') || 'Build error',
                      lastFailedRun.cwd
                    )
                  }
                  disabled={isLoading}
                  className="px-3 py-1.5 rounded-lg bg-zinc-100 hover:bg-white text-zinc-950 font-semibold text-xs transition-colors shadow-sm disabled:opacity-40"
                >
                  Ask Gemini for Advice
                </button>
              )}
            </div>

            {advisorySuggestion && (
              <div className="mt-3 p-3 bg-zinc-900 border border-zinc-800 rounded text-zinc-200 text-xs leading-relaxed font-sans space-y-3">
                <div className="text-[10px] text-zinc-400 uppercase font-semibold font-mono">
                  Gemini Advisory Suggestion:
                </div>
                <div className="whitespace-pre-wrap">{advisorySuggestion}</div>
                {onApplyAdvisoryToWorker && (
                  <div className="flex justify-end pt-2 border-t border-zinc-800">
                    <button
                      type="button"
                      onClick={() => onApplyAdvisoryToWorker(advisorySuggestion)}
                      disabled={isLoading}
                      className="px-3 py-1.5 rounded-lg bg-zinc-100 hover:bg-white text-zinc-950 font-medium text-xs transition-colors shadow-sm disabled:opacity-40"
                    >
                      Apply Advice & Retry Attempt
                    </button>
                  </div>
                )}
              </div>
            )}
          </div>
        );
      })()}

      {/* Bottom padding anchor for auto-scroll */}
      <div ref={scrollEndRef} className="h-4" />
    </div>
  );
};