import { useEffect, useRef, type FC } from 'react';
import { EditorState, RangeSetBuilder } from '@codemirror/state';
import { EditorView, lineNumbers, Decoration } from '@codemirror/view';

interface CodeViewerProps {
  content: string;
  filePath?: string;
  isDiff?: boolean;
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

const viewerTheme = EditorView.theme(
  {
    '&': {
      backgroundColor: '#faf9f5',
      color: '#141413',
      fontSize: '11px',
      fontFamily: '"JetBrains Mono", ui-monospace, SFMono-Regular, monospace',
      height: '100%',
    },
    '.cm-scroller': {
      overflow: 'auto',
      fontFamily: '"JetBrains Mono", ui-monospace, SFMono-Regular, monospace',
    },
    '.cm-content': {
      padding: '8px 0',
    },
    '.cm-line': {
      padding: '0 12px',
      lineHeight: '1.6',
    },
    '.cm-diff-addition': {
      backgroundColor: '#edf7ed',
      color: '#1e4620',
    },
    '.cm-diff-deletion': {
      backgroundColor: '#fdf0ed',
      color: '#7c2d12',
      textDecoration: 'line-through',
    },
    '.cm-diff-header': {
      backgroundColor: '#ede9de',
      color: '#535146',
      fontWeight: '600',
    },
    '.cm-gutters': {
      backgroundColor: '#f5f4ee',
      color: '#83827d',
      borderRight: '1px solid #dad9d4',
    },
    '.cm-lineNumbers .cm-gutterElement': {
      padding: '0 8px 0 4px',
      minWidth: '28px',
    },
  },
  { dark: false }
);

export const CodeViewer: FC<CodeViewerProps> = ({ content, isDiff = false }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    if (viewRef.current) {
      viewRef.current.destroy();
    }

    const extensions = [
      lineNumbers(),
      viewerTheme,
      EditorView.editable.of(false),
      EditorState.readOnly.of(true),
      EditorView.lineWrapping,
    ];

    if (isDiff) {
      extensions.push(diffPlugin);
    }

    const state = EditorState.create({
      doc: content,
      extensions,
    });

    viewRef.current = new EditorView({
      state,
      parent: containerRef.current,
    });

    return () => {
      if (viewRef.current) {
        viewRef.current.destroy();
        viewRef.current = null;
      }
    };
  }, [content, isDiff]);

  return <div ref={containerRef} className="h-full w-full overflow-hidden select-text" />;
};
