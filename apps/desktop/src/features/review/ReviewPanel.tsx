import { useState, type FC } from 'react';
import type { ChangeInventory as ChangeInventoryType, FileChange, MergeResult } from '../../types';
import { ChangeInventory } from './ChangeInventory';
import { FileDiff } from './FileDiff';

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
    <div className="flex-1 flex flex-col h-full space-y-3 overflow-hidden font-mono text-xs">
      {/* Top Banner - Monochromatic */}
      <div className="p-2.5 rounded bg-[#121214] border border-[#27272a] flex items-center justify-between shrink-0">
        <div className="space-y-0.5">
          <div className="flex items-center gap-2">
            <span className="text-zinc-100 font-semibold">[review_changes]</span>
            <span className="text-[10px] text-zinc-400 border border-[#3f3f46] px-1.5 py-0.2 rounded">
              isolated worktree
            </span>
          </div>
          <div className="text-[11px] text-zinc-500">
            branch: <span className="text-zinc-300">{inventory.branch_name}</span> | primary tree untouched
          </div>
        </div>

        {/* Action Buttons */}
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setShowRejectModal(true)}
            disabled={isLoading}
            className="px-2.5 py-1 rounded bg-[#18181b] hover:bg-[#27272a] text-zinc-300 hover:text-white border border-[#27272a] text-xs transition-colors disabled:opacity-40"
          >
            [reject_task]
          </button>

          <button
            type="button"
            onClick={() => setShowAcceptModal(true)}
            disabled={isLoading}
            className="px-3 py-1 rounded bg-zinc-200 hover:bg-white text-black font-medium text-xs transition-colors disabled:opacity-40"
          >
            [accept_all]
          </button>
        </div>
      </div>

      {actionError && (
        <div className="p-2 bg-[#18181b] border border-danger/50 text-danger text-xs flex items-center justify-between shrink-0">
          <span>[error]: {actionError}</span>
          <button
            type="button"
            onClick={() => setActionError(null)}
            className="text-zinc-500 hover:text-zinc-200 px-1"
          >
            [x]
          </button>
        </div>
      )}

      {/* Main Diff Area */}
      <div className="flex-1 flex gap-3 min-h-0 overflow-hidden">
        <ChangeInventory
          inventory={inventory}
          selectedFile={selectedFile}
          onSelectFile={setSelectedFile}
        />
        <FileDiff file={selectedFile} />
      </div>

      {/* ACCEPT CONFIRMATION MODAL */}
      {showAcceptModal && (
        <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4 font-mono text-xs">
          <div className="bg-[#121214] border border-[#3f3f46] rounded max-w-md w-full p-5 space-y-4 shadow-2xl">
            <div className="flex items-center justify-between border-b border-[#27272a] pb-2">
              <span className="font-semibold text-zinc-100">[confirm_accept]</span>
              <button
                type="button"
                onClick={() => setShowAcceptModal(false)}
                className="text-zinc-500 hover:text-zinc-200"
              >
                [x]
              </button>
            </div>

            <p className="text-zinc-300 leading-relaxed">
              Accepting will verify the primary working tree is clean and fast-forward or merge the task branch.
              The temporary worktree and branch will then be safely deleted.
            </p>

            <div className="p-2.5 bg-[#09090b] border border-[#27272a] text-[11px] text-zinc-400 space-y-1">
              <div className="flex justify-between">
                <span>task_id:</span>
                <span className="text-zinc-200">{taskId}</span>
              </div>
              <div className="flex justify-between">
                <span>worktree_branch:</span>
                <span className="text-zinc-200">{inventory.branch_name}</span>
              </div>
              <div className="flex justify-between">
                <span>total_changes:</span>
                <span className="text-zinc-200">
                  +{inventory.total_additions} / -{inventory.total_deletions} ({inventory.files.length} files)
                </span>
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-2 border-t border-[#27272a]">
              <button
                type="button"
                onClick={() => setShowAcceptModal(false)}
                disabled={isLoading}
                className="px-3 py-1 rounded bg-[#18181b] hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200"
              >
                [cancel]
              </button>
              <button
                type="button"
                onClick={handleConfirmAccept}
                disabled={isLoading}
                className="px-3 py-1 rounded bg-zinc-200 hover:bg-white text-black font-medium"
              >
                {isLoading ? '[merging...]' : '[confirm_merge]'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* REJECT CONFIRMATION MODAL */}
      {showRejectModal && (
        <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4 font-mono text-xs">
          <div className="bg-[#121214] border border-[#3f3f46] rounded max-w-md w-full p-5 space-y-4 shadow-2xl">
            <div className="flex items-center justify-between border-b border-[#27272a] pb-2">
              <span className="font-semibold text-zinc-100">[confirm_reject]</span>
              <button
                type="button"
                onClick={() => setShowRejectModal(false)}
                className="text-zinc-500 hover:text-zinc-200"
              >
                [x]
              </button>
            </div>

            <p className="text-zinc-300 leading-relaxed">
              Discard all proposed changes in worktree <span className="text-zinc-100 underline">{inventory.branch_name}</span>?
            </p>

            <div className="p-2.5 bg-[#09090b] border border-[#27272a] text-[11px] text-zinc-400">
              * Safety guarantee: Your primary repository and working directory remain untouched.
              The temporary worktree and branch will be cleanly removed.
            </div>

            <div className="flex items-center justify-end gap-2 pt-2 border-t border-[#27272a]">
              <button
                type="button"
                onClick={() => setShowRejectModal(false)}
                disabled={isLoading}
                className="px-3 py-1 rounded bg-[#18181b] hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200"
              >
                [cancel]
              </button>
              <button
                type="button"
                onClick={handleConfirmReject}
                disabled={isLoading}
                className="px-3 py-1 rounded bg-[#27272a] hover:bg-zinc-700 text-zinc-200 font-medium"
              >
                {isLoading ? '[removing...]' : '[discard_changes]'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* MERGE CONFLICT MODAL */}
      {conflictFiles && (
        <div className="fixed inset-0 z-50 bg-black/85 flex items-center justify-center p-4 font-mono text-xs">
          <div className="bg-[#121214] border border-zinc-600 rounded max-w-lg w-full p-5 space-y-4 shadow-2xl">
            <div className="flex items-center justify-between border-b border-[#27272a] pb-2">
              <span className="font-semibold text-zinc-100">[merge_conflict: stopped]</span>
              <button
                type="button"
                onClick={() => setConflictFiles(null)}
                className="text-zinc-500 hover:text-zinc-200"
              >
                [x]
              </button>
            </div>

            <p className="text-zinc-300 leading-relaxed">
              The task branch cannot be fast-forwarded or automatically merged into the target branch.
              Automatic conflict resolution is disabled for safety.
            </p>

            <div className="max-h-40 overflow-y-auto p-2.5 bg-[#09090b] border border-[#27272a] text-zinc-300 space-y-1">
              <div className="text-[10px] text-zinc-500 uppercase pb-1 border-b border-[#1f1f23]">
                Conflicting files:
              </div>
              {conflictFiles.map((file, i) => (
                <div key={i} className="text-[11px] text-zinc-300">
                  ! {file}
                </div>
              ))}
            </div>

            <div className="p-2 bg-[#09090b] border border-[#27272a] text-[11px] text-zinc-500">
              * Merge aborted (git merge --abort). Primary working tree remains clean.
            </div>

            <div className="flex items-center justify-end gap-2 pt-2 border-t border-[#27272a]">
              <button
                type="button"
                onClick={() => setConflictFiles(null)}
                className="px-3 py-1 rounded bg-[#18181b] hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200"
              >
                [close]
              </button>
              <button
                type="button"
                onClick={() => {
                  setConflictFiles(null);
                  setShowRejectModal(true);
                }}
                className="px-3 py-1 rounded bg-[#27272a] hover:bg-zinc-700 text-zinc-200 font-medium"
              >
                [reject_and_clean_up]
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
