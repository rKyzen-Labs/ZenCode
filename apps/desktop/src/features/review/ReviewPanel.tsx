import { useState, type FC } from 'react';
import type { ChangeInventory as ChangeInventoryType, FileChange, MergeResult } from '../../types';
import { ChangeInventory } from './ChangeInventory';
import { FileDiff } from './FileDiff';
import {
  CheckCheck,
  Trash2,
  AlertTriangle,
  GitMerge,
  ShieldCheck,
  X,
  AlertOctagon,
} from 'lucide-react';

interface ReviewPanelProps {
  inventory: ChangeInventoryType;
  taskId: string;
  onAccept: (taskId: string) => Promise<MergeResult>;
  onReject: (taskId: string) => Promise<void>;
  isLoading: boolean;
}

export const ReviewPanel: FC<ReviewPanelProps> = ({
  inventory,
  taskId,
  onAccept,
  onReject,
  isLoading,
}) => {
  const [selectedFile, setSelectedFile] = useState<FileChange | null>(
    inventory.files.length > 0 ? inventory.files[0] : null
  );
  const [showAcceptModal, setShowAcceptModal] = useState(false);
  const [showRejectModal, setShowRejectModal] = useState(false);
  const [conflictFiles, setConflictFiles] = useState<string[] | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const handleConfirmAccept = async () => {
    setActionError(null);
    try {
      const res = await onAccept(taskId);
      if ('Conflict' in res || ('status' in res && (res as any).status === 'Conflict')) {
        const files = (res as any).conflicting_files || (res as any).Conflict?.conflicting_files || [];
        setConflictFiles(files);
        setShowAcceptModal(false);
      } else {
        setShowAcceptModal(false);
      }
    } catch (err: any) {
      setActionError(typeof err === 'string' ? err : err.message || 'Failed to accept task changes');
      setShowAcceptModal(false);
    }
  };

  const handleConfirmReject = async () => {
    setActionError(null);
    try {
      await onReject(taskId);
      setShowRejectModal(false);
    } catch (err: any) {
      setActionError(typeof err === 'string' ? err : err.message || 'Failed to reject task changes');
      setShowRejectModal(false);
    }
  };

  return (
    <div className="flex-1 flex flex-col h-full space-y-4 overflow-hidden">
      {/* Top Banner */}
      <div className="p-3.5 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-between shrink-0">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 rounded-lg bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400">
            <ShieldCheck className="w-4 h-4" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <h2 className="text-sm font-semibold text-zinc-100">Task Diff Review</h2>
              <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/30">
                Isolated Worktree
              </span>
            </div>
            <p className="text-[11px] text-zinc-400">
              Agent changes exist only in worktree branch <span className="font-mono text-zinc-300">{inventory.branch_name}</span>.
              The primary working tree is untouched until you accept.
            </p>
          </div>
        </div>

        {/* Action Buttons */}
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setShowRejectModal(true)}
            disabled={isLoading}
            className="px-3 py-1.5 rounded-lg bg-rose-500/10 hover:bg-rose-500/20 text-rose-400 border border-rose-500/30 text-xs font-medium flex items-center gap-1.5 transition-colors disabled:opacity-50"
          >
            <Trash2 className="w-3.5 h-3.5" />
            <span>Reject Task</span>
          </button>

          <button
            type="button"
            onClick={() => setShowAcceptModal(true)}
            disabled={isLoading}
            className="px-3.5 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white font-medium text-xs flex items-center gap-1.5 shadow-sm transition-colors disabled:opacity-50"
          >
            <CheckCheck className="w-3.5 h-3.5" />
            <span>Accept All Changes</span>
          </button>
        </div>
      </div>

      {actionError && (
        <div className="p-3 rounded-lg bg-rose-500/10 border border-rose-500/30 text-rose-400 text-xs flex items-center justify-between shrink-0">
          <div className="flex items-center gap-2">
            <AlertTriangle className="w-4 h-4 shrink-0" />
            <span>{actionError}</span>
          </div>
          <button
            type="button"
            onClick={() => setActionError(null)}
            className="text-zinc-400 hover:text-zinc-200"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      )}

      {/* Main Diff Area: ChangeInventory on the left, FileDiff on the right */}
      <div className="flex-1 flex gap-4 min-h-0 overflow-hidden">
        <ChangeInventory
          inventory={inventory}
          selectedFile={selectedFile}
          onSelectFile={setSelectedFile}
        />
        <FileDiff file={selectedFile} />
      </div>

      {/* ACCEPT CONFIRMATION MODAL */}
      {showAcceptModal && (
        <div className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-zinc-900 border border-zinc-700 rounded-xl max-w-md w-full p-6 shadow-2xl space-y-4">
            <div className="flex items-center gap-3 text-emerald-400">
              <div className="w-10 h-10 rounded-full bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center">
                <GitMerge className="w-5 h-5" />
              </div>
              <div>
                <h3 className="text-sm font-semibold text-zinc-100">Accept Task Changes?</h3>
                <p className="text-xs text-zinc-400">Apply agent changes to primary branch</p>
              </div>
            </div>

            <div className="p-3 rounded-lg bg-zinc-950 border border-zinc-800 text-xs text-zinc-300 space-y-2 font-mono">
              <div className="flex justify-between text-zinc-400">
                <span>Task ID:</span>
                <span className="text-zinc-200">{taskId}</span>
              </div>
              <div className="flex justify-between text-zinc-400">
                <span>Branch:</span>
                <span className="text-zinc-200">{inventory.branch_name}</span>
              </div>
              <div className="flex justify-between text-zinc-400">
                <span>Total Changes:</span>
                <span>
                  <span className="text-emerald-400">+{inventory.total_additions}</span>{' '}
                  <span className="text-rose-400">-{inventory.total_deletions}</span> across{' '}
                  {inventory.files.length} file(s)
                </span>
              </div>
            </div>

            <p className="text-xs text-zinc-400 leading-relaxed">
              Accepting will verify your primary working tree is clean and merge the task branch.
              The temporary worktree and branch will be cleanly removed.
            </p>

            <div className="flex items-center justify-end gap-2 pt-2">
              <button
                type="button"
                onClick={() => setShowAcceptModal(false)}
                disabled={isLoading}
                className="px-3 py-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-xs text-zinc-300 font-medium transition-colors"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleConfirmAccept}
                disabled={isLoading}
                className="px-4 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white font-medium text-xs shadow transition-colors flex items-center gap-1.5"
              >
                <CheckCheck className="w-3.5 h-3.5" />
                <span>Confirm & Accept</span>
              </button>
            </div>
          </div>
        </div>
      )}

      {/* REJECT CONFIRMATION MODAL */}
      {showRejectModal && (
        <div className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-zinc-900 border border-zinc-700 rounded-xl max-w-md w-full p-6 shadow-2xl space-y-4">
            <div className="flex items-center gap-3 text-rose-400">
              <div className="w-10 h-10 rounded-full bg-rose-500/10 border border-rose-500/30 flex items-center justify-center">
                <Trash2 className="w-5 h-5" />
              </div>
              <div>
                <h3 className="text-sm font-semibold text-zinc-100">Reject Task Changes?</h3>
                <p className="text-xs text-zinc-400">Discard all proposed modifications</p>
              </div>
            </div>

            <p className="text-xs text-zinc-300 leading-relaxed">
              Are you sure you want to reject this task? This action will completely remove worktree{' '}
              <span className="font-mono text-zinc-200">{inventory.branch_name}</span>.
            </p>

            <div className="p-3 rounded-lg bg-rose-950/20 border border-rose-500/30 text-rose-300 text-xs flex items-center gap-2">
              <ShieldCheck className="w-4 h-4 shrink-0 text-rose-400" />
              <span>
                Your primary repository and working directory will remain completely untouched.
              </span>
            </div>

            <div className="flex items-center justify-end gap-2 pt-2">
              <button
                type="button"
                onClick={() => setShowRejectModal(false)}
                disabled={isLoading}
                className="px-3 py-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-xs text-zinc-300 font-medium transition-colors"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleConfirmReject}
                disabled={isLoading}
                className="px-4 py-1.5 rounded-lg bg-rose-600 hover:bg-rose-500 text-white font-medium text-xs shadow transition-colors flex items-center gap-1.5"
              >
                <Trash2 className="w-3.5 h-3.5" />
                <span>Discard Task Changes</span>
              </button>
            </div>
          </div>
        </div>
      )}

      {/* MERGE CONFLICT DIALOG (Strictly stops and shows options, NEVER auto-resolves) */}
      {conflictFiles && (
        <div className="fixed inset-0 z-50 bg-black/75 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-zinc-900 border border-amber-500/40 rounded-xl max-w-lg w-full p-6 shadow-2xl space-y-4">
            <div className="flex items-center gap-3 text-amber-400">
              <div className="w-10 h-10 rounded-full bg-amber-500/10 border border-amber-500/30 flex items-center justify-center">
                <AlertOctagon className="w-5 h-5" />
              </div>
              <div>
                <h3 className="text-sm font-semibold text-zinc-100">Merge Conflict Detected</h3>
                <p className="text-xs text-zinc-400">Auto-resolution disabled for safety</p>
              </div>
            </div>

            <p className="text-xs text-zinc-300 leading-relaxed">
              The task branch cannot be fast-forwarded or automatically merged into your target branch
              because conflicting changes exist in the following files:
            </p>

            <div className="max-h-40 overflow-y-auto p-3 rounded-lg bg-zinc-950 border border-zinc-800 font-mono text-xs text-amber-300 space-y-1">
              {conflictFiles.map((file, i) => (
                <div key={i} className="flex items-center gap-2">
                  <span className="w-1.5 h-1.5 rounded-full bg-amber-400" />
                  <span>{file}</span>
                </div>
              ))}
            </div>

            <div className="p-3 rounded-lg bg-zinc-800/50 border border-zinc-700 text-xs text-zinc-300 space-y-1">
              <p className="font-semibold text-zinc-200">Safety Guarantee:</p>
              <p className="text-zinc-400">
                The merge attempt was automatically aborted (<code className="text-zinc-300 font-mono">git merge --abort</code>).
                Your primary working tree remains clean and untouched.
              </p>
            </div>

            <div className="flex items-center justify-end gap-2 pt-2">
              <button
                type="button"
                onClick={() => setConflictFiles(null)}
                className="px-3 py-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-xs text-zinc-300 font-medium transition-colors"
              >
                Close & Keep Reviewing
              </button>
              <button
                type="button"
                onClick={() => {
                  setConflictFiles(null);
                  setShowRejectModal(true);
                }}
                className="px-3.5 py-1.5 rounded-lg bg-rose-600 hover:bg-rose-500 text-white font-medium text-xs transition-colors flex items-center gap-1.5"
              >
                <Trash2 className="w-3.5 h-3.5" />
                <span>Reject & Clean Up</span>
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
