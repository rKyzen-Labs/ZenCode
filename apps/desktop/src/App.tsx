import React, { useState, useEffect, useCallback, useRef } from 'react';
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
  CommandRequest,
  ProcessOutputEvent,
  ExecutionResult,
  AdvisoryPreview,
  ChatSummaryRecord,
  DependencyStatus,
  ChatMessage,
} from './types';
import { isTaskState } from './types';
import {
  openProject,
  getProjectState,
  verifyStartupDependencies,
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
  undoTaskChanges,
  subscribeToInventoryReady,
  subscribeToChatMessageAdded,
  requestCommandApproval,
  subscribeToCommandApproval,
  approveAndRunCommand,
  cancelCommandExecution,
  previewAdvisoryContext,
  submitAdvisoryRequest,
  sendAdvisoryToWorker,
  subscribeToExecOutput,
  subscribeToExecFinished,
  listWorkspaces,
  isWorkspaceContextPreviewTrusted,
  setWorkspaceContextPreviewTrusted,
  saveChat,
  loadChat,
  addChatMessage,
  getChatMessages,
  getChangeInventory,
  initGitRepo,
  createInitialEmptyCommit,
  getGeminiKeyStatus,
} from './lib/ipc';
import { TopBar, type ChatTabItem } from './components/TopBar';
import { FileTreePane } from './components/FileTreePane';
import { LandingPage } from './components/LandingPage';
import { ChatThread } from './components/ChatThread';
import { SearchPage } from './components/SearchPage';
import { SettingsPage } from './components/SettingsPage';
import { OnboardingWizard } from './components/OnboardingWizard';
import { AdvisoryPreviewModal } from './components/AdvisoryPreviewModal';
import { ShortcutsModal } from './components/ShortcutsModal';
import { loadKeybindings, matchesKeybinding } from './lib/keybindings';

export default function App() {
  // Navigation View State: 'landing' | 'chat' | 'search' | 'settings'
  const [activeView, setActiveView] = useState<'landing' | 'chat' | 'search' | 'settings'>('landing');

  // Step 3 Onboarding Wizard State
  const [isOnboardingOpen, setIsOnboardingOpen] = useState(false);
  const [onboardingStep, setOnboardingStep] = useState<1 | 2 | 3 | 4>(1);

  // Keyboard Shortcuts Modal State
  const [isShortcutsModalOpen, setIsShortcutsModalOpen] = useState(false);

  // Open Chat Sessions (TitleBar Tabs)
  const [openChatTabs, setOpenChatTabs] = useState<ChatTabItem[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const activeChatIdRef = useRef<string | null>(null);
  useEffect(() => {
    activeChatIdRef.current = activeChatId;
  }, [activeChatId]);

  // Persistent Chat Messages for Active Session
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);

  // Project / Workspace State
  const [project, setProject] = useState<ProjectState | null>(null);

  // File Tree Right Pane Toggle & Width State (Persisted per workspace)
  const [isFilePaneOpen, setIsFilePaneOpen] = useState<boolean>(() => {
    try {
      const saved = localStorage.getItem('zencode_file_pane_open');
      return saved !== null ? JSON.parse(saved) : true;
    } catch {
      return true;
    }
  });

  const [filePaneWidth, setFilePaneWidth] = useState<number>(() => {
    try {
      const saved = localStorage.getItem('zencode_file_pane_width');
      return saved ? Math.max(200, Math.min(parseInt(saved, 10), 700)) : 280;
    } catch {
      return 280;
    }
  });

  // Toggle file pane and persist
  const handleToggleFilePane = () => {
    setIsFilePaneOpen((prev) => {
      const next = !prev;
      try {
        localStorage.setItem('zencode_file_pane_open', JSON.stringify(next));
      } catch {}
      return next;
    });
  };

  // Horizontal resizing for right file pane
  const handleResizeStart = (e: React.MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = filePaneWidth;

    const handleMouseMove = (moveEvent: MouseEvent) => {
      // Dragging cursor to the left expands the right pane
      const delta = startX - moveEvent.clientX;
      const newWidth = Math.max(200, Math.min(startWidth + delta, window.innerWidth * 0.6));
      setFilePaneWidth(newWidth);
    };

    const handleMouseUp = () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      try {
        localStorage.setItem('zencode_file_pane_width', String(filePaneWidth));
      } catch {}
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  };

  // System & Engine Status
  const [systemStatus, setSystemStatus] = useState<SystemStatus | null>(null);
  const [quotaState, setQuotaState] = useState<QuotaUpdateEvent | null>(null);
  const [lastActiveModel, setLastActiveModel] = useState<string>('gemini-3.5-flash-lite');

  // Startup Dependency Check State
  const [isStartupVerifying, setIsStartupVerifying] = useState<boolean>(true);
  const [dependencyIssues, setDependencyIssues] = useState<DependencyStatus[]>([]);

  // Fix 3: Git Auto-init Notice
  const [gitAutoInitNotice, setGitAutoInitNotice] = useState<string | null>(null);

  // Task & Planning State
  const [activeChatTitle, setActiveChatTitle] = useState<string>('New Task');
  const [activePromptText, setActivePromptText] = useState<string>('');
  const [isPlanning, setIsPlanning] = useState<boolean>(false);
  const [planningElapsedSeconds, setPlanningElapsedSeconds] = useState<number>(0);
  const [contextPreview, setContextPreview] = useState<ContextPreview | null>(null);
  const [plan, setPlan] = useState<ZencodePlan | null>(null);
  const [workerEvents, setWorkerEvents] = useState<WorkerEventPayload[]>([]);
  const [workerSessionId, setWorkerSessionId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Milestone 2 Git Isolation & Auto-Apply / Undo State
  const [inventory, setInventory] = useState<ChangeInventory | null>(null);
  const [activeTaskId, setActiveTaskId] = useState<string | null>(null);
  const activeTaskIdRef = useRef<string | null>(null);
  useEffect(() => {
    activeTaskIdRef.current = activeTaskId;
  }, [activeTaskId]);

  // Open file diff in the right file reader pane
  const [openDiffRequest, setOpenDiffRequest] = useState<{
    path: string;
    diffContent: string;
    additions?: number;
    deletions?: number;
  } | null>(null);
  const [isUndone, setIsUndone] = useState<boolean>(false);

  // Milestone 3 Permissions, Execution, and Diagnostics State
  const [pendingCommandApproval, setPendingCommandApproval] = useState<CommandRequest | null>(null);
  const [requestedPaneTab, setRequestedPaneTab] = useState<string | null>(null);
  const [activeCommandId, setActiveCommandId] = useState<string | null>(null);
  const [activeProgram, setActiveProgram] = useState<string>('');
  const [activeArgs, setActiveArgs] = useState<string[]>([]);
  const [activeCwd, setActiveCwd] = useState<string>('');
  const [isCommandRunning, setIsCommandRunning] = useState(false);
  const [commandOutputLines, setCommandOutputLines] = useState<ProcessOutputEvent[]>([]);
  const [executionResult, setExecutionResult] = useState<ExecutionResult | null>(null);
  const [verificationHistory, setVerificationHistory] = useState<ExecutionResult[]>([]);

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

  // Subscribe to real-time events on mount
  useEffect(() => {
    refreshStatus();

    // Fix 4: Startup Dependency Verification
    verifyStartupDependencies()
      .then((res) => {
        const issues = res.dependencies.filter((d) => !d.is_ready);
        setDependencyIssues(issues);
      })
      .catch((err) => console.warn('Could not verify startup dependencies:', err))
      .finally(() => setIsStartupVerifying(false));

    // Request desktop notification permission on launch (Step 8)
    if ('Notification' in window && Notification.permission === 'default') {
      Notification.requestPermission().catch(() => {});
    }

    // Fetch initial rate-limit telemetry
    getQuotaState()
      .then((qs) => {
        setQuotaState(qs);
        if (qs.active_model) {
          setLastActiveModel(qs.active_model);
        }
      })
      .catch((err) => console.warn('Could not fetch initial quota state:', err));

    // Restore last opened workspace on launch
    listWorkspaces()
      .then(async (records) => {
        if (records && records.length > 0) {
          const lastWorkspace = records[0].path;
          try {
            await handleOpenProject(lastWorkspace);
          } catch (e) {
            console.warn('Could not restore last workspace on launch:', e);
          }
        }
      })
      .catch((err) => console.warn('Could not list workspaces from SQLite:', err));

    // First-run onboarding check
    getGeminiKeyStatus()
      .then((status) => {
        const completed = localStorage.getItem('zencode_onboarding_completed') === 'true';
        if (!status?.has_key && !completed) {
          setIsOnboardingOpen(true);
        }
      })
      .catch((err) => console.warn('Could not check onboarding initial status:', err));

    let unlistenWorkerMsg: (() => void) | undefined;
    let unlistenWorkerStatus: (() => void) | undefined;
    let unlistenEngineHealth: (() => void) | undefined;
    let unlistenQuotaUpdate: (() => void) | undefined;
    let unlistenInventoryReady: (() => void) | undefined;
    let unlistenChatMessageAdded: (() => void) | undefined;
    let unlistenExecOutput: (() => void) | undefined;
    let unlistenExecFinished: (() => void) | undefined;
    let unlistenCommandApproval: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        unlistenCommandApproval = await subscribeToCommandApproval((req: CommandRequest) => {
          console.log('[App] Received command approval request:', req);
          setPendingCommandApproval(req);
        });
        unlistenWorkerMsg = await subscribeToWorkerMessages((payload) => {
          setWorkerEvents((prev) => [...prev, payload]);
        });

        unlistenWorkerStatus = await subscribeToWorkerStatus(async (taskState: TaskState) => {
          setSystemStatus((prev) => (prev ? { ...prev, task_state: taskState } : null));
          if (!isTaskState(taskState, 'WorkerRunning')) {
            setWorkerSessionId(null);
            setIsLoading(false);
          }
          if (isTaskState(taskState, 'Complete')) {
            const currentChatId = activeChatIdRef.current;
            if (currentChatId) {
              try {
                const msgs = await getChatMessages(currentChatId);
                setChatMessages(msgs || []);
              } catch (e) {
                console.warn('Could not refresh messages on task complete:', e);
              }
            }
          }
          if (isTaskState(taskState, 'Failed')) {
            const errMsg = typeof taskState === 'object' && 'Failed' in taskState
              ? (taskState as any).Failed
              : typeof taskState === 'object' && 'detail' in taskState
              ? (taskState as any).detail
              : 'Worker execution terminated unexpectedly';
            setError(errMsg);
          }
        });

        unlistenChatMessageAdded = await subscribeToChatMessageAdded((msg: ChatMessage) => {
          if (msg.chat_id === activeChatIdRef.current) {
            setChatMessages((prev) => {
              if (prev.some((m) => m.id === msg.id)) {
                return prev.map((m) => (m.id === msg.id ? msg : m));
              }
              return [...prev, msg];
            });
          }
        });

        unlistenEngineHealth = await subscribeToEngineHealth((health: EngineHealth) => {
          setSystemStatus((prev) => (prev ? { ...prev, opencode_health: health } : null));
          if (typeof health === 'object' && 'Failed' in health) {
            setError((health as any).Failed || 'OpenCode worker engine stopped or crashed');
            setWorkerSessionId(null);
            setIsLoading(false);
          }
        });

        unlistenQuotaUpdate = await subscribeToQuotaUpdates((qs: QuotaUpdateEvent) => {
          setQuotaState(qs);
          if (qs.active_model) {
            setLastActiveModel(qs.active_model);
          }
        });

        unlistenInventoryReady = await subscribeToInventoryReady(async (inv: ChangeInventory) => {
          setInventory(inv);
          setWorkerSessionId(null);

          // Desktop notification on completion
          if ('Notification' in window && Notification.permission === 'granted') {
            try {
              new Notification('ZenCode Task Complete', {
                body: `${inv.files.length} file(s) updated (+${inv.total_additions}/-${inv.total_deletions})`,
              });
            } catch {}
          }

          const currentId = activeChatIdRef.current || activeTaskIdRef.current;
          if (currentId) {
            try {
              const msgs = await getChatMessages(currentId);
              setChatMessages(msgs || []);
            } catch (e) {
              console.warn('Could not refresh messages on inventory ready:', e);
            }
            await refreshStatus();
          }
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
      unlistenChatMessageAdded?.();
      unlistenEngineHealth?.();
      unlistenQuotaUpdate?.();
      unlistenInventoryReady?.();
      unlistenExecOutput?.();
      unlistenExecFinished?.();
      unlistenCommandApproval?.();
    };
  }, [refreshStatus]);

  // Live real-time planning duration timer
  useEffect(() => {
    let timer: any = null;
    if (isPlanning) {
      setPlanningElapsedSeconds(1);
      const start = Date.now();
      timer = setInterval(() => {
        setPlanningElapsedSeconds(Math.max(1, Math.floor((Date.now() - start) / 1000)));
      }, 1000);
    } else {
      setPlanningElapsedSeconds(0);
    }
    return () => {
      if (timer) clearInterval(timer);
    };
  }, [isPlanning]);

  // Handler: Pin / Unpin model
  const handlePinModel = async (modelId: string | null) => {
    try {
      const updated = await setPinnedModel(modelId);
      setQuotaState(updated);
      setLastActiveModel(updated.active_model);
    } catch (err: any) {
      console.error('Failed to pin model:', err);
    }
  };

  // Handler: Select and restore a previous chat
  const handleSelectChat = async (taskId: string) => {
    setIsLoading(true);
    setError(null);
    try {
      const detail = await loadChat(taskId);
      if (!detail) return;

      activeChatIdRef.current = detail.id;
      activeTaskIdRef.current = detail.id;
      setActiveTaskId(detail.id);
      setActiveChatId(detail.id);
      setActiveChatTitle(detail.title);
      setActivePromptText(detail.title || '');

      try {
        const msgs = await getChatMessages(detail.id);
        if (msgs && msgs.length > 0) {
          if (!msgs.some((m) => m.role === 'user') && detail.title) {
            msgs.unshift({
              id: `msg_init_${detail.id}`,
              chat_id: detail.id,
              role: 'user',
              content: detail.title,
              plan_json: null,
              worker_summary: null,
              files_changed_json: null,
              created_at: detail.created_at || new Date().toISOString(),
              commit_hash: null,
            });
          }
          setChatMessages(msgs);
        } else if (detail.title) {
          setChatMessages([
            {
              id: `msg_init_${detail.id}`,
              chat_id: detail.id,
              role: 'user',
              content: detail.title,
              plan_json: null,
              worker_summary: null,
              files_changed_json: null,
              created_at: detail.created_at || new Date().toISOString(),
              commit_hash: null,
            },
          ]);
        } else {
          setChatMessages([]);
        }
      } catch {
        if (detail.title) {
          setChatMessages([
            {
              id: `msg_init_${detail.id}`,
              chat_id: detail.id,
              role: 'user',
              content: detail.title,
              plan_json: null,
              worker_summary: null,
              files_changed_json: null,
              created_at: detail.created_at || new Date().toISOString(),
              commit_hash: null,
            },
          ]);
        } else {
          setChatMessages([]);
        }
      }

      if (detail.plan_json) {
        try {
          setPlan(JSON.parse(detail.plan_json));
        } catch {
          setPlan(null);
        }
      } else {
        setPlan(null);
      }

      if (detail.events_json) {
        try {
          setWorkerEvents(JSON.parse(detail.events_json));
        } catch {
          setWorkerEvents([]);
        }
      } else {
        setWorkerEvents([]);
      }

      if (detail.verification_json) {
        try {
          setVerificationHistory(JSON.parse(detail.verification_json));
        } catch {
          setVerificationHistory([]);
        }
      } else {
        setVerificationHistory([]);
      }

      if (detail.status === 'review' || detail.status === 'active' || detail.worktree_path) {
        try {
          const inv = await getChangeInventory(detail.id);
          if (inv && inv.files.length > 0) {
            setInventory(inv);
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

  // Handler: Open Project / Workspace
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

  const handleSelectWorkspaceDialog = () => {
    const p = prompt('Enter workspace folder path:', project?.path || 'A:\\ZenCode');
    if (p && p.trim()) {
      handleOpenProject(p.trim());
    }
  };

  // Planning & Worker Session Handlers
  const handlePreviewContext = async (taskDescription: string, chatId?: string | null) => {
    setIsLoading(true);
    setError(null);
    setIsUndone(false);
    setContextPreview(null);
    setPlan(null);
    setWorkerEvents([]);
    setWorkerSessionId(null);
    setInventory(null);
    setActivePromptText(taskDescription);

    const targetChatId = chatId || activeChatIdRef.current;
    if (targetChatId) {
      activeChatIdRef.current = targetChatId;
      activeTaskIdRef.current = targetChatId;
    }

    try {
      const preview = await previewPlanningContext(taskDescription, targetChatId);

      // Context preview — trust once per workspace
      const isTrusted = project?.path
        ? await isWorkspaceContextPreviewTrusted(project.path)
        : false;

      if (isTrusted) {
        // Workspace is trusted: proceed directly to planning with zero prompts!
        setContextPreview(null);
        await handleConfirmPlanRequest(preview.preview_id, targetChatId);
      } else {
        // Fix 5: First time in workspace: show preview inline for transparency as message sends,
        // and proceed automatically into planning without requiring a second explicit button click!
        setContextPreview(preview);
        await handleConfirmPlanRequest(preview.preview_id, targetChatId);
      }
    } catch (err: any) {
      console.error('Error previewing context:', err);
      setError(typeof err === 'string' ? err : err.message || 'Failed to assemble planning context');
    } finally {
      setIsLoading(false);
    }
  };

  const handleConfirmPlanRequest = async (previewId: string, explicitChatId?: string | null) => {
    const planStartTime = Date.now();
    setIsPlanning(true);
    setIsLoading(true);
    setError(null);
    try {
      // Store trust for this workspace so future tasks proceed automatically
      if (project?.path) {
        try {
          await setWorkspaceContextPreviewTrusted(project.path, true);
        } catch (e) {
          console.warn('Could not persist workspace context preview trust:', e);
        }
      }

      const result = await submitPlanRequest(previewId, true);
      const measuredSeconds = Math.max(1, Math.round((Date.now() - planStartTime) / 1000));
      if (!result.plan.planning_duration_seconds) {
        result.plan.planning_duration_seconds = measuredSeconds;
      }
      setPlan(result.plan);
      if (result.active_model) {
        setLastActiveModel(result.active_model);
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

      const tid = explicitChatId || activeChatIdRef.current || activeTaskIdRef.current || `task_${Date.now()}`;
      activeChatIdRef.current = tid;
      activeTaskIdRef.current = tid;
      setActiveTaskId(tid);
      setActiveChatId(tid);

      // Fix 2: Check if this is an actionable coding task vs conversational message / question / small talk
      if (result.plan.is_actionable === false) {
        // Non-actionable conversational message:
        // Do NOT create a worktree, do NOT start an OpenCode session, do NOT touch Git at all!
        const reply = result.plan.conversational_reply || result.plan.task_summary || '';
        try {
          const asstMsg = await addChatMessage(tid, 'assistant', reply, JSON.stringify(result.plan), reply, []);
          setChatMessages((prev) => [...prev, asstMsg]);
        } catch (e) {
          console.warn('Could not save assistant conversational message to db:', e);
        }

        if (project) {
          try {
            await saveChat({
              id: tid,
              workspace_path: project.path,
              title: activeChatTitle,
              status: 'idle',
              branch_name: null,
              worktree_path: null,
              plan_json: JSON.stringify(result.plan),
              worker_summary: reply,
              events_json: null,
              verification_json: null,
            });
          } catch (e) {
            console.warn('Could not save conversational chat record:', e);
          }
        }
      } else {
        // Actionable coding task: record plan and proceed with the full existing pipeline!
        const summary = result.plan.task_summary || 'Implementation Plan';
        try {
          const asstMsg = await addChatMessage(
            tid,
            'assistant',
            summary,
            JSON.stringify(result.plan),
            undefined, // worker_summary is populated when the worker completes
            []
          );
          setChatMessages((prev) => [...prev, asstMsg]);
        } catch (e) {
          console.warn('Could not record plan message in db:', e);
        }

        await handleStartWorkerSession(result.plan, tid);
      }
    } catch (err: any) {
      console.error('Error submitting plan request:', err);
      setError(typeof err === 'string' ? err : err.message || 'Planning request failed');
    } finally {
      setIsPlanning(false);
      setIsLoading(false);
    }
  };

  const handleCancelPlanRequest = () => {
    setContextPreview(null);
  };

  // Undo task changes: Reverts changes made by a specific prompt using git revert
  const handleUndoTask = async (taskId: string, commitHash?: string, messageId?: string) => {
    setIsLoading(true);
    setError(null);
    try {
      await undoTaskChanges(taskId, commitHash, messageId);
      setIsUndone(true);
      await refreshStatus();
      if (activeChatIdRef.current) {
        const msgs = await getChatMessages(activeChatIdRef.current);
        setChatMessages(msgs);
      }
    } catch (err: any) {
      console.error('Error undoing task:', err);
      setError(typeof err === 'string' ? err : err.message || 'Failed to undo task changes');
    } finally {
      setIsLoading(false);
    }
  };

  const handleStartWorkerSession = async (
    planToExecute: ZencodePlan,
    taskId?: string,
    acknowledgeDirty?: boolean
  ) => {
    setIsLoading(true);
    setError(null);
    const tid = taskId || activeTaskId || `task_${Date.now()}`;
    try {
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
      }
      const sessionId = await startWorkerSession(planToExecute, tid, acknowledgeDirty);
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

  const handleStopWorkerSession = async (sessionId?: string) => {
    const idToStop = workerSessionId || sessionId;
    if (!idToStop) return;
    setIsLoading(true);
    try {
      await stopWorkerSession(idToStop);
      setWorkerSessionId(null);
      setSystemStatus((prev) =>
        prev ? { ...prev, task_state: 'Idle' } : null
      );
    } catch (err: any) {
      console.error('Error stopping worker session:', err);
    } finally {
      setIsLoading(false);
    }
  };

  // Command Approvals & Execution
  const handleApproveCommand = async (req: CommandRequest) => {
    setPendingCommandApproval(null);
    setActiveCommandId(req.id);
    setActiveProgram(req.program);
    setActiveArgs(req.args);
    setActiveCwd(req.cwd);
    setCommandOutputLines([]);
    setExecutionResult(null);
    setIsCommandRunning(true);
    try {
      const res = await approveAndRunCommand(req.id);
      setExecutionResult(res);
      setIsCommandRunning(false);

      // Append verified execution result message to active chat thread
      const currentChatId = activeChatIdRef.current;
      if (currentChatId) {
        const outSummary = [
          `Command finished with exit code ${res.exit_code ?? 'unknown'}:`,
          `$ ${res.program} ${res.args.join(' ')}`,
          ...(res.stdout_tail.length > 0 ? res.stdout_tail.slice(-10) : []),
          ...(res.stderr_tail.length > 0 ? res.stderr_tail.slice(-5) : []),
        ].join('\n');
        try {
          const asstMsg = await addChatMessage(
            currentChatId,
            'assistant',
            outSummary,
            null,
            `Ran ${res.program} ${res.args.join(' ')}`,
            []
          );
          setChatMessages((prev) => [...prev, asstMsg]);
        } catch (e) {
          console.warn('Could not save command result message to db:', e);
        }
      }
    } catch (err: any) {
      setIsCommandRunning(false);
      setError(typeof err === 'string' ? err : err.message || 'Failed to approve command execution');
    }
  };

  const handleDenyCommand = async (req: CommandRequest) => {
    setPendingCommandApproval(null);
    try {
      await cancelCommandExecution(req.id);
    } catch (err: any) {
      console.warn('Failed to send denial:', err);
    }
  };

  const handleCancelCommand = async (commandId: string) => {
    try {
      await cancelCommandExecution(commandId);
      setIsCommandRunning(false);
    } catch (err: any) {
      console.warn('Failed to cancel running command:', err);
    }
  };

  const handleTriggerCommand = async (
    program: string,
    args: string[],
    cwd?: string,
    description?: string
  ) => {
    try {
      setError(null);
      console.log('[App] handleTriggerCommand dispatching:', { program, args, cwd, description });
      const req = await requestCommandApproval(program, args, cwd, description);
      console.log('[App] requestCommandApproval response:', req);
      if (!req.auto_approved) {
        setPendingCommandApproval(req);
      } else {
        await handleApproveCommand(req);
      }
    } catch (err: any) {
      console.error('[App] handleTriggerCommand error:', err);
      setError(typeof err === 'string' ? err : err.message || 'Failed to request command approval');
    }
  };

  // Advisory Consultation
  const [advisoryPreview, setAdvisoryPreview] = useState<AdvisoryPreview | null>(null);
  const [isAdvisoryModalOpen, setIsAdvisoryModalOpen] = useState(false);
  const [isAdvisoryLoading, setIsAdvisoryLoading] = useState(false);
  const [advisorySuggestion, setAdvisorySuggestion] = useState<string | null>(null);

  const handleTriggerAdvisory = async (errorText: string, filePath?: string) => {
    setIsAdvisoryLoading(true);
    setError(null);
    try {
      const preview = await previewAdvisoryContext(errorText, filePath);
      setAdvisoryPreview(preview);
      setIsAdvisoryModalOpen(true);
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to assemble advisory context preview');
    } finally {
      setIsAdvisoryLoading(false);
    }
  };

  const handleConfirmAdvisory = async (previewId: string) => {
    setIsAdvisoryLoading(true);
    setError(null);
    try {
      const suggestion = await submitAdvisoryRequest(previewId, true);
      setAdvisorySuggestion(suggestion);
      setIsAdvisoryModalOpen(false);
      setAdvisoryPreview(null);
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Advisory request failed');
    } finally {
      setIsAdvisoryLoading(false);
    }
  };

  const handleCloseAdvisory = () => {
    setIsAdvisoryModalOpen(false);
    setAdvisoryPreview(null);
  };

  const handleApplyAdvisoryToWorker = async (suggestion: string) => {
    if (!workerSessionId) return;
    setIsLoading(true);
    try {
      await sendAdvisoryToWorker(workerSessionId, suggestion);
      setAdvisorySuggestion(null);
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err.message || 'Failed to deliver advisory to local worker');
    } finally {
      setIsLoading(false);
    }
  };

  // TitleBar Chat Tab Management
  const handleSelectTab = (chatId: string) => {
    setActiveChatId(chatId);
    setActiveView('chat');
    handleSelectChat(chatId);
  };

  const handleCloseTab = (chatId: string) => {
    setOpenChatTabs((prev) => {
      const nextTabs = prev.filter((t) => t.id !== chatId);
      if (activeChatId === chatId) {
        if (nextTabs.length > 0) {
          const nextActive = nextTabs[nextTabs.length - 1];
          setActiveChatId(nextActive.id);
          setActiveView('chat');
          handleSelectChat(nextActive.id);
        } else {
          setActiveChatId(null);
          setActiveView('landing');
        }
      }
      return nextTabs;
    });
  };

  const handleNewChat = () => {
    setActiveView('landing');
    setContextPreview(null);
    setPlan(null);
    setWorkerEvents([]);
    setWorkerSessionId(null);
    setInventory(null);
    setActiveTaskId(null);
    setActiveChatTitle('New Task');
    setActivePromptText('');
    setActiveChatId(null);
    setChatMessages([]);
  };

  // Global Keyboard Shortcuts (Customizable via Settings or Ctrl+/)
  useEffect(() => {
    const handleGlobalKeyDown = (e: KeyboardEvent) => {
      const bindings = loadKeybindings();

      // Escape: close shortcuts modal or dismiss panels
      if (e.key === 'Escape') {
        if (isShortcutsModalOpen) {
          setIsShortcutsModalOpen(false);
          e.preventDefault();
          return;
        }
      }

      // Help modal: Ctrl + /
      if (matchesKeybinding(e, bindings.shortcuts_help || 'ctrl+/')) {
        e.preventDefault();
        setIsShortcutsModalOpen((prev) => !prev);
        return;
      }

      // New Chat: Ctrl + T
      if (matchesKeybinding(e, bindings.new_chat || 'ctrl+t')) {
        e.preventDefault();
        handleNewChat();
        return;
      }

      // Close Tab: Ctrl + W
      if (matchesKeybinding(e, bindings.close_tab || 'ctrl+w')) {
        e.preventDefault();
        if (activeChatIdRef.current) {
          handleCloseTab(activeChatIdRef.current);
        }
        return;
      }

      // Next Tab: Ctrl + Tab
      if (matchesKeybinding(e, bindings.next_tab || 'ctrl+tab')) {
        e.preventDefault();
        if (openChatTabs.length > 1) {
          const currentIndex = openChatTabs.findIndex((t) => t.id === activeChatIdRef.current);
          const nextIndex = (currentIndex + 1) % openChatTabs.length;
          handleSelectTab(openChatTabs[nextIndex].id);
        }
        return;
      }

      // Previous Tab: Ctrl + Shift + Tab
      if (matchesKeybinding(e, bindings.prev_tab || 'ctrl+shift+tab')) {
        e.preventDefault();
        if (openChatTabs.length > 1) {
          const currentIndex = openChatTabs.findIndex((t) => t.id === activeChatIdRef.current);
          const prevIndex = (currentIndex - 1 + openChatTabs.length) % openChatTabs.length;
          handleSelectTab(openChatTabs[prevIndex].id);
        }
        return;
      }

      // Toggle Files/Workspace Pane: Ctrl + B
      if (matchesKeybinding(e, bindings.toggle_files_pane || 'ctrl+b')) {
        e.preventDefault();
        handleToggleFilePane();
        return;
      }

      // Toggle Terminal Tab in Pane: Ctrl + `
      if (matchesKeybinding(e, bindings.toggle_terminal || 'ctrl+`')) {
        e.preventDefault();
        setIsFilePaneOpen(true);
        setRequestedPaneTab('terminal');
        return;
      }

      // Search / Command Palette: Ctrl + K
      if (matchesKeybinding(e, bindings.open_search || 'ctrl+k')) {
        e.preventDefault();
        setActiveView('search');
        return;
      }

      // Settings: Ctrl + ,
      if (matchesKeybinding(e, bindings.open_settings || 'ctrl+,')) {
        e.preventDefault();
        setActiveView('settings');
        return;
      }

      // Focus prompt input: Ctrl + L
      if (matchesKeybinding(e, bindings.focus_prompt || 'ctrl+l')) {
        e.preventDefault();
        const textarea = document.querySelector('textarea') as HTMLTextAreaElement | null;
        if (textarea) {
          textarea.focus();
        }
        return;
      }

      // Direct tab index switching: Ctrl + 1..9
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key >= '1' && e.key <= '9') {
        const index = parseInt(e.key, 10) - 1;
        if (index < openChatTabs.length) {
          e.preventDefault();
          handleSelectTab(openChatTabs[index].id);
        }
      }
    };

    window.addEventListener('keydown', handleGlobalKeyDown);
    return () => {
      window.removeEventListener('keydown', handleGlobalKeyDown);
    };
  }, [openChatTabs, isShortcutsModalOpen]);

// Shell command detector helper: recognizes $ prefix, direct CLI tools, or natural language run requests
function parseShellCommand(input: string): { program: string; args: string[] } | null {
  const trimmed = input.trim();
  if (!trimmed || trimmed.includes('\n')) return null;

  const knownBinaries = new Set([
    'pip', 'pip3', 'cargo', 'npm', 'npx', 'pnpm', 'yarn', 'git', 'python',
    'python3', 'node', 'pytest', 'make', 'docker', 'cmd', 'echo', 'poetry',
    'uv', 'pipenv', 'flask', 'ollama'
  ]);

  const tokenize = (cmdStr: string): string[] => {
    const tokens: string[] = [];
    let current = '';
    let inQuote: string | null = null;
    for (let i = 0; i < cmdStr.length; i++) {
      const char = cmdStr[i];
      if (inQuote) {
        if (char === inQuote) {
          inQuote = null;
        } else {
          current += char;
        }
      } else if (char === '"' || char === "'") {
        inQuote = char;
      } else if (/\s/.test(char)) {
        if (current.length > 0) {
          tokens.push(current);
          current = '';
        }
      } else {
        current += char;
      }
    }
    if (current.length > 0) tokens.push(current);
    return tokens;
  };

  // 1. Direct command or $ command
  const startsWithDollar = trimmed.startsWith('$');
  const directCandidate = startsWithDollar ? trimmed.replace(/^\$\s*/, '') : trimmed;
  const directTokens = tokenize(directCandidate);
  if (directTokens.length > 0) {
    const firstProg = directTokens[0].toLowerCase();
    if (startsWithDollar || knownBinaries.has(firstProg)) {
      return {
        program: directTokens[0],
        args: directTokens.slice(1),
      };
    }
  }

  // 2. Natural language wrapper (e.g. 'In this project, run pip install requests for me.')
  const naturalPattern = /(?:run|execute|install|start|launch)\s+(?:the\s+command\s+|command\s+)?([`'"]?[a-zA-Z0-9_\-./\\]+[^\n]+)/i;
  const match = naturalPattern.exec(trimmed);
  if (match) {
    let candidate = match[1].replace(/[`'"]/g, '').trim();
    candidate = candidate.replace(/\s+for\s+me[.!?;]*$/i, '');
    candidate = candidate.replace(/\s+please[.!?;]*$/i, '');
    candidate = candidate.replace(/[!?;]+$/, '');
    candidate = candidate.replace(/\.$/, '');

    const candidateTokens = tokenize(candidate);
    if (candidateTokens.length > 0 && knownBinaries.has(candidateTokens[0].toLowerCase())) {
      return {
        program: candidateTokens[0],
        args: candidateTokens.slice(1),
      };
    }

    for (const bin of knownBinaries) {
      const idx = candidate.toLowerCase().indexOf(`${bin} `);
      if (idx !== -1) {
        const subTokens = tokenize(candidate.slice(idx));
        if (subTokens.length > 0 && subTokens[0].toLowerCase() === bin) {
          return {
            program: subTokens[0],
            args: subTokens.slice(1),
          };
        }
      }
    }
  }

  return null;
}

  // Landing Page Submissions (Start New Chat Session)
  const handleLandingSubmitPrompt = async (promptText: string) => {
    // Auto-manage Git: auto-init repo if missing, or auto-create initial commit if 0-commit repo
    if (project && !project.is_git_repo) {
      try {
        const updated = await initGitRepo(project.path);
        setProject(updated);
        setGitAutoInitNotice('Initialized a new Git repository for this workspace');
        await refreshStatus();
      } catch (e) {
        console.warn('Failed to auto-init git:', e);
      }
    } else if (project && project.is_git_repo && !project.has_commits) {
      try {
        const updated = await createInitialEmptyCommit(project.path);
        setProject(updated);
        await refreshStatus();
      } catch (e) {
        console.warn('Failed to auto-create initial commit:', e);
      }
    } else {
      setGitAutoInitNotice(null);
    }

    const newChatId = `task_${Date.now()}`;
    const title = promptText.length > 35 ? promptText.slice(0, 32) + '...' : promptText;

    // Synchronously assign active refs
    activeChatIdRef.current = newChatId;
    activeTaskIdRef.current = newChatId;

    setOpenChatTabs((prev) => {
      if (prev.some((t) => t.id === newChatId)) return prev;
      return [...prev, { id: newChatId, title, workspacePath: project?.path || '' }];
    });
    setActiveChatId(newChatId);
    setActiveTaskId(newChatId);
    setActiveChatTitle(title);
    setActivePromptText(promptText);
    setActiveView('chat');

    // Pre-save chat in SQLite so parent record exists
    if (project) {
      try {
        await saveChat({
          id: newChatId,
          workspace_path: project.path,
          title,
          status: 'idle',
          branch_name: null,
          worktree_path: null,
          plan_json: null,
          worker_summary: null,
          events_json: null,
          verification_json: null,
        });
      } catch (e) {
        console.warn('Failed to pre-save chat in handleLandingSubmitPrompt:', e);
      }
    }

    const fallbackUserMessage: ChatMessage = {
      id: `msg_${Date.now()}`,
      chat_id: newChatId,
      role: 'user',
      content: promptText,
      plan_json: null,
      worker_summary: null,
      files_changed_json: null,
      created_at: new Date().toISOString(),
      commit_hash: null,
    };

    // Create user message in SQLite
    try {
      const userMsg = await addChatMessage(newChatId, 'user', promptText);
      setChatMessages([userMsg]);
    } catch (e) {
      console.warn('Failed to record initial user message:', e);
      setChatMessages([fallbackUserMessage]);
    }

    // Check if prompt is a direct shell command (e.g. pip install requests)
    const parsedCmd = parseShellCommand(promptText);
    if (parsedCmd) {
      await handleTriggerCommand(
        parsedCmd.program,
        parsedCmd.args,
        project?.path,
        `Run ${parsedCmd.program} ${parsedCmd.args.join(' ')}`
      );
      return;
    }

    await handlePreviewContext(promptText, newChatId);
  };

  // Chat Thread Submissions (Follow-up message in active chat)
  const handleChatThreadSubmitPrompt = async (promptText: string) => {
    const currentChatId = activeChatIdRef.current || `task_${Date.now()}`;
    activeChatIdRef.current = currentChatId;
    activeTaskIdRef.current = currentChatId;
    setActiveChatId(currentChatId);
    setActiveTaskId(currentChatId);
    setActivePromptText(promptText);

    const fallbackUserMsg: ChatMessage = {
      id: `msg_${Date.now()}`,
      chat_id: currentChatId,
      role: 'user',
      content: promptText,
      plan_json: null,
      worker_summary: null,
      files_changed_json: null,
      created_at: new Date().toISOString(),
      commit_hash: null,
    };

    try {
      const userMsg = await addChatMessage(currentChatId, 'user', promptText);
      setChatMessages((prev) => [...prev, userMsg]);
    } catch (e) {
      console.warn('Failed to record follow-up user message:', e);
      setChatMessages((prev) => [...prev, fallbackUserMsg]);
    }

    // Check if follow-up prompt is a shell command (e.g. pip install requests or $ cargo check)
    const parsedCmd = parseShellCommand(promptText);
    if (parsedCmd) {
      await handleTriggerCommand(
        parsedCmd.program,
        parsedCmd.args,
        project?.path,
        `Run ${parsedCmd.program} ${parsedCmd.args.join(' ')}`
      );
      return;
    }

    await handlePreviewContext(promptText, currentChatId);
  };

  const handleSelectRecentChat = async (chat: ChatSummaryRecord) => {
    if (chat.workspace_path && (!project || project.path !== chat.workspace_path)) {
      await handleOpenProject(chat.workspace_path);
    }

    setOpenChatTabs((prev) => {
      if (prev.some((t) => t.id === chat.id)) return prev;
      return [...prev, { id: chat.id, title: chat.title || 'Untitled Chat', workspacePath: chat.workspace_path }];
    });

    setActiveChatId(chat.id);
    setActiveView('chat');
    await handleSelectChat(chat.id);
  };

  const isWorkerRunning =
    isTaskState(systemStatus?.task_state, 'WorkerRunning') &&
    !isTaskState(systemStatus?.task_state, 'Failed') &&
    !isTaskState(systemStatus?.task_state, 'Complete') &&
    !isTaskState(systemStatus?.task_state, 'AwaitingReview');
  const isTaskActive = isWorkerRunning || isPlanning;
  const runningChatId = isTaskActive ? (activeTaskId || activeChatId) : null;

  return (
    <div className="flex flex-col h-screen w-screen bg-[#faf9f5] text-[#141413] overflow-hidden select-none font-sans">
      {/* Title Bar (with Tabs, Home, Search, Settings on Left, Quota & Window Controls on Right) */}
      <TopBar
        activeView={activeView}
        activeChatId={activeChatId}
        openChatTabs={openChatTabs}
        onSelectTab={handleSelectTab}
        onCloseTab={handleCloseTab}
        onNewChat={handleNewChat}
        onGoHome={() => setActiveView('landing')}
        onGoSearch={() => setActiveView('search')}
        onGoSettings={() => setActiveView('settings')}
        onOpenShortcuts={() => setIsShortcutsModalOpen(true)}
        quotaState={quotaState}
        isTaskRunning={isTaskActive}
        runningChatId={runningChatId}
      />

      {/* Main Layout: Center Pane in the middle, File Tree Pane on the RIGHT side */}
      <div className="flex-1 flex flex-row overflow-hidden relative">
        {/* Center Pane: Landing, Primary Chat Thread, Search, or Settings */}
        <main className="flex-1 flex flex-col h-full overflow-hidden bg-[#faf9f5] relative min-w-0">
          {activeView === 'landing' && (
            <LandingPage
              currentProject={project}
              onOpenWorkspace={handleOpenProject}
              onSubmitPrompt={handleLandingSubmitPrompt}
              onSelectRecentChat={handleSelectRecentChat}
              selectedModel={lastActiveModel}
              onSelectModel={handlePinModel}
              onOpenOnboarding={(step) => {
                setOnboardingStep(step || 1);
                setIsOnboardingOpen(true);
              }}
              isLoading={isLoading}
              dependencyIssues={dependencyIssues}
              isStartupVerifying={isStartupVerifying}
            />
          )}

          {activeView === 'chat' && (
            <div className="flex-1 flex flex-col h-full overflow-hidden relative">
              <ChatThread
                chatId={activeChatId || 'new'}
                chatTitle={activeChatTitle}
                userPromptText={activePromptText}
                project={project}
                contextPreview={contextPreview}
                plan={plan}
                workerEvents={workerEvents}
                isWorkerRunning={
                  isTaskState(systemStatus?.task_state, 'WorkerRunning') &&
                  !isTaskState(systemStatus?.task_state, 'Failed') &&
                  !isTaskState(systemStatus?.task_state, 'Complete') &&
                  !isTaskState(systemStatus?.task_state, 'AwaitingReview')
                }
                isPlanning={isPlanning}
                planningElapsedSeconds={planningElapsedSeconds}
                inventory={inventory}
                verificationHistory={verificationHistory}
                pendingCommandApproval={pendingCommandApproval}
                onConfirmPlanRequest={handleConfirmPlanRequest}
                onCancelPlanRequest={handleCancelPlanRequest}
                onStopWorkerSession={handleStopWorkerSession}
                onApproveCommand={handleApproveCommand}
                onDenyCommand={handleDenyCommand}
                onTriggerAdvisory={handleTriggerAdvisory}
                advisorySuggestion={advisorySuggestion}
                onApplyAdvisoryToWorker={handleApplyAdvisoryToWorker}
                quotaState={quotaState}
                chatMessages={chatMessages}
                onSubmitPrompt={handleChatThreadSubmitPrompt}
                selectedModel={lastActiveModel}
                onSelectModel={handlePinModel}
                onOpenOnboarding={(step) => {
                  setOnboardingStep(step || 1);
                  setIsOnboardingOpen(true);
                }}
                onUndoTask={handleUndoTask}
                onOpenFileDiff={(file) => {
                  setOpenDiffRequest(file);
                  setIsFilePaneOpen(true);
                }}
                isUndone={isUndone}
                gitAutoInitNotice={gitAutoInitNotice}
                isLoading={isLoading}
                error={error}
                isCommandRunning={isCommandRunning}
                activeCommandProgram={activeProgram}
                activeCommandArgs={activeArgs}
                commandOutputLines={commandOutputLines}
                onOpenTerminalTab={() => {
                  setIsFilePaneOpen(true);
                  setRequestedPaneTab('terminal');
                }}
              />
            </div>
          )}

          {activeView === 'search' && (
            <SearchPage
              onSelectChat={handleSelectRecentChat}
              onSwitchWorkspace={async (path) => {
                await handleOpenProject(path);
                setActiveView('landing');
              }}
              onNewSession={handleNewChat}
              onGoSettings={() => setActiveView('settings')}
              currentWorkspacePath={project?.path}
            />
          )}

          {activeView === 'settings' && (
            <SettingsPage
              quotaState={quotaState}
              onPinModel={handlePinModel}
              onOpenWorkspace={async (path) => {
                await handleOpenProject(path);
                setActiveView('landing');
              }}
              onReviewWorktree={async (taskId) => {
                try {
                  const inv = await getChangeInventory(taskId);
                  if (inv) {
                    setInventory(inv);
                    setActiveTaskId(taskId);
                    setActiveChatTitle(`Task: ${taskId}`);
                    setActiveView('chat');
                  }
                } catch (e) {
                  console.error('Failed to load inventory for task:', e);
                }
              }}
              currentWorkspacePath={project?.path}
              onOpenOnboarding={(step) => {
                setOnboardingStep(step || 1);
                setIsOnboardingOpen(true);
              }}
            />
          )}
        </main>

        {/* Right Pane: ONLY shown when activeView === 'chat' (Step 1 Pane Visibility Scope) */}
        {activeView === 'chat' && (
          <FileTreePane
            workspacePath={project?.path || null}
            workspaceName={project?.name}
            isOpen={isFilePaneOpen}
            onToggleOpen={handleToggleFilePane}
            width={filePaneWidth}
            onResizeStart={handleResizeStart}
            onOpenWorkspaceFolder={handleSelectWorkspaceDialog}
            openDiffRequest={openDiffRequest}
            terminalState={(activeProgram || isCommandRunning || commandOutputLines.length > 0) ? {
              program: activeProgram,
              args: activeArgs,
              cwd: activeCwd,
              commandId: activeCommandId || undefined,
              outputLines: commandOutputLines,
              isRunning: isCommandRunning,
              result: executionResult,
              onCancel: () => {
                if (activeCommandId) handleCancelCommand(activeCommandId);
              },
            } : null}
            requestedTab={requestedPaneTab}
            onClearRequestedTab={() => setRequestedPaneTab(null)}
            onCloseTerminalTab={() => {
              setActiveCommandId(null);
              setActiveProgram('');
              setActiveArgs([]);
              setCommandOutputLines([]);
              setExecutionResult(null);
            }}
          />
        )}
      </div>

      {/* Milestone 3 Advisory Confirmation Modal */}
      {(() => {
        const relevantModels = quotaState?.models.filter(m =>
          m.model_id === 'gemini-3.5-flash-lite' || m.model_id === 'gemini-3.8-flash'
        );
        const isQuotaExhausted = Boolean(relevantModels && relevantModels.length > 0 && relevantModels.every(m => !m.is_available));
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

      {/* Step 3 First-Run & Settings Onboarding Wizard */}
      <OnboardingWizard
        isOpen={isOnboardingOpen}
        onClose={() => setIsOnboardingOpen(false)}
        onComplete={() => {
          setIsOnboardingOpen(false);
          try {
            localStorage.setItem('zencode_onboarding_completed', 'true');
          } catch {}
          refreshStatus();
        }}
        initialStep={onboardingStep}
      />

      {/* Global Keyboard Shortcuts Cheat Sheet Modal */}
      <ShortcutsModal
        isOpen={isShortcutsModalOpen}
        onClose={() => setIsShortcutsModalOpen(false)}
        onOpenSettings={() => setActiveView('settings')}
      />
    </div>
  );
}
