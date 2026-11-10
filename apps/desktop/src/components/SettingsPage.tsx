import { useState, useEffect, useRef, type FC } from 'react';
import {
  Cpu,
  Check,
  Eye,
  EyeOff,
  Cloud,
  Loader2,
  AlertTriangle,
  FolderPlus,
} from 'lucide-react';
import {
  listWorkspaces,
  removeWorkspace,
  setWorkspaceContextPreviewTrusted,
  listActiveWorktrees,
  recoverOrphanedWorktree,
  getGeminiKeyStatus,
  setGeminiApiKey,
  validateGeminiApiKey,
  getGroqKeyStatus,
  setGroqApiKey,
  deleteGroqApiKey,
  validateGroqApiKey,
  getWorkspaceWorkerConfig,
  setWorkspaceWorkerConfig,
  listInstalledOllamaModels,
  getWorkerModel,
  setWorkerModel,
  pickFolder,
} from '../lib/ipc';
import type {
  WorkspaceRecord,
  OrphanWorktree,
  QuotaUpdateEvent,
  GeminiKeyStatus,
  GroqKeyStatus,
  WorkspaceWorkerConfig,
  OllamaModelItem,
} from '../types';

interface SettingsPageProps {
  quotaState: QuotaUpdateEvent | null;
  onPinModel: (modelId: string | null) => Promise<void>;
  onOpenWorkspace: (path: string) => void;
  onReviewWorktree: (taskId: string) => Promise<void>;
  currentWorkspacePath?: string | null;
  onOpenOnboarding?: (step?: 1 | 2 | 3 | 4) => void;
}

type SettingsSection =
  | 'workspaces'
  | 'worktrees'
  | 'apikey'
  | 'quota'
  | 'worker'
  | 'mode';

const GROQ_MODELS = [
  { id: 'openai/gpt-oss-120b', name: 'OpenAI GPT-OSS 120B', desc: 'Highest reasoning capability (Primary)' },
  { id: 'openai/gpt-oss-20b', name: 'OpenAI GPT-OSS 20B', desc: 'High throughput, low latency (Secondary)' },
  { id: 'qwen/qwen3.8-27b', name: 'Qwen 3.8 27B', desc: 'Specialized code generation (Tertiary)' },
];

export const SettingsPage: FC<SettingsPageProps> = ({
  quotaState,
  onPinModel,
  onOpenWorkspace,
  onReviewWorktree,
  currentWorkspacePath,
  onOpenOnboarding,
}) => {
  const [activeSection, setActiveSection] = useState<SettingsSection>('workspaces');

  // Workspaces state
  const [workspaces, setWorkspaces] = useState<WorkspaceRecord[]>([]);
  const [isLoadingWorkspaces, setIsLoadingWorkspaces] = useState(false);

  // Worktrees state
  const [worktrees, setWorktrees] = useState<OrphanWorktree[]>([]);
  const [isLoadingWorktrees, setIsLoadingWorktrees] = useState(false);
  const [worktreeNotice, setWorktreeNotice] = useState<string | null>(null);

  // Gemini API Key state
  const [keyStatus, setKeyStatus] = useState<GeminiKeyStatus | null>(null);
  const [newKeyInput, setNewKeyInput] = useState('');
  const [isSavingKey, setIsSavingKey] = useState(false);
  const [isValidatingGemini, setIsValidatingGemini] = useState(false);
  const [keyNotice, setKeyNotice] = useState<string | null>(null);
  const [showKeyText, setShowKeyText] = useState(false);
  const [isEditingGeminiKey, setIsEditingGeminiKey] = useState(false);

  // Groq API Key state
  const [groqKeyStatus, setGroqKeyStatus] = useState<GroqKeyStatus | null>(null);
  const [newGroqKeyInput, setNewGroqKeyInput] = useState('');
  const [isSavingGroqKey, setIsSavingGroqKey] = useState(false);
  const [isValidatingGroq, setIsValidatingGroq] = useState(false);
  const [groqKeyNotice, setGroqKeyNotice] = useState<string | null>(null);
  const [showGroqKeyText, setShowGroqKeyText] = useState(false);
  const [isEditingGroqKey, setIsEditingGroqKey] = useState(false);

  // Worker model state
  const [workspaceWorkerConfig, setWorkspaceWorkerConfigState] = useState<WorkspaceWorkerConfig | null>(null);
  const [installedModels, setInstalledModels] = useState<OllamaModelItem[]>([]);
  const [currentWorkerModel, setCurrentWorkerModel] = useState<string>('granite4.1:8b');
  const [isUpdatingModel, setIsUpdatingModel] = useState(false);
  const [modelNotice, setModelNotice] = useState<string | null>(null);
  const [selectedWorkspacePath, setSelectedWorkspacePath] = useState<string>(currentWorkspacePath || '');

  // Mode state
  const [executionMode, setExecutionMode] = useState<'default' | 'turbo'>(() => {
    try {
      return (localStorage.getItem('zencode_execution_mode') as 'default' | 'turbo') || 'default';
    } catch {
      return 'default';
    }
  });

  // Section element refs for smooth scrolling
  const workspacesRef = useRef<HTMLDivElement>(null);
  const worktreesRef = useRef<HTMLDivElement>(null);
  const apikeyRef = useRef<HTMLDivElement>(null);
  const quotaRef = useRef<HTMLDivElement>(null);
  const workerRef = useRef<HTMLDivElement>(null);
  const modeRef = useRef<HTMLDivElement>(null);

  const scrollToSection = (section: SettingsSection) => {
    setActiveSection(section);
    const refMap: Record<SettingsSection, React.RefObject<HTMLDivElement | null>> = {
      workspaces: workspacesRef,
      worktrees: worktreesRef,
      apikey: apikeyRef,
      quota: quotaRef,
      worker: workerRef,
      mode: modeRef,
    };
    refMap[section]?.current?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  };

  const loadAll = async () => {
    setIsLoadingWorkspaces(true);
    try {
      const ws = await listWorkspaces();
      setWorkspaces(ws || []);
      if (!selectedWorkspacePath && ws && ws.length > 0) {
        setSelectedWorkspacePath(ws[0].path);
      }
    } catch (e) {
      console.warn('Failed to load workspaces:', e);
    } finally {
      setIsLoadingWorkspaces(false);
    }

    setIsLoadingWorktrees(true);
    try {
      const wt = await listActiveWorktrees();
      setWorktrees(wt || []);
    } catch (e) {
      console.warn('Failed to load worktrees:', e);
    } finally {
      setIsLoadingWorktrees(false);
    }

    try {
      const ks = await getGeminiKeyStatus();
      setKeyStatus(ks);
    } catch (e) {
      console.warn('Failed to get Gemini key status:', e);
    }

    try {
      const gks = await getGroqKeyStatus();
      setGroqKeyStatus(gks);
    } catch (e) {
      console.warn('Failed to get Groq key status:', e);
    }

    try {
      const models = await listInstalledOllamaModels();
      setInstalledModels(models || []);
      const current = await getWorkerModel();
      if (current) setCurrentWorkerModel(current);
    } catch (e) {
      console.warn('Failed to get worker models:', e);
    }
  };

  useEffect(() => {
    loadAll();
  }, []);

  useEffect(() => {
    if (!selectedWorkspacePath) return;
    getWorkspaceWorkerConfig(selectedWorkspacePath)
      .then((cfg) => {
        setWorkspaceWorkerConfigState(cfg);
      })
      .catch((err) => {
        console.warn('Could not load workspace worker config:', err);
      });
  }, [selectedWorkspacePath]);

  const handleAddWorkspace = async () => {
    try {
      const selected = await pickFolder();
      if (selected && selected.trim()) {
        onOpenWorkspace(selected.trim());
        await loadAll();
      }
    } catch (err) {
      console.error('Failed to pick folder:', err);
    }
  };

  const handleRemoveWorkspace = async (path: string) => {
    try {
      await removeWorkspace(path);
      setWorkspaces((prev) => prev.filter((w) => w.path !== path));
    } catch (err: any) {
      console.error('Failed to remove workspace:', err);
    }
  };

  const handleResetTrust = async (path: string) => {
    try {
      await setWorkspaceContextPreviewTrusted(path, false);
      const ws = await listWorkspaces();
      setWorkspaces(ws || []);
    } catch (err: any) {
      console.error('Failed to reset workspace trust:', err);
    }
  };

  const handleCleanWorktree = async (taskId: string) => {
    try {
      await recoverOrphanedWorktree(taskId, 'reject');
      setWorktrees((prev) => prev.filter((w) => w.task_id !== taskId));
      setWorktreeNotice(`Cleaned worktree for task ${taskId}`);
      setTimeout(() => setWorktreeNotice(null), 3000);
    } catch (err: any) {
      console.error('Failed to clean worktree:', err);
    }
  };

  const handleValidateGemini = async () => {
    if (!newKeyInput.trim()) return;
    setIsValidatingGemini(true);
    setKeyNotice(null);
    try {
      const isValid = await validateGeminiApiKey(newKeyInput.trim());
      if (isValid) {
        setKeyNotice('Validation successful! Gemini API is active.');
      } else {
        setKeyNotice('Validation failed: Invalid Gemini API key.');
      }
    } catch (err: any) {
      setKeyNotice(`Validation failed: ${err?.message || err}`);
    } finally {
      setIsValidatingGemini(false);
    }
  };

  const handleSaveKey = async () => {
    if (!newKeyInput.trim()) return;
    setIsSavingKey(true);
    setKeyNotice(null);
    try {
      const updated = await setGeminiApiKey(newKeyInput.trim());
      setKeyStatus(updated);
      setNewKeyInput('');
      setIsEditingGeminiKey(false);
      setKeyNotice('Gemini API key securely saved to Windows Credential Manager');
      setTimeout(() => setKeyNotice(null), 4000);
    } catch (err: any) {
      setKeyNotice(`Failed to save key: ${err?.message || err}`);
    } finally {
      setIsSavingKey(false);
    }
  };

  const handleValidateGroq = async () => {
    if (!newGroqKeyInput.trim()) return;
    setIsValidatingGroq(true);
    setGroqKeyNotice(null);
    try {
      const isValid = await validateGroqApiKey(newGroqKeyInput.trim());
      if (isValid) {
        setGroqKeyNotice('Validation successful! Groq API is active.');
      } else {
        setGroqKeyNotice('Validation failed: Invalid Groq API key.');
      }
    } catch (err: any) {
      setGroqKeyNotice(`Validation failed: ${err?.message || err}`);
    } finally {
      setIsValidatingGroq(false);
    }
  };

  const handleSaveGroqKey = async () => {
    if (!newGroqKeyInput.trim()) return;
    setIsSavingGroqKey(true);
    setGroqKeyNotice(null);
    try {
      const updated = await setGroqApiKey(newGroqKeyInput.trim());
      setGroqKeyStatus(updated);
      setNewGroqKeyInput('');
      setIsEditingGroqKey(false);
      setGroqKeyNotice('Groq API key securely saved to Windows Credential Manager');
      setTimeout(() => setGroqKeyNotice(null), 4000);
    } catch (err: any) {
      setGroqKeyNotice(`Failed to save Groq key: ${err?.message || err}`);
    } finally {
      setIsSavingGroqKey(false);
    }
  };

  const handleDeleteGroqKey = async () => {
    try {
      await deleteGroqApiKey();
      setGroqKeyStatus({ has_key: false, masked_key: '', status_text: 'Not configured' });
      setGroqKeyNotice('Groq API key removed from Windows Credential Manager');
      setTimeout(() => setGroqKeyNotice(null), 3000);
    } catch (err: any) {
      setGroqKeyNotice(`Failed to remove Groq key: ${err?.message || err}`);
    }
  };

  const handleSelectDefaultWorkerType = async (workerType: 'local' | 'groq') => {
    if (workerType === 'groq' && !groqKeyStatus?.has_key) {
      if (onOpenOnboarding) {
        onOpenOnboarding(3);
      } else {
        scrollToSection('apikey');
        setIsEditingGroqKey(true);
      }
      return;
    }

    if (!selectedWorkspacePath) return;

    setIsUpdatingModel(true);
    setModelNotice(null);
    try {
      await setWorkspaceWorkerConfig(
        selectedWorkspacePath,
        workerType,
        workspaceWorkerConfig?.groq_model || 'openai/gpt-oss-120b'
      );
      setWorkspaceWorkerConfigState({
        worker_type: workerType,
        groq_model: workspaceWorkerConfig?.groq_model || 'openai/gpt-oss-120b',
      });
      setModelNotice(`Default worker set to ${workerType === 'groq' ? 'Groq (Cloud)' : 'Local (Ollama)'}`);
      setTimeout(() => setModelNotice(null), 3000);
    } catch (e: any) {
      setModelNotice(`Failed to update default worker: ${e?.message || e}`);
    } finally {
      setIsUpdatingModel(false);
    }
  };

  const handleSelectGroqModel = async (modelId: string) => {
    if (!selectedWorkspacePath) return;
    setIsUpdatingModel(true);
    setModelNotice(null);
    try {
      await setWorkspaceWorkerConfig(selectedWorkspacePath, 'groq', modelId);
      setWorkspaceWorkerConfigState({
        worker_type: 'groq',
        groq_model: modelId,
      });
      setModelNotice(`Groq worker model set to ${modelId}`);
      setTimeout(() => setModelNotice(null), 3000);
    } catch (e: any) {
      setModelNotice(`Failed to update Groq model: ${e?.message || e}`);
    } finally {
      setIsUpdatingModel(false);
    }
  };

  const handleSelectLocalWorkerModel = async (modelName: string) => {
    setIsUpdatingModel(true);
    setModelNotice(null);
    try {
      await setWorkerModel(modelName);
      setCurrentWorkerModel(modelName);
      if (selectedWorkspacePath) {
        await setWorkspaceWorkerConfig(selectedWorkspacePath, 'local', undefined);
        setWorkspaceWorkerConfigState({
          worker_type: 'local',
          groq_model: workspaceWorkerConfig?.groq_model || 'openai/gpt-oss-120b',
        });
      }
      setModelNotice(`Worker model set to ${modelName}`);
      setTimeout(() => setModelNotice(null), 3000);
    } catch (err: any) {
      setModelNotice(`Failed to set model: ${err?.message || err}`);
    } finally {
      setIsUpdatingModel(false);
    }
  };

  const handleToggleMode = (mode: 'default' | 'turbo') => {
    setExecutionMode(mode);
    try {
      localStorage.setItem('zencode_execution_mode', mode);
    } catch {}
  };

  const currentDefaultWorker = workspaceWorkerConfig?.worker_type || 'local';
  const currentGroqModel = workspaceWorkerConfig?.groq_model || 'openai/gpt-oss-120b';

  return (
    <div className="flex-1 flex h-full w-full bg-[#faf9f5] text-[#141413] font-sans select-none overflow-hidden">
      {/* Left Navigation Sidebar matching PDF Page 5 */}
      <aside className="w-56 h-full bg-[#faf9f5] border-r border-[#dad9d4] flex flex-col p-6 shrink-0 overflow-y-auto">
        <div className="text-[11px] font-semibold uppercase tracking-wider text-[#83827d] pb-4">
          SETTINGS
        </div>

        <nav className="space-y-1">
          <button
            type="button"
            onClick={() => scrollToSection('workspaces')}
            className={`w-full text-left px-3 py-2 rounded-lg text-xs font-sans transition-colors cursor-pointer ${
              activeSection === 'workspaces'
                ? 'bg-[#ede9de] text-[#141413] font-medium'
                : 'text-[#535146] hover:text-[#141413] hover:bg-[#ede9de]/50'
            }`}
          >
            Workspaces
          </button>

          <button
            type="button"
            onClick={() => scrollToSection('worktrees')}
            className={`w-full text-left px-3 py-2 rounded-lg text-xs font-sans transition-colors cursor-pointer ${
              activeSection === 'worktrees'
                ? 'bg-[#ede9de] text-[#141413] font-medium'
                : 'text-[#535146] hover:text-[#141413] hover:bg-[#ede9de]/50'
            }`}
          >
            Worktrees
          </button>

          <button
            type="button"
            onClick={() => scrollToSection('apikey')}
            className={`w-full text-left px-3 py-2 rounded-lg text-xs font-sans transition-colors cursor-pointer ${
              activeSection === 'apikey'
                ? 'bg-[#ede9de] text-[#141413] font-medium'
                : 'text-[#535146] hover:text-[#141413] hover:bg-[#ede9de]/50'
            }`}
          >
            API Keys
          </button>

          <button
            type="button"
            onClick={() => scrollToSection('quota')}
            className={`w-full text-left px-3 py-2 rounded-lg text-xs font-sans transition-colors cursor-pointer ${
              activeSection === 'quota'
                ? 'bg-[#ede9de] text-[#141413] font-medium'
                : 'text-[#535146] hover:text-[#141413] hover:bg-[#ede9de]/50'
            }`}
          >
            Usage
          </button>

          <button
            type="button"
            onClick={() => scrollToSection('worker')}
            className={`w-full text-left px-3 py-2 rounded-lg text-xs font-sans transition-colors cursor-pointer ${
              activeSection === 'worker'
                ? 'bg-[#ede9de] text-[#141413] font-medium'
                : 'text-[#535146] hover:text-[#141413] hover:bg-[#ede9de]/50'
            }`}
          >
            Worker Model
          </button>

          <button
            type="button"
            onClick={() => scrollToSection('mode')}
            className={`w-full text-left px-3 py-2 rounded-lg text-xs font-sans transition-colors cursor-pointer ${
              activeSection === 'mode'
                ? 'bg-[#ede9de] text-[#141413] font-medium'
                : 'text-[#535146] hover:text-[#141413] hover:bg-[#ede9de]/50'
            }`}
          >
            Mode
          </button>
        </nav>
      </aside>

      {/* Main Content Area (Scrollable Stacked Sections matching PDF Page 5) */}
      <main className="flex-1 h-full overflow-y-auto px-12 py-8 max-w-4xl min-w-0">
        <div className="space-y-12 pb-16">
          {/* ============================================================== */}
          {/* SECTION 1: WORKSPACES (Matches PDF Page 5)                     */}
          {/* ============================================================== */}
          <section ref={workspacesRef} className="space-y-4">
            <h2 className="text-base font-semibold text-[#141413] font-sans">Workspaces</h2>

            <div className="space-y-0 divide-y divide-[#dad9d4]">
              {isLoadingWorkspaces ? (
                <div className="py-4 text-xs text-[#83827d]">Loading workspaces...</div>
              ) : workspaces.length === 0 ? (
                <div className="py-6 text-center text-xs text-[#83827d]">
                  No workspaces registered yet.
                </div>
              ) : (
                workspaces.map((ws) => (
                  <div
                    key={ws.path}
                    className="py-3 flex items-center justify-between gap-4 text-xs"
                  >
                    <div className="space-y-0.5 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-[#141413] text-sm truncate font-sans">
                          {ws.name}
                        </span>
                        {ws.is_context_preview_trusted && (
                          <span className="text-[10px] text-[#5f8a5a] bg-[#5f8a5a]/10 border border-[#5f8a5a]/20 px-1.5 py-0.2 rounded font-sans">
                            trusted
                          </span>
                        )}
                      </div>
                      <div className="text-xs text-[#83827d] font-mono truncate">{ws.path}</div>
                    </div>

                    <div className="flex items-center gap-3 shrink-0">
                      {ws.is_context_preview_trusted && (
                        <button
                          type="button"
                          onClick={() => handleResetTrust(ws.path)}
                          className="text-xs text-[#83827d] hover:text-[#141413] transition-colors cursor-pointer"
                          title="Reset consent card"
                        >
                          Reset Trust
                        </button>
                      )}
                      <button
                        type="button"
                        onClick={() => onOpenWorkspace(ws.path)}
                        className="text-xs text-[#83827d] hover:text-[#141413] transition-colors cursor-pointer"
                        title="Open workspace"
                      >
                        Open
                      </button>
                      <button
                        type="button"
                        onClick={() => handleRemoveWorkspace(ws.path)}
                        className="text-xs text-[#83827d] hover:text-[#c96442] transition-colors cursor-pointer font-sans"
                        title="Remove workspace"
                      >
                        Remove
                      </button>
                    </div>
                  </div>
                ))
              )}
            </div>

            <div className="pt-2">
              <button
                type="button"
                onClick={handleAddWorkspace}
                className="inline-flex items-center gap-1.5 text-xs text-[#c96442] hover:text-[#b5523f] font-medium font-sans cursor-pointer transition-colors"
              >
                <FolderPlus className="w-3.5 h-3.5" />
                <span>+ Add workspace</span>
              </button>
            </div>
          </section>

          {/* ============================================================== */}
          {/* SECTION 2: WORKTREES (Matches PDF Page 5)                      */}
          {/* ============================================================== */}
          <section ref={worktreesRef} className="space-y-4">
            <h2 className="text-base font-semibold text-[#141413] font-sans">Worktrees</h2>

            {worktreeNotice && (
              <div className="p-2.5 bg-[#f5f4ee] border border-[#5f8a5a]/30 rounded-lg text-xs text-[#5f8a5a] flex items-center gap-2">
                <Check className="w-3.5 h-3.5" />
                <span>{worktreeNotice}</span>
              </div>
            )}

            <div className="space-y-0 divide-y divide-[#dad9d4]">
              {isLoadingWorktrees ? (
                <div className="py-4 text-xs text-[#83827d]">Checking worktrees...</div>
              ) : worktrees.length === 0 ? (
                <div className="py-6 text-center text-xs text-[#83827d]">
                  No active or orphaned worktrees found. Primary repository is clean.
                </div>
              ) : (
                worktrees.map((wt) => {
                  const isOrphaned = (wt as any).is_orphaned ?? (wt.age_seconds > 300);
                  return (
                    <div
                      key={wt.worktree_path}
                      className="py-3 flex items-center justify-between gap-4 text-xs"
                    >
                      <div className="flex items-center gap-3 min-w-0">
                        <span className="font-mono text-xs text-[#141413] truncate">
                          {wt.branch_name || wt.task_id}
                        </span>
                        <span
                          className={`text-[11px] px-2 py-0.5 rounded font-sans font-medium ${
                            isOrphaned
                              ? 'text-[#83827d] bg-[#ede9de] border border-[#dad9d4]'
                              : 'text-[#5f8a5a] bg-[#5f8a5a]/10 border border-[#5f8a5a]/20'
                          }`}
                        >
                          {isOrphaned ? 'orphaned' : 'active'}
                        </span>
                      </div>

                      <div className="flex items-center gap-3 shrink-0">
                        <button
                          type="button"
                          onClick={() => onReviewWorktree(wt.task_id)}
                          className="text-xs text-[#c96442] hover:text-[#b5523f] font-medium font-sans transition-colors cursor-pointer"
                        >
                          Review
                        </button>
                        <button
                          type="button"
                          onClick={() => handleCleanWorktree(wt.task_id)}
                          className="text-xs text-[#83827d] hover:text-[#c96442] font-sans transition-colors cursor-pointer"
                        >
                          Clean
                        </button>
                      </div>
                    </div>
                  );
                })
              )}
            </div>
          </section>

          {/* ============================================================== */}
          {/* SECTION 3: API KEYS (Matches PDF Page 5)                       */}
          {/* ============================================================== */}
          <section ref={apikeyRef} className="space-y-4">
            <h2 className="text-base font-semibold text-[#141413] font-sans">API Keys</h2>

            {keyNotice && (
              <div className="p-2.5 bg-[#f5f4ee] border border-[#5f8a5a]/30 rounded-lg text-xs text-[#5f8a5a] flex items-center gap-2">
                <Check className="w-3.5 h-3.5 shrink-0" />
                <span>{keyNotice}</span>
              </div>
            )}

            {groqKeyNotice && (
              <div className="p-2.5 bg-[#f5f4ee] border border-[#5f8a5a]/30 rounded-lg text-xs text-[#5f8a5a] flex items-center gap-2">
                <Check className="w-3.5 h-3.5 shrink-0" />
                <span>{groqKeyNotice}</span>
              </div>
            )}

            <div className="space-y-0 divide-y divide-[#dad9d4]">
              {/* Planning Model (Gemini) Row matching PDF Page 5 */}
              <div className="py-3.5 space-y-2">
                <div className="flex items-center justify-between gap-4">
                  <div className="space-y-0.5 min-w-0">
                    <div className="text-sm font-medium text-[#141413] font-sans">
                      Planning model (Gemini)
                    </div>
                    <div className="text-xs text-[#535146] font-mono truncate">
                      {keyStatus?.has_key
                        ? (keyStatus.masked_key || 'AIzaSy••••••••••••••••••••••••')
                        : 'Not configured'}
                    </div>
                  </div>

                  <div className="flex items-center gap-3 shrink-0">
                    <span
                      className={`text-[11px] px-2 py-0.5 rounded font-sans font-medium ${
                        keyStatus?.has_key
                          ? 'text-[#5f8a5a] bg-[#5f8a5a]/10 border border-[#5f8a5a]/20'
                          : 'text-[#83827d] bg-[#ede9de] border border-[#dad9d4]'
                      }`}
                    >
                      {keyStatus?.has_key ? 'Valid' : 'Empty'}
                    </span>
                    <button
                      type="button"
                      onClick={() => setIsEditingGeminiKey(!isEditingGeminiKey)}
                      className="text-xs text-[#c96442] hover:text-[#b5523f] font-medium font-sans transition-colors cursor-pointer"
                    >
                      {keyStatus?.has_key ? 'Replace' : 'Add key'}
                    </button>
                  </div>
                </div>

                {/* Inline Replace Form */}
                {isEditingGeminiKey && (
                  <div className="p-3 bg-[#f5f4ee] border border-[#dad9d4] rounded-xl space-y-2 mt-2">
                    <div className="flex items-center gap-2">
                      <div className="relative flex-1">
                        <input
                          type={showKeyText ? 'text' : 'password'}
                          value={newKeyInput}
                          onChange={(e) => setNewKeyInput(e.target.value)}
                          placeholder="Paste Gemini API key (AIzaSy...)"
                          className="w-full bg-[#faf9f5] border border-[#dad9d4] focus:border-[#c96442] rounded-lg px-3 py-1.5 text-xs text-[#141413] placeholder-[#83827d] font-mono focus:outline-none"
                        />
                        <button
                          type="button"
                          onClick={() => setShowKeyText(!showKeyText)}
                          className="absolute right-2 top-2 text-[#83827d] hover:text-[#141413]"
                        >
                          {showKeyText ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                        </button>
                      </div>

                      <button
                        type="button"
                        onClick={handleValidateGemini}
                        disabled={!newKeyInput.trim() || isValidatingGemini}
                        className="px-2.5 py-1.5 rounded-lg border border-[#dad9d4] bg-[#ede9de] hover:bg-[#e9e6dc] text-xs text-[#141413] transition-colors disabled:opacity-40 cursor-pointer"
                      >
                        {isValidatingGemini ? <Loader2 className="w-3.5 h-3.5 animate-spin text-[#c96442]" /> : 'Validate'}
                      </button>

                      <button
                        type="button"
                        onClick={handleSaveKey}
                        disabled={!newKeyInput.trim() || isSavingKey}
                        className="px-3 py-1.5 rounded-lg bg-[#c96442] hover:bg-[#b5523f] text-white font-medium text-xs disabled:opacity-40 transition-colors shadow-sm cursor-pointer"
                      >
                        {isSavingKey ? 'Saving...' : 'Save'}
                      </button>

                      <button
                        type="button"
                        onClick={() => {
                          setIsEditingGeminiKey(false);
                          setNewKeyInput('');
                        }}
                        className="px-2.5 py-1.5 rounded-lg text-xs text-[#83827d] hover:text-[#141413] cursor-pointer"
                      >
                        Cancel
                      </button>
                    </div>
                  </div>
                )}
              </div>

              {/* Cloud Worker (Groq) Row matching PDF Page 5 */}
              <div className="py-3.5 space-y-2">
                <div className="flex items-center justify-between gap-4">
                  <div className="space-y-0.5 min-w-0">
                    <div className="text-sm font-medium text-[#141413] font-sans">
                      Cloud worker (optional)
                    </div>
                    <div className="text-xs text-[#83827d] font-mono truncate">
                      {groqKeyStatus?.has_key
                        ? (groqKeyStatus.masked_key || 'gsk_••••••••••••••••••••••••')
                        : 'Not configured'}
                    </div>
                  </div>

                  <div className="flex items-center gap-3 shrink-0">
                    <span
                      className={`text-[11px] px-2 py-0.5 rounded font-sans font-medium ${
                        groqKeyStatus?.has_key
                          ? 'text-[#5f8a5a] bg-[#5f8a5a]/10 border border-[#5f8a5a]/20'
                          : 'text-[#83827d] bg-[#ede9de] border border-[#dad9d4]'
                      }`}
                    >
                      {groqKeyStatus?.has_key ? 'Valid' : 'Empty'}
                    </span>
                    <button
                      type="button"
                      onClick={() => setIsEditingGroqKey(!isEditingGroqKey)}
                      className="text-xs text-[#c96442] hover:text-[#b5523f] font-medium font-sans transition-colors cursor-pointer"
                    >
                      {groqKeyStatus?.has_key ? 'Replace' : 'Add key'}
                    </button>
                    {groqKeyStatus?.has_key && (
                      <button
                        type="button"
                        onClick={handleDeleteGroqKey}
                        className="text-xs text-[#83827d] hover:text-[#c96442] font-sans transition-colors cursor-pointer"
                        title="Delete Groq key"
                      >
                        Remove
                      </button>
                    )}
                  </div>
                </div>

                {/* Inline Groq Key Form */}
                {isEditingGroqKey && (
                  <div className="p-3 bg-[#f5f4ee] border border-[#dad9d4] rounded-xl space-y-2 mt-2">
                    <div className="flex items-center gap-2">
                      <div className="relative flex-1">
                        <input
                          type={showGroqKeyText ? 'text' : 'password'}
                          value={newGroqKeyInput}
                          onChange={(e) => setNewGroqKeyInput(e.target.value)}
                          placeholder="Paste Groq API key (gsk_...)"
                          className="w-full bg-[#faf9f5] border border-[#dad9d4] focus:border-[#c96442] rounded-lg px-3 py-1.5 text-xs text-[#141413] placeholder-[#83827d] font-mono focus:outline-none"
                        />
                        <button
                          type="button"
                          onClick={() => setShowGroqKeyText(!showGroqKeyText)}
                          className="absolute right-2 top-2 text-[#83827d] hover:text-[#141413]"
                        >
                          {showGroqKeyText ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                        </button>
                      </div>

                      <button
                        type="button"
                        onClick={handleValidateGroq}
                        disabled={!newGroqKeyInput.trim() || isValidatingGroq}
                        className="px-2.5 py-1.5 rounded-lg border border-[#dad9d4] bg-[#ede9de] hover:bg-[#e9e6dc] text-xs text-[#141413] transition-colors disabled:opacity-40 cursor-pointer"
                      >
                        {isValidatingGroq ? <Loader2 className="w-3.5 h-3.5 animate-spin text-[#c96442]" /> : 'Validate'}
                      </button>

                      <button
                        type="button"
                        onClick={handleSaveGroqKey}
                        disabled={!newGroqKeyInput.trim() || isSavingGroqKey}
                        className="px-3 py-1.5 rounded-lg bg-[#c96442] hover:bg-[#b5523f] text-white font-medium text-xs disabled:opacity-40 transition-colors shadow-sm cursor-pointer"
                      >
                        {isSavingGroqKey ? 'Saving...' : 'Save'}
                      </button>

                      <button
                        type="button"
                        onClick={() => {
                          setIsEditingGroqKey(false);
                          setNewGroqKeyInput('');
                        }}
                        className="px-2.5 py-1.5 rounded-lg text-xs text-[#83827d] hover:text-[#141413] cursor-pointer"
                      >
                        Cancel
                      </button>
                    </div>
                  </div>
                )}
              </div>
            </div>
          </section>

          {/* ============================================================== */}
          {/* SECTION 4: USAGE (Matches PDF Page 5)                          */}
          {/* ============================================================== */}
          <section ref={quotaRef} className="space-y-4">
            <h2 className="text-base font-semibold text-[#141413] font-sans">Usage</h2>

            {/* Combined Groq Cloud Capacity Card */}
            <div className="p-4 rounded-xl border border-[#dad9d4] bg-[#f5f4ee] space-y-3 shadow-sm">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Cloud className="w-4 h-4 text-[#c96442]" />
                  <span className="text-xs font-semibold text-[#141413]">Combined Groq Cloud Worker Capacity</span>
                  <span className="text-[10px] text-[#c96442] bg-[#c96442]/10 border border-[#c96442]/20 px-1.5 py-0.5 rounded font-sans">
                    BYOK Cloud
                  </span>
                </div>
                <div className="text-xs font-mono text-[#535146]">
                  <span className="font-semibold text-[#141413]">
                    {quotaState?.groq_combined_rpd_remaining ?? 3000}
                  </span>
                  <span className="text-[#83827d]"> / {quotaState?.groq_combined_rpd_limit ?? 3000} RPD</span>
                </div>
              </div>

              {/* Progress bar */}
              {(() => {
                const total = quotaState?.groq_combined_rpd_limit || 3000;
                const rem = quotaState?.groq_combined_rpd_remaining ?? 3000;
                const consumed = Math.max(0, total - rem);
                const percent = Math.min(100, Math.round((consumed / total) * 100));
                return (
                  <div className="space-y-1.5">
                    <div className="h-2 bg-[#ede9de] rounded-full overflow-hidden border border-[#dad9d4]">
                      <div
                        style={{ width: `${percent}%` }}
                        className={`h-full rounded-full transition-all ${
                          quotaState?.all_groq_exhausted || rem === 0
                            ? 'bg-rose-500'
                            : percent >= 80
                            ? 'bg-amber-500'
                            : 'bg-[#c96442]'
                        }`}
                      />
                    </div>
                    <div className="flex items-center justify-between text-[11px] text-[#83827d]">
                      <span>3 Rotation Models: 120B (1k) + 20B (1k) + Qwen (1k)</span>
                      <span>Resets midnight Pacific Time</span>
                    </div>
                  </div>
                );
              })()}

              {quotaState?.all_groq_exhausted && (
                <div className="p-2.5 rounded-lg border border-rose-200 bg-rose-50 text-xs text-rose-800 flex items-center gap-2">
                  <AlertTriangle className="w-4 h-4 text-rose-600 shrink-0" />
                  <span>
                    Groq daily quota is exhausted. Tasks will route to Local (Ollama) worker until reset.
                  </span>
                </div>
              )}
            </div>

            {/* Models Quota Cards */}
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              {quotaState?.models.map((model) => {
                const rpmPercent = model.rpm_limit > 0 ? Math.round((model.rpm_consumed / model.rpm_limit) * 100) : 0;
                const tpmPercent = model.tpm_limit > 0 ? Math.round((model.tpm_consumed / model.tpm_limit) * 100) : 0;
                const rpdPercent = model.rpd_limit > 0 ? Math.round((model.rpd_consumed / model.rpd_limit) * 100) : 0;
                const isPinned = quotaState.pinned_model === model.model_id;
                const isActive = quotaState.active_model === model.model_id;
                const isGroqModel =
                  model.model_id.includes('gpt-oss') ||
                  model.model_id.includes('qwen') ||
                  model.display_name.toLowerCase().includes('groq');

                return (
                  <div
                    key={model.model_id}
                    className={`p-3.5 rounded-xl border space-y-2.5 ${
                      isActive
                        ? 'bg-[#f5f4ee] border-[#c96442]'
                        : 'bg-[#f5f4ee] border-[#dad9d4]'
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <span className="text-xs font-semibold text-[#141413]">{model.display_name}</span>
                        {isActive && (
                          <span className="text-[10px] text-[#c96442] bg-[#c96442]/10 px-1.5 py-0.5 rounded border border-[#c96442]/20 font-medium">
                            Active
                          </span>
                        )}
                        {isPinned && (
                          <span className="text-[10px] text-[#5f8a5a] bg-[#5f8a5a]/10 px-1.5 py-0.5 rounded border border-[#5f8a5a]/20 font-medium">
                            Pinned
                          </span>
                        )}
                      </div>

                      {!isGroqModel && (
                        <button
                          type="button"
                          onClick={() => onPinModel(isPinned ? null : model.model_id)}
                          className={`px-2 py-0.5 text-[10px] rounded border transition-colors cursor-pointer ${
                            isPinned
                              ? 'bg-[#ede9de] border-[#dad9d4] text-[#141413]'
                              : 'bg-[#faf9f5] border-[#dad9d4] text-[#83827d] hover:text-[#141413]'
                          }`}
                        >
                          {isPinned ? 'Unpin' : 'Pin'}
                        </button>
                      )}
                    </div>

                    <div className="grid grid-cols-3 gap-2 text-[11px] pt-1">
                      <div className="space-y-1">
                        <div className="flex justify-between text-[#83827d]">
                          <span>RPM</span>
                          <span className="font-mono text-[#141413]">{model.rpm_consumed}/{model.rpm_limit}</span>
                        </div>
                        <div className="h-1 bg-[#ede9de] rounded-full overflow-hidden">
                          <div
                            style={{ width: `${Math.min(rpmPercent, 100)}%` }}
                            className={`h-full rounded-full ${rpmPercent >= 80 ? 'bg-rose-500' : 'bg-[#c96442]'}`}
                          />
                        </div>
                      </div>

                      <div className="space-y-1">
                        <div className="flex justify-between text-[#83827d]">
                          <span>TPM</span>
                          <span className="font-mono text-[#141413]">{Math.round(model.tpm_consumed / 1000)}k/{Math.round(model.tpm_limit / 1000)}k</span>
                        </div>
                        <div className="h-1 bg-[#ede9de] rounded-full overflow-hidden">
                          <div
                            style={{ width: `${Math.min(tpmPercent, 100)}%` }}
                            className={`h-full rounded-full ${tpmPercent >= 80 ? 'bg-rose-500' : 'bg-[#c96442]'}`}
                          />
                        </div>
                      </div>

                      <div className="space-y-1">
                        <div className="flex justify-between text-[#83827d]">
                          <span>RPD</span>
                          <span className="font-mono text-[#141413]">{model.rpd_consumed}/{model.rpd_limit}</span>
                        </div>
                        <div className="h-1 bg-[#ede9de] rounded-full overflow-hidden">
                          <div
                            style={{ width: `${Math.min(rpdPercent, 100)}%` }}
                            className={`h-full rounded-full ${rpdPercent >= 80 ? 'bg-rose-500' : 'bg-[#c96442]'}`}
                          />
                        </div>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </section>

          {/* ============================================================== */}
          {/* SECTION 5: WORKER MODEL (Matches PDF Page 5)                   */}
          {/* ============================================================== */}
          <section ref={workerRef} className="space-y-4">
            <div className="flex items-center gap-2">
              <h2 className="text-base font-semibold text-[#141413] font-sans">Worker Model</h2>
              {isUpdatingModel && <Loader2 className="w-3.5 h-3.5 animate-spin text-[#c96442]" />}
            </div>

            {modelNotice && (
              <div className="p-2.5 bg-[#f5f4ee] border border-[#5f8a5a]/30 rounded-lg text-xs text-[#5f8a5a] flex items-center gap-2">
                <Check className="w-3.5 h-3.5" />
                <span>{modelNotice}</span>
              </div>
            )}

            {/* Workspace selector dropdown */}
            {workspaces.length > 0 && (
              <div className="p-3 bg-[#f5f4ee] border border-[#dad9d4] rounded-xl flex items-center justify-between text-xs">
                <span className="text-[#83827d]">Configuring Workspace:</span>
                <select
                  value={selectedWorkspacePath}
                  onChange={(e) => setSelectedWorkspacePath(e.target.value)}
                  className="bg-[#faf9f5] border border-[#dad9d4] rounded-lg px-2.5 py-1 text-xs text-[#141413] font-sans focus:outline-none cursor-pointer"
                >
                  {workspaces.map((ws) => (
                    <option key={ws.path} value={ws.path}>
                      {ws.name} ({ws.path})
                    </option>
                  ))}
                </select>
              </div>
            )}

            {/* Local vs Groq Card Choice */}
            <div className="grid grid-cols-2 gap-3">
              {/* Option 1: Local (Ollama) */}
              <div
                onClick={() => handleSelectDefaultWorkerType('local')}
                className={`p-4 rounded-xl border transition-all cursor-pointer flex flex-col justify-between space-y-2 ${
                  currentDefaultWorker === 'local'
                    ? 'bg-[#f5f4ee] border-[#c96442] shadow-sm'
                    : 'bg-[#faf9f5] border-[#dad9d4] hover:bg-[#f5f4ee]'
                }`}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Cpu className={`w-4 h-4 ${currentDefaultWorker === 'local' ? 'text-[#c96442]' : 'text-[#83827d]'}`} />
                    <span className="text-xs font-semibold text-[#141413]">Local (Ollama)</span>
                  </div>
                  {currentDefaultWorker === 'local' && (
                    <span className="text-[10px] text-[#c96442] bg-[#c96442]/10 px-1.5 py-0.5 rounded font-medium">
                      Active
                    </span>
                  )}
                </div>
                <p className="text-[11px] text-[#535146] leading-relaxed">
                  100% free, runs entirely offline on your GPU/CPU in isolated Git worktrees.
                </p>
              </div>

              {/* Option 2: Groq (Cloud, BYOK) */}
              <div
                onClick={() => handleSelectDefaultWorkerType('groq')}
                className={`p-4 rounded-xl border transition-all cursor-pointer flex flex-col justify-between space-y-2 ${
                  currentDefaultWorker === 'groq'
                    ? 'bg-[#f5f4ee] border-[#c96442] shadow-sm'
                    : 'bg-[#faf9f5] border-[#dad9d4] hover:bg-[#f5f4ee]'
                }`}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Cloud className={`w-4 h-4 ${currentDefaultWorker === 'groq' ? 'text-[#c96442]' : 'text-[#83827d]'}`} />
                    <span className="text-xs font-semibold text-[#141413]">Groq (Cloud, BYOK)</span>
                  </div>
                  {currentDefaultWorker === 'groq' && (
                    <span className="text-[10px] text-[#c96442] bg-[#c96442]/10 px-1.5 py-0.5 rounded font-medium">
                      Active
                    </span>
                  )}
                </div>
                <p className="text-[11px] text-[#535146] leading-relaxed">
                  Cloud worker via fast Groq LPU inference. Requires your own Groq API key.
                </p>
              </div>
            </div>

            {/* Model cascade options */}
            {currentDefaultWorker === 'groq' ? (
              <div className="p-4 bg-[#f5f4ee] border border-[#dad9d4] rounded-xl space-y-3">
                <div className="text-xs font-semibold text-[#141413]">Groq Worker Model Cascade</div>
                <div className="space-y-1.5">
                  {GROQ_MODELS.map((item) => {
                    const isSelected = currentGroqModel === item.id;
                    return (
                      <div
                        key={item.id}
                        onClick={() => handleSelectGroqModel(item.id)}
                        className={`p-2.5 rounded-lg border transition-colors cursor-pointer flex items-center justify-between ${
                          isSelected
                            ? 'bg-[#ede9de] border-[#dad9d4] text-[#141413]'
                            : 'bg-[#faf9f5] border-[#dad9d4] text-[#535146] hover:bg-[#ede9de]/50'
                        }`}
                      >
                        <div>
                          <div className="text-xs font-medium text-[#141413]">{item.name}</div>
                          <div className="text-[10px] text-[#83827d] font-mono">{item.id} &bull; {item.desc}</div>
                        </div>
                        {isSelected && (
                          <span className="text-[10px] text-[#c96442] font-medium">Primary</span>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            ) : (
              <div className="p-4 bg-[#f5f4ee] border border-[#dad9d4] rounded-xl space-y-3">
                <div className="flex items-center justify-between text-xs">
                  <span className="text-[#83827d]">Active Model:</span>
                  <span className="text-[#141413] font-semibold font-mono">{currentWorkerModel}</span>
                </div>
                <div className="space-y-1.5">
                  {installedModels.map((item) => {
                    const isCurrent = currentWorkerModel === item.name;
                    return (
                      <div
                        key={item.name}
                        onClick={() => handleSelectLocalWorkerModel(item.name)}
                        className={`p-2.5 rounded-lg border transition-colors cursor-pointer flex items-center justify-between ${
                          isCurrent
                            ? 'bg-[#ede9de] border-[#dad9d4] text-[#141413]'
                            : 'bg-[#faf9f5] border-[#dad9d4] text-[#535146] hover:bg-[#ede9de]/50'
                        }`}
                      >
                        <div className="flex items-center gap-2">
                          <Cpu className="w-3.5 h-3.5 text-[#83827d]" />
                          <span className="text-xs font-medium text-[#141413]">{item.name}</span>
                          {item.is_recommended && (
                            <span className="text-[10px] text-[#5f8a5a] bg-[#5f8a5a]/10 px-1.5 py-0.2 rounded font-sans">
                              Recommended
                            </span>
                          )}
                        </div>
                        <span className="text-[10px] text-[#83827d] font-mono">{item.size}</span>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
          </section>

          {/* ============================================================== */}
          {/* SECTION 6: MODE (Matches PDF Page 5)                           */}
          {/* ============================================================== */}
          <section ref={modeRef} className="space-y-4">
            <h2 className="text-base font-semibold text-[#141413] font-sans">Mode</h2>

            <div className="grid grid-cols-2 gap-3">
              <div
                onClick={() => handleToggleMode('default')}
                className={`p-4 rounded-xl border transition-colors cursor-pointer flex flex-col justify-between space-y-2 ${
                  executionMode === 'default'
                    ? 'bg-[#f5f4ee] border-[#c96442] shadow-sm'
                    : 'bg-[#faf9f5] border-[#dad9d4] hover:bg-[#f5f4ee]'
                }`}
              >
                <div className="flex items-center justify-between">
                  <span className="text-xs font-semibold text-[#141413]">Default Mode</span>
                  {executionMode === 'default' && (
                    <span className="text-[10px] text-[#c96442] bg-[#c96442]/10 px-1.5 py-0.5 rounded font-medium">
                      Active
                    </span>
                  )}
                </div>
                <p className="text-[11px] text-[#535146] leading-relaxed">
                  Requires consent for shell commands outside the safe allowlist. Isolated Git worktrees auto-apply with undo.
                </p>
              </div>

              <div
                onClick={() => handleToggleMode('turbo')}
                className={`p-4 rounded-xl border transition-colors cursor-pointer flex flex-col justify-between space-y-2 ${
                  executionMode === 'turbo'
                    ? 'bg-[#f5f4ee] border-[#c96442] shadow-sm'
                    : 'bg-[#faf9f5] border-[#dad9d4] hover:bg-[#f5f4ee]'
                }`}
              >
                <div className="flex items-center justify-between">
                  <span className="text-xs font-semibold text-[#141413]">Turbo Mode</span>
                  {executionMode === 'turbo' && (
                    <span className="text-[10px] text-[#c96442] bg-[#c96442]/10 px-1.5 py-0.5 rounded font-medium">
                      Active
                    </span>
                  )}
                </div>
                <p className="text-[11px] text-[#535146] leading-relaxed">
                  Autonomous execution for allowlisted test and build commands without interrupting you.
                </p>
              </div>
            </div>
          </section>
        </div>
      </main>
    </div>
  );
};
