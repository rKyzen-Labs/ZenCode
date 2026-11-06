import { useState, type FC, type FormEvent } from 'react';
import type { SystemStatus } from '../types';

interface WelcomeScreenProps {
  status: SystemStatus | null;
  onOpenProject: (path: string) => Promise<void>;
  onRefreshStatus: () => Promise<void>;
  isLoading: boolean;
  error: string | null;
}

export const WelcomeScreen: FC<WelcomeScreenProps> = ({
  status,
  onOpenProject,
  onRefreshStatus,
  isLoading,
  error,
}) => {
  const [folderPath, setFolderPath] = useState('A:\\ZenCode');

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (folderPath.trim()) {
      onOpenProject(folderPath.trim());
    }
  };

  const isGeminiReady = status?.gemini_ready ?? false;
  const isOllamaReady = status?.ollama_ready ?? false;
  const isOpencodeReady = status?.opencode_health === 'Ready';

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-8 bg-[#0d0d0f] max-w-3xl mx-auto w-full font-mono text-xs select-none">
      <div className="w-full space-y-6">
        {/* Title / Hero */}
        <div className="space-y-1.5 border-b border-[#27272a] pb-4">
          <div className="text-zinc-500 text-[11px] tracking-wider uppercase">
            zencode :: local-first agent ide
          </div>
          <h1 className="text-xl font-bold tracking-tight text-zinc-100">
            [open_project]
          </h1>
          <p className="text-[11px] text-zinc-400">
            Cloud planning with Gemini free tier. Local offline execution with Ollama (Qwen 3.5 9B).
            Every task runs in an isolated Git worktree branch.
          </p>
        </div>

        {/* Project Picker Card */}
        <div className="bg-[#121214] border border-[#27272a] rounded p-4 space-y-3">
          <div className="flex items-center justify-between text-zinc-300">
            <span className="font-semibold">[target_directory]</span>
            <span className="text-[10px] text-zinc-500">git repository root</span>
          </div>

          <form onSubmit={handleSubmit} className="flex gap-2">
            <input
              type="text"
              value={folderPath}
              onChange={(e) => setFolderPath(e.target.value)}
              placeholder="e.g. A:\ZenCode"
              className="flex-1 bg-[#09090b] border border-[#27272a] rounded px-3 py-1.5 text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-zinc-500 font-mono"
              disabled={isLoading}
            />
            <button
              type="submit"
              disabled={isLoading || !folderPath.trim()}
              className="px-4 py-1.5 rounded bg-zinc-200 hover:bg-white disabled:opacity-40 text-black font-medium text-xs transition-colors shrink-0"
            >
              {isLoading ? '[opening...]' : '[open_project &gt;]'}
            </button>
          </form>

          {/* Quick open presets */}
          <div className="flex items-center gap-2 pt-1 text-[11px] text-zinc-500">
            <span>presets:</span>
            <button
              type="button"
              onClick={() => {
                setFolderPath('A:\\ZenCode');
                onOpenProject('A:\\ZenCode');
              }}
              className="px-2 py-0.5 rounded bg-[#18181b] hover:bg-[#27272a] border border-[#27272a] text-zinc-300 text-[10px] transition-colors"
            >
              A:\ZenCode
            </button>
          </div>

          {error && (
            <div className="p-2 bg-[#18181b] border border-rose-900/60 text-rose-300 text-[11px]">
              [error]: {error}
            </div>
          )}
        </div>

        {/* System & Engine Status */}
        <div className="bg-[#121214] border border-[#27272a] rounded p-4 space-y-3">
          <div className="flex items-center justify-between border-b border-[#27272a] pb-2 text-zinc-300">
            <span className="font-semibold">[subsystem_status]</span>
            <button
              type="button"
              onClick={onRefreshStatus}
              className="text-zinc-500 hover:text-zinc-200 text-[11px]"
            >
              [refresh]
            </button>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-2.5">
            {/* Gemini Planner */}
            <div className="p-2.5 rounded bg-[#09090b] border border-[#27272a] space-y-1">
              <div className="flex items-center justify-between">
                <span className="font-semibold text-zinc-200">planner</span>
                <span className={isGeminiReady ? 'text-zinc-200' : 'text-zinc-500'}>
                  {isGeminiReady ? '[ready]' : '[pending]'}
                </span>
              </div>
              <div className="text-[10px] text-zinc-500">
                {status?.gemini_status_text || 'credentials check'}
              </div>
              <div className="text-[10px] text-zinc-600">
                gemini-3.5-flash-lite
              </div>
            </div>

            {/* Ollama Worker */}
            <div className="p-2.5 rounded bg-[#09090b] border border-[#27272a] space-y-1">
              <div className="flex items-center justify-between">
                <span className="font-semibold text-zinc-200">worker</span>
                <span className={isOllamaReady ? 'text-zinc-200' : 'text-zinc-500'}>
                  {isOllamaReady ? '[ready]' : '[offline]'}
                </span>
              </div>
              <div className="text-[10px] text-zinc-500">
                {status?.ollama_status_text || 'checking service'}
              </div>
              <div className="text-[10px] text-zinc-600">
                qwen3.5:9b (ollama)
              </div>
            </div>

            {/* OpenCode Sidecar */}
            <div className="p-2.5 rounded bg-[#09090b] border border-[#27272a] space-y-1">
              <div className="flex items-center justify-between">
                <span className="font-semibold text-zinc-200">sidecar</span>
                <span className={isOpencodeReady ? 'text-zinc-200' : 'text-zinc-500'}>
                  [{status?.opencode_health?.toLowerCase() || 'standby'}]
                </span>
              </div>
              <div className="text-[10px] text-zinc-500">
                opencode tool server
              </div>
              <div className="text-[10px] text-zinc-600">
                worktree isolated
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
