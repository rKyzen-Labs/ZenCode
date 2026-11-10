import { useState, useEffect, useRef, type FC } from 'react';
import {
  Paperclip,
  ArrowUp,
  ChevronDown,
  AlertTriangle,
  Check,
} from 'lucide-react';
import {
  listAllRecentChats,
  pickFile,
  getWorkspaceWorkerConfig,
} from '../lib/ipc';
import type { ChatSummaryRecord, DependencyStatus, ProjectState } from '../types';
import { WorkspaceSelectorRow } from './WorkspaceSelectorRow';

interface LandingPageProps {
  currentProject: ProjectState | null;
  onOpenWorkspace: (path: string) => void;
  onSubmitPrompt: (promptText: string, model: string) => void;
  onSelectRecentChat: (chat: ChatSummaryRecord) => void;
  selectedModel: string;
  onSelectModel: (model: string) => void;
  onOpenOnboarding?: (step?: 1 | 2 | 3 | 4) => void;
  isLoading?: boolean;
  dependencyIssues?: DependencyStatus[];
  isStartupVerifying?: boolean;
}

const AVAILABLE_DIRECTOR_MODELS = [
  { id: 'gemini-3.5-flash-lite', label: 'Gemini 3.5 Flash Lite' },
  { id: 'gemini-3.8-flash', label: 'Gemini 3.8 Flash' },
  { id: 'granite4.1:8b', label: 'Ollama granite4.1:8b' },
];

export const LandingPage: FC<LandingPageProps> = ({
  currentProject,
  onOpenWorkspace,
  onSubmitPrompt,
  onSelectRecentChat,
  selectedModel,
  onSelectModel,
  onOpenOnboarding: _onOpenOnboarding,
  isLoading = false,
  dependencyIssues = [],
  isStartupVerifying = false,
}) => {
  const [promptInput, setPromptInput] = useState('');
  const [recentChats, setRecentChats] = useState<ChatSummaryRecord[]>([]);
  const [_isLoadingChats, setIsLoadingChats] = useState(false);
  const [_workerType, setWorkerType] = useState<'local' | 'groq'>('local');
  const [isModelDropdownOpen, setIsModelDropdownOpen] = useState(false);
  const modelDropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (modelDropdownRef.current && !modelDropdownRef.current.contains(e.target as Node)) {
        setIsModelDropdownOpen(false);
      }
    };
    if (isModelDropdownOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isModelDropdownOpen]);

  useEffect(() => {
    if (currentProject?.path) {
      getWorkspaceWorkerConfig(currentProject.path)
        .then((cfg) => {
          if (cfg?.worker_type) {
            setWorkerType(cfg.worker_type);
          }
        })
        .catch(() => {});
    }
  }, [currentProject?.path]);

  const handleAttachFile = async () => {
    try {
      const file = await pickFile(currentProject?.path);
      if (file) {
        let displayPath = file;
        if (currentProject?.path && file.startsWith(currentProject.path)) {
          displayPath = file.slice(currentProject.path.length).replace(/^[/\\]+/, '');
        }
        setPromptInput((prev) => (prev ? `${prev} @${displayPath}` : `@${displayPath} `));
      }
    } catch (e) {
      console.warn('Failed to pick file:', e);
    }
  };

  useEffect(() => {
    const fetchChats = async () => {
      setIsLoadingChats(true);
      try {
        const chats = await listAllRecentChats(20);
        setRecentChats(chats || []);
      } catch (err) {
        console.error('Failed to load recent chats for landing page:', err);
      } finally {
        setIsLoadingChats(false);
      }
    };
    fetchChats();
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleSend = () => {
    if (!promptInput.trim() || isLoading) return;
    onSubmitPrompt(promptInput.trim(), selectedModel);
  };

  const formatRelativeTime = (timestamp: string) => {
    try {
      const date = new Date(timestamp);
      if (isNaN(date.getTime())) return timestamp;
      const now = new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffMins = Math.floor(diffMs / 60000);
      if (diffMins < 1) return 'now';
      if (diffMins < 60) return `${diffMins}m`;
      const diffHours = Math.floor(diffMins / 60);
      if (diffHours < 24) return `${diffHours}h`;
      const diffDays = Math.floor(diffHours / 24);
      if (diffDays === 1) return 'Yesterday';
      if (diffDays < 7) {
        const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
        return days[date.getDay()];
      }
      return `${diffDays}d`;
    } catch {
      return timestamp;
    }
  };

  const activeModelMeta = AVAILABLE_DIRECTOR_MODELS.find((m) => m.id === selectedModel) || AVAILABLE_DIRECTOR_MODELS[0];

  return (
    <div className="flex-1 flex flex-col items-center justify-start pt-14 pb-8 px-6 max-w-2xl mx-auto w-full select-none h-full overflow-y-auto font-sans">
      {/* Centered ZenCode Wordmark: matching PDF Page 1 */}
      <div className="flex flex-col items-center mb-7 shrink-0 text-center">
        <h1 className="text-[52px] leading-tight font-serif tracking-tight select-none">
          <span className="text-[#141413]">Zen</span>
          <span className="text-[#c96442]">Code</span>
        </h1>
      </div>

      {/* Startup dependency verification banner if any */}
      {isStartupVerifying && (
        <div className="flex items-center gap-2 mb-3 text-[11px] text-[#83827d] shrink-0 font-sans">
          <span className="w-1.5 h-1.5 rounded-full bg-[#c96442] animate-pulse" />
          <span>Verifying local environment...</span>
        </div>
      )}

      {/* Startup Dependency Issues Banner */}
      {dependencyIssues.length > 0 && (
        <div className="w-full bg-[#ede9de] border border-[#dad9d4] rounded-xl p-3 mb-4 space-y-2 font-sans text-xs shrink-0 shadow-sm">
          <div className="flex items-center gap-2 text-[#b5523f] font-semibold text-[11px]">
            <AlertTriangle className="w-4 h-4 shrink-0 text-[#b5523f]" />
            <span>Local Dependency Notice</span>
          </div>
          <div className="space-y-1 text-[11px]">
            {dependencyIssues.map((issue, idx) => (
              <div key={idx} className="p-2 bg-[#faf9f5] border border-[#dad9d4] rounded space-y-0.5">
                <div className="text-[#141413] font-medium">
                  {issue.name}: <span className="text-[#b5523f]">{issue.details}</span>
                </div>
                {issue.fix_recommendation && (
                  <div className="text-[#535146] text-[10px]">&rarr; {issue.fix_recommendation}</div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Prompt Input Box: matching PDF Page 1 */}
      <div className="w-full bg-[#f5f4ee] border border-[#dad9d4] rounded-2xl p-4 flex flex-col shadow-sm focus-within:border-[#83827d]/60 transition-colors shrink-0">
        <textarea
          value={promptInput}
          onChange={(e) => setPromptInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Describe what you want to build or fix…"
          rows={3}
          className="w-full bg-transparent px-1 pt-1 pb-3 text-[13.5px] text-[#141413] placeholder-[#83827d] font-sans resize-none focus:outline-none leading-relaxed"
        />

        {/* Input Dock Bottom Controls Bar */}
        <div className="flex items-center justify-between pt-2 border-t border-[#dad9d4]/60">
          <div className="flex items-center gap-2">
            {/* Attachment Button */}
            <button
              type="button"
              onClick={handleAttachFile}
              title="Attach files or context"
              className="w-7 h-7 flex items-center justify-center rounded text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] transition-colors"
            >
              <Paperclip className="w-4 h-4" />
            </button>

            {/* Planner Model Selector: Planner · Gemini (with dropdown) */}
            <div ref={modelDropdownRef} className="relative">
              <button
                type="button"
                onClick={() => setIsModelDropdownOpen(!isModelDropdownOpen)}
                title="Select Planner Model"
                className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-sans text-[#535146] hover:text-[#141413] bg-[#f5f4ee] border border-[#dad9d4] hover:bg-[#ede9de] transition-colors"
              >
                <span>Planner · {activeModelMeta.label.replace('Director: ', '').replace('Agent: ', '')}</span>
                <ChevronDown className="w-3 h-3 text-[#83827d]" />
              </button>

              {isModelDropdownOpen && (
                <div className="absolute left-0 bottom-8 w-60 p-1.5 bg-[#faf9f5] border border-[#dad9d4] rounded-lg shadow-xl z-50 text-[11px] font-sans space-y-0.5">
                  <div className="px-2 py-1 text-[10px] text-[#83827d] font-medium uppercase tracking-wider">
                    Planner Model
                  </div>
                  {AVAILABLE_DIRECTOR_MODELS.map((model) => {
                    const isSelected = selectedModel === model.id;
                    return (
                      <div
                        key={model.id}
                        onClick={() => {
                          onSelectModel(model.id);
                          setIsModelDropdownOpen(false);
                        }}
                        className={`flex items-center justify-between px-2.5 py-1.5 rounded cursor-pointer transition-colors ${
                          isSelected
                            ? 'bg-[#ede9de] text-[#141413] font-medium'
                            : 'text-[#535146] hover:bg-[#f5f4ee] hover:text-[#141413]'
                        }`}
                      >
                        <span className="truncate">{model.label}</span>
                        {isSelected && <Check className="w-3.5 h-3.5 text-[#c96442] shrink-0" />}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </div>

          {/* Send Button: Terracotta accent with white text & arrow */}
          <button
            type="button"
            onClick={handleSend}
            disabled={!promptInput.trim() || isLoading}
            className={`flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg text-white text-[12px] font-medium shadow-sm transition-colors ${
              !promptInput.trim() || isLoading
                ? 'bg-[#c96442]/60 cursor-not-allowed'
                : 'bg-[#c96442] hover:bg-[#b5523f] active:bg-[#b05730] cursor-pointer'
            }`}
          >
            <span>Send</span>
            <ArrowUp className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Workspace Selector Row: directly beneath prompt box matching PDF Page 1 */}
      <div className="w-full mt-3 shrink-0">
        <WorkspaceSelectorRow
          currentProject={currentProject}
          onOpenWorkspace={onOpenWorkspace}
        />
      </div>

      {/* Recent Chats Section: stretching to bottom matching PDF Page 1 */}
      <div className="w-full mt-7 flex-1 min-h-0 flex flex-col">
        <div className="text-[10.5px] font-sans font-medium text-[#83827d] uppercase tracking-wider mb-2 shrink-0">
          RECENT CHATS
        </div>

        <div className="flex-1 overflow-y-auto space-y-0.5 pr-1">
          {recentChats.length === 0 ? (
            <div className="py-8 text-center text-[#83827d] text-xs font-sans">
              No recent chats found. Start a new prompt above!
            </div>
          ) : (
            recentChats.map((chat, idx) => {
              const wsName = chat.workspace_path
                ? chat.workspace_path.replace(/\\/g, '/').split('/').filter(Boolean).pop() || 'zen-calc'
                : 'zen-calc';
              const isFirst = idx === 0;

              return (
                <div
                  key={chat.id}
                  onClick={() => onSelectRecentChat(chat)}
                  className="flex items-center justify-between py-2.5 px-2 -mx-2 rounded-lg hover:bg-[#ede9de]/60 cursor-pointer transition-colors group select-none"
                >
                  {/* Left: Chat Title with small dot for active/first */}
                  <div className="flex items-center gap-2 truncate pr-4">
                    {isFirst ? (
                      <span className="w-1.5 h-1.5 rounded-full bg-[#c96442] shrink-0" />
                    ) : (
                      <span className="w-1.5 h-1.5 shrink-0" />
                    )}
                    <span className="text-[13px] text-[#141413] group-hover:text-[#c96442] transition-colors truncate font-sans">
                      {chat.title || 'Untitled Task'}
                    </span>
                  </div>

                  {/* Right: Workspace Tag & Relative Timestamp */}
                  <div className="flex items-center gap-4 shrink-0">
                    <span className="text-[11px] font-mono text-[#83827d]">
                      {wsName}
                    </span>
                    <span className="text-[11px] font-sans text-[#83827d] w-14 text-right">
                      {formatRelativeTime(chat.created_at)}
                    </span>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
};
