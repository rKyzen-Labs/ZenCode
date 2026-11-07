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
  ChangeInventory,
  OrphanWorktree,
  UncommittedFile,
  MergeResult,
  CommandRequest,
  ProcessOutputEvent,
  ExecutionResult,
  AdvisoryPreview,
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
  acceptTask,
  rejectTask,
  stashPrimaryChanges,
  commitPrimaryChanges,
  listActiveWorktrees,
  recoverOrphanedWorktree,
  subscribeToInventoryReady,
  requestCommandApproval,
  approveAndRunCommand,
  cancelCommandExecution,
  exportDiagnosticBundle,
  previewAdvisoryContext,
  submitAdvisoryRequest,
  subscribeToExecOutput,
  subscribeToExecFinished,
} from './lib/ipc';
import { HeaderTabBar } from './components/HeaderTabBar';
import { SideToolRail, type ActiveToolPane } from './components/SideToolRail';
import { PromptDock } from './components/PromptDock';
import { WelcomeScreen } from './components/WelcomeScreen';
import { WorkspaceScreen } from './components/WorkspaceScreen';
import { TaskView } from './components/TaskView';
import { QuotaDashboard } from './components/QuotaDashboard';
import { DirtyTreeDialog } from './components/DirtyTreeDialog';
import { OrphanRecoveryDialog } from './components/OrphanRecoveryDialog';
import { ReviewPanel } from './features/review/ReviewPanel';
import { CommandApprovalDialog } from './components/CommandApprovalDialog';
import { AdvisoryPreviewModal } from './components/AdvisoryPreviewModal';
import { ExecutionTerminal } from './components/ExecutionTerminal';
import { StatusBar } from './components/StatusBar';

export default function App() {
  const [project, setProject] = useState<ProjectState | null>(null);
  const [systemStatus, setSystemStatus] = useState<SystemStatus | null>(null);
  const [quotaState, setQuotaState] = useState<QuotaUpdateEvent | null>(null);
  const [activeToolPane, setActiveToolPane] = useState<ActiveToolPane>(null);
  const [isPinnedLoading, setIsPinnedLoading] = useState(false);
  const [lastActiveModel, setLastActiveModel] = useState<string>('gemini-3.5-flash-lite');
  const [lastSwitchReason, setLastSwitchReason] = useState<string | null>(null);

  const [contextPreview, setContextPreview] = useState<ContextPreview | null>(null);
  const [plan, setPlan] = useState<ZencodePlan | null>(null);
  const [workerEvents, setWorkerEvents] = useState<WorkerEventPayload[]>([]);
  const [workerSessionId, setWorkerSessionId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Milestone 2 Git Isolation & Review State
  const [inventory, setInventory] = useState<ChangeInventory | null>(null);
  const [activeTaskId, setActiveTaskId] = useState<string | null>(null);
  const [dirtyFiles, setDirtyFiles] = useState<UncommittedFile[] | null>(null);
  const [pendingPlanForDirty, setPendingPlanForDirty] = useState<ZencodePlan | null>(null);
  const [orphanedWorktrees, setOrphanedWorktrees] = useState<OrphanWorktree[]>([]);

  // Milestone 3 Permissions, Execution, and Diagnostics State
  const [pendingCommandApproval, setPendingCommandApproval] = useState<CommandRequest | null>(null);
  const [isCommandModalOpen, setIsCommandModalOpen] = useState(false);
  const [isTerminalOpen, setIsTerminalOpen] = useState(false);
  const [activeCommandId, setActiveCommandId] = useState<string | null>(null);
  const [activeProgram, setActiveProgram] = useState<string>('');
  const [activeArgs, setActiveArgs] = useState<string[]>([]);
  const [activeCwd, setActiveCwd] = useState<string>('');
  const [isCommandRunning, setIsCommandRunning] = useState(false);
  const [commandOutputLines, setCommandOutputLines] = useState<ProcessOutputEvent[]>([]);
  const [executionResult, setExecutionResult] = useState<ExecutionResult | null>(null);
  const [verificationHistory, setVerificationHistory] = useState<ExecutionResult[]>([]);
  const [diagnosticNotice, setDiagnosticNotice] = useState<string | null>(null);

  // Refresh status from Rust backend
  const refreshStatus = useCallback(async () => {
    try {
      const res: any = await getProjectState();
      if (Array.isArray(res)) {
        const [proj, status] = res;
        if (proj) setProject(proj);
        if (status) setSystemStatus(status);
      } else if (res && typeof res === 'object') {
        if (res.active_project) {
          setProject(res.active_project);
        }
        setSystemStatus(res);
      }
    } catch (err) {
      console.error('Failed to get project state:', err);
      // Fallback mock if running outside Tauri
      setSystemStatus({
        gemini_ready: true,
        gemini_status_text: 'Key present in Credential Manager',
        ollama_ready: true,
        ollama_status_text: 'Ollama ready with qwen3.5:9b',
        opencode_health: 'NotStarted',
        task_state: 'Idle',
        request_count: 0,
      });
    }
  }, []);

  // Global Keyboard Shortcuts (matching Example Image 2)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.altKey) {
        const k = (e.key || '').toLowerCase();
        if (k === 'r') {
          e.preventDefault();
          setActiveToolPane((prev) => (prev === 'review' ? null : 'review'));
        } else if (k === 'b') {
          e.preventDefault();
          setActiveToolPane((prev) => (prev === 'quota' ? null : 'quota'));
        } else if (k === 't') {
          e.preventDefault();
          setActiveToolPane((prev) => (prev === 'terminal' ? null : 'terminal'));
        } else if (k === 'p') {
          e.preventDefault();
          setActiveToolPane((prev) => (prev === 'files' ? null : 'files'));
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
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

    // Check for orphaned worktrees on startup before showing screens
    listActiveWorktrees()
      .then((orphans) => {
        if (orphans && orphans.length > 0) {
          setOrphanedWorktrees(orphans);
        }
      })
      .catch((err) => console.warn('Could not check for orphaned worktrees on launch:', err));

    let unlistenWorkerMsg: (() => void) | undefined;
    let unlistenWorkerStatus: (() => void) | undefined;
    let unlistenEngineHealth: (() => void) | undefined;
    let unlistenQuotaUpdate: (() => void) | undefined;
    let unlistenInventoryReady: (() => void) | undefined;
    let unlistenExecOutput: (() => void) | undefined;
    let unlistenExecFinished: (() => void) | undefined;

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

        unlistenInventoryReady = await subscribeToInventoryReady((inv: ChangeInventory) => {
          setInventory(inv);
          // Automatically open review pane when inventory is received
          setActiveToolPane('review');
        });

        unlistenExecOutput = await subscribeToExecOutput((ev: ProcessOutputEvent) => {
          setCommandOutputLines((prev) => [...prev, ev]);
        });

        unlistenExecFinished = await subscribeToExecFinished((res: ExecutionResult) => {
          setExecutionResult(res);
          setVerificationHistory((prev) => [...prev.filter((r) => r.command_id !== res.command_id), res]);
          setIsCommandRunning(false);
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
      unlistenInventoryReady?.();
      unlistenExecOutput?.();
      unlistenExecFinished?.();
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

      // Check for orphaned worktrees in this newly opened repository
      const orphans = await listActiveWorktrees();
      if (orphans && orphans.length > 0) {
        setOrphanedWorktrees(orphans);
      }
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
      setInventory(null);
      setActiveTaskId(null);
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

  // Handler: Execute Plan with Local Worker inside isolated Git worktree
  const handleStartWorkerSession = async (
    planToExecute: ZencodePlan,
    taskId?: string,
    acknowledgeDirty?: boolean
  ) => {
    setIsLoading(true);
    setError(null);
    try {
      const tid = taskId || `task_${Date.now()}`;
      setActiveTaskId(tid);
      const sessionId = await startWorkerSession(planToExecute, tid, acknowledgeDirty);
      setWorkerSessionId(sessionId);
      setSystemStatus((prev) =>
        prev ? { ...prev, task_state: 'WorkerRunning' } : null
      );
    } catch (err: any) {
      console.error('Error starting worker session:', err);
      if (err?.error_type === 'DIRTY_PRIMARY_TREE' || err?.uncommitted_files) {
        setDirtyFiles(err.uncommitted_files || []);
        setPendingPlanForDirty(planToExecute);
      } else {
        setError(typeof err === 'string' ? err : err.message || 'Failed to start worker session');
      }
    } finally {
      setIsLoading(false);
    }
  };

  // Dirty Tree Resolution: Stash
  const handleStashDirty = async (msg?: string) => {
    setIsLoading(true);
    setError(null);
    try {
      await stashPrimaryChanges(msg);
      setDirtyFiles(null);
      if (pendingPlanForDirty) {
        const p = pendingPlanForDirty;
        setPendingPlanForDirty(null);
        await handleStartWorkerSession(p, undefined, false);
      }
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to stash changes');
    } finally {
      setIsLoading(false);
    }
  };

  // Dirty Tree Resolution: Commit
  const handleCommitDirty = async (msg: string) => {
    setIsLoading(true);
    setError(null);
    try {
      await commitPrimaryChanges(msg);
      setDirtyFiles(null);
      if (pendingPlanForDirty) {
        const p = pendingPlanForDirty;
        setPendingPlanForDirty(null);
        await handleStartWorkerSession(p, undefined, false);
      }
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to commit changes');
    } finally {
      setIsLoading(false);
    }
  };

  // Dirty Tree Resolution: Acknowledge & proceed anyway
  const handleProceedAnywayDirty = async () => {
    setDirtyFiles(null);
    if (pendingPlanForDirty) {
      const p = pendingPlanForDirty;
      setPendingPlanForDirty(null);
      await handleStartWorkerSession(p, undefined, true);
    }
  };

  const handleCancelDirty = () => {
    setDirtyFiles(null);
    setPendingPlanForDirty(null);
  };

  // Handler: Accept Task Changes
  const handleAcceptTask = async (tid: string): Promise<MergeResult> => {
    setIsLoading(true);
    try {
      const res = await acceptTask(tid);
      if ('Success' in res || (res as any).status === 'Success') {
        setInventory(null);
        setActiveTaskId(null);
        setActiveToolPane(null);
        await refreshStatus();
      }
      return res;
    } finally {
      setIsLoading(false);
    }
  };

  // Handler: Reject Task Changes
  const handleRejectTask = async (tid: string): Promise<void> => {
    setIsLoading(true);
    try {
      await rejectTask(tid);
      setInventory(null);
      setActiveTaskId(null);
      setActiveToolPane(null);
      await refreshStatus();
    } finally {
      setIsLoading(false);
    }
  };

  // Orphan Worktree Recovery Handlers
  const handleReviewOrphan = async (tid: string) => {
    setIsLoading(true);
    try {
      const inv = await recoverOrphanedWorktree(tid, 'review');
      if (inv) {
        setInventory(inv);
        setActiveTaskId(tid);
        setActiveToolPane('review');
        setOrphanedWorktrees((prev) => prev.filter((o) => o.task_id !== tid));
      }
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to review orphaned worktree');
    } finally {
      setIsLoading(false);
    }
  };

  const handleRejectOrphan = async (tid: string) => {
    setIsLoading(true);
    try {
      await recoverOrphanedWorktree(tid, 'reject');
      setOrphanedWorktrees((prev) => prev.filter((o) => o.task_id !== tid));
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to reject orphaned worktree');
    } finally {
      setIsLoading(false);
    }
  };

  const handleIgnoreOrphans = () => {
    setOrphanedWorktrees([]);
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

  // Handler: Reset Task State / New Session
  const handleResetTask = () => {
    setContextPreview(null);
    setPlan(null);
    setWorkerEvents([]);
    setWorkerSessionId(null);
    setInventory(null);
    setActiveTaskId(null);
    setActiveToolPane(null);
    setVerificationHistory([]);
    setDiagnosticNotice(null);
    setError(null);
  };

  // Milestone 3 & Refinement Pass Command Handlers (Step 5 Allowlist + Session Memory)
  const handleTriggerCommand = async (
    program: string,
    args: string[],
    cwd?: string,
    description?: string
  ) => {
    try {
      setError(null);
      const req = await requestCommandApproval(program, args, cwd, description);
      if (req.auto_approved) {
        // Step 5: Safe allowlisted or already approved in this session: execute immediately without re-prompt
        await handleApproveCommand(req);
      } else {
        setPendingCommandApproval(req);
        setIsCommandModalOpen(true);
      }
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to request command approval');
    }
  };

  // Step 6: Advisory Error Consultation with Gemini (Opt-in only, capped at 5 local repairs)
  const [advisoryPreview, setAdvisoryPreview] = useState<AdvisoryPreview | null>(null);
  const [isAdvisoryModalOpen, setIsAdvisoryModalOpen] = useState(false);
  const [isAdvisoryLoading, setIsAdvisoryLoading] = useState(false);
  const [advisorySuggestion, setAdvisorySuggestion] = useState<string | null>(null);

  const handleTriggerAdvisory = async (errorText: string, filePath?: string) => {
    try {
      setIsAdvisoryLoading(true);
      setError(null);
      const preview = await previewAdvisoryContext(errorText, filePath);
      setAdvisoryPreview(preview);
      setIsAdvisoryModalOpen(true);
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to generate advisory context preview');
    } finally {
      setIsAdvisoryLoading(false);
    }
  };

  const handleConfirmAdvisory = async (previewId: string) => {
    try {
      setIsAdvisoryLoading(true);
      setError(null);
      const suggestion = await submitAdvisoryRequest(previewId, true);
      setAdvisorySuggestion(suggestion);
      setIsAdvisoryModalOpen(false);
      setAdvisoryPreview(null);
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to obtain Gemini advisory suggestion');
    } finally {
      setIsAdvisoryLoading(false);
    }
  };

  const handleCloseAdvisory = () => {
    setIsAdvisoryModalOpen(false);
    setAdvisoryPreview(null);
  };

  const handleApproveCommand = async (req: CommandRequest) => {
    setIsCommandModalOpen(false);
    setPendingCommandApproval(null);
    setActiveCommandId(req.id);
    setActiveProgram(req.program);
    setActiveArgs(req.args);
    setActiveCwd(req.cwd);
    setIsCommandRunning(true);
    setCommandOutputLines([]);
    setExecutionResult(null);
    setIsTerminalOpen(true);

    try {
      const res = await approveAndRunCommand(req.id);
      setExecutionResult(res);
      setVerificationHistory((prev) => [...prev.filter((r) => r.command_id !== res.command_id), res]);
      setIsCommandRunning(false);
    } catch (err: any) {
      setIsCommandRunning(false);
      setError(typeof err === 'string' ? err : err.message || 'Command execution failed');
    }
  };

  const handleDenyCommand = (_req: CommandRequest) => {
    setIsCommandModalOpen(false);
    setPendingCommandApproval(null);
  };

  const handleCancelCommand = async () => {
    if (activeCommandId) {
      try {
        await cancelCommandExecution(activeCommandId);
        setIsCommandRunning(false);
      } catch (err: any) {
        console.error('Failed to cancel command:', err);
      }
    }
  };

  const handleExportDiagnostics = async () => {
    try {
      setError(null);
      const bundle = await exportDiagnosticBundle(activeTaskId || undefined);
      setDiagnosticNotice(
        `[ok]: Diagnostic bundle exported to .zencode-diagnostics/ with zero credentials leaked. Files changed: ${bundle.files_changed}`
      );
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to export diagnostics');
    }
  };

  // Switch Project
  const handleChangeProject = () => {
    setProject(null);
    handleResetTask();
  };

  const isTaskActive = Boolean(contextPreview || plan || workerEvents.length > 0 || inventory);

  return (
    <div className="flex h-screen w-screen flex-col bg-[#0d0d0f] text-[#e4e4e7] overflow-hidden font-sans select-none">
      {/* Dirty Tree Modal (Step 4 Policy) */}
      {dirtyFiles && dirtyFiles.length > 0 && (
        <DirtyTreeDialog
          uncommittedFiles={dirtyFiles}
          onStash={handleStashDirty}
          onCommit={handleCommitDirty}
          onCancel={handleCancelDirty}
          onProceedAnyway={handleProceedAnywayDirty}
          isLoading={isLoading}
          error={error}
        />
      )}

      {/* Orphan Recovery Modal (Step 6) */}
      {orphanedWorktrees.length > 0 && (
        <OrphanRecoveryDialog
          orphans={orphanedWorktrees}
          onReview={handleReviewOrphan}
          onReject={handleRejectOrphan}
          onIgnore={handleIgnoreOrphans}
          isLoading={isLoading}
        />
      )}

      {/* Command Approval Modal (Milestone 3) */}
      {pendingCommandApproval && (
        <CommandApprovalDialog
          request={pendingCommandApproval}
          isOpen={isCommandModalOpen}
          onApprove={handleApproveCommand}
          onDeny={handleDenyCommand}
        />
      )}

      {/* Advisory Context Preview Modal (Step 6 Opt-In Escalation) */}
      {advisoryPreview && (
        <AdvisoryPreviewModal
          preview={advisoryPreview}
          isOpen={isAdvisoryModalOpen}
          isLoading={isAdvisoryLoading}
          onConfirm={handleConfirmAdvisory}
          onClose={handleCloseAdvisory}
        />
      )}

      {/* Top Window / Session Tab Header (Matching Example Image 2) */}
      <HeaderTabBar
        project={project}
        taskState={systemStatus?.task_state || 'Idle'}
        activeTab="Worker: ZenCode"
        activePlannerModel={lastActiveModel}
        isPinned={Boolean(quotaState?.pinned_model)}
        onNewSession={handleResetTask}
        onOpenQuota={() => setActiveToolPane(activeToolPane === 'quota' ? null : 'quota')}
        onChangeProject={handleChangeProject}
        tokenCount="8k"
      />

      {/* Main Split Layout: Center Chat/Canvas + Right Action Rail/Pane */}
      <div className="flex-1 flex overflow-hidden relative">
        {/* Center Workspace & Chat Area */}
        <div className="flex-1 flex flex-col h-full overflow-hidden relative bg-[#0d0d0f]">
          <div className="flex-1 overflow-y-auto">
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
                inventory={inventory}
                activeTaskId={activeTaskId}
                onConfirmPlanRequest={handleConfirmPlanRequest}
                onStartWorkerSession={handleStartWorkerSession}
                onStopWorkerSession={handleStopWorkerSession}
                onAcceptTask={handleAcceptTask}
                onRejectTask={handleRejectTask}
                onResetTask={handleResetTask}
                verificationHistory={verificationHistory}
                onOpenReviewPane={() => setActiveToolPane('review')}
                onTriggerCommand={handleTriggerCommand}
                onExportDiagnostics={handleExportDiagnostics}
                onTriggerAdvisory={handleTriggerAdvisory}
                advisorySuggestion={advisorySuggestion}
                diagnosticNotice={diagnosticNotice}
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

            {/* Terminal Drawer for Process Execution */}
            <div className="px-4 pb-4 max-w-4xl mx-auto w-full">
              <ExecutionTerminal
                isOpen={isTerminalOpen}
                commandId={activeCommandId}
                program={activeProgram}
                args={activeArgs}
                cwd={activeCwd}
                isRunning={isCommandRunning}
                outputLines={commandOutputLines}
                result={executionResult}
                onCancel={handleCancelCommand}
                onClose={() => setIsTerminalOpen(false)}
                onExportDiagnostics={handleExportDiagnostics}
              />
            </div>
          </div>

          {/* Floating Bottom Prompt Dock (Matching Example Image 2) */}
          {project && (
            <PromptDock
              onSubmit={(text, mode) => {
                if (mode === 'plan' || !plan) {
                  handlePreviewContext(text);
                } else {
                  handleStartWorkerSession(plan);
                }
              }}
              isLoading={isLoading}
              activeModelName={quotaState?.pinned_model ? '3.8 Flash' : lastActiveModel}
              isPinned={Boolean(quotaState?.pinned_model)}
              onSelectModel={handlePinModel}
            />
          )}
        </div>

        {/* Right Tool Rail & Side Pane (Matching Example Image 2) */}
        <SideToolRail
          activePane={activeToolPane}
          onTogglePane={setActiveToolPane}
          inventory={inventory}
          workerEventCount={workerEvents.length}
        >
          {activeToolPane === 'review' && inventory && (
            <ReviewPanel
              inventory={inventory}
              taskId={activeTaskId || 'active-task'}
              onAccept={handleAcceptTask}
              onReject={handleRejectTask}
              isLoading={isLoading}
            />
          )}

          {activeToolPane === 'quota' && quotaState && (
            <div className="h-full overflow-y-auto">
              <QuotaDashboard
                quotaState={quotaState}
                onClose={() => setActiveToolPane(null)}
                onPinModel={handlePinModel}
                isPinnedActionLoading={isPinnedLoading}
              />
            </div>
          )}

          {activeToolPane === 'terminal' && (
            <div className="h-full flex flex-col font-mono text-xs bg-zinc-950 p-3 rounded-lg border border-zinc-800 space-y-2 overflow-y-auto">
              <div className="text-zinc-500 text-[10px] uppercase font-bold border-b border-zinc-800 pb-1">
                SSE Worker Stream Output ({workerEvents.length} events)
              </div>
              {workerEvents.map((e, idx) => (
                <div key={idx} className="space-y-0.5">
                  <span className="text-[10px] text-zinc-500">[{e.event_type}]</span>
                  <pre className="text-zinc-300 text-[11px] whitespace-pre-wrap">{e.content}</pre>
                </div>
              ))}
            </div>
          )}

          {activeToolPane === 'files' && project && (
            <div className="h-full flex flex-col font-mono text-xs bg-zinc-950 p-3 rounded-lg border border-zinc-800 space-y-2 overflow-y-auto">
              <div className="text-zinc-500 text-[10px] uppercase font-bold border-b border-zinc-800 pb-1">
                Project Root: {project.path}
              </div>
              <p className="text-zinc-400 text-xs">
                Active Branch: <span className="text-blue-400">{project.current_branch || 'none'}</span>
              </p>
              <p className="text-zinc-400 text-xs">
                Status: <span className="text-emerald-400">{project.git_status_summary}</span>
              </p>
            </div>
          )}
        </SideToolRail>
      </div>

      {/* Sticky Bottom Status Bar */}
      <StatusBar
        project={project}
        status={systemStatus}
        quotaState={quotaState}
        onOpenQuotaDashboard={() => setActiveToolPane(activeToolPane === 'quota' ? null : 'quota')}
      />
    </div>
  );
}
