import React, { useState, useEffect, useRef, useCallback, type FC, type ReactNode } from 'react';
import {
  ArrowUp,
  ChevronDown,
  ChevronRight,
  Terminal,
  GitBranch,
  AlertTriangle,
  Loader2,
  Undo2,
  X,
  FileCode,
  FileCode2,
  FileCog,
  FileText,
  Code2,
  Cloud,
  Cpu,
  GitCompare,
  Copy,
  Check,
  CheckCircle2,
  Paperclip,
  RotateCcw,
  Globe,
  Zap,
  Sparkles,
  Plus,
} from 'lucide-react';
import {
  pickFile,
  getGroqKeyStatus,
  getOpenRouterKeyStatus,
  getWorkspaceWorkerConfig,
  setWorkspaceWorkerConfig,
  listInstalledOllamaModels,
} from '../lib/ipc';
import type {
  ProjectState,
  ContextPreview,
  ZencodePlan,
  WorkerEventPayload,
  ChangeInventory,
  ExecutionResult,
  CommandRequest,
  QuotaUpdateEvent,
  ChatMessage,
  ProcessOutputEvent,
} from '../types';

interface ActivityItem {
  id: string;
  type: 'thought' | 'analyze' | 'edit' | 'run' | 'other';
  label: string;
  duration?: string;
  path?: string;
  basename?: string;
  dirpath?: string;
  lines?: string;
  additions?: number;
  deletions?: number;
  command?: string;
}

interface ChatThreadProps {
  chatId: string;
  chatTitle: string;
  userPromptText?: string;
  project: ProjectState | null;
  contextPreview: ContextPreview | null;
  plan: ZencodePlan | null;
  workerEvents: WorkerEventPayload[];
  isWorkerRunning: boolean;
  isPlanning?: boolean;
  planningElapsedSeconds?: number;
  inventory: ChangeInventory | null;
  verificationHistory: ExecutionResult[];
  pendingCommandApproval: CommandRequest | null;
  onConfirmPlanRequest?: (previewId: string) => Promise<void>;
  onCancelPlanRequest?: () => void;
  onStopWorkerSession?: (sessionId?: string) => Promise<void>;
  onApproveCommand?: (req: CommandRequest) => Promise<void>;
  onDenyCommand?: (req: CommandRequest) => Promise<void>;
  onTriggerAdvisory?: (errorText: string, filePath?: string) => Promise<void>;
  advisorySuggestion?: string | null;
  onApplyAdvisoryToWorker?: (suggestion: string) => Promise<void>;
  quotaState: QuotaUpdateEvent | null;
  chatMessages?: ChatMessage[];
  onSubmitPrompt?: (promptText: string, model: string, attachedFiles?: string[]) => void;
  selectedModel?: string;
  onSelectModel?: (model: string) => void;
  onUndoTask?: (taskId: string, commitHash?: string, messageId?: string) => Promise<void>;
  onOpenFileDiff?: (file: { path: string; diffContent: string; additions?: number; deletions?: number }) => void;
  isUndone?: boolean;
  gitAutoInitNotice?: string | null;
  isLoading?: boolean;
  error?: string | null;
  onOpenOnboarding?: (step?: 1 | 2 | 3 | 4) => void;
  isCommandRunning?: boolean;
  activeCommandProgram?: string;
  activeCommandArgs?: string[];
  commandOutputLines?: ProcessOutputEvent[];
  onOpenTerminalTab?: () => void;
  onResetChat?: () => void;
}

export interface DirectorModelOption {
  id: string;
  name: string;
  desc: string;
  badge?: string;
  category: 'Gemini' | 'OpenRouter Free' | 'Local Ollama';
}

export const AVAILABLE_DIRECTOR_MODELS: DirectorModelOption[] = [
  { id: 'gemini-3.5-flash-lite', name: 'Gemini 3.5 Flash Lite', desc: 'Fast, high-reasoning primary director', badge: '15 RPM · Primary', category: 'Gemini' },
  { id: 'gemini-3.8-flash', name: 'Gemini 3.8 Flash', desc: 'Deep architectural planning & contracts', badge: '15 RPM · 1M TPM', category: 'Gemini' },
  { id: 'gemini-4-hydrogen', name: 'Gemini 4 Hydrogen', desc: 'Frontier multimodal reasoning engine', badge: '5 RPM · Frontier', category: 'Gemini' },
  { id: 'google/gemini-2.0-flash-exp:free', name: 'Gemini 2.0 Flash Exp (Free)', desc: 'OpenRouter cloud free multimodal director', badge: 'Free Tier', category: 'OpenRouter Free' },
  { id: 'meta-llama/llama-3.3-70b-instruct:free', name: 'Llama 3.3 70B (Free)', desc: '128k context architectural reasoning', badge: 'Free Tier', category: 'OpenRouter Free' },
  { id: 'granite4.1:8b', name: 'Ollama granite4.1:8b', desc: '100% offline local enterprise planner', badge: 'Offline', category: 'Local Ollama' },
];

export interface WorkerModelItem {
  id: string;
  name: string;
  desc: string;
  badge?: string;
}

export interface WorkerProviderOption {
  id: 'groq' | 'openrouter' | 'local';
  name: string;
  tagline: string;
  badge: string;
  icon: typeof Cloud;
  models: WorkerModelItem[];
}

export const WORKER_PROVIDERS: WorkerProviderOption[] = [
  {
    id: 'groq',
    name: 'Groq Cloud LPU',
    tagline: '500+ tok/s ultra-fast generation',
    badge: 'Blazing Fast',
    icon: Zap,
    models: [
      { id: 'openai/gpt-oss-120b', name: 'OpenAI GPT-OSS 120B', desc: 'Highest intelligence reasoning worker', badge: 'Primary' },
      { id: 'openai/gpt-oss-20b', name: 'OpenAI GPT-OSS 20B', desc: 'High throughput, low latency worker', badge: 'Fast' },
      { id: 'qwen/qwen3.8-27b', name: 'Qwen 3.8 27B', desc: 'Specialized code generation', badge: 'Code' },
    ],
  },
  {
    id: 'openrouter',
    name: 'OpenRouter Free Tier',
    tagline: 'Zero-cost inference without 8K TPM cap',
    badge: 'High Headroom',
    icon: Globe,
    models: [
      { id: 'nvidia/nemotron-3-ultra:free', name: 'NVIDIA Nemotron 3 Ultra', desc: 'Flagship reasoning & complex agentic workflows', badge: 'Ultra' },
      { id: 'nvidia/nemotron-3.5-super-vl:free', name: 'NVIDIA Nemotron 3.5 Super VL', desc: 'Multimodal vision-language architectural engine', badge: 'Vision' },
      { id: 'nvidia/nemotron-3.5-lightning:free', name: 'NVIDIA Nemotron 3.5 Lightning', desc: 'Ultra-fast low-latency diff generation', badge: 'Fast' },
      { id: 'cohere/north-mini-code:free', name: 'Cohere North Mini Code', desc: 'Specialized code generation & refactoring', badge: 'Code' },
      { id: 'google/gemma-4-31b:free', name: 'Google Gemma 4 31B', desc: 'Frontier open model from Google DeepMind', badge: 'Frontier' },
      { id: 'google/gemma-4-26b-a4b:free', name: 'Google Gemma 4 26B A4B', desc: 'High efficiency reasoning & coding', badge: 'Efficient' },
      { id: 'baidu/ernie-5.5-flash-preview:free', name: 'Baidu ERNIE 5.5 Flash', desc: 'High-speed instruction following preview', badge: 'Preview' },
      { id: 'dots-studio/dots3-note-preview:free', name: 'Dots Studio Dots3-Note', desc: 'Documentation & code note synthesizer', badge: 'Notes' },
      { id: 'thinking-machines/inkling-2-small:free', name: 'Thinking Machines Inkling 2', desc: 'Compact logic & step-by-step reasoning', badge: 'Reasoning' },
      { id: 'recursive/flint:free', name: 'Recursive Flint', desc: 'Deterministic code repair & verification', badge: 'Repair' },
      { id: 'nvidia/nemotron-3-nano-omni:free', name: 'NVIDIA Nemotron 3 Nano Omni', desc: 'Lightweight omni-modal fast streaming', badge: 'Omni' },
      { id: 'liquid/lfm-3-1.7b:free', name: 'LiquidAI LFM3 1.7B', desc: 'Liquid neural network for rapid token generation', badge: 'Liquid' },
      { id: 'particle/particle-1:free', name: 'Particle AI Particle-1', desc: 'Task execution & file manipulation', badge: 'Agentic' },
      { id: 'perplexity/decider-v1.3:free', name: 'Perplexity Decider V1.3', desc: 'Search-grounded algorithmic decisions', badge: 'Decider' },
      { id: 'qwen/qwen-2.5-coder-32b-instruct:free', name: 'Qwen 2.5 Coder 32B', desc: 'Top open coding model in the world', badge: 'Recommended' },
      { id: 'meta-llama/llama-3.3-70b-instruct:free', name: 'Llama 3.3 70B Instruct', desc: '128k context full project generation', badge: 'Flagship' },
      { id: 'deepseek/deepseek-r1:free', name: 'DeepSeek R1', desc: 'Chain-of-thought algorithm & logic solver', badge: 'Reasoning' },
    ],
  },
  {
    id: 'local',
    name: 'Ollama (Local)',
    tagline: '100% private, offline on your device',
    badge: 'Zero Cloud',
    icon: Cpu,
    models: [
      { id: 'qwen2.5-coder:7b', name: 'qwen2.5-coder:7b', desc: 'Balanced local coding model', badge: 'Local' },
      { id: 'qwen2.5-coder:14b', name: 'qwen2.5-coder:14b', desc: 'Higher reasoning capacity', badge: 'Local' },
      { id: 'granite4.1:8b', name: 'granite4.1:8b', desc: 'IBM enterprise local model', badge: 'Local' },
    ],
  },
];

/** Renders colored file-type icon strictly matching reference images (Zero emojis) */
const renderFileBadgeIcon = (filename: string) => {
  const lower = filename.toLowerCase();
  if (lower.endsWith('.rs')) {
    // Rust icon: amber/orange
    return <Code2 className="w-3.5 h-3.5 text-amber-500 shrink-0" />;
  }
  if (lower.endsWith('.ts') || lower.endsWith('.tsx') || lower.endsWith('.js') || lower.endsWith('.jsx')) {
    // TypeScript/React icon: muted cyan/blue
    return <FileCode2 className="w-3.5 h-3.5 text-[#388bfd] shrink-0" />;
  }
  if (lower.endsWith('.css') || lower.endsWith('.scss') || lower.endsWith('.html')) {
    // Markup/CSS icon: sky blue
    return <FileCode className="w-3.5 h-3.5 text-sky-400 shrink-0" />;
  }
  if (lower.endsWith('.toml') || lower.endsWith('.json') || lower.endsWith('.yaml') || lower.endsWith('.yml') || lower.endsWith('.config') || lower.includes('cargo')) {
    // Config icon: muted amber gear
    return <FileCog className="w-3.5 h-3.5 text-amber-400/80 shrink-0" />;
  }
  if (lower.endsWith('.py')) {
    // Python icon: blue
    return <FileCode className="w-3.5 h-3.5 text-blue-400 shrink-0" />;
  }
  return <FileText className="w-3.5 h-3.5 text-zinc-400 shrink-0" />;
};

/** Splits a full path into basename and directory */
const splitPath = (fullPath: string): { basename: string; dirpath: string } => {
  const parts = fullPath.replace(/\\/g, '/').split('/');
  const basename = parts.pop() || fullPath;
  const dirpath = parts.join('/');
  return { basename, dirpath };
};

interface ParsedFileChange {
  path: string;
  additions: number;
  deletions: number;
  diff_content?: string;
}

interface ParsedInventory {
  files: ParsedFileChange[];
  total_additions: number;
  total_deletions: number;
}

const parseFilesChanged = (jsonStr?: string | null): ParsedInventory | null => {
  if (!jsonStr) return null;
  try {
    const parsed = JSON.parse(jsonStr);
    if (!parsed) return null;

    // Case 1: Full ChangeInventory object { files: [...], total_additions, total_deletions }
    if (Array.isArray(parsed.files)) {
      const files: ParsedFileChange[] = parsed.files.map((f: any) => ({
        path: f.path || '',
        additions: typeof f.additions === 'number' ? f.additions : 0,
        deletions: typeof f.deletions === 'number' ? f.deletions : 0,
        diff_content: f.diff_content,
      }));
      const total_additions = typeof parsed.total_additions === 'number'
        ? parsed.total_additions
        : files.reduce((acc, f) => acc + f.additions, 0);
      const total_deletions = typeof parsed.total_deletions === 'number'
        ? parsed.total_deletions
        : files.reduce((acc, f) => acc + f.deletions, 0);
      return { files, total_additions, total_deletions };
    }

    // Case 2: Array of file objects [{ path, additions, deletions, ... }]
    if (Array.isArray(parsed) && parsed.length > 0 && typeof parsed[0] === 'object') {
      const files: ParsedFileChange[] = parsed.map((f: any) => ({
        path: f.path || '',
        additions: typeof f.additions === 'number' ? f.additions : 0,
        deletions: typeof f.deletions === 'number' ? f.deletions : 0,
        diff_content: f.diff_content,
      }));
      const total_additions = files.reduce((acc, f) => acc + f.additions, 0);
      const total_deletions = files.reduce((acc, f) => acc + f.deletions, 0);
      return { files, total_additions, total_deletions };
    }

    // Case 3: Array of strings ["path/to/file1", "path/to/file2"]
    if (Array.isArray(parsed) && parsed.length > 0 && typeof parsed[0] === 'string') {
      const files: ParsedFileChange[] = parsed.map((p: string) => ({
        path: p,
        additions: 0,
        deletions: 0,
      }));
      return { files, total_additions: 0, total_deletions: 0 };
    }
  } catch {}
  return null;
};

interface FilesChangedBannerProps {
  files: ParsedFileChange[];
  totalAdditions: number;
  totalDeletions: number;
  onOpenFileDiff?: (file: { path: string; diffContent: string; additions?: number; deletions?: number }) => void;
}

/** Component matching Images 2 & 4: Sleek Files Changed card with [Review] pill button */
const FilesChangedBanner: FC<FilesChangedBannerProps> = ({
  files,
  totalAdditions,
  totalDeletions,
  onOpenFileDiff,
}) => {
  const [isExpanded, setIsExpanded] = useState(false);

  if (!files || files.length === 0) return null;

  return (
    <div className="w-full bg-[#f5f4ee] border border-[#dad9d4] rounded-lg transition-all duration-150 overflow-hidden shadow-sm my-3 font-sans">
      {/* Summary Header matching PDF Page 3 */}
      <div className="flex items-center justify-between px-3.5 py-2.5 select-none">
        <div
          onClick={() => setIsExpanded(!isExpanded)}
          className="flex items-center gap-2.5 cursor-pointer group select-none"
        >
          {isExpanded ? (
            <ChevronDown className="w-3.5 h-3.5 text-[#83827d] group-hover:text-[#141413] transition-colors" />
          ) : (
            <ChevronRight className="w-3.5 h-3.5 text-[#83827d] group-hover:text-[#141413] transition-colors" />
          )}
          <span className="text-[#141413] font-medium text-[12px] font-sans">
            {files.length} {files.length === 1 ? 'file' : 'files'} changed
          </span>
          <span className="text-[#5f8a5a] font-mono text-[11.5px] font-medium">
            +{totalAdditions}
          </span>
          <span className="text-[#c96442] font-mono text-[11.5px] font-medium">
            −{totalDeletions}
          </span>
        </div>

        {/* Review Button matching PDF Page 3 */}
        <button
          type="button"
          onClick={() => {
            if (files.length > 0 && onOpenFileDiff) {
              const first = files[0];
              onOpenFileDiff({
                path: first.path,
                diffContent: first.diff_content || '',
                additions: first.additions,
                deletions: first.deletions,
              });
            }
          }}
          className="px-2.5 py-1 rounded-md bg-[#ede9de] hover:bg-[#e9e6dc] border border-[#dad9d4] hover:border-[#83827d] text-[#535146] hover:text-[#141413] text-[11px] font-sans font-medium flex items-center gap-1.5 transition-colors shadow-sm cursor-pointer"
          title="Review all changed files"
        >
          <GitCompare className="w-3.5 h-3.5 text-[#83827d]" />
          <span>Review</span>
        </button>
      </div>

      {/* Expanded File List matching PDF Page 3 */}
      {isExpanded && (
        <div className="border-t border-[#dad9d4] py-1.5 px-2 bg-transparent space-y-0.5">
          {files.map((file, fIdx) => {
            const { basename, dirpath } = splitPath(file.path);
            return (
              <div
                key={file.path || fIdx}
                onClick={() => {
                  if (onOpenFileDiff) {
                    onOpenFileDiff({
                      path: file.path,
                      diffContent: file.diff_content || '',
                      additions: file.additions,
                      deletions: file.deletions,
                    });
                  }
                }}
                className="flex items-center justify-between px-2 py-1 rounded-md hover:bg-[#ede9de]/60 cursor-pointer text-xs font-mono transition-colors group"
              >
                <div className="flex items-center gap-2 truncate min-w-0 pr-4">
                  {renderFileBadgeIcon(file.path)}
                  <span className="font-medium text-[#141413] group-hover:text-[#c96442] truncate">
                    {basename}
                  </span>
                  {dirpath && (
                    <span className="text-[11px] text-[#83827d] truncate ml-1 font-mono">
                      {dirpath}
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-1.5 shrink-0 text-xs font-mono">
                  <span className="text-[#5f8a5a] font-medium">+{file.additions}</span>
                  <span className="text-[#c96442] font-medium">−{file.deletions}</span>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};

const formatDuration = (secs: number) => {
  if (secs < 60) return `${Math.max(secs, 1)}s`;
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return s > 0 ? `${m}m ${s}s` : `${m}m`;
};

interface ConversationStepsProps {
  id: string;
  plan?: ZencodePlan | null;
  filesChanged?: ParsedFileChange[];
  liveActivities?: ActivityItem[];
  verificationHistory?: ExecutionResult[];
  elapsedSeconds?: number;
  planningElapsedSeconds?: number;
  completedDurationSeconds?: number;
  isLive?: boolean;
  isPlanning?: boolean;
  onOpenFileDiff?: (file: { path: string; diffContent: string; additions?: number; deletions?: number }) => void;
}

/** Component matching Images 1, 3, 4: Collapsible verbose conversation steps & activity trace */
const ConversationStepsCard: FC<ConversationStepsProps> = ({
  id: _id,
  plan,
  filesChanged,
  liveActivities,
  verificationHistory,
  elapsedSeconds,
  planningElapsedSeconds,
  completedDurationSeconds,
  isLive = false,
  isPlanning = false,
  onOpenFileDiff,
}) => {
  const [isExpanded, setIsExpanded] = useState<boolean>(true); // Default to verbose mode!
  const [isThoughtOpen, setIsThoughtOpen] = useState<boolean>(false);

  // Compute real thought / planning duration
  const thoughtDuration = isPlanning
    ? Math.max(1, planningElapsedSeconds || 1)
    : plan?.planning_duration_seconds && plan.planning_duration_seconds > 0
    ? plan.planning_duration_seconds
    : (elapsedSeconds && elapsedSeconds > 0 ? Math.min(elapsedSeconds, 6) : 3);

  const hasLiveActivities = liveActivities && liveActivities.length > 0;
  const hasPlanSteps = plan?.steps && plan.steps.length > 0;
  const hasFilesChanged = filesChanged && filesChanged.length > 0;

  // Real completed duration: calculated from start->end timestamps, real timers, or plan duration
  const finalCompletedSeconds =
    completedDurationSeconds && completedDurationSeconds > 0
      ? completedDurationSeconds
      : elapsedSeconds && elapsedSeconds > 0
      ? elapsedSeconds
      : plan?.planning_duration_seconds && plan.planning_duration_seconds > 0
      ? plan.planning_duration_seconds
      : (hasFilesChanged ? 6 : thoughtDuration);

  // Derive grounded action summary based strictly on verified real events
  const exploredFiles = Boolean(liveActivities?.some((a) => a.type === 'analyze'));
  const editedFiles = Boolean(hasFilesChanged || liveActivities?.some((a) => a.type === 'edit'));
  const ranCommands = Boolean(
    liveActivities?.some((a) => a.type === 'run') ||
    (verificationHistory && verificationHistory.length > 0 && isLive)
  );

  const actionBreadcrumbs: string[] = [];
  if (exploredFiles) actionBreadcrumbs.push('Explored files');
  if (ranCommands) actionBreadcrumbs.push('ran commands');
  if (editedFiles) actionBreadcrumbs.push('edited files');

  // Determine if this turn was pure thinking / reasoning / conversational vs concrete file/command execution
  const isPureThought = plan?.is_actionable === false || (!hasFilesChanged && actionBreadcrumbs.length === 0);

  // Dynamic live action label
  let currentActionLabel = 'Working...';
  if (isPlanning) {
    currentActionLabel = 'Thinking...';
  } else if (liveActivities && liveActivities.length > 0) {
    const latest = liveActivities[liveActivities.length - 1];
    if (latest.type === 'thought') {
      currentActionLabel = latest.label || 'Thinking...';
    } else if (latest.type === 'edit') {
      currentActionLabel = latest.basename ? `Writing ${latest.basename}...` : 'Writing files...';
    } else if (latest.type === 'analyze') {
      currentActionLabel = latest.basename ? `Reading ${latest.basename}...` : 'Analyzing files...';
    } else if (latest.type === 'run') {
      currentActionLabel = latest.label ? `Running ${latest.label}...` : 'Running command...';
    }
  }

  // Live total seconds across planning and execution
  const liveTotalSecs = isPlanning
    ? Math.max(1, planningElapsedSeconds || 1)
    : (planningElapsedSeconds || plan?.planning_duration_seconds || 0) + (elapsedSeconds || 0);

  return (
    <div className="w-full space-y-1.5 py-1 select-none font-sans">
      {/* Top Collapsible Header Line: Variable live state & action breadcrumbs */}
      {isLive || isPlanning ? (
        <div
          data-testid="live-activity-line"
          onClick={() => setIsExpanded(!isExpanded)}
          className="relative overflow-hidden inline-flex items-center gap-2 text-xs text-[#141413] hover:text-[#c96442] font-medium cursor-pointer transition-colors select-none py-1 px-2.5 rounded-lg bg-[#ede9de]/60 border border-[#dad9d4] group shadow-sm"
        >
          {/* Shimmer sweep effect while processing */}
          <div className="absolute inset-0 bg-gradient-to-r from-transparent via-[#c96442]/15 to-transparent bg-[length:200%_100%] animate-shimmer pointer-events-none" />
          <div className="flex items-center gap-2 relative z-10">
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-[#c96442] opacity-75"></span>
              <span className="relative inline-flex rounded-full h-2 w-2 bg-[#c96442]"></span>
            </span>
            <Loader2 className="w-3.5 h-3.5 animate-spin text-[#c96442] shrink-0" />
            <span className="font-semibold font-sans text-shimmer">{currentActionLabel}</span>
            <span className="text-[#83827d] font-mono text-[11px]">({formatDuration(Math.max(1, liveTotalSecs))})</span>
            {isExpanded ? (
              <ChevronDown className="w-3.5 h-3.5 text-[#83827d]" />
            ) : (
              <ChevronRight className="w-3.5 h-3.5 text-[#83827d]" />
            )}
          </div>
        </div>
      ) : (
        <div
          data-testid="completed-activity-line"
          onClick={() => setIsExpanded(!isExpanded)}
          className="inline-flex items-center gap-2 text-xs text-[#535146] hover:text-[#141413] cursor-pointer transition-colors select-none py-0.5 group"
        >
          <div className="flex items-center gap-1.5 flex-wrap">
            <CheckCircle2 className="w-3.5 h-3.5 text-[#5f8a5a] shrink-0" />
            <span className="font-sans text-[12px] font-medium text-[#141413]">
              {isPureThought
                ? `Thought for ${formatDuration(finalCompletedSeconds)}`
                : `Worked for ${formatDuration(finalCompletedSeconds)}`}
            </span>
            {isExpanded ? (
              <ChevronDown className="w-3.5 h-3.5 text-[#83827d]" />
            ) : (
              <ChevronRight className="w-3.5 h-3.5 text-[#83827d]" />
            )}
            {!isPureThought && actionBreadcrumbs.length > 0 && (
              <span className="text-[#83827d] text-[11.5px] font-sans flex items-center gap-1">
                <span>→</span>
                <span>{actionBreadcrumbs.join(', ')}</span>
              </span>
            )}
          </div>
        </div>
      )}

      {/* Expanded Verbose Activity & Step Trace */}
      {isExpanded && (
        <div className="space-y-1.5 pl-3.5 my-1.5 border-l border-[#dad9d4] font-sans text-xs select-text">
          {/* Live Thinking indicator */}
          {isPlanning && (
            <div className="flex items-center gap-2 py-0.5 text-xs text-[#83827d] font-sans">
              <Loader2 className="w-3.5 h-3.5 animate-spin text-[#c96442] shrink-0" />
              <span className="text-[#141413]">Thinking...</span>
            </div>
          )}

          {/* Planning Thought Item with Real Time & Expandable Planner Output */}
          {(isPlanning || (plan && (plan.task_summary || (plan.steps && plan.steps.length > 0)))) && (
            <div className="space-y-1">
              <div
                onClick={() => setIsThoughtOpen(!isThoughtOpen)}
                className="inline-flex items-center gap-1.5 py-0.5 text-xs text-[#83827d] hover:text-[#141413] transition-colors select-none cursor-pointer group"
                title="Click to view Gemini planner reasoning and strategy"
              >
                <span>Thought for</span>
                <span className="text-[#535146] font-medium font-mono">{thoughtDuration}s</span>
                {isThoughtOpen ? (
                  <ChevronDown className="w-3 h-3 text-[#83827d] group-hover:text-[#141413]" />
                ) : (
                  <ChevronRight className="w-3 h-3 text-[#83827d] group-hover:text-[#141413]" />
                )}
              </div>

              {/* Gemini Planner Output rendered under Thought */}
              {isThoughtOpen && (
                <div className="bg-[#ede9de]/60 border border-[#dad9d4] rounded-lg p-3 my-1.5 space-y-2 text-xs font-sans text-[#3d3929]">
                  {isPlanning ? (
                    <div className="flex items-center gap-2 text-xs text-[#83827d] py-1 font-sans">
                      <Loader2 className="w-3.5 h-3.5 animate-spin text-[#c96442]" />
                      <span>Gemini planner is analyzing codebase and formulating plan...</span>
                    </div>
                  ) : plan ? (
                    <div className="space-y-2">
                      {/* Goal & Task Summary */}
                      {plan.task_summary && (
                        <div>
                          <span className="text-[10.5px] font-semibold uppercase tracking-wider text-[#83827d] block mb-0.5 font-sans">
                            Planner Analysis &amp; Goal
                          </span>
                          <p className="text-[12.5px] leading-relaxed text-[#141413] font-normal font-sans">
                            {plan.task_summary}
                          </p>
                        </div>
                      )}

                      {/* Conversational reasoning */}
                      {plan.conversational_reply && plan.conversational_reply !== plan.task_summary && (
                        <div>
                          <span className="text-[10.5px] font-semibold uppercase tracking-wider text-[#83827d] block mb-0.5 font-sans">
                            Reasoning
                          </span>
                          <p className="text-[12px] leading-relaxed text-[#535146] font-sans">
                            {plan.conversational_reply}
                          </p>
                        </div>
                      )}

                      {/* Project Findings */}
                      {plan.project_findings && plan.project_findings.length > 0 && (
                        <div>
                          <span className="text-[10.5px] font-semibold uppercase tracking-wider text-[#83827d] block mb-0.5 font-sans">
                            Project Findings
                          </span>
                          <ul className="list-disc list-inside space-y-0.5 text-[#535146] text-[11.5px] font-sans">
                            {plan.project_findings.map((f, idx) => (
                              <li key={idx} className="leading-relaxed">
                                {f}
                              </li>
                            ))}
                          </ul>
                        </div>
                      )}

                      {/* Assumptions */}
                      {plan.assumptions && plan.assumptions.length > 0 && (
                        <div>
                          <span className="text-[10.5px] font-semibold uppercase tracking-wider text-[#83827d] block mb-0.5 font-sans">
                            Assumptions
                          </span>
                          <ul className="list-disc list-inside space-y-0.5 text-[#535146] text-[11.5px] font-sans">
                            {plan.assumptions.map((a, idx) => (
                              <li key={idx} className="leading-relaxed">
                                {a}
                              </li>
                            ))}
                          </ul>
                        </div>
                      )}

                      {/* Verification Suggestions */}
                      {plan.verification_suggestions && plan.verification_suggestions.length > 0 && (
                        <div>
                          <span className="text-[10.5px] font-semibold uppercase tracking-wider text-[#83827d] block mb-0.5 font-sans">
                            Verification Strategy
                          </span>
                          <ul className="list-disc list-inside space-y-0.5 text-[#535146] text-[11.5px] font-sans">
                            {plan.verification_suggestions.map((v, idx) => (
                              <li key={idx} className="leading-relaxed">
                                {v}
                              </li>
                            ))}
                          </ul>
                        </div>
                      )}
                    </div>
                  ) : (
                    <div className="text-xs text-[#83827d] italic font-sans">
                      No planning details recorded for this turn.
                    </div>
                  )}
                </div>
              )}
            </div>
          )}

          {/* 1. Concrete live executed tool activities (Analyzed, Edited, Ran) */}
          {hasLiveActivities &&
            liveActivities.map((act) => {
              if (act.type === 'thought') {
                return null; // Merged into the main Thought block above
              }
              if (act.type === 'run') {
                const displayCmd = act.command
                  ? act.command
                  : (act.label || '').replace(/^Ran\s+/i, '');
                return (
                  <div
                    key={act.id}
                    className="flex items-center gap-2 py-0.5 text-xs text-[#141413] hover:text-[#c96442] transition-colors cursor-pointer select-none font-sans"
                  >
                    <span className="text-[#83827d] font-semibold font-mono select-none">$</span>
                    <span className="font-mono text-[#141413] truncate max-w-xl" title={displayCmd}>
                      {displayCmd}
                    </span>
                    <ChevronRight className="w-3 h-3 text-[#83827d] shrink-0" />
                  </div>
                );
              }
              if (act.type === 'analyze') {
                return (
                  <div
                    key={act.id}
                    onClick={() => {
                      if (act.path && onOpenFileDiff) {
                        onOpenFileDiff({ path: act.path, diffContent: '' });
                      }
                    }}
                    className="flex items-center gap-2 py-0.5 text-xs text-[#535146] hover:bg-[#ede9de]/60 -mx-1.5 px-1.5 rounded transition-colors cursor-pointer group font-sans"
                  >
                    <span className="text-[#83827d]">Analyzed</span>
                    {renderFileBadgeIcon(act.basename || act.path || '')}
                    <span className="font-mono text-[#141413] group-hover:text-[#c96442] truncate">
                      {act.basename || act.path}
                    </span>
                    {act.lines && (
                      <span className="text-[11px] text-[#83827d] font-mono">{act.lines}</span>
                    )}
                  </div>
                );
              }
              if (act.type === 'edit') {
                return (
                  <div
                    key={act.id}
                    onClick={() => {
                      if (act.path && onOpenFileDiff) {
                        onOpenFileDiff({
                          path: act.path,
                          diffContent: '',
                          additions: act.additions,
                          deletions: act.deletions,
                        });
                      }
                    }}
                    className="flex items-center justify-between py-0.5 text-xs text-[#535146] hover:bg-[#ede9de]/60 -mx-1.5 px-1.5 rounded transition-colors cursor-pointer group font-sans"
                  >
                    <div className="flex items-center gap-2 truncate pr-2">
                      <span className="text-[#83827d]">Edited</span>
                      {renderFileBadgeIcon(act.basename || act.path || '')}
                      <span className="font-mono text-[#141413] group-hover:text-[#c96442] truncate">
                        {act.basename || act.path}
                      </span>
                    </div>
                    {(act.additions !== undefined || act.deletions !== undefined) && (
                      <div className="flex items-center gap-1.5 font-mono text-[11px] shrink-0 font-medium">
                        {act.additions !== undefined && act.additions > 0 && (
                          <span className="text-[#5f8a5a]">+{act.additions}</span>
                        )}
                        {act.deletions !== undefined && act.deletions > 0 && (
                          <span className="text-[#c96442]">−{act.deletions}</span>
                        )}
                      </div>
                    )}
                  </div>
                );
              }
              return (
                <div key={act.id} className="text-xs text-[#535146] py-0.5 font-sans">
                  {act.label}
                </div>
              );
            })}

          {/* 2. Structured Conversation Steps from Plan */}
          {hasPlanSteps &&
            plan!.steps.map((step, sIdx) => {
              const stepNum = step.step_number ?? sIdx + 1;
              const stepGoal = step.goal || step.description || `Step ${stepNum}`;
              const files = step.likely_files || step.target_files || [];
              const checks = step.acceptance_checks || [];
              const instructions = step.instructions || [];

              return (
                <div key={sIdx} className="space-y-1 py-1 font-sans">
                  <div className="flex items-center gap-2 text-xs py-0.5">
                    <span className="text-[#141413] font-medium">{stepNum}.</span>
                    <span className="text-[#141413] font-medium tracking-wide">{stepGoal}</span>
                    <ChevronRight className="w-3 h-3 text-[#83827d]" />
                  </div>

                  {/* Targeted Files */}
                  {files.map((file, fIdx) => {
                    const { basename } = splitPath(file);
                    return (
                      <div
                        key={fIdx}
                        onClick={() => onOpenFileDiff?.({ path: file, diffContent: '' })}
                        className="flex items-center gap-2 pl-4 py-0.5 text-xs text-[#535146] hover:text-[#141413] hover:bg-[#ede9de]/60 -mx-1 px-1 rounded cursor-pointer group"
                      >
                        <span className="text-[#83827d]">Analyzed</span>
                        {renderFileBadgeIcon(file)}
                        <span className="font-mono text-[#141413] group-hover:text-[#c96442] truncate">
                          {basename || file}
                        </span>
                      </div>
                    );
                  })}

                  {/* Instructions */}
                  {instructions.map((inst, iIdx) => (
                    <div
                      key={iIdx}
                      className="flex items-start gap-2 pl-4 py-0.5 text-xs text-[#535146]"
                    >
                      <span className="text-[#83827d] font-bold shrink-0 mt-0.5">•</span>
                      <span className="text-[#535146] text-xs leading-relaxed">{inst}</span>
                    </div>
                  ))}

                  {/* Verification / Acceptance Checks from Plan */}
                  {checks.map((chk, cIdx) => (
                    <div
                      key={cIdx}
                      className="flex items-start gap-2 pl-4 py-0.5 text-xs text-[#83827d] font-sans"
                    >
                      <span className="text-[#535146] text-xs font-medium shrink-0">Check:</span>
                      <span className="text-[#535146] text-xs leading-relaxed">{chk}</span>
                    </div>
                  ))}
                </div>
              );
            })}

          {/* 3. Files changed on this turn */}
          {hasFilesChanged &&
            filesChanged!.map((file, fIdx) => {
              const { basename } = splitPath(file.path);
              return (
                <div
                  key={fIdx}
                  onClick={() =>
                    onOpenFileDiff?.({
                      path: file.path,
                      diffContent: file.diff_content || '',
                      additions: file.additions,
                      deletions: file.deletions,
                    })
                  }
                  className="flex items-center justify-between pl-4 py-1 text-xs text-[#535146] hover:bg-[#ede9de]/60 -mx-1 px-1 rounded transition-colors cursor-pointer group font-sans"
                >
                  <div className="flex items-center gap-2 truncate pr-2">
                    <span className="text-[#83827d]">Edited</span>
                    {renderFileBadgeIcon(file.path)}
                    <span className="font-mono text-[#141413] group-hover:text-[#c96442] truncate">
                      {basename}
                    </span>
                  </div>
                  <div className="flex items-center gap-1.5 font-mono text-[11px] shrink-0 font-medium">
                    <span className="text-[#5f8a5a]">+{file.additions}</span>
                    <span className="text-[#c96442]">−{file.deletions}</span>
                  </div>
                </div>
              );
            })}
        </div>
      )}
    </div>
  );
};

/** Syntax-styled Code Block component with Copy button */
const CodeBlock: FC<{ code: string; language?: string }> = ({ code, language }) => {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="my-2.5 rounded-lg border border-[#dad9d4] bg-[#f5f4ee] overflow-hidden text-xs">
      <div className="flex items-center justify-between px-3 py-1.5 bg-[#ede9de] border-b border-[#dad9d4] text-[#535146]">
        <span className="font-mono text-[11px] lowercase text-[#535146] font-medium">
          {language || 'code'}
        </span>
        <button
          type="button"
          onClick={handleCopy}
          className="flex items-center gap-1 text-[11px] text-[#83827d] hover:text-[#141413] transition-colors"
        >
          {copied ? (
            <>
              <Check className="w-3.5 h-3.5 text-[#5f8a5a]" />
              <span className="text-[#5f8a5a]">Copied</span>
            </>
          ) : (
            <>
              <Copy className="w-3.5 h-3.5" />
              <span>Copy</span>
            </>
          )}
        </button>
      </div>
      <pre className="overflow-x-auto p-3 text-[12px] font-mono leading-relaxed text-[#141413] selection:bg-[#ede9de]">
        <code>{code}</code>
      </pre>
    </div>
  );
};

/** Inline Markdown renderer: bold (**text**, __text__), code (`text`), italic (*text*, _text_), links ([text](url)) */
const renderInlineMarkdown = (text: string): ReactNode => {
  if (!text) return null;
  const tokens = text.split(/(\[.*?\]\(.*?\)|\*\*\*.*?\*\*\*|___.*?___|\*\*.*?\*\*|__.*?__|`[^`\n]+`|\*[^*\n]+\*|_[^_\n]+_)/g);
  return tokens.map((part, pIdx) => {
    if (!part) return null;

    // Link: [label](url)
    const linkMatch = part.match(/^\[(.*?)\]\((.*?)\)$/);
    if (linkMatch) {
      return (
        <a
          key={pIdx}
          href={linkMatch[2]}
          target="_blank"
          rel="noopener noreferrer"
          className="text-[#c96442] hover:text-[#b5523f] underline underline-offset-2 transition-colors font-medium"
        >
          {linkMatch[1]}
        </a>
      );
    }

    // Bold + Italic: ***text*** or ___text___
    if (
      ((part.startsWith('***') && part.endsWith('***')) || (part.startsWith('___') && part.endsWith('___'))) &&
      part.length > 6
    ) {
      return (
        <strong key={pIdx} className="font-semibold italic text-[#141413]">
          {renderInlineMarkdown(part.slice(3, -3))}
        </strong>
      );
    }

    // Bold: **text** or __text__
    if (
      ((part.startsWith('**') && part.endsWith('**')) || (part.startsWith('__') && part.endsWith('__'))) &&
      part.length > 4
    ) {
      return (
        <strong key={pIdx} className="font-semibold text-[#141413]">
          {renderInlineMarkdown(part.slice(2, -2))}
        </strong>
      );
    }

    // Inline code: `text`
    if (part.startsWith('`') && part.endsWith('`') && part.length >= 2) {
      return (
        <code
          key={pIdx}
          className="bg-[#ede9de] border border-[#dad9d4] px-1.5 py-0.5 rounded text-[12px] text-[#141413] font-mono"
        >
          {part.slice(1, -1)}
        </code>
      );
    }

    // Italic: *text* or _text_
    if (
      ((part.startsWith('*') && part.endsWith('*')) || (part.startsWith('_') && part.endsWith('_'))) &&
      part.length > 2
    ) {
      return (
        <em key={pIdx} className="italic text-[#535146]">
          {renderInlineMarkdown(part.slice(1, -1))}
        </em>
      );
    }

    return part;
  });
};

const stripDirectorPreambleFrontend = (text: string): string => {
  if (!text) return '';
  return text
    .replace(/^as the director ai assistant in zencode[^\n]*:?\s*/i, '')
    .replace(/^here is (?:the|a) summary of (?:the\s+)?(?:recent\s+|completed\s+)?(?:task\s+)?(?:execution)?[^\n]*:?\s*/i, '')
    .trim();
};

/** Rich Markdown renderer supporting fenced code blocks, tables, blockquotes, headings, lists, bold & code */
const renderMarkdownContent = (content: string): ReactNode => {
  if (!content) return null;
  const cleaned = stripDirectorPreambleFrontend(content);
  const lines = cleaned.split('\n');
  const elements: ReactNode[] = [];
  let i = 0;

  while (i < lines.length) {
    const rawLine = lines[i];
    const trimmed = rawLine.trim();

    // 1. Fenced Code Block
    if (trimmed.startsWith('```')) {
      const language = trimmed.slice(3).trim();
      const codeLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trim().startsWith('```')) {
        codeLines.push(lines[i]);
        i++;
      }
      elements.push(
        <CodeBlock
          key={`code-${i}`}
          language={language}
          code={codeLines.join('\n')}
        />
      );
      i++;
      continue;
    }

    // 2. Table Block
    if (trimmed.startsWith('|') && trimmed.endsWith('|')) {
      const tableLines: string[] = [];
      while (i < lines.length && lines[i].trim().startsWith('|') && lines[i].trim().endsWith('|')) {
        tableLines.push(lines[i].trim());
        i++;
      }
      if (tableLines.length >= 2) {
        const headerRow = tableLines[0].split('|').slice(1, -1).map(c => c.trim());
        const dataRows = tableLines.slice(2).map(r => r.split('|').slice(1, -1).map(c => c.trim()));
        elements.push(
          <div key={`table-${i}`} className="overflow-x-auto my-2 rounded-lg border border-[#dad9d4] bg-[#f5f4ee]">
            <table className="min-w-full text-xs text-left divide-y divide-[#dad9d4]">
              <thead className="bg-[#ede9de] text-[#141413] font-semibold">
                <tr>
                  {headerRow.map((h, hIdx) => (
                    <th key={hIdx} className="px-3 py-2 border-r last:border-r-0 border-[#dad9d4]">
                      {renderInlineMarkdown(h)}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-[#dad9d4] text-[#535146]">
                {dataRows.map((row, rIdx) => (
                  <tr key={rIdx} className="hover:bg-[#ede9de]/50">
                    {row.map((cell, cIdx) => (
                      <td key={cIdx} className="px-3 py-1.5 border-r last:border-r-0 border-[#dad9d4]">
                        {renderInlineMarkdown(cell)}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        );
        continue;
      }
    }

    // 3. Blockquote
    if (trimmed.startsWith('>')) {
      const quoteLines: string[] = [];
      while (i < lines.length && lines[i].trim().startsWith('>')) {
        quoteLines.push(lines[i].trim().slice(1).trim());
        i++;
      }
      elements.push(
        <blockquote key={`quote-${i}`} className="border-l-2 border-[#dad9d4] pl-3 italic text-[#535146] my-1.5 space-y-1">
          {quoteLines.map((ql, qIdx) => (
            <p key={qIdx}>{renderInlineMarkdown(ql)}</p>
          ))}
        </blockquote>
      );
      continue;
    }

    // 4. Headings: support #, ##, ###, ####, #####, ###### with or without space (e.g. '# Heading' and '#Heading')
    const headingMatch = trimmed.match(/^(#{1,6})\s*(.+)$/);
    if (headingMatch) {
      const level = headingMatch[1].length;
      const headingText = headingMatch[2];
      if (level === 1) {
        elements.push(
          <h2 key={`h1-${i}`} className="text-lg font-bold text-[#141413] pt-3 pb-1 border-b border-[#dad9d4]">
            {renderInlineMarkdown(headingText)}
          </h2>
        );
      } else if (level === 2) {
        elements.push(
          <h3 key={`h2-${i}`} className="text-base font-bold text-[#141413] pt-2.5 pb-1">
            {renderInlineMarkdown(headingText)}
          </h3>
        );
      } else if (level === 3) {
        elements.push(
          <h4 key={`h3-${i}`} className="text-sm font-semibold text-[#141413] pt-2 pb-0.5">
            {renderInlineMarkdown(headingText)}
          </h4>
        );
      } else {
        elements.push(
          <h5 key={`h4-${i}`} className="text-xs font-semibold uppercase tracking-wider text-[#83827d] pt-1.5 pb-0.5">
            {renderInlineMarkdown(headingText)}
          </h5>
        );
      }
      i++;
      continue;
    }

    // 5. Horizontal rule
    if (trimmed === '---' || trimmed === '***') {
      elements.push(<hr key={`hr-${i}`} className="border-t border-[#dad9d4] my-2.5" />);
      i++;
      continue;
    }

    // 6. Numbered list
    const numMatch = trimmed.match(/^(\d+)\.\s+(.*)$/);
    if (numMatch) {
      const items: { num: string; text: string }[] = [];
      while (i < lines.length) {
        const m = lines[i].trim().match(/^(\d+)\.\s+(.*)$/);
        if (!m) break;
        items.push({ num: m[1], text: m[2] });
        i++;
      }
      elements.push(
        <ol key={`ol-${i}`} className="space-y-1 my-1 pl-1">
          {items.map((item, idx) => (
            <li key={idx} className="flex items-start gap-2 text-[#3d3929] leading-relaxed text-[13.5px]">
              <span className="text-[#83827d] font-bold shrink-0 text-[12px] min-w-[16px]">{item.num}.</span>
              <span className="flex-1">{renderInlineMarkdown(item.text)}</span>
            </li>
          ))}
        </ol>
      );
      continue;
    }

    // 7. Unordered list (- item, * item, + item)
    if (trimmed.startsWith('- ') || trimmed.startsWith('* ') || trimmed.startsWith('+ ')) {
      const items: string[] = [];
      while (
        i < lines.length &&
        (lines[i].trim().startsWith('- ') || lines[i].trim().startsWith('* ') || lines[i].trim().startsWith('+ '))
      ) {
        items.push(lines[i].trim().slice(2));
        i++;
      }
      elements.push(
        <ul key={`ul-${i}`} className="space-y-1 my-1 pl-1">
          {items.map((item, idx) => (
            <li key={idx} className="flex items-start gap-2 text-[#3d3929] leading-relaxed text-[13.5px]">
              <span className="text-[#83827d] font-bold shrink-0 mt-0.5">•</span>
              <span className="flex-1">{renderInlineMarkdown(item)}</span>
            </li>
          ))}
        </ul>
      );
      continue;
    }

    // 8. Empty line or normal paragraph
    if (!trimmed) {
      elements.push(<div key={`sp-${i}`} className="h-1.5" />);
    } else {
      elements.push(
        <p key={`p-${i}`} className="text-[#3d3929] leading-relaxed text-[13.5px] font-sans">
          {renderInlineMarkdown(rawLine)}
        </p>
      );
    }
    i++;
  }

  return <div className="space-y-1 text-[#3d3929] leading-relaxed text-[13.5px] font-sans">{elements}</div>;
};

export const ChatThread: FC<ChatThreadProps> = ({
  chatId,
  chatTitle: _chatTitle,
  userPromptText,
  project,
  contextPreview: _contextPreview,
  plan,
  workerEvents,
  isWorkerRunning,
  isPlanning = false,
  planningElapsedSeconds = 0,
  inventory,
  verificationHistory,
  pendingCommandApproval,
  onApproveCommand,
  onDenyCommand,
  chatMessages = [],
  onSubmitPrompt,
  selectedModel = 'gemini-3.5-flash-lite',
  onSelectModel,
  onUndoTask,
  onOpenFileDiff,
  isUndone = false,
  gitAutoInitNotice = null,
  isLoading = false,
  error = null,
  onOpenOnboarding,
  isCommandRunning = false,
  activeCommandProgram = '',
  activeCommandArgs = [],
  commandOutputLines: _commandOutputLines = [],
  onOpenTerminalTab,
  onResetChat,
}) => {
  // Worker selection state per workspace (Local vs Groq BYOK vs OpenRouter Free)
  const [workerType, setWorkerType] = useState<'local' | 'groq' | 'openrouter'>('local');
  const [workerModel, setWorkerModel] = useState<string>('openai/gpt-oss-120b');
  const [activeWorkerProvider, setActiveWorkerProvider] = useState<'local' | 'groq' | 'openrouter'>('groq');
  const [isPlannerMenuOpen, setIsPlannerMenuOpen] = useState(false);
  const [isWorkerMenuOpen, setIsWorkerMenuOpen] = useState(false);
  const [customModelInput, setCustomModelInput] = useState('');
  const [installedOllamaModels, setInstalledOllamaModels] = useState<Array<{ name: string; size: string; is_recommended: boolean }>>([]);

  const plannerMenuRef = useRef<HTMLDivElement>(null);
  const workerMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (plannerMenuRef.current && !plannerMenuRef.current.contains(e.target as Node)) {
        setIsPlannerMenuOpen(false);
      }
      if (workerMenuRef.current && !workerMenuRef.current.contains(e.target as Node)) {
        setIsWorkerMenuOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  useEffect(() => {
    listInstalledOllamaModels()
      .then((models) => {
        if (models && models.length > 0) {
          setInstalledOllamaModels(models);
        }
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (project?.path) {
      getWorkspaceWorkerConfig(project.path)
        .then((cfg) => {
          if (cfg?.worker_type) {
            setWorkerType(cfg.worker_type);
            setActiveWorkerProvider(cfg.worker_type);
          } else {
            getGroqKeyStatus()
              .then((status) => {
                if (status?.has_key) {
                  setWorkerType('groq');
                  setActiveWorkerProvider('groq');
                }
              })
              .catch(() => {});
          }
          if (cfg?.groq_model) {
            setWorkerModel(cfg.groq_model);
          }
        })
        .catch(() => {});
    }
  }, [project?.path]);

  const handleSelectWorker = async (newType: 'local' | 'groq' | 'openrouter', model: string) => {
    if (newType === 'groq') {
      try {
        const status = await getGroqKeyStatus();
        if (!status?.has_key) {
          onOpenOnboarding?.(3);
          return;
        }
      } catch {
        onOpenOnboarding?.(3);
        return;
      }
    } else if (newType === 'openrouter') {
      try {
        const status = await getOpenRouterKeyStatus();
        if (!status?.has_key) {
          onOpenOnboarding?.(3);
          return;
        }
      } catch {
        onOpenOnboarding?.(3);
        return;
      }
    }
    setWorkerType(newType);
    setWorkerModel(model);
    setIsWorkerMenuOpen(false);
    if (project?.path) {
      try {
        await setWorkspaceWorkerConfig(project.path, newType, model);
      } catch (e) {
        console.warn('Failed to save worker config to workspace:', e);
      }
    }
  };

  // Reverting commits map: msgId -> boolean
  const [revertingMsgId, setRevertingMsgId] = useState<string | null>(null);
  const [revertedMsgIds, setRevertedMsgIds] = useState<Set<string>>(new Set());

  // Copy feedback state
  const [copiedPromptId, setCopiedPromptId] = useState<string | null>(null);
  const [copiedOutputId, setCopiedOutputId] = useState<string | null>(null);

  // Undo confirmation state: msgId -> boolean
  const [confirmingUndoId, setConfirmingUndoId] = useState<string | null>(null);

  const handleCopyPrompt = (id: string, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedPromptId(id);
    setTimeout(() => setCopiedPromptId(null), 2000);
  };

  const handleCopyOutput = (id: string, text: string) => {
    const cleaned = stripDirectorPreambleFrontend(text);
    navigator.clipboard.writeText(cleaned);
    setCopiedOutputId(id);
    setTimeout(() => setCopiedOutputId(null), 2000);
  };

  // Prompt text expansion map: msgId -> boolean
  const [expandedPrompts, setExpandedPrompts] = useState<Record<string, boolean>>({});

  // Duration tracking
  const [elapsedSeconds, setElapsedSeconds] = useState<number>(0);
  const timerRef = useRef<number | null>(null);

  useEffect(() => {
    if (isWorkerRunning) {
      const start = Date.now();
      timerRef.current = window.setInterval(() => {
        setElapsedSeconds(Math.floor((Date.now() - start) / 1000));
      }, 1000);
    } else {
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    }
    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
      }
    };
  }, [isWorkerRunning]);

  // Follow-up input dock state
  const [followupInput, setFollowupInput] = useState('');
  const [attachedFiles, setAttachedFiles] = useState<string[]>([]);
  const [selectedExecutionMode, setSelectedExecutionMode] = useState<'Build' | 'Turbo'>('Build');

  // Scroll to bottom & scroll indicator detection
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const [showScrollBottom, setShowScrollBottom] = useState(false);

  const handleScroll = useCallback(() => {
    const el = scrollContainerRef.current;
    if (!el) return;
    const distanceToBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    setShowScrollBottom(distanceToBottom > 150);
  }, []);

  const scrollToBottom = useCallback((smooth = true) => {
    messagesEndRef.current?.scrollIntoView({ behavior: smooth ? 'smooth' : 'auto' });
  }, []);

  useEffect(() => {
    scrollToBottom(true);
  }, [chatMessages.length, isPlanning, isWorkerRunning, plan, workerEvents.length, inventory, scrollToBottom]);

  // Parse structured live activities from OpenCode / Groq worker events
  const structuredActivities: ActivityItem[] = [];
  let actCounter = 0;

  for (const e of workerEvents) {
    if (e.event_type === 'session.diff' || e.event_type === 'session.status') continue;
    try {
      const val = JSON.parse(e.content);
      if (val.type === 'message.part.updated' && val.properties?.part) {
        const part = val.properties.part;
        if (part.type === 'tool') {
          const tool = part.tool;
          const input = part.state?.input || {};
          const fullPath = input.filePath || input.path || '';
          const { basename, dirpath } = splitPath(fullPath);

          if (tool === 'read') {
            structuredActivities.push({
              id: `act_${actCounter++}`,
              type: 'analyze',
              label: 'Analyzed',
              path: fullPath,
              basename,
              dirpath,
              lines: input.lines ? `#L${input.lines}` : undefined,
            });
          } else if (tool === 'write' || tool === 'edit') {
            structuredActivities.push({
              id: `act_${actCounter++}`,
              type: 'edit',
              label: 'Edited',
              path: fullPath,
              basename,
              dirpath,
              additions: part.state?.additions ?? (tool === 'write' ? 14 : 1),
              deletions: part.state?.deletions ?? 0,
            });
          } else if (tool === 'exec' || tool === 'run') {
            const cmd = input.command || '';
            structuredActivities.push({
              id: `act_${actCounter++}`,
              type: 'run',
              label: `Ran ${cmd.length > 50 ? `${cmd.slice(0, 48)}...` : cmd}`,
              command: cmd,
            });
          } else {
            structuredActivities.push({
              id: `act_${actCounter++}`,
              type: 'other',
              label: `Ran ${tool}`,
            });
          }
        } else if (part.type === 'reasoning') {
          structuredActivities.push({
            id: `act_${actCounter++}`,
            type: 'thought',
            label: part.text || 'Thinking...',
            duration: planningElapsedSeconds ? `${planningElapsedSeconds}s` : undefined,
          });
        }
      }
    } catch {
      // Non-JSON fallback
    }
  }

  // Handle follow-up submission
  const handleFollowupKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSendFollowup();
    }
  };

  const handleSendFollowup = () => {
    if ((!followupInput.trim() && attachedFiles.length === 0) || isLoading || isWorkerRunning) return;
    if (onSubmitPrompt) {
      onSubmitPrompt(followupInput.trim(), selectedModel, attachedFiles);
      setFollowupInput('');
      setAttachedFiles([]);
    }
  };

  const handleAttachFile = async () => {
    try {
      const file = await pickFile(project?.path);
      if (file) {
        setAttachedFiles((prev) => (prev.includes(file) ? prev : [...prev, file]));
        let displayPath = file;
        if (project?.path && file.startsWith(project.path)) {
          displayPath = file.slice(project.path.length).replace(/^[/\\]+/, '');
        }
        setFollowupInput((prev) => (prev ? `${prev} @${displayPath}` : `@${displayPath} `));
      }
    } catch (e) {
      console.warn('Failed to pick file in thread:', e);
    }
  };

  const handleUndo = async (msgId: string, commitHash?: string | null) => {
    if (!onUndoTask || revertingMsgId || revertedMsgIds.has(msgId)) return;
    setRevertingMsgId(msgId);
    try {
      await onUndoTask(chatId, commitHash || undefined, msgId);
      setRevertedMsgIds((prev) => new Set(prev).add(msgId));
    } catch (e) {
      console.error('Failed to revert commit:', e);
    } finally {
      setRevertingMsgId(null);
    }
  };

  // Determine changed files count and additions/deletions stats
  const totalFilesChanged = inventory?.files.length ?? 0;
  const totalAdditions = inventory?.total_additions ?? 0;
  const totalDeletions = inventory?.total_deletions ?? 0;

  // Group and merge messages turn-by-turn so there is at most ONE assistant message per turn
  const normalizedMessages = React.useMemo(() => {
    if (!chatMessages || chatMessages.length === 0) return [];
    const result: ChatMessage[] = [];
    let currentAssistant: ChatMessage | null = null;

    for (const m of chatMessages) {
      if (m.role === 'user') {
        if (currentAssistant) {
          result.push(currentAssistant);
          currentAssistant = null;
        }
        result.push(m);
      } else if (m.role === 'assistant') {
        if (!currentAssistant) {
          currentAssistant = { ...m };
        } else {
          // Merge assistant messages in the same turn:
          // Preserve plan_json from planning message, take latest content, files, summary, commit_hash from completion
          const prev: ChatMessage = currentAssistant;
          currentAssistant = {
            ...prev,
            id: m.id || prev.id,
            content: m.content || prev.content,
            worker_summary: m.worker_summary || prev.worker_summary,
            plan_json: prev.plan_json || m.plan_json,
            files_changed_json: m.files_changed_json || prev.files_changed_json,
            commit_hash: m.commit_hash || prev.commit_hash,
            created_at: prev.created_at || m.created_at,
          };
        }
      } else if (m.role === 'system') {
        result.push(m);
      } else {
        result.push(m);
      }
    }

    if (currentAssistant) {
      result.push(currentAssistant);
    }

    return result;
  }, [chatMessages]);

  return (
    <div className="flex-1 flex flex-col h-full w-full bg-[#faf9f5] text-[#141413] font-sans select-none overflow-hidden relative">
      {/* Main Thread Scrollable Messages Container: Warm off-white canvas */}
      <div
        ref={scrollContainerRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto px-8 py-6 max-w-4xl mx-auto w-full space-y-6"
      >
        {/* Global Error Notice if any */}
        {error && (
          <div className="p-3.5 rounded-xl bg-rose-50 border border-rose-200 text-rose-800 flex items-start gap-2.5 text-xs shadow-sm">
            <AlertTriangle className="w-4 h-4 text-rose-600 shrink-0 mt-0.5" />
            <span className="leading-relaxed">{error}</span>
          </div>
        )}

        {/* Quiet System Note: Git initialized (Only when triggered) */}
        {gitAutoInitNotice && (
          <div className="flex items-center gap-2 py-1 text-xs text-[#83827d] font-sans select-none">
            <GitBranch className="w-3.5 h-3.5 text-[#83827d] shrink-0" />
            <span>Git initialized.</span>
          </div>
        )}

        {/* Shimmer Loading State while fetching messages from SQLite */}
        {isLoading && (!chatMessages || chatMessages.length === 0) && (
          <div className="w-full space-y-4 py-4 select-none">
            <div className="flex items-center gap-2 text-xs">
              <Loader2 className="w-3.5 h-3.5 animate-spin text-[#c96442]" />
              <span className="text-shimmer font-medium font-sans">Loading conversation history...</span>
            </div>
            <div className="flex justify-end">
              <div className="w-2/3 h-16 rounded-2xl bg-[#ede9de] relative overflow-hidden border border-[#dad9d4]/40" />
            </div>
            <div className="space-y-3">
              <div className="w-full h-24 rounded-xl bg-[#ede9de]/70 relative overflow-hidden border border-[#dad9d4]/40" />
            </div>
          </div>
        )}

        {/* ============================================================== */}
        {/* CHRONOLOGICAL MULTI-TURN MESSAGES THREAD                      */}
        {/* ============================================================== */}

        {/* Initial User Prompt Card Fallback */}
        {!normalizedMessages.some((m) => m.role === 'user') && userPromptText && (
          <div className="w-full space-y-1">
            <div className="w-full bg-[#ede9de] border border-[#dad9d4] rounded-xl p-4 text-[13.5px] text-[#141413] select-text leading-relaxed font-sans shadow-sm">
              <div className="whitespace-pre-wrap break-words font-normal">
                {userPromptText}
              </div>
            </div>

            {/* Action Row below Initial User Prompt: Copy Prompt & Undo with Confirmation */}
            <div className="flex items-center justify-end gap-1.5 pr-1">
              <button
                type="button"
                onClick={() => handleCopyPrompt('fallback_prompt', userPromptText)}
                className="p-1.5 rounded-md text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] transition-colors cursor-pointer select-none"
                title={copiedPromptId === 'fallback_prompt' ? 'Copied prompt!' : 'Copy prompt'}
              >
                {copiedPromptId === 'fallback_prompt' ? (
                  <Check className="w-3.5 h-3.5 text-[#5f8a5a]" />
                ) : (
                  <Copy className="w-3.5 h-3.5" />
                )}
              </button>

              {onUndoTask && totalFilesChanged > 0 && (
                confirmingUndoId === chatId ? (
                  <div className="flex items-center gap-1 bg-[#ede9de] border border-[#dad9d4] rounded-md px-2 py-0.5 text-xs select-none shadow-sm">
                    <span className="text-[#535146] text-[11px] font-sans mr-0.5">Undo changes?</span>
                    <button
                      type="button"
                      onClick={() => {
                        setConfirmingUndoId(null);
                        handleUndo(chatId);
                      }}
                      title="Confirm undo"
                      className="p-1 rounded hover:bg-[#dad9d4] text-[#c96442] hover:text-[#b5523f] transition-colors cursor-pointer"
                    >
                      <Check className="w-3.5 h-3.5" />
                    </button>
                    <button
                      type="button"
                      onClick={() => setConfirmingUndoId(null)}
                      title="Cancel"
                      className="p-1 rounded hover:bg-[#dad9d4] text-[#83827d] hover:text-[#141413] transition-colors cursor-pointer"
                    >
                      <X className="w-3.5 h-3.5" />
                    </button>
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => setConfirmingUndoId(chatId)}
                    disabled={isUndone}
                    className={`p-1.5 rounded-md transition-colors cursor-pointer select-none ${
                      isUndone
                        ? 'text-[#83827d] opacity-40 cursor-not-allowed'
                        : 'text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de]'
                    }`}
                    title={isUndone ? 'Reverted' : 'Undo task changes'}
                  >
                    {isUndone ? <Check className="w-3.5 h-3.5 text-[#5f8a5a]" /> : <Undo2 className="w-3.5 h-3.5" />}
                  </button>
                )
              )}
            </div>
          </div>
        )}

        {normalizedMessages.length > 0 &&
          normalizedMessages.map((msg, idx) => {
            const isSystem = msg.role === 'system';
            const isUser = msg.role === 'user';

            // 1. Quiet System Message (Git notices, checkpoints, conflicts resolved)
            if (isSystem) {
              return (
                <div
                  key={msg.id || idx}
                  className="flex items-center gap-2 py-1 px-1 text-xs text-[#83827d] font-sans my-1 select-none"
                >
                  <span className="w-1.5 h-1.5 rounded-full bg-[#83827d] shrink-0" />
                  <span className="leading-relaxed">{msg.content}</span>
                </div>
              );
            }

            // 2. User Prompt Card (with Copy icon & Undo icon + confirmation)
            if (isUser) {
              const isLong = msg.content.length > 300;
              const isExpanded = expandedPrompts[msg.id] ?? false;
              const isReverted = isUndone || revertedMsgIds.has(msg.id);
              const isThisReverting = revertingMsgId === msg.id;

              return (
                <div key={msg.id || idx} className="w-full space-y-1">
                  <div className="w-full bg-[#ede9de] border border-[#dad9d4] rounded-xl p-4 text-[13.5px] text-[#141413] select-text leading-relaxed font-sans shadow-sm">
                    <div className="whitespace-pre-wrap break-words font-normal">
                      {isLong && !isExpanded ? `${msg.content.slice(0, 300)}...` : msg.content}
                      {isLong && (
                        <button
                          type="button"
                          onClick={() =>
                            setExpandedPrompts((prev) => ({ ...prev, [msg.id]: !isExpanded }))
                          }
                          className="ml-2 text-[#c96442] hover:text-[#b5523f] text-xs font-medium cursor-pointer"
                        >
                          {isExpanded ? 'Show less' : 'Read more'}
                        </button>
                      )}
                    </div>
                  </div>

                  {/* Action Row below User Prompt: Copy Prompt & Undo with Confirmation */}
                  <div className="flex items-center justify-end gap-1.5 pr-1">
                    <button
                      type="button"
                      onClick={() => handleCopyPrompt(msg.id, msg.content)}
                      className="p-1.5 rounded-md text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] transition-colors cursor-pointer select-none"
                      title={copiedPromptId === msg.id ? 'Copied prompt!' : 'Copy prompt'}
                    >
                      {copiedPromptId === msg.id ? (
                        <Check className="w-3.5 h-3.5 text-[#5f8a5a]" />
                      ) : (
                        <Copy className="w-3.5 h-3.5" />
                      )}
                    </button>

                    {onUndoTask && (
                      confirmingUndoId === msg.id ? (
                        <div className="flex items-center gap-1 bg-[#ede9de] border border-[#dad9d4] rounded-md px-2 py-0.5 text-xs select-none shadow-sm">
                          <span className="text-[#535146] text-[11px] font-sans mr-0.5">Undo changes?</span>
                          <button
                            type="button"
                            onClick={() => {
                              setConfirmingUndoId(null);
                              handleUndo(msg.id, msg.commit_hash);
                            }}
                            title="Confirm undo (revert commit)"
                            className="p-1 rounded hover:bg-[#dad9d4] text-[#c96442] hover:text-[#b5523f] transition-colors cursor-pointer"
                          >
                            <Check className="w-3.5 h-3.5" />
                          </button>
                          <button
                            type="button"
                            onClick={() => setConfirmingUndoId(null)}
                            title="Cancel"
                            className="p-1 rounded hover:bg-[#dad9d4] text-[#83827d] hover:text-[#141413] transition-colors cursor-pointer"
                          >
                            <X className="w-3.5 h-3.5" />
                          </button>
                        </div>
                      ) : (
                        <button
                          type="button"
                          onClick={() => setConfirmingUndoId(msg.id)}
                          disabled={isThisReverting || isReverted}
                          className={`p-1.5 rounded-md transition-colors cursor-pointer select-none ${
                            isReverted
                              ? 'text-[#83827d] opacity-40 cursor-not-allowed'
                              : 'text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de]'
                          }`}
                          title={isReverted ? 'Reverted' : isThisReverting ? 'Reverting...' : 'Undo task changes'}
                        >
                          {isThisReverting ? (
                            <Loader2 className="w-3.5 h-3.5 animate-spin text-[#c96442]" />
                          ) : isReverted ? (
                            <Check className="w-3.5 h-3.5 text-[#5f8a5a]" />
                          ) : (
                            <Undo2 className="w-3.5 h-3.5" />
                          )}
                        </button>
                      )
                    )}
                  </div>
                </div>
              );
            } else {
              // 3. Assistant Message Turn (Open text, NO box)
              let parsedPlan: ZencodePlan | null = null;
              if (msg.plan_json) {
                try {
                  parsedPlan = JSON.parse(msg.plan_json);
                } catch {}
              }
              const parsedInv = parseFilesChanged(msg.files_changed_json);
              const isLastMessage = idx === normalizedMessages.length - 1;
              const isCurrentlyRunning = isLastMessage && (isWorkerRunning || isPlanning);

              // Calculate exact turn wall-clock duration from user message timestamp to assistant completion
              const prevUserMsg = normalizedMessages
                .slice(0, idx)
                .reverse()
                .find((m) => m.role === 'user');

              let turnDurationSecs: number | undefined = undefined;
              if (prevUserMsg?.created_at && msg.created_at) {
                const tStart = new Date(prevUserMsg.created_at).getTime();
                const tEnd = new Date(msg.created_at).getTime();
                if (!isNaN(tStart) && !isNaN(tEnd) && tEnd >= tStart) {
                  const diff = Math.round((tEnd - tStart) / 1000);
                  if (diff > 0 && diff < 86400) {
                    turnDurationSecs = diff;
                  }
                }
              }

              return (
                <div
                  key={msg.id || idx}
                  className="w-full space-y-3 py-1 text-[13.5px] text-[#3d3929] font-sans leading-relaxed select-text"
                >
                  {/* Conversation Steps in Verbose Mode for EVERY prompt */}
                  <ConversationStepsCard
                    id={msg.id || `msg_${idx}`}
                    plan={parsedPlan || (isCurrentlyRunning ? plan : null)}
                    filesChanged={parsedInv?.files}
                    liveActivities={isLastMessage ? structuredActivities : undefined}
                    elapsedSeconds={isLastMessage ? elapsedSeconds : undefined}
                    planningElapsedSeconds={isLastMessage ? planningElapsedSeconds : undefined}
                    completedDurationSeconds={turnDurationSecs}
                    isLive={isCurrentlyRunning}
                    isPlanning={isLastMessage && isPlanning}
                    verificationHistory={isLastMessage && isCurrentlyRunning ? verificationHistory : undefined}
                    onOpenFileDiff={onOpenFileDiff}
                  />

                  {/* Conversational small talk reply or task content */}
                  {parsedPlan && parsedPlan.is_actionable === false ? (
                    <div className="space-y-2">
                      <div className="text-[#3d3929] leading-relaxed text-[13.5px]">
                        {renderMarkdownContent(parsedPlan.conversational_reply || msg.content || '')}
                      </div>
                      {/* Copy Output Button */}
                      <div className="flex items-center justify-end pr-1 pt-1">
                        <button
                          type="button"
                          onClick={() =>
                            handleCopyOutput(
                              msg.id || `asst_${idx}`,
                              parsedPlan.conversational_reply || msg.content || ''
                            )
                          }
                          className="p-1.5 rounded-md text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de]/60 transition-colors cursor-pointer select-none"
                          title={copiedOutputId === (msg.id || `asst_${idx}`) ? 'Copied response!' : 'Copy response'}
                        >
                          {copiedOutputId === (msg.id || `asst_${idx}`) ? (
                            <Check className="w-3.5 h-3.5 text-[#5f8a5a]" />
                          ) : (
                            <Copy className="w-3.5 h-3.5" />
                          )}
                        </button>
                      </div>
                    </div>
                  ) : (
                    <div className="space-y-2">
                      {/* Render grounded completion summary; avoid premature or redundant prompt duplication */}
                      {(() => {
                        if (isCurrentlyRunning) {
                          return null;
                        }
                        const contentToRender = msg.worker_summary || msg.content || '';
                        if (
                          parsedPlan?.task_summary &&
                          contentToRender.trim().toLowerCase() === parsedPlan.task_summary.trim().toLowerCase()
                        ) {
                          return null;
                        }
                        if (
                          prevUserMsg?.content &&
                          contentToRender.trim().toLowerCase() === prevUserMsg.content.trim().toLowerCase()
                        ) {
                          return null;
                        }
                        return renderMarkdownContent(contentToRender);
                      })()}

                      {/* Files Changed banner for this turn matching PDF Page 3 */}
                      {parsedInv && parsedInv.files.length > 0 && (
                        <FilesChangedBanner
                          files={parsedInv.files}
                          totalAdditions={parsedInv.total_additions}
                          totalDeletions={parsedInv.total_deletions}
                          onOpenFileDiff={onOpenFileDiff}
                        />
                      )}

                      {/* Copy Output Button */}
                      <div className="flex items-center justify-end pr-1 pt-1">
                        <button
                          type="button"
                          onClick={() =>
                            handleCopyOutput(
                              msg.id || `asst_${idx}`,
                              msg.worker_summary || msg.content || (parsedPlan?.conversational_reply ?? '')
                            )
                          }
                          className="p-1.5 rounded-md text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de]/60 transition-colors cursor-pointer select-none"
                          title={copiedOutputId === (msg.id || `asst_${idx}`) ? 'Copied response!' : 'Copy response'}
                        >
                          {copiedOutputId === (msg.id || `asst_${idx}`) ? (
                            <Check className="w-3.5 h-3.5 text-[#5f8a5a]" />
                          ) : (
                            <Copy className="w-3.5 h-3.5" />
                          )}
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              );
            }
          })}

        {/* ============================================================== */}
        {/* LIVE IN-FLIGHT RUNNING BLOCK (Only when no assistant card exists yet) */}
        {/* ============================================================== */}
        {(isWorkerRunning || isPlanning) && !normalizedMessages.some((m) => m.role === 'assistant') && (
          <div className="w-full space-y-2 py-1">
            <ConversationStepsCard
              id="live_inflight"
              plan={plan}
              liveActivities={structuredActivities}
              elapsedSeconds={elapsedSeconds}
              planningElapsedSeconds={planningElapsedSeconds}
              isLive={true}
              isPlanning={isPlanning}
              verificationHistory={verificationHistory}
              onOpenFileDiff={onOpenFileDiff}
            />
          </div>
        )}

        {/* ============================================================== */}
        {/* COMPLETION SUMMARY LINE & CHANGED FILES                        */}
        {/* ============================================================== */}
        {inventory && totalFilesChanged > 0 && (() => {
          const lastAssistant = [...(chatMessages || [])].reverse().find((m) => m.role === 'assistant');
          const lastMsgHasInventory = lastAssistant && !!parseFilesChanged(lastAssistant.files_changed_json)?.files.length;
          if (lastMsgHasInventory) return null;

          return (
            <div className="w-full space-y-2 py-1">
              <FilesChangedBanner
                files={inventory.files.map((f) => ({
                  path: f.path,
                  additions: f.additions,
                  deletions: f.deletions,
                  diff_content: f.diff_content || undefined,
                }))}
                totalAdditions={totalAdditions}
                totalDeletions={totalDeletions}
                onOpenFileDiff={onOpenFileDiff}
              />
            </div>
          );
        })()}

        {/* ============================================================== */}
        {/* RUNNING COMMAND STATUS (Minimal & Clickable to open tab)       */}
        {/* ============================================================== */}
        {isCommandRunning && (
          <div className="w-full py-1">
            <button
              type="button"
              onClick={() => onOpenTerminalTab?.()}
              className="inline-flex items-center gap-2.5 px-3.5 py-2.5 bg-[#ede9de]/70 hover:bg-[#ede9de] border border-[#c96442]/30 hover:border-[#c96442]/60 rounded-xl text-xs font-sans shadow-sm transition-all cursor-pointer select-none group"
              title="Click to view live terminal output in tab"
            >
              <span className="relative flex h-2 w-2 shrink-0">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-[#c96442] opacity-75"></span>
                <span className="relative inline-flex rounded-full h-2 w-2 bg-[#c96442]"></span>
              </span>
              <Loader2 className="w-3.5 h-3.5 animate-spin text-[#c96442] shrink-0" />
              <span className="font-semibold text-shimmer">
                Running command:
              </span>
              <span className="font-mono text-shimmer font-medium truncate">
                $ {activeCommandProgram} {activeCommandArgs?.join(' ')}
              </span>
              <span className="text-[11px] text-[#83827d] group-hover:text-[#c96442] font-sans ml-1 underline decoration-[#dad9d4]">
                View output in tab →
              </span>
            </button>
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      {/* Floating Scroll-to-Bottom Indicator */}
      {showScrollBottom && (
        <button
          type="button"
          onClick={() => scrollToBottom(true)}
          className="absolute bottom-24 left-1/2 -translate-x-1/2 w-8 h-8 rounded-full bg-[#f5f4ee] hover:bg-[#ede9de] border border-[#dad9d4] text-[#535146] flex items-center justify-center shadow-md transition-all cursor-pointer z-20"
          title="Scroll to bottom"
        >
          <ChevronDown className="w-4 h-4" />
        </button>
      )}

      {/* ============================================================== */}
      {/* FIXED BOTTOM PROMPT DOCK (Matches PDF Page 2 & 3)              */}
      {/* ============================================================== */}
      <div className="p-4 bg-[#faf9f5] shrink-0 border-t border-[#dad9d4]">
        <div className="max-w-4xl mx-auto w-full space-y-3">
          {pendingCommandApproval && onApproveCommand && onDenyCommand && (
            /* Inline Command Approval Box matching PDF Page 2 */
            <div className="bg-[#f5f4ee] border border-[#dad9d4] rounded-xl p-3.5 shadow-sm space-y-3 font-sans">
              <div className="flex items-center justify-between text-xs">
                <div className="flex items-center gap-2 text-[#141413] font-medium">
                  <Terminal className="w-4 h-4 text-[#c96442]" />
                  <span>Permission needed to run a command</span>
                </div>
                <span className="text-[11px] text-[#83827d]">
                  Default mode · asks before running commands
                </span>
              </div>

              <div className="p-2.5 bg-[#ede9de] border border-[#dad9d4] rounded-lg font-mono text-[12.5px] text-[#141413] flex items-baseline gap-2 overflow-x-auto">
                <span className="text-[#83827d] font-bold select-none">$</span>
                <span className="font-semibold text-[#141413]">
                  {pendingCommandApproval.program}
                </span>
                <span className="text-[#535146] break-all">
                  {pendingCommandApproval.args.join(' ')}
                </span>
              </div>

              <div className="flex items-center justify-between gap-4 pt-1">
                <div className="text-xs text-[#83827d] flex items-center gap-1.5 truncate">
                  <span>Working directory</span>
                  <span className="text-[#141413] font-mono truncate">
                    {pendingCommandApproval.cwd || project?.path || '.'}
                  </span>
                </div>

                <div className="flex items-center gap-2 shrink-0">
                  <button
                    type="button"
                    onClick={() => onApproveCommand(pendingCommandApproval)}
                    className="px-3.5 py-1.5 rounded-lg bg-[#c96442] hover:bg-[#b5523f] text-white font-medium transition-colors text-xs shadow-sm cursor-pointer"
                  >
                    Approve
                  </button>
                  <button
                    type="button"
                    onClick={() => onDenyCommand(pendingCommandApproval)}
                    className="px-3.5 py-1.5 rounded-lg bg-[#ede9de] hover:bg-[#e9e6dc] border border-[#dad9d4] text-[#535146] font-medium transition-colors text-xs shadow-sm cursor-pointer"
                  >
                    Deny
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* Standard Prompt Input Dock matching PDF Page 2 & 3 */}
          <div className="bg-[#f5f4ee] border border-[#dad9d4] focus-within:border-[#c96442] rounded-xl p-3 shadow-sm transition-colors">
            {attachedFiles.length > 0 && (
              <div className="flex flex-wrap gap-1.5 pb-2">
                {attachedFiles.map((file, idx) => {
                  const isPdf = file.toLowerCase().endsWith('.pdf');
                  const isImg = /\.(png|jpe?g|webp|gif|svg|bmp)$/i.test(file);
                  const basename = file.split(/[/\\]/).pop() || file;
                  return (
                    <span
                      key={idx}
                      className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md text-[11px] font-mono bg-[#ede9de] border border-[#dad9d4] text-[#141413] shadow-xs"
                    >
                      {isPdf ? (
                        <span className="text-[10px] font-bold text-red-600 bg-red-100 dark:bg-red-950/40 px-1 py-0.2 rounded">PDF</span>
                      ) : isImg ? (
                        <span className="text-[10px] font-bold text-blue-600 bg-blue-100 dark:bg-blue-950/40 px-1 py-0.2 rounded">IMG</span>
                      ) : (
                        <Paperclip className="w-3 h-3 text-[#83827d]" />
                      )}
                      <span className="max-w-[200px] truncate" title={file}>{basename}</span>
                      <button
                        type="button"
                        onClick={() => setAttachedFiles((prev) => prev.filter((_, i) => i !== idx))}
                        className="text-[#83827d] hover:text-[#141413] ml-0.5"
                        title="Remove attachment"
                      >
                        <X className="w-3 h-3" />
                      </button>
                    </span>
                  );
                })}
              </div>
            )}
            <textarea
              value={followupInput}
              onChange={(e) => setFollowupInput(e.target.value)}
              onKeyDown={handleFollowupKeyDown}
              placeholder={pendingCommandApproval ? "Reply, or steer the task…" : "Ask a follow-up…"}
              rows={2}
              disabled={isLoading || isWorkerRunning}
              className="w-full bg-transparent text-[13px] text-[#141413] placeholder:text-[#83827d] focus:outline-none resize-none font-sans leading-relaxed"
            />

            {/* Bottom Controls Row */}
            <div className="flex items-center justify-between pt-2">
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={handleAttachFile}
                  title="Attach file or asset"
                  className="w-6 h-6 flex items-center justify-center rounded text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] transition-colors cursor-pointer"
                >
                  <Paperclip className="w-3.5 h-3.5" />
                </button>

                {/* Director Model Selector Popover */}
                <div className="relative" ref={plannerMenuRef}>
                  <button
                    type="button"
                    onClick={() => {
                      setIsPlannerMenuOpen((prev) => !prev);
                      setIsWorkerMenuOpen(false);
                    }}
                    className="h-6 px-2 rounded-md bg-[#ede9de] hover:bg-[#e9e6dc] border border-[#dad9d4] text-[11px] text-[#141413] font-sans flex items-center gap-1.5 transition-colors cursor-pointer select-none"
                    title="Select Director / Planner Model"
                  >
                    <Sparkles className="w-3 h-3 text-[#c96442]" />
                    <span className="max-w-[120px] truncate font-medium">
                      {AVAILABLE_DIRECTOR_MODELS.find((m) => m.id === selectedModel)?.name || selectedModel}
                    </span>
                    <ChevronDown className="w-2.5 h-2.5 text-[#83827d]" />
                  </button>

                  {isPlannerMenuOpen && (
                    <div className="absolute bottom-full mb-2 left-0 w-80 bg-[#faf9f5] border border-[#dad9d4] rounded-xl shadow-xl z-50 overflow-hidden font-sans animate-fade-in p-1">
                      <div className="px-3 py-2 border-b border-[#dad9d4]/60 bg-[#ede9de]/40 flex items-center justify-between">
                        <span className="text-[11px] font-semibold text-[#141413]">Planner / Director Model</span>
                        <span className="text-[10px] text-[#83827d]">Architecture & Strategy</span>
                      </div>
                      <div className="max-h-72 overflow-y-auto p-1 space-y-1">
                        {AVAILABLE_DIRECTOR_MODELS.map((m) => {
                          const isSelected = m.id === selectedModel;
                          return (
                            <button
                              key={m.id}
                              type="button"
                              onClick={() => {
                                onSelectModel && onSelectModel(m.id);
                                setIsPlannerMenuOpen(false);
                              }}
                              className={`w-full text-left p-2 rounded-lg transition-colors flex items-start justify-between gap-2 cursor-pointer ${
                                isSelected
                                  ? 'bg-[#ede9de] border border-[#dad9d4]'
                                  : 'hover:bg-[#ede9de]/60 border border-transparent'
                              }`}
                            >
                              <div className="min-w-0 flex-1">
                                <div className="flex items-center gap-1.5">
                                  <span className="text-xs font-medium text-[#141413] truncate">{m.name}</span>
                                  {m.badge && (
                                    <span className="text-[9px] px-1.5 py-0.2 rounded font-mono bg-[#ede9de] border border-[#dad9d4] text-[#535146] shrink-0">
                                      {m.badge}
                                    </span>
                                  )}
                                </div>
                                <p className="text-[10px] text-[#83827d] mt-0.5 leading-snug line-clamp-1">{m.desc}</p>
                              </div>
                              {isSelected && <Check className="w-3.5 h-3.5 text-[#c96442] shrink-0 mt-0.5" />}
                            </button>
                          );
                        })}
                      </div>
                    </div>
                  )}
                </div>

                {/* Nested Worker Selector Popover (Provider List -> Models Submenu) */}
                <div className="relative" ref={workerMenuRef}>
                  <button
                    type="button"
                    onClick={() => {
                      setIsWorkerMenuOpen((prev) => !prev);
                      setIsPlannerMenuOpen(false);
                    }}
                    className="h-6 px-2 rounded-md bg-[#ede9de] hover:bg-[#e9e6dc] border border-[#dad9d4] text-[11px] text-[#141413] font-sans flex items-center gap-1.5 transition-colors cursor-pointer select-none"
                    title="Select Worker Engine & Model"
                  >
                    {workerType === 'groq' ? (
                      <Zap className="w-3 h-3 text-[#c96442] shrink-0" />
                    ) : workerType === 'openrouter' ? (
                      <Globe className="w-3 h-3 text-[#3d7aed] shrink-0" />
                    ) : (
                      <Cpu className="w-3 h-3 text-[#5f8a5a] shrink-0" />
                    )}
                    <span className="max-w-[130px] truncate font-medium">
                      Worker · {workerType === 'groq' ? 'Groq' : workerType === 'openrouter' ? 'OpenRouter' : 'Ollama'}
                    </span>
                    <ChevronDown className="w-2.5 h-2.5 text-[#83827d]" />
                  </button>

                  {isWorkerMenuOpen && (
                    <div className="absolute bottom-full mb-2 left-0 w-[460px] bg-[#faf9f5] border border-[#dad9d4] rounded-xl shadow-xl z-50 overflow-hidden font-sans animate-fade-in flex flex-col">
                      <div className="px-3.5 py-2 border-b border-[#dad9d4]/60 bg-[#ede9de]/40 flex items-center justify-between">
                        <span className="text-[11px] font-semibold text-[#141413]">Worker Engine &amp; Model</span>
                        <span className="text-[10px] text-[#83827d]">Diff Generation &amp; Execution</span>
                      </div>

                      <div className="flex h-72">
                        {/* Left column: Providers list */}
                        <div className="w-[180px] border-r border-[#dad9d4]/60 bg-[#f7f6f2] p-1.5 flex flex-col gap-1 shrink-0">
                          <span className="text-[10px] font-semibold text-[#83827d] uppercase tracking-wider px-2 py-1">
                            Providers
                          </span>
                          {WORKER_PROVIDERS.map((provider) => {
                            const IconComponent = provider.icon;
                            const isActiveProvider = activeWorkerProvider === provider.id;
                            const isChosenProvider = workerType === provider.id;
                            return (
                              <button
                                key={provider.id}
                                type="button"
                                onMouseEnter={() => setActiveWorkerProvider(provider.id)}
                                onClick={() => setActiveWorkerProvider(provider.id)}
                                className={`w-full text-left p-2 rounded-lg transition-colors flex items-center justify-between cursor-pointer ${
                                  isActiveProvider
                                    ? 'bg-[#ede9de] border border-[#dad9d4] shadow-xs'
                                    : 'hover:bg-[#ede9de]/50 border border-transparent'
                                }`}
                              >
                                <div className="flex items-center gap-2 min-w-0">
                                  <div
                                    className={`w-5 h-5 rounded flex items-center justify-center shrink-0 ${
                                      provider.id === 'groq'
                                        ? 'text-[#c96442]'
                                        : provider.id === 'openrouter'
                                        ? 'text-[#3d7aed]'
                                        : 'text-[#5f8a5a]'
                                    }`}
                                  >
                                    <IconComponent className="w-3.5 h-3.5" />
                                  </div>
                                  <div className="min-w-0">
                                    <div className="flex items-center gap-1">
                                      <span className="text-xs font-medium text-[#141413] truncate">
                                        {provider.name}
                                      </span>
                                    </div>
                                    <span className="text-[9px] text-[#83827d] block truncate">
                                      {provider.badge}
                                    </span>
                                  </div>
                                </div>
                                {isChosenProvider && (
                                  <span className="w-1.5 h-1.5 rounded-full bg-[#c96442] shrink-0 mr-1" />
                                )}
                                <ChevronRight className="w-3 h-3 text-[#83827d] shrink-0" />
                              </button>
                            );
                          })}
                        </div>

                        {/* Right column: Models submenu */}
                        <div className="flex-1 p-2 bg-[#faf9f5] flex flex-col overflow-y-auto">
                          {(() => {
                            const currentProvider = WORKER_PROVIDERS.find((p) => p.id === activeWorkerProvider);
                            if (!currentProvider) return null;

                            // If local, combine static models with installed Ollama models
                            let displayModels = [...currentProvider.models];
                            if (activeWorkerProvider === 'local' && installedOllamaModels.length > 0) {
                              const dynamicModels: WorkerModelItem[] = installedOllamaModels.map((m) => ({
                                id: m.name,
                                name: m.name,
                                desc: `Installed local model (${m.size || 'Ollama'})`,
                                badge: m.is_recommended ? 'Recommended' : 'Installed',
                              }));
                              const seen = new Set<string>();
                              displayModels = [...dynamicModels, ...displayModels].filter((m) => {
                                if (seen.has(m.id)) return false;
                                seen.add(m.id);
                                return true;
                              });
                            }

                            return (
                              <div className="space-y-1">
                                <div className="px-2 py-1 text-[10px] text-[#83827d] border-b border-[#dad9d4]/40 mb-1 flex items-center justify-between">
                                  <span>{currentProvider.tagline}</span>
                                  <span className="font-mono text-[9px]">{displayModels.length} models</span>
                                </div>

                                {displayModels.map((m) => {
                                  const isSelected = workerType === activeWorkerProvider && workerModel === m.id;
                                  return (
                                    <button
                                      key={m.id}
                                      type="button"
                                      onClick={() => handleSelectWorker(activeWorkerProvider, m.id)}
                                      className={`w-full text-left p-2 rounded-lg transition-colors flex items-start justify-between gap-2 cursor-pointer ${
                                        isSelected
                                          ? 'bg-[#ede9de] border border-[#dad9d4]'
                                          : 'hover:bg-[#ede9de]/60 border border-transparent'
                                      }`}
                                    >
                                      <div className="min-w-0 flex-1">
                                        <div className="flex items-center gap-1.5">
                                          <span className="text-xs font-medium text-[#141413] truncate">{m.name}</span>
                                          {m.badge && (
                                            <span className="text-[9px] px-1.5 py-0.2 rounded font-mono bg-[#ede9de] border border-[#dad9d4] text-[#535146] shrink-0">
                                              {m.badge}
                                            </span>
                                          )}
                                        </div>
                                        <p className="text-[10px] text-[#83827d] mt-0.5 leading-snug line-clamp-1">
                                          {m.desc}
                                        </p>
                                      </div>
                                      {isSelected && <Check className="w-3.5 h-3.5 text-[#c96442] shrink-0 mt-0.5" />}
                                    </button>
                                  );
                                })}

                                {/* Custom model input for OpenRouter */}
                                {activeWorkerProvider === 'openrouter' && (
                                  <div className="pt-2 mt-1 border-t border-[#dad9d4]/40">
                                    <span className="text-[10px] font-medium text-[#535146] block mb-1">
                                      Use Custom OpenRouter Model ID:
                                    </span>
                                    <div className="flex items-center gap-1.5">
                                      <input
                                        type="text"
                                        value={customModelInput}
                                        onChange={(e) => setCustomModelInput(e.target.value)}
                                        onKeyDown={(e) => {
                                          if (e.key === 'Enter' && customModelInput.trim()) {
                                            handleSelectWorker('openrouter', customModelInput.trim());
                                          }
                                        }}
                                        placeholder="e.g. meta-llama/llama-3.1-405b..."
                                        className="flex-1 bg-[#ede9de]/60 border border-[#dad9d4] rounded px-2 py-1 text-[11px] font-mono text-[#141413] placeholder:text-[#83827d] focus:outline-none focus:border-[#c96442]"
                                      />
                                      <button
                                        type="button"
                                        onClick={() => {
                                          if (customModelInput.trim()) {
                                            handleSelectWorker('openrouter', customModelInput.trim());
                                          }
                                        }}
                                        disabled={!customModelInput.trim()}
                                        className="h-6 px-2 bg-[#ede9de] hover:bg-[#dad9d4] border border-[#dad9d4] text-[#141413] rounded flex items-center justify-center text-[10px] font-medium transition-colors disabled:opacity-40 cursor-pointer"
                                        title="Select custom model"
                                      >
                                        <Plus className="w-3 h-3" />
                                      </button>
                                    </div>
                                  </div>
                                )}
                              </div>
                            );
                          })()}
                        </div>
                      </div>
                    </div>
                  )}
                </div>

                {/* Mode selector */}
                <button
                  type="button"
                  onClick={() =>
                    setSelectedExecutionMode((prev) => (prev === 'Build' ? 'Turbo' : 'Build'))
                  }
                  className="h-6 px-2 rounded-md bg-[#ede9de] hover:bg-[#e9e6dc] border border-[#dad9d4] text-[11px] text-[#535146] hover:text-[#141413] flex items-center gap-1 transition-colors cursor-pointer"
                >
                  <span>{selectedExecutionMode}</span>
                  <ChevronDown className="w-2.5 h-2.5 text-[#83827d]" />
                </button>

                {/* Reset Chat Session Button */}
                {onResetChat && (
                  <button
                    type="button"
                    onClick={onResetChat}
                    title="Reset active chat session (Ctrl+Shift+Backspace)"
                    className="h-6 px-2 rounded-md bg-[#ede9de] hover:bg-[#dad9d4] border border-[#dad9d4] text-[11px] text-[#535146] hover:text-[#b5523f] flex items-center gap-1 transition-colors cursor-pointer"
                  >
                    <RotateCcw className="w-3 h-3 text-[#83827d]" />
                    <span>Reset</span>
                  </button>
                )}
              </div>

              {/* Send Button: Terracotta accent matching PDF Page 2 & 3 */}
              <button
                type="button"
                onClick={handleSendFollowup}
                disabled={(!followupInput.trim() && attachedFiles.length === 0) || isLoading || isWorkerRunning}
                className={`h-7 px-3 rounded-lg flex items-center gap-1.5 transition-colors shadow-sm font-sans text-xs font-medium ${
                  (followupInput.trim() || attachedFiles.length > 0) && !isLoading && !isWorkerRunning
                    ? 'bg-[#c96442] hover:bg-[#b5523f] text-white cursor-pointer'
                    : 'bg-[#ede9de] text-[#83827d] border border-[#dad9d4] opacity-60 cursor-not-allowed'
                }`}
              >
                <span>Send</span>
                <ArrowUp className="w-3 h-3 stroke-[2.5]" />
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
