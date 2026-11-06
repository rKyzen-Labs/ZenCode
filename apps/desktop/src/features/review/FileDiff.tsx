import { useEffect, useRef, type FC } from 'react';
import { EditorState, RangeSetBuilder } from '@codemirror/state';
import { EditorView, lineNumbers, Decoration } from '@codemirror/view';
import type { FileChange } from '../../types';

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
      fontSize: '11px',
      fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
      height: '100%',
    },
    '.cm-scroller': {
      overflow: 'auto',
      fontFamily: 'inherit',
    },
    '.cm-content': {
      padding: '4px 0',
    },
    '.cm-line': {
      padding: '0 8px',
      lineHeight: '1.6',
    },
    '.cm-diff-addition': {
      backgroundColor: 'rgba(255, 255, 255, 0.08)',
      color: '#f4f4f5',
    },
    '.cm-diff-deletion': {
      backgroundColor: 'rgba(255, 255, 255, 0.04)',
      color: '#71717a',
      textDecoration: 'line-through',
    },
    '.cm-diff-header': {
      backgroundColor: '#18181b',
      color: '#a1a1aa',
      fontWeight: '600',
    },
    '.cm-gutters': {
      backgroundColor: '#09090b',
      color: '#3f3f46',
      borderRight: '1px solid #1f1f23',
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
      <div className="flex-1 flex flex-col items-center justify-center p-8 text-center text-zinc-600 bg-[#09090b] rounded border border-[#27272a] font-mono text-xs">
        <span className="text-zinc-500 mb-1">[no_file_selected]</span>
        <span className="text-[11px] text-zinc-600">
          select a changed file from the list to view its diff
        </span>
      </div>
    );
  }

  // Binary file placeholder - DO NOT attempt diff
  if (file.is_binary) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center p-8 text-center bg-[#09090b] rounded border border-[#27272a] font-mono text-xs space-y-2">
        <span className="text-zinc-300 font-semibold">[binary_file]</span>
        <div className="text-[11px] text-zinc-500 max-w-sm">
          {file.path} is a binary file. Line-by-line diff is omitted.
        </div>
        <div className="text-[10px] text-zinc-600">
          [status: tracked in task worktree]
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col h-full overflow-hidden bg-[#09090b] rounded border border-[#27272a] font-mono">
      {/* File header bar */}
      <div className="px-3 py-1.5 bg-[#121214] border-b border-[#27272a] flex items-center justify-between text-xs shrink-0">
        <div className="flex items-center gap-2 truncate">
          <span className="text-zinc-200 font-medium truncate">{file.path}</span>
          {file.original_path && (
            <span className="text-zinc-500 text-[10px]">
              (from: {file.original_path})
            </span>
          )}
        </div>
        <div className="flex items-center gap-2 shrink-0 text-[10px] text-zinc-400">
          <span>+{file.additions}</span>
          <span>-{file.deletions}</span>
          {file.mode_change && (
            <span className="text-zinc-500 border border-[#27272a] px-1 rounded">
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
