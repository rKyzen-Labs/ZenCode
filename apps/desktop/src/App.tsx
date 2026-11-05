import { useState, useEffect, useCallback } from 'react';
import type {
  ProjectState,
  SystemStatus,
  ContextPreview,
  ZencodePlan,
  WorkerEventPayload,
  TaskState,
  EngineHealth,
  QuotaUpdateEvent,
} from './types';
import {
  openProject,
  getProjectState,
  previewPlanningContext,
  submitPlanRequest,
  startWorkerSession,
  stopWorkerSession,
  subscribeToWorkerMessages,
  subscribeToWorkerStatus,
  subscribeToEngineHealth,
  getQuotaState,
  setPinnedModel,
  subscribeToQuotaUpdates,
} from './lib/ipc';
import { StatusBar } from './components/StatusBar';
import { WelcomeScreen } from './components/WelcomeScreen';
import { WorkspaceScreen } from './components/WorkspaceScreen';
import { TaskView } from './components/TaskView';
import { QuotaDashboard } from './components/QuotaDashboard';
import { Shield, Sparkles, Gauge } from 'lucide-react';

export default function App() {
  const [project, setProject] = useState<ProjectState | null>(null);
  const [systemStatus, setSystemStatus] = useState<SystemStatus | null>(null);
  const [quotaState, setQuotaState] = useState<QuotaUpdateEvent | null>(null);
  const [showQuotaDashboard, setShowQuotaDashboard] = useState(false);
  const [isPinnedLoading, setIsPinnedLoading] = useState(false);
  const [lastActiveModel, setLastActiveModel] = useState<string>('gemini-3.5-flash-lite');
  const [lastSwitchReason, setLastSwitchReason] = useState<string | null>(null);

  const [contextPreview, setContextPreview] = useState<ContextPreview | null>(null);
  const [plan, setPlan] = useState<ZencodePlan | null>(null);
  const [workerEvents, setWorkerEvents] = useState<WorkerEventPayload[]>([]);
  const [workerSessionId, setWorkerSessionId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Refresh status from Rust backend
  const refreshStatus = useCallback(async () => {
    try {
      const [proj, status] = await getProjectState();
      if (proj) {
        setProject(proj);
      }
      setSystemStatus(status);
    } catch (err) {
      console.error('Failed to get project state:', err);
      // Fallback mock if running outside Tauri
      setSystemStatus({
        gemini_ready: true,
        gemini_status_text: 'Key present in Credential Manager',
        ollama_ready: true,
        ollama_status_text: 'Ollama ready with qwen2.5-coder:7b',
        opencode_health: 'Ready',
        task_state: 'Idle',
        request_count: 0,
      });
    }
  }, []);

  // Subscribe to real-time events on mount
  useEffect(() => {
    refreshStatus();

    // Fetch initial rate-limit telemetry
    getQuotaState()
      .then((qs) => {
        setQuotaState(qs);
        if (qs.active_model) {
          setLastActiveModel(qs.active_model);
        }
      })
      .catch((err) => console.warn('Could not fetch initial quota state:', err));

    let unlistenWorkerMsg: (() => void) | undefined;
    let unlistenWorkerStatus: (() => void) | undefined;
    let unlistenEngineHealth: (() => void) | undefined;
    let unlistenQuotaUpdate: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        unlistenWorkerMsg = await subscribeToWorkerMessages((payload) => {
          setWorkerEvents((prev) => [...prev, payload]);
        });

        unlistenWorkerStatus = await subscribeToWorkerStatus((taskState: TaskState) => {
          setSystemStatus((prev) => (prev ? { ...prev, task_state: taskState } : null));
        });

        unlistenEngineHealth = await subscribeToEngineHealth((health: EngineHealth) => {
          setSystemStatus((prev) => (prev ? { ...prev, opencode_health: health } : null));
        });

        unlistenQuotaUpdate = await subscribeToQuotaUpdates((qs: QuotaUpdateEvent) => {
          setQuotaState(qs);
          if (qs.active_model) {
            setLastActiveModel(qs.active_model);
          }
          if (qs.last_switch_reason) {
            setLastSwitchReason(qs.last_switch_reason);
          }
        });
      } catch (err) {
        console.warn('Tauri event listeners could not be attached:', err);
      }
    };

    setupListeners();

    return () => {
      unlistenWorkerMsg?.();
      unlistenWorkerStatus?.();
      unlistenEngineHealth?.();
      unlistenQuotaUpdate?.();
    };
  }, [refreshStatus]);

  // Handler: Pin / Unpin model (e.g. gemini-3.8-flash)
  const handlePinModel = async (modelId: string | null) => {
    setIsPinnedLoading(true);
    try {
      const updated = await setPinnedModel(modelId);
      setQuotaState(updated);
      setLastActiveModel(updated.active_model);
      if (updated.last_switch_reason) {
        setLastSwitchReason(updated.last_switch_reason);
      }
    } catch (err: any) {
      console.error('Failed to pin model:', err);
    } finally {
      setIsPinnedLoading(false);
    }
  };

  // Handler: Open Project
  const handleOpenProject = async (path: string) => {
    setIsLoading(true);
    setError(null);
    try {
      const proj = await openProject(path);
      setProject(proj);
      await refreshStatus();
    } catch (err: any) {
      console.error('Error opening project:', err);
      setError(typeof err === 'string' ? err : err.message || 'Failed to open project folder');
    } finally {
      setIsLoading(false);
    }
  };

  // Handler: Generate Context Preview (No network call)
  const handlePreviewContext = async (taskDescription: string) => {
    setIsLoading(true);
    setError(null);
    try {
      const preview = await previewPlanningContext(taskDescription);
      setContextPreview(preview);
      setPlan(null);
      setWorkerEvents([]);
      setWorkerSessionId(null);
    } catch (err: any) {
      console.error('Error previewing context:', err);
      setError(typeof err === 'string' ? err : err.message || 'Failed to assemble planning context');
    } finally {
      setIsLoading(false);
    }
  };

  // Handler: Confirm & Send to Gemini Planner
  const handleConfirmPlanRequest = async (previewId: string) => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await submitPlanRequest(previewId, true);
      setPlan(result.plan);
      if (result.active_model) {
        setLastActiveModel(result.active_model);
      }
      if (result.switch_reason) {
        setLastSwitchReason(result.switch_reason);
      }
      setSystemStatus((prev) =>
        prev
          ? {
              ...prev,
              request_count: result.request_count ?? prev.request_count + 1,
              task_state: 'Idle',
            }
          : null
      );
    } catch (err: any) {
      console.error('Error submitting plan request:', err);
      setError(typeof err === 'string' ? err : err.message || 'Planning request failed');
    } finally {
      setIsLoading(false);
    }
  };

  // Handler: Execute Plan with Local Worker
  const handleStartWorkerSession = async (planToExecute: ZencodePlan) => {
    setIsLoading(true);
    setError(null);
    try {
      const sessionId = await startWorkerSession(planToExecute);
      setWorkerSessionId(sessionId);
      setSystemStatus((prev) =>
        prev ? { ...prev, task_state: 'WorkerRunning' } : null
      );
    } catch (err: any) {
      console.error('Error starting worker session:', err);
      setError(typeof err === 'string' ? err : err.message || 'Failed to start worker session');
    } finally {
      setIsLoading(false);
    }
  };

  // Handler: Stop Worker Session
  const handleStopWorkerSession = async (sessionId: string) => {
    try {
      await stopWorkerSession(sessionId);
      setSystemStatus((prev) =>
        prev ? { ...prev, task_state: 'Idle' } : null
      );
    } catch (err: any) {
      console.error('Error stopping worker session:', err);
      setError(typeof err === 'string' ? err : err.message || 'Failed to stop worker');
    }
  };

  // Handler: Reset Task State
  const handleResetTask = () => {
    setContextPreview(null);
    setPlan(null);
    setWorkerEvents([]);
    setWorkerSessionId(null);
    setError(null);
  };

  // Switch Project
  const handleChangeProject = () => {
    setProject(null);
    handleResetTask();
  };

  const isTaskActive = Boolean(contextPreview || plan || workerEvents.length > 0);

  return (
    <div className="flex h-screen w-screen flex-col bg-[var(--background)] text-[var(--foreground)] overflow-hidden font-sans">
      {/* Top Application Header */}
      <header className="h-11 w-full border-b border-[var(--surface-border)] bg-[var(--surface)] px-4 flex items-center justify-between text-xs select-none shrink-0 z-10">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 font-semibold tracking-wide text-zinc-100">
            <div className="w-5 h-5 rounded-md bg-blue-600 flex items-center justify-center text-white font-bold text-[10px] shadow-sm">
              Z
            </div>
            <span>Zencode</span>
          </div>

          <span className="text-zinc-600">|</span>

          <div className="flex items-center gap-1.5 text-zinc-400">
            <Shield className="w-3.5 h-3.5 text-emerald-400" />
            <span className="text-[11px]">Free-first Local-first</span>
          </div>
        </div>

        {/* Right side indicators */}
        <div className="flex items-center gap-2">
          {project && (
            <div className="flex items-center gap-1.5 px-2.5 py-1 rounded bg-[var(--surface-raised)] border border-[var(--surface-border)] text-zinc-300 font-mono text-[11px]">
              <span className="w-2 h-2 rounded-full bg-emerald-400"></span>
              <span>{project.name}</span>
            </div>
          )}

          {/* Interactive Rate Limit / Planner Model Pill */}
          <button
            type="button"
            onClick={() => setShowQuotaDashboard(!showQuotaDashboard)}
            title="Click to view real-time rate limit telemetry & waterfall cascade"
            className={`flex items-center gap-1.5 px-2.5 py-1 rounded border text-[11px] font-medium transition-colors cursor-pointer ${
              showQuotaDashboard
                ? 'bg-blue-600 text-white border-blue-500 shadow-sm'
                : quotaState?.pinned_model
                ? 'bg-purple-500/20 text-purple-300 border-purple-500/40 hover:bg-purple-500/30'
                : 'bg-purple-500/10 text-purple-400 border-purple-500/20 hover:bg-purple-500/20'
            }`}
          >
            <Sparkles className="w-3 h-3 text-purple-400" />
            <span className="font-mono">
              {quotaState?.pinned_model
                ? 'Pinned: 3.8 Flash'
                : quotaState?.active_model || 'gemini-3.5-flash-lite'}
            </span>
            <Gauge className="w-3 h-3 text-blue-400" />
          </button>
        </div>
      </header>

      {/* Main View Area */}
      <main className="flex-1 flex flex-col overflow-hidden relative">
        {/* Quota Dashboard Modal / Drawer */}
        {showQuotaDashboard && (
          <div className="absolute inset-0 bg-black/60 backdrop-blur-sm z-50 p-6 flex items-center justify-center overflow-y-auto">
            <QuotaDashboard
              quotaState={quotaState}
              onPinModel={handlePinModel}
              onClose={() => setShowQuotaDashboard(false)}
              isPinnedActionLoading={isPinnedLoading}
            />
          </div>
        )}

        {!project ? (
          <WelcomeScreen
            status={systemStatus}
            onOpenProject={handleOpenProject}
            onRefreshStatus={refreshStatus}
            isLoading={isLoading}
            error={error}
          />
        ) : isTaskActive ? (
          <TaskView
            contextPreview={contextPreview}
            plan={plan}
            taskState={systemStatus?.task_state || 'Idle'}
            workerEvents={workerEvents}
            workerSessionId={workerSessionId}
            activePlannerModel={lastActiveModel}
            switchReason={lastSwitchReason}
            onConfirmPlanRequest={handleConfirmPlanRequest}
            onStartWorkerSession={handleStartWorkerSession}
            onStopWorkerSession={handleStopWorkerSession}
            onResetTask={handleResetTask}
            isLoading={isLoading}
            error={error}
          />
        ) : (
          <WorkspaceScreen
            project={project}
            onPreviewContext={handlePreviewContext}
            onChangeProject={handleChangeProject}
            isLoading={isLoading}
            error={error}
          />
        )}
      </main>

      {/* Sticky Bottom Status Bar */}
      <StatusBar
        project={project}
        status={systemStatus}
        quotaState={quotaState}
        onOpenQuotaDashboard={() => setShowQuotaDashboard((prev) => !prev)}
      />
    </div>
  );
}
