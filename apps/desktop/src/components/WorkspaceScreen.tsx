import { type FC } from 'react';
import type { ProjectState } from '../types';

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
    <div className="flex-1 flex flex-col justify-center items-center p-6 max-w-3xl mx-auto w-full space-y-6 font-mono text-xs select-none">
      {error && (
        <div className="w-full p-2.5 bg-[#18181b] border border-danger/50 text-danger text-xs">
          [error]: {error}
        </div>
      )}

      {/* Hero Welcome in Workspace */}
      <div className="text-center space-y-2">
        <div className="text-zinc-500 text-[10px] uppercase tracking-wider">
          project workspace :: {project && typeof project.name === 'string' ? project.name.toLowerCase() : ''}
        </div>
        <h2 className="text-base font-semibold text-zinc-100">
          [new_task_session]
        </h2>
        <p className="text-[11px] text-zinc-400 max-w-md mx-auto">
          Tasks run inside isolated Git worktrees (<code className="text-zinc-300">../.zencode-worktrees/task_*</code>).
          The primary working tree is never polluted.
        </p>
      </div>

      {/* Suggested Quick Starts */}
      <div className="w-full space-y-2">
        <div className="text-[10px] font-semibold text-zinc-500 uppercase tracking-wider px-1">
          preset task prompts
        </div>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
          {EXAMPLE_PROMPTS.map((promptText, idx) => (
            <button
              key={idx}
              type="button"
              onClick={() => onPreviewContext(promptText)}
              disabled={isLoading}
              className="p-2.5 rounded bg-[#121214] hover:bg-[#18181b] border border-[#27272a] hover:border-zinc-600 text-left text-xs text-zinc-300 hover:text-white transition-colors flex items-start justify-between group disabled:opacity-40"
            >
              <span className="leading-relaxed line-clamp-2">{promptText}</span>
              <span className="text-zinc-600 group-hover:text-zinc-300 ml-2 text-[11px] shrink-0">
                [&gt;]
              </span>
            </button>
          ))}
        </div>
      </div>

      {/* Architecture Guarantees */}
      <div className="grid grid-cols-3 gap-2.5 w-full text-[11px]">
        <div className="p-2.5 rounded bg-[#121214] border border-[#27272a] space-y-1">
          <div className="text-zinc-200 font-semibold">
            [planner]
          </div>
          <p className="text-zinc-500 text-[10px]">
            Gemini 3.5 Flash-Lite / 3.8 Flash cascade with RPM/TPM sliding windows.
          </p>
        </div>

        <div className="p-2.5 rounded bg-[#121214] border border-[#27272a] space-y-1">
          <div className="text-zinc-200 font-semibold">
            [worker]
          </div>
          <p className="text-zinc-500 text-[10px]">
            Local Ollama (Qwen 3.5 9B) executing OpenCode tools offline.
          </p>
        </div>

        <div className="p-2.5 rounded bg-[#121214] border border-[#27272a] space-y-1">
          <div className="text-zinc-200 font-semibold">
            [worktree]
          </div>
          <p className="text-zinc-500 text-[10px]">
            Dedicated branch per task. User diff review before any merge.
          </p>
        </div>
      </div>
    </div>
  );
};
