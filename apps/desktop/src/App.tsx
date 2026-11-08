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
  sendAdvisoryToWorker,
  subscribeToExecOutput,
  subscribeToExecFinished,
  listWorkspaces,
  listChats,
  saveChat,
  loadChat,
  deleteChat,
  getChangeInventory,
  initGitRepo,
  createInitialEmptyCommit,
  createStandardGitignore,
} from './lib/ipc';
import { Sidebar, type StoredChatSummary } from './components/Sidebar';
import { EmptyState } from './components/EmptyState';
import { ChatHeader } from './components/ChatHeader';
import { ChatInputDock } from './components/ChatInputDock';
import { RightPanel } from './components/RightPanel';
import { QuotaDashboard } from './components/QuotaDashboard';
import { DirtyTreeDialog } from './components/DirtyTreeDialog';
import { OrphanRecoveryDialog } from './components/OrphanRecoveryDialog';
import { AdvisoryPreviewModal } from './components/AdvisoryPreviewModal';
import { ExecutionTerminal } from './components/ExecutionTerminal';
import { StatusBar } from './components/StatusBar';
import { TaskView } from './components/TaskView';

export default function App() {
  const [project, setProject] = useState<ProjectState | null>(null);
  const [systemStatus, setSystemStatus] = useState<SystemStatus | null>(null);
  const [quotaState, setQuotaState] = useState<QuotaUpdateEvent | null>(null);
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(false);
  const [isRightPanelOpen, setIsRightPanelOpen] = useState(false);
  const [isQuotaModalOpen, setIsQuotaModalOpen] = useState(false);
  const [isPinnedLoading, setIsPinnedLoading] = useState(false);
  const [lastActiveModel, setLastActiveModel] = useState<string>('gemini-3.5-flash-lite');
  const [lastSwitchReason, setLastSwitchReason] = useState<string | null>(null);

  const [recentWorkspaces, setRecentWorkspaces] = useState<string[]>(() => {
    try {
      const saved = localStorage.getItem('zencode_recent_workspaces');
      return saved ? JSON.parse(saved) : ['A:\\ZenCode'];
    } catch {
      return ['A:\\ZenCode'];
    }
  });
  const [storedChats, setStoredChats] = useState<StoredChatSummary[]>([]);
  const [activeChatTitle, setActiveChatTitle] = useState<string>('New Task');

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
  const [pendingAcceptTaskId, setPendingAcceptTaskId] = useState<string | null>(null);
  const [orphanedWorktrees, setOrphanedWorktrees] = useState<OrphanWorktree[]>([]);

  // Automatically open right panel when inventory is ready with file changes
  useEffect(() => {
    if (inventory && inventory.files.length > 0) {
      setIsRightPanelOpen(true);
    }
  }, [inventory]);

  // Sync active task into storedChats summary list
  useEffect(() => {
    if (activeTaskId && project) {
      setStoredChats((prev) => {
        const title = activeChatTitle || 'Task Session';
        const status: StoredChatSummary['status'] = inventory
          ? 'review'
          : systemStatus?.task_state === 'WorkerRunning'
          ? 'active'
          : 'idle';
        const existing = prev.find((c) => c.id === activeTaskId);
        if (existing) {
          return prev.map((c) =>
            c.id === activeTaskId
              ? { ...c, title, status, files_count: inventory?.files.length }
              : c
          );
        }
        return [
          {
            id: activeTaskId,
            workspace_path: project.path,
            title,
            created_at: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
            status,
            files_count: inventory?.files.length,
          },
          ...prev,
        ];
      });
    }
  }, [activeTaskId, project, activeChatTitle, inventory, systemStatus?.task_state]);

  // Milestone 3 Permissions, Execution, and Diagnostics State
  const [pendingCommandApproval, setPendingCommandApproval] = useState<CommandRequest | null>(null);
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

  // Step 3 Offer-Based Git and .gitignore State
  const [isGitInitDeclined, setIsGitInitDeclined] = useState(false);
  const [isGitignoreDeclined, setIsGitignoreDeclined] = useState(false);
  const [gitignoreAddedNotice, setGitignoreAddedNotice] = useState<string | null>(null);
  const [isUnbornHeadDeclined, setIsUnbornHeadDeclined] = useState(false);
  const [initialCommitNotice, setInitialCommitNotice] = useState<string | null>(null);

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
        ollama_status_text: 'Ollama ready with granite4.1:8b',
        opencode_health: 'NotStarted',
        task_state: 'Idle',
        request_count: 0,
      });
    }
  }, []);

  // Global Keyboard Shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.altKey) {
        const k = typeof e.key === 'string' ? e.key.toLowerCase() : '';
        if (k === 'r') {
          e.preventDefault();
          setIsRightPanelOpen((prev) => !prev);
        } else if (k === 'b') {
          e.preventDefault();
          setIsSidebarCollapsed((prev) => !prev);
        } else if (k === 'q') {
          e.preventDefault();
          setIsQuotaModalOpen((prev) => !prev);
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

    // Step 4: Restore last opened workspace and its chat list automatically on launch
    listWorkspaces()
      .then(async (records) => {
        if (records && records.length > 0) {
          setRecentWorkspaces(records.map((r) => r.path));
          const lastWorkspace = records[0].path;
          try {
            await handleOpenProject(lastWorkspace);
          } catch (e) {
            console.warn('Could not restore last workspace on launch:', e);
          }
        }
      })
      .catch((err) => console.warn('Could not list workspaces from SQLite:', err));

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
          // Automatically open right panel when inventory is received
          setIsRightPanelOpen(true);
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

  // Load stored chats from SQLite for current workspace
  const loadStoredChats = useCallback(async (workspacePath: string) => {
    try {
      const records = await listChats(workspacePath);
      const summaries: StoredChatSummary[] = records.map((r) => ({
        id: r.id,
        workspace_path: r.workspace_path,
        title: r.title,
        created_at: new Date(r.created_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
        status: r.status as StoredChatSummary['status'],
        files_count: r.files_count,
      }));
      setStoredChats(summaries);
    } catch (err) {
      console.warn('Could not load stored chats from SQLite:', err);
    }
  }, []);

  // Handler: Select and restore a previous chat
  const handleSelectChat = async (taskId: string) => {
    setIsLoading(true);
    setError(null);
    try {
      const detail = await loadChat(taskId);
      if (!detail) return;

      setActiveTaskId(detail.id);
      setActiveChatTitle(detail.title);

      // Restore plan if present
      if (detail.plan_json) {
        try {
          const parsedPlan = JSON.parse(detail.plan_json);
          setPlan(parsedPlan);
        } catch {
          setPlan(null);
        }
      } else {
        setPlan(null);
      }

      // Restore worker events scrollback
      if (detail.events_json) {
        try {
          const parsedEvents = JSON.parse(detail.events_json);
          setWorkerEvents(parsedEvents);
        } catch {
          setWorkerEvents([]);
        }
      } else {
        setWorkerEvents([]);
      }

      // Restore verification runs
      if (detail.verification_json) {
        try {
          const parsedVer = JSON.parse(detail.verification_json);
          setVerificationHistory(parsedVer);
        } catch {
          setVerificationHistory([]);
        }
      } else {
        setVerificationHistory([]);
      }

      // Restore exact diff state from authoritative Git worktree
      if (detail.status === 'review' || detail.status === 'active' || detail.worktree_path) {
        try {
          const inv = await getChangeInventory(detail.id);
          if (inv && inv.files.length > 0) {
            setInventory(inv);
            setIsRightPanelOpen(true);
          } else {
            setInventory(null);
          }
        } catch {
          setInventory(null);
        }
      } else {
        setInventory(null);
      }
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to restore chat');
    } finally {
      setIsLoading(false);
    }
  };

  // Handler: Open Project
  const handleOpenProject = async (path: string) => {
    setIsLoading(true);
    setError(null);
    try {
      const proj = await openProject(path);
      setProject(proj);
      setRecentWorkspaces((prev) => {
        const updated = [path, ...prev.filter((p) => p !== path)].slice(0, 8);
        try {
          localStorage.setItem('zencode_recent_workspaces', JSON.stringify(updated));
        } catch {}
        return updated;
      });
      await refreshStatus();
      await loadStoredChats(path);
      setIsGitInitDeclined(false);
      setIsGitignoreDeclined(false);
      setGitignoreAddedNotice(null);
      setIsUnbornHeadDeclined(false);
      setInitialCommitNotice(null);

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

  // Step 3: Handle explicit per-action Git Init
  const handleInitGitRepo = async () => {
    if (!project) return;
    setIsLoading(true);
    setError(null);
    try {
      const updated = await initGitRepo(project.path);
      setProject(updated);
      setIsGitInitDeclined(false);
      await refreshStatus();
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to initialize Git repository');
    } finally {
      setIsLoading(false);
    }
  };

  // Step 3: Handle explicit per-action .gitignore creation
  const handleCreateGitignore = async () => {
    if (!project) return;
    setIsLoading(true);
    setError(null);
    try {
      const msg = await createStandardGitignore(project.path, project.project_type);
      setGitignoreAddedNotice(msg);
      setProject((prev) => (prev ? { ...prev, has_gitignore: true } : null));
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to create standard .gitignore');
    } finally {
      setIsLoading(false);
    }
  };

  // Step 3: Handle explicit per-action initial empty commit on unborn/0-commit repo
  const handleCreateInitialCommit = async () => {
    if (!project) return;
    setIsLoading(true);
    setError(null);
    try {
      const updated = await createInitialEmptyCommit(project.path);
      setProject(updated);
      setIsUnbornHeadDeclined(false);
      setInitialCommitNotice("Created initial empty commit. Repository is ready for isolated task branches.");
      await refreshStatus();
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to create initial commit');
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
      if (project) {
        await saveChat({
          id: tid,
          workspace_path: project.path,
          title: activeChatTitle,
          status: 'active',
          branch_name: `zencode/task-${tid}`,
          worktree_path: null,
          plan_json: JSON.stringify(planToExecute),
          worker_summary: null,
          events_json: null,
          verification_json: null,
        });
        await loadStoredChats(project.path);
      }
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
      } else if (err?.error_type === 'UNBORN_HEAD') {
        setIsUnbornHeadDeclined(false);
        setError('This repository has no commits yet. Create an initial commit to proceed.');
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
      if (pendingAcceptTaskId) {
        const tid = pendingAcceptTaskId;
        setPendingAcceptTaskId(null);
        await handleAcceptTask(tid, false);
      } else if (pendingPlanForDirty) {
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
      if (pendingAcceptTaskId) {
        const tid = pendingAcceptTaskId;
        setPendingAcceptTaskId(null);
        await handleAcceptTask(tid, false);
      } else if (pendingPlanForDirty) {
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
    if (pendingAcceptTaskId) {
      const tid = pendingAcceptTaskId;
      setPendingAcceptTaskId(null);
      await handleAcceptTask(tid, true);
    } else if (pendingPlanForDirty) {
      const p = pendingPlanForDirty;
      setPendingPlanForDirty(null);
      await handleStartWorkerSession(p, undefined, true);
    }
  };

  const handleCancelDirty = () => {
    setDirtyFiles(null);
    setPendingPlanForDirty(null);
    setPendingAcceptTaskId(null);
  };

  // Handler: Accept Task Changes
  const handleAcceptTask = async (tid: string, acknowledgeDirty?: boolean): Promise<MergeResult> => {
    setIsLoading(true);
    try {
      const res = await acceptTask(tid, acknowledgeDirty);
      if ('Success' in res || (res as any).status === 'Success') {
        if (project) {
          await saveChat({
            id: tid,
            workspace_path: project.path,
            title: activeChatTitle,
            status: 'merged',
            branch_name: null,
            worktree_path: null,
            plan_json: plan ? JSON.stringify(plan) : null,
            worker_summary: null,
            events_json: workerEvents.length > 0 ? JSON.stringify(workerEvents) : null,
            verification_json: verificationHistory.length > 0 ? JSON.stringify(verificationHistory) : null,
          });
          await loadStoredChats(project.path);
        }
        setInventory(null);
        setActiveTaskId(null);
        setIsRightPanelOpen(false);
        await refreshStatus();
      }
      return res;
    } catch (err: any) {
      if (err?.error_type === 'DIRTY_PRIMARY_TREE' || err?.uncommitted_files) {
        setDirtyFiles(err.uncommitted_files || []);
        setPendingAcceptTaskId(tid);
      }
      throw err;
    } finally {
      setIsLoading(false);
    }
  };

  // Handler: Reject Task Changes
  const handleRejectTask = async (tid: string): Promise<void> => {
    setIsLoading(true);
    try {
      await rejectTask(tid);
      if (project) {
        await saveChat({
          id: tid,
          workspace_path: project.path,
          title: activeChatTitle,
          status: 'rejected',
          branch_name: null,
          worktree_path: null,
          plan_json: plan ? JSON.stringify(plan) : null,
          worker_summary: null,
          events_json: workerEvents.length > 0 ? JSON.stringify(workerEvents) : null,
          verification_json: verificationHistory.length > 0 ? JSON.stringify(verificationHistory) : null,
        });
        await loadStoredChats(project.path);
      }
      setInventory(null);
      setActiveTaskId(null);
      setIsRightPanelOpen(false);
      await refreshStatus();
    } finally {
      setIsLoading(false);
    }
  };

  // Handler: Delete Stored Chat
  const handleDeleteChat = async (tid: string) => {
    try {
      await deleteChat(tid);
      if (activeTaskId === tid) {
        handleNewChat();
      }
      if (project) {
        await loadStoredChats(project.path);
      }
    } catch (err) {
      console.warn('Failed to delete chat:', err);
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
        setIsRightPanelOpen(true);
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
    // Check quota availability before attempting to generate preview or request advice
    const isPinned = Boolean(quotaState?.pinned_model);
    const relevantModels = isPinned
      ? quotaState?.models.filter(m => m.model_id === quotaState.pinned_model)
      : quotaState?.models;
    const isQuotaExhausted = Boolean(
      relevantModels && relevantModels.length > 0 && relevantModels.every(m => !m.is_available)
    );
    if (isQuotaExhausted) {
      const reason = relevantModels?.find(m => !m.is_available)?.unavailability_reason || 'Rate limit capacity reached';
      setError(`Cannot request Gemini advisory: Free-tier quota is currently exhausted (${reason}). Please wait for the quota window to reset.`);
      return;
    }

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

      // Forward advisory guidance to the running local worker session
      if (workerSessionId) {
        await sendAdvisoryToWorker(workerSessionId, suggestion);
      }
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to obtain Gemini advisory suggestion');
    } finally {
      setIsAdvisoryLoading(false);
    }
  };

  const handleApplyAdvisoryToWorker = async (suggestionText: string) => {
    if (!workerSessionId) return;
    setIsLoading(true);
    try {
      await sendAdvisoryToWorker(workerSessionId, suggestionText);
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to dispatch advisory advice to worker');
    } finally {
      setIsLoading(false);
    }
  };

  const handleCloseAdvisory = () => {
    setIsAdvisoryModalOpen(false);
    setAdvisoryPreview(null);
  };

  const handleApproveCommand = async (req: CommandRequest) => {
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

  // New Chat / Task reset
  const handleNewChat = () => {
    setActiveTaskId(null);
    setActiveChatTitle('New Task');
    setContextPreview(null);
    setPlan(null);
    setWorkerEvents([]);
    setWorkerSessionId(null);
    setInventory(null);
    setVerificationHistory([]);
    setDiagnosticNotice(null);
    setError(null);
  };

  return (
    <div className="flex h-screen w-screen flex-col bg-[#09090b] text-[#e4e4e7] overflow-hidden font-sans select-none">
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

      {/* Advisory Context Preview Modal (Step 6 Opt-In Escalation) */}
      {advisoryPreview && (() => {
        const isPinned = Boolean(quotaState?.pinned_model);
        const relevantModels = isPinned
          ? quotaState?.models.filter(m => m.model_id === quotaState.pinned_model)
          : quotaState?.models;
        const isQuotaExhausted = Boolean(
          relevantModels && relevantModels.length > 0 && relevantModels.every(m => !m.is_available)
        );
        const exhaustionReason = relevantModels?.find(m => !m.is_available)?.unavailability_reason || undefined;

        return (
          <AdvisoryPreviewModal
            preview={advisoryPreview}
            isOpen={isAdvisoryModalOpen}
            isLoading={isAdvisoryLoading}
            onConfirm={handleConfirmAdvisory}
            onClose={handleCloseAdvisory}
            isQuotaExhausted={isQuotaExhausted}
            exhaustionReason={exhaustionReason}
          />
        );
      })()}

      {/* Quota Dashboard Modal */}
      {isQuotaModalOpen && quotaState && (
        <div className="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="max-w-2xl w-full h-[80vh] bg-zinc-950 border border-zinc-800 rounded-xl overflow-hidden shadow-2xl flex flex-col font-mono text-xs">
            <div className="h-10 px-4 border-b border-zinc-800 flex items-center justify-between bg-zinc-900 shrink-0">
              <span className="font-semibold text-zinc-100">Rate Limits &amp; Waterfall Cascade</span>
              <button
                type="button"
                onClick={() => setIsQuotaModalOpen(false)}
                className="text-zinc-400 hover:text-white"
              >
                [close]
              </button>
            </div>
            <div className="flex-1 overflow-y-auto">
              <QuotaDashboard
                quotaState={quotaState}
                onClose={() => setIsQuotaModalOpen(false)}
                onPinModel={handlePinModel}
                isPinnedActionLoading={isPinnedLoading}
              />
            </div>
          </div>
        </div>
      )}

      {/* Main Persistent Three-Pane Shell */}
      <div className="flex-1 flex overflow-hidden relative">
        {/* Left Sidebar: Workspace switcher + Projects / Chats tree + Engine status */}
        <Sidebar
          currentWorkspace={project}
          workspaces={recentWorkspaces}
          chats={storedChats}
          activeChatId={activeTaskId}
          onSelectChat={handleSelectChat}
          onNewChat={handleNewChat}
          onSwitchWorkspace={handleOpenProject}
          onOpenProjectDialog={() => {
            const p = prompt('Enter project path:', 'A:\\ZenCode');
            if (p) handleOpenProject(p);
          }}
          status={systemStatus}
          quotaState={quotaState}
          onOpenQuota={() => setIsQuotaModalOpen(true)}
          onDeleteChat={handleDeleteChat}
          isCollapsed={isSidebarCollapsed}
          onToggleCollapse={() => setIsSidebarCollapsed(!isSidebarCollapsed)}
        />

        {/* Center Panel: Primary Chat/Agent Thread */}
        <div className="flex-1 flex flex-col h-full overflow-hidden bg-[#09090b] relative">
          {!project ? (
            <EmptyState
              status={systemStatus}
              recentWorkspaces={recentWorkspaces}
              onOpenProject={handleOpenProject}
              isLoading={isLoading}
              error={error}
            />
          ) : (
            <>
              {/* Center Panel Top Header */}
              <ChatHeader
                project={project}
                taskState={systemStatus?.task_state || 'Idle'}
                activeChatTitle={activeChatTitle}
                activePlannerModel={quotaState?.pinned_model ? '3.8 Flash' : lastActiveModel}
                isPinned={Boolean(quotaState?.pinned_model)}
                onOpenQuota={() => setIsQuotaModalOpen(true)}
                isRightPanelOpen={isRightPanelOpen}
                onToggleRightPanel={() => setIsRightPanelOpen(!isRightPanelOpen)}
                changesCount={inventory?.files.length ?? 0}
              />

              {/* Chat Thread Container */}
              <div className="flex-1 overflow-y-auto">
                <TaskView
                  project={project}
                  onInitGitRepo={handleInitGitRepo}
                  onCreateGitignore={handleCreateGitignore}
                  gitignoreAddedNotice={gitignoreAddedNotice}
                  isGitInitDeclined={isGitInitDeclined}
                  onDeclineGitInit={() => setIsGitInitDeclined(true)}
                  isGitignoreDeclined={isGitignoreDeclined}
                  onDeclineGitignore={() => setIsGitignoreDeclined(true)}
                  isUnbornHeadDeclined={isUnbornHeadDeclined}
                  onDeclineInitialCommit={() => setIsUnbornHeadDeclined(true)}
                  onCreateInitialCommit={handleCreateInitialCommit}
                  initialCommitNotice={initialCommitNotice}
                  contextPreview={contextPreview}
                  plan={plan}
                  taskState={systemStatus?.task_state || 'Idle'}
                  workerEvents={workerEvents}
                  workerSessionId={workerSessionId}
                  activePlannerModel={lastActiveModel}
                  switchReason={lastSwitchReason}
                  inventory={inventory}
                  activeTaskId={activeTaskId}
                  pendingCommandApproval={pendingCommandApproval}
                  onApproveCommand={handleApproveCommand}
                  onDenyCommand={handleDenyCommand}
                  onConfirmPlanRequest={handleConfirmPlanRequest}
                  onStartWorkerSession={handleStartWorkerSession}
                  onStopWorkerSession={handleStopWorkerSession}
                  onAcceptTask={handleAcceptTask}
                  onRejectTask={handleRejectTask}
                  onResetTask={handleNewChat}
                  verificationHistory={verificationHistory}
                  onOpenReviewPane={() => setIsRightPanelOpen(true)}
                  onTriggerCommand={handleTriggerCommand}
                  onExportDiagnostics={handleExportDiagnostics}
                  onTriggerAdvisory={handleTriggerAdvisory}
                  advisorySuggestion={advisorySuggestion}
                  onApplyAdvisoryToWorker={handleApplyAdvisoryToWorker}
                  quotaState={quotaState}
                  diagnosticNotice={diagnosticNotice}
                  isLoading={isLoading}
                  error={error}
                />

                {/* Inline Process Execution Terminal if running */}
                {isTerminalOpen && (
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
                )}
              </div>

              {/* Center Bottom Chat Input Dock */}
              <ChatInputDock
                onSubmit={(text) => {
                  if (project && !project.is_git_repo) {
                    setError('Task creation is blocked: initialize a Git repository to enable safe worktree branch isolation.');
                    return;
                  }
                  if (project && project.is_git_repo && project.has_commits === false) {
                    setError('Task creation is blocked: repository has no commits yet. Create an initial commit to enable worktree branch isolation.');
                    return;
                  }
                  if (!plan) {
                    setActiveChatTitle(text.slice(0, 32));
                    handlePreviewContext(text);
                  } else {
                    handleStartWorkerSession(plan);
                  }
                }}
                disabled={Boolean(project && (!project.is_git_repo || project.has_commits === false))}
                placeholder={
                  project && !project.is_git_repo
                    ? 'Task creation blocked: Git is required to safely isolate and review AI changes.'
                    : project && project.is_git_repo && project.has_commits === false
                    ? 'Task creation blocked: repository has no commits yet. Create an initial commit first.'
                    : undefined
                }
                isLoading={isLoading}
                activeModelName={quotaState?.pinned_model ? '3.8 Flash' : lastActiveModel}
                isPinned={Boolean(quotaState?.pinned_model)}
                onSelectModel={handlePinModel}
              />
            </>
          )}
        </div>

        {/* Right Panel: Context-Sensitive Diff & Changes Viewer / Explorer */}
        <RightPanel
          inventory={inventory}
          taskId={activeTaskId}
          onAccept={handleAcceptTask}
          onReject={handleRejectTask}
          isLoading={isLoading}
          isOpen={isRightPanelOpen}
          onClose={() => setIsRightPanelOpen(false)}
          project={project}
        />
      </div>

      {/* Sticky Bottom Status Bar (M3 Invariants Preserved) */}
      <StatusBar
        project={project}
        status={systemStatus}
        quotaState={quotaState}
        onOpenQuotaDashboard={() => setIsQuotaModalOpen(true)}
      />
    </div>
  );
}
