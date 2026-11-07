import React from 'react';
import type { CommandRequest } from '../types';

interface CommandApprovalDialogProps {
  request: CommandRequest | null;
  isOpen: boolean;
  onApprove: (request: CommandRequest) => void;
  onDeny: (request: CommandRequest) => void;
}

export const CommandApprovalDialog: React.FC<CommandApprovalDialogProps> = ({
  request,
  isOpen,
  onApprove,
  onDeny,
}) => {
  if (!isOpen || !request) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm p-4 font-mono">
      <div className="w-full max-w-xl border border-zinc-700 bg-zinc-950 p-6 shadow-2xl text-xs space-y-4">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-zinc-800 pb-3">
          <div className="flex items-center gap-2">
            <span className="text-amber-400 font-bold">[!]</span>
            <span className="text-zinc-100 font-bold tracking-wider uppercase">
              Command Execution Approval Required
            </span>
          </div>
          <span className="text-[10px] text-zinc-500">{request.id}</span>
        </div>

        {/* Notice */}
        <div className="text-zinc-400 text-[11px] leading-relaxed">
          The agent requests permission to execute a shell command inside the task environment.
          Review the command array and working directory before approving:
        </div>

        {/* Command Box */}
        <div className="border border-zinc-800 bg-zinc-900/60 p-3 space-y-2">
          <div className="flex items-start gap-2">
            <span className="text-zinc-500 w-16 shrink-0">program:</span>
            <span className="text-emerald-400 font-bold">{request.program}</span>
          </div>
          <div className="flex items-start gap-2">
            <span className="text-zinc-500 w-16 shrink-0">args:</span>
            <span className="text-zinc-200">
              {request.args.length > 0 ? (
                <span className="bg-zinc-800 px-1.5 py-0.5 rounded text-zinc-300">
                  [{request.args.map((a) => `"${a}"`).join(', ')}]
                </span>
              ) : (
                <span className="text-zinc-600">(none)</span>
              )}
            </span>
          </div>
          <div className="flex items-start gap-2">
            <span className="text-zinc-500 w-16 shrink-0">cwd:</span>
            <span className="text-zinc-300 break-all select-all font-sans text-[11px]">
              {request.cwd}
            </span>
          </div>
          {request.description && (
            <div className="flex items-start gap-2 pt-1 border-t border-zinc-800/80">
              <span className="text-zinc-500 w-16 shrink-0">purpose:</span>
              <span className="text-zinc-400 italic">{request.description}</span>
            </div>
          )}
        </div>

        {/* Verification Guarantee */}
        <div className="text-[10px] text-zinc-500 border border-zinc-900 bg-zinc-950 p-2 space-y-1">
          <div>* Process supervised via Windows Job Object with KILL_ON_JOB_CLOSE.</div>
          <div>* Zero orphaned background processes guaranteed upon cancellation.</div>
          <div>* All stdout/stderr outputs pass through strict credential redaction.</div>
        </div>

        {/* Actions */}
        <div className="flex items-center justify-end gap-3 pt-2 border-t border-zinc-800">
          <button
            onClick={() => onDeny(request)}
            className="px-3 py-1.5 border border-zinc-700 bg-zinc-900 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 transition-colors"
          >
            [deny_and_cancel]
          </button>
          <button
            onClick={() => onApprove(request)}
            className="px-3 py-1.5 border border-emerald-500/60 bg-emerald-950/40 hover:bg-emerald-900/60 text-emerald-300 font-bold transition-colors"
          >
            [approve_and_run]
          </button>
        </div>
      </div>
    </div>
  );
};
