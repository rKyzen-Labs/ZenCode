import { useState, useEffect, useMemo, type FC } from 'react';
import {
  Search,
  FolderPlus,
  HelpCircle,
  X,
  Folder,
} from 'lucide-react';
import { searchChats, listWorkspaces, listAllRecentChats, pickFolder } from '../lib/ipc';
import type { ChatSummaryRecord, WorkspaceRecord } from '../types';

interface SearchPageProps {
  onSelectChat: (chat: ChatSummaryRecord) => void;
  onSwitchWorkspace: (workspacePath: string) => void;
  onNewSession: () => void;
  onGoSettings: () => void;
  currentWorkspacePath?: string | null;
}

export const SearchPage: FC<SearchPageProps> = ({
  onSelectChat,
  onSwitchWorkspace,
  onNewSession: _onNewSession,
  onGoSettings: _onGoSettings,
  currentWorkspacePath,
}) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [matchingChats, setMatchingChats] = useState<ChatSummaryRecord[]>([]);
  const [allRecentChats, setAllRecentChats] = useState<ChatSummaryRecord[]>([]);
  const [allWorkspaces, setAllWorkspaces] = useState<WorkspaceRecord[]>([]);
  const [selectedWorkspaceFilter, setSelectedWorkspaceFilter] = useState<string | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [isHelpOpen, setIsHelpOpen] = useState(false);

  // Load all workspaces on mount
  const loadWorkspaces = async () => {
    try {
      const records = await listWorkspaces();
      setAllWorkspaces(records || []);
    } catch (err) {
      console.error('Failed to list workspaces for search page:', err);
    }
  };

  useEffect(() => {
    loadWorkspaces();
  }, []);

  // Fetch initial recent chats for counts and default display
  useEffect(() => {
    listAllRecentChats(100)
      .then((records) => {
        setAllRecentChats(records || []);
      })
      .catch((err) => console.error('Failed to load recent chats:', err));
  }, []);

  // Fetch search results based on query
  useEffect(() => {
    let active = true;
    const fetchResults = async () => {
      setIsSearching(true);
      try {
        if (!searchQuery.trim()) {
          const recent = await listAllRecentChats(40);
          if (active) setMatchingChats(recent || []);
        } else {
          const results = await searchChats(searchQuery.trim());
          if (active) setMatchingChats(results || []);
        }
      } catch (err) {
        console.error('Search error:', err);
      } finally {
        if (active) setIsSearching(false);
      }
    };

    const timer = setTimeout(fetchResults, 120);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [searchQuery]);

  const handleAddWorkspace = async () => {
    try {
      const selected = await pickFolder();
      if (selected && selected.trim()) {
        onSwitchWorkspace(selected.trim());
        await loadWorkspaces();
      }
    } catch (err) {
      console.error('Failed to pick folder:', err);
    }
  };

  // Chat counts per workspace
  const workspaceCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const chat of allRecentChats) {
      if (chat.workspace_path) {
        counts.set(chat.workspace_path, (counts.get(chat.workspace_path) || 0) + 1);
      }
    }
    return counts;
  }, [allRecentChats]);

  // Filtered chats based on workspace selection (if user clicked on a workspace in sidebar)
  const displayedChats = useMemo(() => {
    if (!selectedWorkspaceFilter) return matchingChats;
    return matchingChats.filter((c) => c.workspace_path === selectedWorkspaceFilter);
  }, [matchingChats, selectedWorkspaceFilter]);

  // Highlight search terms in snippet
  const renderHighlightedSnippet = (text: string, query: string) => {
    if (!query.trim()) return text;
    const parts = text.split(new RegExp(`(${query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi'));
    return parts.map((part, i) =>
      part.toLowerCase() === query.toLowerCase() ? (
        <span key={i} className="font-semibold text-[#141413] bg-[#ede9de] px-0.5 rounded">
          {part}
        </span>
      ) : (
        part
      )
    );
  };

  return (
    <div className="flex-1 flex h-full w-full bg-[#faf9f5] text-[#141413] font-sans select-none overflow-hidden">
      {/* Left/Center Main Column (Matches PDF Page 4) */}
      <div className="flex-1 flex flex-col h-full overflow-hidden px-10 py-8 min-w-0">
        {/* Search Input Box with Results Counter Pill */}
        <div className="max-w-3xl w-full mb-8">
          <div className="flex items-center justify-between bg-[#f5f4ee] border border-[#dad9d4] focus-within:border-[#c96442] rounded-xl px-4 py-2.5 shadow-sm transition-colors">
            <div className="flex items-center gap-3 flex-1 min-w-0">
              <Search className="w-4 h-4 text-[#83827d] shrink-0 pointer-events-none" />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Search past tasks and chats..."
                autoFocus
                className="w-full bg-transparent text-[13.5px] text-[#141413] placeholder-[#83827d] focus:outline-none font-sans"
              />
            </div>
            <div className="text-xs text-[#83827d] font-sans shrink-0 pl-3 select-none">
              {displayedChats.length} {displayedChats.length === 1 ? 'result' : 'results'}
            </div>
          </div>
        </div>

        {/* Results List */}
        <div className="flex-1 overflow-y-auto space-y-6 max-w-3xl w-full pr-4 pb-12">
          {displayedChats.length === 0 && !isSearching && (
            <div className="py-12 text-center text-[#83827d] text-sm font-sans">
              {searchQuery ? `No results match "${searchQuery}"` : 'No tasks recorded yet'}
            </div>
          )}

          {displayedChats.map((chat) => {
            const workspaceBasename = chat.workspace_path
              ? chat.workspace_path.split(/[/\\]/).filter(Boolean).pop() || 'ZenCode'
              : 'ZenCode';

            const snippet = (chat as any).last_message_content || (chat.branch_name ? `Branch: ${chat.branch_name}` : chat.title) || 'Task completed';

            return (
              <div
                key={chat.id}
                onClick={() => onSelectChat(chat)}
                className="group cursor-pointer select-text space-y-1"
              >
                {/* Title & Workspace Tag matching PDF Page 4 */}
                <div className="flex items-baseline gap-2.5">
                  <h3 className="text-[14px] font-medium text-[#141413] group-hover:text-[#c96442] transition-colors leading-snug font-sans">
                    {chat.title || 'Task execution'}
                  </h3>
                  <span className="text-[11.5px] text-[#83827d] font-mono shrink-0">
                    {workspaceBasename}
                  </span>
                </div>

                {/* Excerpt / Snippet matching PDF Page 4 */}
                <p className="text-[13px] text-[#535146] leading-relaxed line-clamp-2 font-sans">
                  …{renderHighlightedSnippet(snippet, searchQuery)}…
                </p>
              </div>
            );
          })}
        </div>
      </div>

      {/* Right Column: ALL WORKSPACES Sidebar (Matches PDF Page 4) */}
      <aside className="w-64 h-full border-l border-[#dad9d4] bg-[#faf9f5] flex flex-col p-6 shrink-0 overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between pb-4">
          <span className="text-[11px] font-semibold uppercase tracking-wider text-[#83827d] font-sans">
            ALL WORKSPACES
          </span>
          <button
            type="button"
            onClick={handleAddWorkspace}
            title="Add Workspace (Browse Folder)"
            className="w-5 h-5 flex items-center justify-center rounded text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] transition-colors cursor-pointer"
          >
            <FolderPlus className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* Workspaces List */}
        <div className="flex-1 overflow-y-auto space-y-1 pr-1">
          {/* Option to clear filter if selected */}
          {selectedWorkspaceFilter && (
            <div
              onClick={() => setSelectedWorkspaceFilter(null)}
              className="flex items-center justify-between px-2.5 py-1.5 rounded-md hover:bg-[#ede9de]/60 cursor-pointer text-xs text-[#c96442] font-medium transition-colors"
            >
              <span>Show all</span>
              <X className="w-3 h-3" />
            </div>
          )}

          {allWorkspaces.map((ws) => {
            const name = ws.name || ws.path.split(/[/\\]/).filter(Boolean).pop() || 'Workspace';
            const count = workspaceCounts.get(ws.path) || 0;
            const isSelected = selectedWorkspaceFilter === ws.path;
            const isCurrent = currentWorkspacePath === ws.path;

            return (
              <div
                key={ws.path}
                onClick={() => {
                  if (selectedWorkspaceFilter === ws.path) {
                    setSelectedWorkspaceFilter(null);
                  } else {
                    setSelectedWorkspaceFilter(ws.path);
                  }
                }}
                className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded-md cursor-pointer transition-colors group ${
                  isSelected
                    ? 'bg-[#ede9de] text-[#141413]'
                    : isCurrent
                    ? 'bg-[#f5f4ee] text-[#141413]'
                    : 'hover:bg-[#ede9de]/60 text-[#535146] hover:text-[#141413]'
                }`}
                title={ws.path}
              >
                <div className="flex items-center gap-2 truncate min-w-0 pr-2">
                  <Folder className="w-3.5 h-3.5 text-[#83827d] shrink-0 group-hover:text-[#141413]" />
                  <span className="text-[12.5px] font-sans truncate">{name}</span>
                </div>
                <span className="text-[11.5px] font-mono text-[#83827d] shrink-0 font-medium">
                  {count > 0 ? count : 0}
                </span>
              </div>
            );
          })}
        </div>

        {/* Bottom Help Trigger */}
        <div className="pt-4 border-t border-[#dad9d4] shrink-0">
          <button
            type="button"
            onClick={() => setIsHelpOpen(true)}
            className="w-full flex items-center gap-2 px-2 py-1.5 rounded text-xs text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de]/60 transition-colors cursor-pointer"
          >
            <HelpCircle className="w-3.5 h-3.5 text-[#83827d]" />
            <span>Help &amp; Shortcuts</span>
          </button>
        </div>
      </aside>

      {/* Help Modal */}
      {isHelpOpen && (
        <div className="fixed inset-0 z-50 bg-black/40 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-[#faf9f5] border border-[#dad9d4] rounded-2xl max-w-lg w-full p-6 space-y-4 shadow-xl font-sans text-xs">
            <div className="flex items-center justify-between pb-3 border-b border-[#dad9d4]">
              <div className="flex items-center gap-2 text-[#141413] font-semibold text-sm">
                <HelpCircle className="w-4 h-4 text-[#c96442]" />
                <span>ZenCode Quick Reference</span>
              </div>
              <button
                type="button"
                onClick={() => setIsHelpOpen(false)}
                className="w-6 h-6 flex items-center justify-center rounded text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="space-y-3 text-[12px] text-[#535146]">
              <div className="p-3 bg-[#f5f4ee] border border-[#dad9d4] rounded-lg space-y-1">
                <div className="font-semibold text-[#141413]">Input &amp; Navigation</div>
                <div>&bull; <span className="font-mono text-[#141413]">Enter</span>: Send prompt / task to Director</div>
                <div>&bull; <span className="font-mono text-[#141413]">Shift + Enter</span>: Insert newline in input</div>
                <div>&bull; <span className="font-mono text-[#141413]">Paperclip</span>: Attach file path to prompt</div>
              </div>

              <div className="p-3 bg-[#f5f4ee] border border-[#dad9d4] rounded-lg space-y-1">
                <div className="font-semibold text-[#141413]">Execution Modes</div>
                <div>&bull; <span className="font-medium text-[#141413]">Build Mode</span>: Strict gating on all terminal actions</div>
                <div>&bull; <span className="font-medium text-[#c96442]">Turbo Mode</span>: Autonomous execution for allowlisted verify commands</div>
              </div>

              <div className="p-3 bg-[#f5f4ee] border border-[#dad9d4] rounded-lg space-y-1">
                <div className="font-semibold text-[#141413]">Safety &amp; Git Isolation</div>
                <div>&bull; All code edits run in isolated Git worktrees, automatically fast-forwarded on completion</div>
                <div>&bull; Revert prompt changes anytime via the <span className="text-[#c96442] font-medium">Undo</span> button</div>
              </div>
            </div>

            <div className="flex justify-end pt-3 border-t border-[#dad9d4]">
              <button
                type="button"
                onClick={() => setIsHelpOpen(false)}
                className="px-4 py-1.5 bg-[#ede9de] hover:bg-[#e9e6dc] text-[#141413] rounded-lg border border-[#dad9d4] text-xs font-medium transition-colors cursor-pointer"
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
