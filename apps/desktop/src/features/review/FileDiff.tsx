import { useEffect, useRef, type FC } from 'react';
import { EditorState, RangeSetBuilder } from '@codemirror/state';
import { EditorView, lineNumbers, Decoration } from '@codemirror/view';
import type { FileChange } from '../../types';
import { Binary, FileCode, CheckCircle2 } from 'lucide-react';

interface FileDiffProps {
  file: FileChange | null;
}

const additionDeco = Decoration.line({
  attributes: { class: 'cm-diff-addition' },
});

const deletionDeco = Decoration.line({
  attributes: { class: 'cm-diff-deletion' },
});

const headerDeco = Decoration.line({
  attributes: { class: 'cm-diff-header' },
});

const diffTheme = EditorView.theme(
  {
    '&': {
      backgroundColor: '#09090b',
      color: '#e4e4e7',
      fontSize: '12px',
      fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
      height: '100%',
    },
    '.cm-scroller': {
      overflow: 'auto',
      fontFamily: 'inherit',
    },
    '.cm-content': {
      padding: '8px 0',
    },
    '.cm-line': {
      padding: '0 8px',
      lineHeight: '1.6',
    },
    '.cm-diff-addition': {
      backgroundColor: 'rgba(16, 185, 129, 0.18)',
      color: '#6ee7b7',
    },
    '.cm-diff-deletion': {
      backgroundColor: 'rgba(244, 63, 94, 0.18)',
      color: '#fda4af',
    },
    '.cm-diff-header': {
      backgroundColor: 'rgba(147, 51, 234, 0.15)',
      color: '#c084fc',
      fontWeight: '600',
    },
    '.cm-gutters': {
      backgroundColor: '#09090b',
      color: '#52525b',
      borderRight: '1px solid #27272a',
    },
    '.cm-lineNumbers .cm-gutterElement': {
      padding: '0 8px 0 4px',
      minWidth: '28px',
    },
  },
  { dark: true }
);

function buildDecorations(state: EditorState) {
  const builder = new RangeSetBuilder<Decoration>();
  for (let i = 1; i <= state.doc.lines; i++) {
    const line = state.doc.line(i);
    const text = line.text;

    if (text.startsWith('+++') || text.startsWith('---')) {
      builder.add(line.from, line.from, headerDeco);
    } else if (text.startsWith('+')) {
      builder.add(line.from, line.from, additionDeco);
    } else if (text.startsWith('-')) {
      builder.add(line.from, line.from, deletionDeco);
    } else if (text.startsWith('@@')) {
      builder.add(line.from, line.from, headerDeco);
    }
  }
  return builder.finish();
}

const diffPlugin = EditorView.decorations.compute(['doc'], (state) => {
  return buildDecorations(state);
});

export const FileDiff: FC<FileDiffProps> = ({ file }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    // Clean up previous view
    if (viewRef.current) {
      viewRef.current.destroy();
      viewRef.current = null;
    }

    if (!file || file.is_binary || !file.diff_content) {
      return;
    }

    const state = EditorState.create({
      doc: file.diff_content,
      extensions: [
        lineNumbers(),
        diffTheme,
        diffPlugin,
        EditorState.readOnly.of(true),
        EditorView.editable.of(false),
        EditorView.lineWrapping,
      ],
    });

    const view = new EditorView({
      state,
      parent: containerRef.current,
    });

    viewRef.current = view;

    return () => {
      view.destroy();
      viewRef.current = null;
    };
  }, [file]);

  if (!file) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center p-8 text-center text-zinc-500 bg-zinc-950/50 rounded-lg border border-zinc-800">
        <FileCode className="w-10 h-10 mb-3 text-zinc-600 stroke-1" />
        <p className="text-sm font-medium text-zinc-400">No file selected</p>
        <p className="text-xs text-zinc-600 mt-1">
          Select a changed file from the inventory on the left to inspect its diff.
        </p>
      </div>
    );
  }

  // Binary file placeholder - DO NOT attempt diff, show clear placeholder
  if (file.is_binary) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center p-8 text-center bg-zinc-950/60 rounded-lg border border-zinc-800">
        <div className="w-12 h-12 rounded-xl bg-purple-500/10 border border-purple-500/30 flex items-center justify-center text-purple-400 mb-3">
          <Binary className="w-6 h-6" />
        </div>
        <h4 className="text-sm font-semibold text-zinc-200">Binary File</h4>
        <p className="text-xs text-zinc-400 max-w-sm mt-1">
          <span className="font-mono text-zinc-300">{file.path}</span> is a binary file.
          Diff content cannot be rendered line-by-line.
        </p>
        <div className="mt-4 px-3 py-1.5 rounded bg-zinc-900 border border-zinc-800 text-[11px] text-zinc-400 flex items-center gap-2">
          <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
          <span>Tracked in task worktree branch</span>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col h-full overflow-hidden bg-zinc-950 rounded-lg border border-zinc-800">
      {/* File header bar */}
      <div className="px-4 py-2 bg-zinc-900/80 border-b border-zinc-800 flex items-center justify-between text-xs font-mono shrink-0">
        <div className="flex items-center gap-2 truncate">
          <span className="text-zinc-200 font-medium">{file.path}</span>
          {file.original_path && (
            <span className="text-zinc-500 text-[11px]">
              (renamed from {file.original_path})
            </span>
          )}
        </div>
        <div className="flex items-center gap-3 shrink-0 text-[11px]">
          <span className="text-emerald-400">+{file.additions}</span>
          <span className="text-rose-400">-{file.deletions}</span>
          {file.mode_change && (
            <span className="text-amber-400 font-sans text-[10px] px-1.5 py-0.5 rounded bg-amber-500/10 border border-amber-500/30">
              mode {file.mode_change}
            </span>
          )}
        </div>
      </div>

      {/* CodeMirror 6 diff container */}
      <div ref={containerRef} className="flex-1 overflow-auto" />
    </div>
  );
};
