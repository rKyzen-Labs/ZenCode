import React, { useState } from 'react';
import type { SystemStatus } from '../types';
import { formatEngineHealth } from '../types';

interface EmptyStateProps {
  status: SystemStatus | null;
  recentWorkspaces: string[];
  onOpenProject: (path: string) => Promise<void>;
  isLoading: boolean;
  error: string | null;
}

export const EmptyState: React.FC<EmptyStateProps> = ({
  status,
  recentWorkspaces,
  onOpenProject,
  isLoading,
  error,
}) => {
  const [customPath, setCustomPath] = useState('');
  const [showPathInput, setShowPathInput] = useState(false);

  const isGeminiReady =
    status?.gemini_ready ?? Boolean(status?.gemini_status?.includes('Configured'));
  const isOllamaReady =
    status?.ollama_ready ??
    Boolean(status?.ollama_status?.includes('Available') || status?.ollama_status?.includes('ready'));
  const formattedOpencode = formatEngineHealth(status?.opencode_health);
  const isOpencodeReady = formattedOpencode === 'ready';

  const handleOpenPath = (path: string) => {
    if (path.trim() && !isLoading) {
      onOpenProject(path.trim());
    }
  };

  const handleFormSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (customPath.trim()) {
      handleOpenPath(customPath);
    }
  };

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-8 bg-[#09090b] text-[#f4f4f5] font-sans select-none min-h-full">
      <div className="max-w-md w-full space-y-8 text-center">
        {/* Wordmark and Tagline */}
        <div className="space-y-3">
          <div className="inline-flex items-center justify-center w-12 h-12 rounded-xl bg-zinc-900 border border-zinc-800 text-zinc-100 font-mono text-xl font-bold shadow-sm">
            Z
          </div>
          <h1 className="text-2xl font-semibold tracking-tight text-zinc-100 font-sans">
            Zencode
          </h1>
          <p className="text-xs text-zinc-400 font-sans leading-relaxed max-w-sm mx-auto">
            Free-first, local-first agentic IDE with Git worktree isolation.
          </p>
        </div>

        {/* Primary Action Button */}
        <div className="space-y-3">
          {!showPathInput ? (
            <div className="space-y-2">
              <button
                type="button"
                onClick={() => {
                  if (recentWorkspaces.length > 0) {
                    handleOpenPath(recentWorkspaces[0]);
                  } else {
                    setShowPathInput(true);
                  }
                }}
                disabled={isLoading}
                className="w-full py-2.5 px-4 rounded-lg bg-zinc-100 hover:bg-white text-zinc-950 font-medium text-xs transition-colors shadow-sm disabled:opacity-50"
              >
                {isLoading ? 'Opening Workspace...' : 'Open Project'}
              </button>
              <button
                type="button"
                onClick={() => setShowPathInput(true)}
                className="text-[11px] text-zinc-500 hover:text-zinc-300 transition-colors"
              >
                Choose another directory path
              </button>
            </div>
          ) : (
            <form onSubmit={handleFormSubmit} className="space-y-2 text-left">
              <div className="flex gap-2">
                <input
                  type="text"
                  value={customPath}
                  onChange={(e) => setCustomPath(e.target.value)}
                  placeholder="e.g. A:\ZenCode"
                  autoFocus
                  className="flex-1 bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-2 text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-zinc-500 font-mono"
                  disabled={isLoading}
                />
                <button
                  type="submit"
                  disabled={isLoading || !customPath.trim()}
                  className="py-2 px-4 rounded-lg bg-zinc-100 hover:bg-white text-zinc-950 font-medium text-xs transition-colors shrink-0 disabled:opacity-40"
                >
                  {isLoading ? '...' : 'Open'}
                </button>
              </div>
              <button
                type="button"
                onClick={() => setShowPathInput(false)}
                className="text-[11px] text-zinc-500 hover:text-zinc-400"
              >
                Cancel
              </button>
            </form>
          )}

          {error && (
            <div className="p-2.5 rounded-lg bg-red-950/40 border border-red-900/60 text-red-300 text-xs text-left font-mono">
              [error]: {error}
            </div>
          )}
        </div>

        {/* Recent Workspaces */}
        {recentWorkspaces.length > 0 && (
          <div className="space-y-2 pt-2 text-left">
            <div className="text-[11px] font-medium text-zinc-500 tracking-wider uppercase px-1">
              Recent Workspaces
            </div>
            <div className="rounded-lg border border-zinc-800/80 bg-zinc-900/50 divide-y divide-zinc-800/60 overflow-hidden">
              {recentWorkspaces.map((path) => {
                const folderName = path.split(/[/\\]/).filter(Boolean).pop() || path;
                return (
                  <button
                    key={path}
                    type="button"
                    onClick={() => handleOpenPath(path)}
                    disabled={isLoading}
                    className="w-full flex items-center justify-between px-3 py-2.5 hover:bg-zinc-800/60 transition-colors text-left group"
                  >
                    <div className="truncate pr-2">
                      <div className="text-xs font-medium text-zinc-200 group-hover:text-white truncate">
                        {folderName}
                      </div>
                      <div className="text-[10px] text-zinc-500 font-mono truncate">
                        {path}
                      </div>
                    </div>
                    <span className="text-[10px] text-zinc-600 group-hover:text-zinc-400 font-mono shrink-0">
                      open &rarr;
                    </span>
                  </button>
                );
              })}
            </div>
          </div>
        )}

        {/* Quiet Engine Status Dots with Tooltips */}
        <div className="pt-4 border-t border-zinc-900 flex items-center justify-center gap-5 text-[11px] text-zinc-500">
          {/* Gemini */}
          <div
            className="flex items-center gap-1.5 cursor-help group relative"
            title={
              isGeminiReady
                ? 'Gemini 3.5 Flash-Lite: Ready (Cloud Planner via OS Storage Key)'
                : 'Gemini: Key missing or check in progress'
            }
          >
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                isGeminiReady ? 'bg-emerald-400' : 'bg-zinc-600'
              }`}
            />
            <span className="group-hover:text-zinc-300 transition-colors">
              Gemini {isGeminiReady ? 'ready' : 'pending'}
            </span>
          </div>

          {/* Ollama */}
          <div
            className="flex items-center gap-1.5 cursor-help group relative"
            title={
              isOllamaReady
                ? 'Ollama Local Worker: Ready (qwen3.5:9b running at 127.0.0.1:11434)'
                : 'Ollama: Offline (run ollama serve)'
            }
          >
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                isOllamaReady ? 'bg-emerald-400' : 'bg-amber-500'
              }`}
            />
            <span className="group-hover:text-zinc-300 transition-colors">
              Ollama {isOllamaReady ? 'ready' : 'offline'}
            </span>
          </div>

          {/* OpenCode */}
          <div
            className="flex items-center gap-1.5 cursor-help group relative"
            title={`OpenCode Sidecar: ${formattedOpencode} (worktree isolated runner)`}
          >
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                isOpencodeReady ? 'bg-emerald-400' : 'bg-zinc-600'
              }`}
            />
            <span className="group-hover:text-zinc-300 transition-colors">
              OpenCode {formattedOpencode}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
};