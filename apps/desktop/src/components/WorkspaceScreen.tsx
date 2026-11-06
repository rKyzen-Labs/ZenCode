import { type FC } from 'react';
import type { ProjectState } from '../types';
import {
  Brain,
  Cpu,
  ArrowRight,
  Shield,
  Code2,
} from 'lucide-react';

interface WorkspaceScreenProps {
  project: ProjectState;
  onPreviewContext: (taskDescription: string) => Promise<void>;
  onChangeProject: () => void;
  isLoading: boolean;
  error: string | null;
}

const EXAMPLE_PROMPTS = [
  'Add a function called add_numbers that takes two integers and returns their sum, with unit tests',
  'Implement health check endpoint with JSON uptime telemetry',
  'Add logging to error handling branches with structured metadata',
  'Create unit test suite covering edge case scenarios',
];

export const WorkspaceScreen: FC<WorkspaceScreenProps> = ({
  project,
  onPreviewContext,
  isLoading,
  error,
}) => {
  return (
    <div className="flex-1 flex flex-col justify-center items-center p-6 max-w-3xl mx-auto w-full space-y-6">
      {error && (
        <div className="w-full p-3.5 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-400 text-xs">
          {error}
        </div>
      )}

      {/* Hero Welcome in Workspace */}
      <div className="text-center space-y-2">
        <div className="w-12 h-12 rounded-2xl bg-blue-600/15 border border-blue-500/30 flex items-center justify-center text-blue-400 mx-auto shadow-inner">
          <Code2 className="w-6 h-6" />
        </div>
        <h2 className="text-lg font-semibold text-zinc-100">
          What would you like to build in <span className="text-blue-400">{project.name}</span>?
        </h2>
        <p className="text-xs text-zinc-400 max-w-md mx-auto">
          Every coding task runs in an isolated Git worktree branch. Your primary working tree is never touched until you accept changes.
        </p>
      </div>

      {/* Suggested Quick Starts */}
      <div className="w-full space-y-2">
        <div className="text-[11px] font-semibold text-zinc-500 uppercase tracking-wider px-1">
          Example Tasks
        </div>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
          {EXAMPLE_PROMPTS.map((promptText, idx) => (
            <button
              key={idx}
              type="button"
              onClick={() => onPreviewContext(promptText)}
              disabled={isLoading}
              className="p-3 rounded-xl bg-[#141417] hover:bg-[#1a1a20] border border-[#27272a] hover:border-zinc-700 text-left text-xs text-zinc-300 hover:text-zinc-100 transition-all flex items-start justify-between group shadow-sm disabled:opacity-50"
            >
              <span className="leading-relaxed line-clamp-2">{promptText}</span>
              <ArrowRight className="w-3.5 h-3.5 text-zinc-600 group-hover:text-blue-400 shrink-0 ml-2 mt-0.5 transition-colors" />
            </button>
          ))}
        </div>
      </div>

      {/* Architecture Guarantees */}
      <div className="grid grid-cols-3 gap-3 w-full text-xs font-mono">
        <div className="p-3 rounded-xl bg-[#141417] border border-[#27272a] space-y-1">
          <div className="flex items-center gap-1.5 text-purple-400 font-sans font-semibold text-[11px]">
            <Brain className="w-3.5 h-3.5" />
            <span>Gemini Planner</span>
          </div>
          <p className="text-zinc-400 text-[10px] font-sans">
            Auto-cascade across free-tier Flash-Lite models with strict quota tracking.
          </p>
        </div>

        <div className="p-3 rounded-xl bg-[#141417] border border-[#27272a] space-y-1">
          <div className="flex items-center gap-1.5 text-emerald-400 font-sans font-semibold text-[11px]">
            <Cpu className="w-3.5 h-3.5" />
            <span>Local Worker</span>
          </div>
          <p className="text-zinc-400 text-[10px] font-sans">
            Ollama Qwen 3.5 (9B) executes tools directly on your local machine.
          </p>
        </div>

        <div className="p-3 rounded-xl bg-[#141417] border border-[#27272a] space-y-1">
          <div className="flex items-center gap-1.5 text-blue-400 font-sans font-semibold text-[11px]">
            <Shield className="w-3.5 h-3.5" />
            <span>Git Worktree</span>
          </div>
          <p className="text-zinc-400 text-[10px] font-sans">
            Dedicated worktree branch per task. Zero primary tree pollution.
          </p>
        </div>
      </div>
    </div>
  );
};
