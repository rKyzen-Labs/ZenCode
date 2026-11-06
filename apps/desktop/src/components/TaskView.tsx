import { useState, useEffect, useRef, type FC } from 'react';
import type {
  ContextPreview,
  ZencodePlan,
  WorkerEventPayload,
  TaskState,
} from '../types';
import {
  ShieldAlert,
  Brain,
  CheckCircle2,
  AlertTriangle,
  FileText,
  Terminal,
  Square,
  Play,
  RotateCcw,
  Clock,
  Layers,
  ChevronDown,
  ChevronRight,
} from 'lucide-react';

interface TaskViewProps {
  contextPreview: ContextPreview | null;
  plan: ZencodePlan | null;
  taskState: TaskState;
  workerEvents: WorkerEventPayload[];
  workerSessionId: string | null;
  activePlannerModel?: string;
  switchReason?: string | null;
  onConfirmPlanRequest: (previewId: string) => Promise<void>;
  onStartWorkerSession: (plan: ZencodePlan) => Promise<void>;
  onStopWorkerSession: (sessionId: string) => Promise<void>;
  onResetTask: () => void;
  isLoading: boolean;
  error: string | null;
}

export const TaskView: FC<TaskViewProps> = ({
  contextPreview,
  plan,
  taskState,
  workerEvents,
  workerSessionId,
  activePlannerModel,
  switchReason,
  onConfirmPlanRequest,
  onStartWorkerSession,
  onStopWorkerSession,
  onResetTask,
  isLoading,
  error,
}) => {
  const [showFullPrompt, setShowFullPrompt] = useState(false);
  const [showRawEvents, setShowRawEvents] = useState(false);
  const eventsEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll worker messages as they stream in
  useEffect(() => {
    if (workerEvents.length > 0) {
      eventsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [workerEvents]);

  const isPlanning = taskState === 'Planning' || isLoading;
  const isWorkerRunning = taskState === 'WorkerRunning';
  const isComplete = taskState === 'Complete';

  return (
    <div className="flex-1 flex flex-col p-6 max-w-5xl mx-auto w-full space-y-6 overflow-y-auto">
      {/* Top Navigation / Breadcrumbs */}
      <div className="flex items-center justify-between border-b border-[var(--surface-border)] pb-3">
        <div className="flex items-center gap-2 text-xs">
          <span className="text-[var(--foreground-muted)]">Task:</span>
          <span className="text-zinc-200 font-medium truncate max-w-md">
            {contextPreview?.task_description || plan?.task_summary || plan?.goal || 'Active Task'}
          </span>
        </div>

        <button
          type="button"
          onClick={onResetTask}
          disabled={isWorkerRunning || isPlanning}
          className="text-xs px-2.5 py-1 rounded bg-[var(--surface)] hover:bg-[var(--surface-raised)] border border-[var(--surface-border)] text-zinc-300 disabled:opacity-40 flex items-center gap-1.5 transition-colors"
        >
          <RotateCcw className="w-3.5 h-3.5 text-zinc-400" />
          <span>New Task</span>
        </button>
      </div>

      {error && (
        <div className="p-3.5 rounded-lg bg-rose-500/10 border border-rose-500/30 text-rose-400 text-xs flex items-center gap-2">
          <AlertTriangle className="w-4 h-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* PHASE 1: CONTEXT PREVIEW PANEL (Non-negotiable requirement) */}
      {contextPreview && !plan && (
        <div className="bg-[var(--surface)] border border-[var(--surface-border)] rounded-xl p-6 shadow-xl space-y-5">
          <div className="flex items-center justify-between border-b border-[var(--surface-border)] pb-3">
            <div className="flex items-center gap-2">
              <ShieldAlert className="w-5 h-5 text-purple-400" />
              <div>
                <h2 className="text-sm font-semibold text-zinc-100">Planning Context Preview</h2>
                <p className="text-xs text-[var(--foreground-muted)]">
                  Verify the exact payload before calling Gemini. No cloud call has been made.
                </p>
              </div>
            </div>
            <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/30 flex items-center gap-1">
              <CheckCircle2 className="w-3 h-3" />
              Zero Secrets Detected
            </span>
          </div>

          {/* Context Breakdown Grid */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3 text-xs">
            <div className="p-3 rounded-lg bg-[var(--surface-raised)] border border-[var(--surface-border)] space-y-1.5">
              <span className="text-zinc-400 font-medium">Task Prompt:</span>
              <p className="text-zinc-200">{contextPreview.task_description}</p>
            </div>

            <div className="p-3 rounded-lg bg-[var(--surface-raised)] border border-[var(--surface-border)] space-y-1.5">
              <span className="text-zinc-400 font-medium">Project & Git State:</span>
              <p className="text-zinc-200 font-mono text-[11px]">{contextPreview.git_branch_and_status}</p>
              <p className="text-zinc-400 text-[10px]">Type: {contextPreview.project_type}</p>
            </div>
          </div>

          {/* Shallow Tree Preview */}
          <div className="p-3 rounded-lg bg-[var(--surface-raised)] border border-[var(--surface-border)] space-y-2">
            <div className="flex items-center justify-between text-xs text-zinc-400 font-medium">
              <span>Shallow File Tree ({contextPreview.shallow_tree?.length || 0} entries):</span>
              <span className="text-[10px] text-zinc-500">Excluded: node_modules, .git, target, .venv</span>
            </div>
            <div className="flex flex-wrap gap-1.5 max-h-24 overflow-y-auto">
              {(contextPreview.shallow_tree || []).map((entry, idx) => (
                <span
                  key={idx}
                  className="px-2 py-0.5 rounded bg-zinc-900 border border-zinc-800 text-zinc-300 font-mono text-[10px]"
                >
                  {entry}
                </span>
              ))}
            </div>
          </div>

          {/* Collapsible Full Assembled Prompt */}
          <div className="rounded-lg border border-[var(--surface-border)] overflow-hidden">
            <button
              type="button"
              onClick={() => setShowFullPrompt(!showFullPrompt)}
              className="w-full px-3.5 py-2.5 bg-[var(--surface-raised)] flex items-center justify-between text-xs text-zinc-300 font-medium hover:bg-zinc-800/80 transition-colors"
            >
              <div className="flex items-center gap-2">
                <FileText className="w-3.5 h-3.5 text-zinc-400" />
                <span>View Raw Assembled Gemini Prompt ({contextPreview.preview_text?.length || 0} chars)</span>
              </div>
              {showFullPrompt ? (
                <ChevronDown className="w-4 h-4 text-zinc-400" />
              ) : (
                <ChevronRight className="w-4 h-4 text-zinc-400" />
              )}
            </button>

            {showFullPrompt && (
              <pre className="p-3.5 bg-zinc-950 text-[11px] font-mono text-zinc-300 overflow-x-auto max-h-60 leading-relaxed border-t border-[var(--surface-border)]">
                {contextPreview.preview_text}
              </pre>
            )}
          </div>

          {/* Confirmation Action Bar */}
          <div className="flex items-center justify-between pt-2 border-t border-[var(--surface-border)]">
            <div className="text-[11px] text-[var(--foreground-muted)] flex items-center gap-2">
              <Brain className="w-4 h-4 text-purple-400 shrink-0" />
              <span>
                Calls Gemini 3.5 Flash-Lite (store=false, 1 call). Requires explicit confirmation.
              </span>
            </div>

            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={onResetTask}
                disabled={isPlanning}
                className="px-4 py-2 rounded-lg bg-[var(--surface-raised)] hover:bg-zinc-800 border border-[var(--surface-border)] text-xs text-zinc-300 font-medium transition-colors"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={() => onConfirmPlanRequest(contextPreview.preview_id)}
                disabled={isPlanning}
                className="px-5 py-2 rounded-lg bg-purple-600 hover:bg-purple-500 disabled:opacity-50 text-white font-medium text-xs flex items-center gap-2 transition-colors shadow-sm"
              >
                {isPlanning ? (
                  <>
                    <span className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin"></span>
                    <span>Planning with Gemini...</span>
                  </>
                ) : (
                  <>
                    <Brain className="w-4 h-4" />
                    <span>Confirm & Send to Gemini</span>
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* PHASE 2: PLAN DISPLAY (Shown once plan is received) */}
      {plan && (
        <div className="bg-[var(--surface)] border border-[var(--surface-border)] rounded-xl p-6 shadow-xl space-y-6">
          <div className="flex items-center justify-between border-b border-[var(--surface-border)] pb-3">
            <div className="flex items-center gap-2.5">
              <div className="w-8 h-8 rounded-lg bg-purple-500/10 border border-purple-500/30 flex items-center justify-center text-purple-400">
                <Brain className="w-4 h-4" />
              </div>
              <div>
                <h2 className="text-sm font-semibold text-zinc-100 flex items-center gap-2">
                  <span>Structured Implementation Plan</span>
                  {activePlannerModel && (
                    <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-purple-500/10 text-purple-400 border border-purple-500/30">
                      {activePlannerModel}
                    </span>
                  )}
                </h2>
                <p className="text-xs text-[var(--foreground-muted)]">
                  Generated via rate-limit protected Google AI Studio free tier.
                </p>
                {switchReason && (
                  <div className="mt-1 text-[11px] text-amber-400/90 flex items-center gap-1 font-mono">
                    <span>⚡ {switchReason}</span>
                  </div>
                )}
              </div>
            </div>

            <div className="flex items-center gap-2">
              {!isWorkerRunning && !isComplete && (
                <button
                  type="button"
                  onClick={() => onStartWorkerSession(plan)}
                  disabled={isLoading}
                  className="px-4 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white font-medium text-xs flex items-center gap-2 transition-colors shadow-sm"
                >
                  <Play className="w-3.5 h-3.5" />
                  <span>Execute with Local Worker</span>
                </button>
              )}

              {isWorkerRunning && workerSessionId && (
                <button
                  type="button"
                  onClick={() => onStopWorkerSession(workerSessionId)}
                  className="px-4 py-2 rounded-lg bg-rose-600 hover:bg-rose-500 text-white font-medium text-xs flex items-center gap-2 transition-colors shadow-sm"
                >
                  <Square className="w-3.5 h-3.5" />
                  <span>Stop Worker</span>
                </button>
              )}
            </div>
          </div>

          {/* Goal & Task Summary */}
          <div className="space-y-3">
            <div className="p-3.5 rounded-lg bg-[var(--surface-raised)] border border-[var(--surface-border)] space-y-1">
              <span className="text-xs font-semibold text-zinc-300">Goal / Task Summary:</span>
              <p className="text-xs text-zinc-200 leading-relaxed font-sans">
                {plan.task_summary || plan.goal || 'No task summary provided.'}
              </p>
            </div>

            {/* Architecture Notes & Project Findings */}
            {((plan.project_findings && plan.project_findings.length > 0) || plan.architecture_notes) && (
              <div className="p-3.5 rounded-lg bg-[var(--surface-raised)] border border-[var(--surface-border)] space-y-1.5">
                <span className="text-xs font-semibold text-zinc-300">Project Findings & Context:</span>
                {plan.project_findings && plan.project_findings.length > 0 ? (
                  <ul className="list-disc list-inside text-xs text-zinc-300 space-y-1">
                    {plan.project_findings.map((finding, idx) => (
                      <li key={idx} className="leading-relaxed">{finding}</li>
                    ))}
                  </ul>
                ) : (
                  <p className="text-xs text-zinc-400 leading-relaxed">{plan.architecture_notes}</p>
                )}
              </div>
            )}

            {/* Assumptions */}
            {plan.assumptions && plan.assumptions.length > 0 && (
              <div className="p-3 rounded-lg bg-[var(--surface-raised)] border border-[var(--surface-border)] space-y-1 text-xs">
                <span className="font-semibold text-zinc-400">Assumptions:</span>
                <ul className="list-disc list-inside text-zinc-400 space-y-0.5">
                  {plan.assumptions.map((item, idx) => (
                    <li key={idx}>{item}</li>
                  ))}
                </ul>
              </div>
            )}
          </div>

          {/* Implementation Steps */}
          <div className="space-y-3">
            <h3 className="text-xs font-semibold text-zinc-300 flex items-center gap-1.5">
              <Layers className="w-3.5 h-3.5 text-blue-400" />
              <span>Implementation Steps ({(plan.steps || []).length})</span>
            </h3>

            <div className="space-y-3">
              {(plan.steps || []).map((step, idx) => {
                const stepNum = step.step_number ?? (idx + 1);
                const stepId = step.id || `step_${stepNum}`;
                const stepGoal = step.goal || step.description || `Step ${stepNum}`;
                const files = step.likely_files || step.target_files || [];
                const instructions = step.instructions || [];
                const checks = step.acceptance_checks || [];

                return (
                  <div
                    key={stepId}
                    className="p-4 rounded-xl bg-[var(--surface-raised)] border border-[var(--surface-border)] space-y-2.5 shadow-sm"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="flex items-start gap-2.5">
                        <span className="w-5 h-5 rounded-full bg-blue-500/20 text-blue-400 border border-blue-500/30 text-[10px] font-mono flex items-center justify-center shrink-0 mt-0.5">
                          {stepNum}
                        </span>
                        <div>
                          <h4 className="text-xs font-semibold text-zinc-100">{stepGoal}</h4>
                          {step.id && (
                            <span className="text-[10px] text-zinc-500 font-mono">ID: {step.id}</span>
                          )}
                        </div>
                      </div>

                      {step.risk_level && (
                        <span
                          className={`text-[9px] px-2 py-0.5 rounded font-mono ml-auto ${
                            step.risk_level === 'High'
                              ? 'bg-rose-500/10 text-rose-400 border border-rose-500/30'
                              : step.risk_level === 'Medium'
                              ? 'bg-amber-500/10 text-amber-400 border border-amber-500/30'
                              : 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/30'
                          }`}
                        >
                          Risk: {step.risk_level}
                        </span>
                      )}
                    </div>

                    {/* Step Instructions */}
                    {instructions.length > 0 && (
                      <div className="space-y-1 pl-7">
                        <span className="text-[11px] font-medium text-zinc-400">Instructions:</span>
                        <ul className="list-disc list-inside text-xs text-zinc-300 space-y-0.5">
                          {instructions.map((inst, iIdx) => (
                            <li key={iIdx} className="leading-relaxed">{inst}</li>
                          ))}
                        </ul>
                      </div>
                    )}

                    {/* Target / Likely Files */}
                    {files.length > 0 && (
                      <div className="flex flex-wrap items-center gap-1.5 pl-7 pt-1">
                        <span className="text-[10px] text-zinc-500">Likely files:</span>
                        {files.map((file, fIdx) => (
                          <span
                            key={fIdx}
                            className="px-1.5 py-0.5 rounded bg-zinc-900 border border-zinc-800 text-[10px] font-mono text-zinc-300"
                          >
                            {file}
                          </span>
                        ))}
                      </div>
                    )}

                    {/* Step Verification Checks */}
                    {checks.length > 0 && (
                      <div className="space-y-0.5 pl-7 pt-1 text-[11px]">
                        <span className="font-medium text-emerald-400/90">Verification checks:</span>
                        <ul className="list-disc list-inside text-zinc-400 space-y-0.5">
                          {checks.map((check, cIdx) => (
                            <li key={cIdx}>{check}</li>
                          ))}
                        </ul>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </div>

          {/* Risks section */}
          {plan.risks && plan.risks.length > 0 && (
            <div className="space-y-2">
              <h3 className="text-xs font-semibold text-zinc-300 flex items-center gap-1.5">
                <AlertTriangle className="w-3.5 h-3.5 text-amber-400" />
                <span>Identified Risks</span>
              </h3>
              <div className="p-3.5 rounded-lg bg-amber-500/10 border border-amber-500/30 space-y-1.5">
                {plan.risks.map((risk, rIdx) => (
                  <div key={rIdx} className="flex items-center gap-2 text-xs text-amber-300">
                    <span className="w-1.5 h-1.5 rounded-full bg-amber-400 shrink-0"></span>
                    <span>{risk}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Acceptance Criteria & Verification Suggestions */}
          {((plan.verification_suggestions && plan.verification_suggestions.length > 0) ||
            (plan.acceptance_criteria && plan.acceptance_criteria.length > 0)) && (
            <div className="space-y-2">
              <h3 className="text-xs font-semibold text-zinc-300 flex items-center gap-1.5">
                <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
                <span>Acceptance & Verification Suggestions</span>
              </h3>
              <div className="p-3 rounded-lg bg-[var(--surface-raised)] border border-[var(--surface-border)] space-y-1.5">
                {(plan.verification_suggestions || plan.acceptance_criteria || []).map((criteria, cIdx) => (
                  <div key={cIdx} className="flex items-center gap-2 text-xs text-zinc-300">
                    <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shrink-0"></span>
                    <span>{criteria}</span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {/* PHASE 3: STREAMING WORKER CONVERSATION */}
      {(isWorkerRunning || workerEvents.length > 0) && (
        <div className="bg-[var(--surface)] border border-[var(--surface-border)] rounded-xl p-5 shadow-xl space-y-4">
          <div className="flex items-center justify-between border-b border-[var(--surface-border)] pb-3">
            <div className="flex items-center gap-2">
              <Terminal className="w-4 h-4 text-emerald-400" />
              <h3 className="text-xs font-semibold text-zinc-200">
                Local Worker Stream (Ollama: Granite 4.1 8B)
              </h3>
            </div>
            <div className="flex items-center gap-3">
              <button
                type="button"
                onClick={() => setShowRawEvents(!showRawEvents)}
                className="text-[10px] font-mono px-2 py-0.5 rounded bg-zinc-800 hover:bg-zinc-700 text-zinc-300 border border-zinc-700 transition-colors"
              >
                {showRawEvents ? 'View Output' : 'View Raw SSE'}
              </button>

              {isWorkerRunning ? (
                <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/30 text-[10px] font-mono">
                  <span className="w-1.5 h-1.5 rounded-full bg-blue-400 animate-pulse"></span>
                  Local Worker Running
                </span>
              ) : (
                <span className="px-2 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/30 text-[10px] font-mono flex items-center gap-1">
                  <CheckCircle2 className="w-3 h-3" />
                  Finished
                </span>
              )}
            </div>
          </div>

          {/* Real-time SSE event log feed */}
          <div className="p-4 rounded-lg bg-zinc-950 border border-zinc-900 font-mono text-xs space-y-2 max-h-80 overflow-y-auto">
            {workerEvents.length === 0 ? (
              <div className="text-zinc-600 italic">Waiting for initial worker response from Ollama...</div>
            ) : (
              workerEvents
                .map((event, idx) => {
                  let parsed: any = null;
                  if (typeof event?.content === 'string') {
                    try {
                      parsed = JSON.parse(event.content);
                    } catch {
                      // plain text
                    }
                  }

                  const eventType = parsed?.type || event?.event_type || 'message';
                  const isNoise = [
                    'plugin.added',
                    'catalog.updated',
                    'reference.updated',
                    'integration.updated',
                    'server.connected',
                  ].includes(eventType);

                  if (!showRawEvents && isNoise) {
                    return null;
                  }

                  const timeStr = event?.timestamp_ms
                    ? new Date(event.timestamp_ms).toLocaleTimeString()
                    : '';

                  let cleanText: string | null = null;
                  let statusInfo: string | null = null;

                  if (parsed?.properties?.part?.text) {
                    cleanText = parsed.properties.part.text;
                  } else if (parsed?.properties?.status?.type) {
                    statusInfo = `Worker status: ${parsed.properties.status.type}`;
                  } else if (parsed?.properties?.info?.title) {
                    statusInfo = `Session: "${parsed.properties.info.title}"`;
                  } else if (parsed?.type === 'session.diff' && parsed.properties?.diff?.length > 0) {
                    statusInfo = `Staged diff across ${parsed.properties.diff.length} file(s)`;
                  }

                  const contentStr =
                    typeof event?.content === 'string'
                      ? event.content
                      : JSON.stringify(event?.content, null, 2);

                  return (
                    <div key={idx} className="space-y-1">
                      <div className="flex items-center gap-2 text-[10px] text-zinc-500">
                        <Clock className="w-3 h-3" />
                        <span>{timeStr}</span>
                        <span className="px-1 rounded bg-zinc-900 text-zinc-400">{eventType}</span>
                        {statusInfo && (
                          <span className="px-1.5 py-0.2 rounded bg-blue-900/40 text-blue-300 font-sans text-[10px]">
                            {statusInfo}
                          </span>
                        )}
                      </div>
                      <pre className="text-zinc-300 font-mono text-[11px] whitespace-pre-wrap leading-relaxed">
                        {!showRawEvents && cleanText ? cleanText : contentStr}
                      </pre>
                    </div>
                  );
                })
                .filter(Boolean)
            )}
            <div ref={eventsEndRef} />
          </div>
        </div>
      )}
    </div>
  );
};
