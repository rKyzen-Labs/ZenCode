import { useState, useEffect, useCallback } from 'react';
import type {
  ProjectState,
  SystemStatus,
  ContextPreview,
  ZencodePlan,
  WorkerEventPayload,
  TaskState,
  EngineHealth,
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
} from './lib/ipc';
import { StatusBar } from './components/StatusBar';
import { WelcomeScreen } from './components/WelcomeScreen';
import { WorkspaceScreen } from './components/WorkspaceScreen';
import { TaskView } from './components/TaskView';
import { Shield, Sparkles } from 'lucide-react';

export default function App() {
  const [project, setProject] = useState<ProjectState | null>(null);
  const [systemStatus, setSystemStatus] = useState<SystemStatus | null>(null);
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
      // If outside Tauri, set mock status for UI testing
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

    let unlistenWorkerMsg: (() => void) | undefined;
    let unlistenWorkerStatus: (() => void) | undefined;
    let unlistenEngineHealth: (() => void) | undefined;

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
      } catch (err) {
        console.warn('Tauri event listeners could not be attached:', err);
      }
    };

    setupListeners();

    return () => {
      unlistenWorkerMsg?.();
      unlistenWorkerStatus?.();
      unlistenEngineHealth?.();
    };
  }, [refreshStatus]);

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
      setSystemStatus((prev) =>
        prev
          ? {
              ...prev,
              request_count: result.request_count,
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

          <div className="flex items-center gap-1 px-2.5 py-1 rounded bg-purple-500/10 text-purple-400 border border-purple-500/20 text-[11px] font-medium">
            <Sparkles className="w-3 h-3" />
            <span>Gemini 3.5 Flash-Lite</span>
          </div>
        </div>
      </header>

      {/* Main View Area */}
      <main className="flex-1 flex flex-col overflow-hidden relative">
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
      <StatusBar project={project} status={systemStatus} />
    </div>
  );
}
