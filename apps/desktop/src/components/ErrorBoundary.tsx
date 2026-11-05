import { Component, type ReactNode, type ErrorInfo } from 'react';
import { AlertOctagon, RotateCcw } from 'lucide-react';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null, errorInfo: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error, errorInfo: null };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('ErrorBoundary caught an unhandled rendering error:', error, errorInfo);
    this.setState({ error, errorInfo });
  }

  handleReset = () => {
    this.setState({ hasError: false, error: null, errorInfo: null });
    window.location.reload();
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="flex h-screen w-screen flex-col items-center justify-center bg-[var(--background)] p-8 text-[var(--foreground)] select-none">
          <div className="max-w-lg w-full bg-[var(--surface)] border border-rose-500/40 rounded-xl p-6 shadow-2xl space-y-4">
            <div className="flex items-center gap-3 border-b border-[var(--surface-border)] pb-3">
              <div className="w-10 h-10 rounded-lg bg-rose-500/10 border border-rose-500/30 flex items-center justify-center text-rose-400 shrink-0">
                <AlertOctagon className="w-5 h-5" />
              </div>
              <div>
                <h2 className="text-sm font-semibold text-zinc-100">Something went wrong</h2>
                <p className="text-xs text-[var(--foreground-muted)]">
                  An unexpected UI error occurred.
                </p>
              </div>
            </div>

            <div className="p-3 bg-rose-500/10 border border-rose-500/20 rounded-lg text-xs font-mono text-rose-300 overflow-x-auto max-h-40 whitespace-pre-wrap">
              {this.state.error?.message || 'Unknown UI Error'}
            </div>

            <div className="flex justify-end pt-2">
              <button
                type="button"
                onClick={this.handleReset}
                className="px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-500 text-white font-medium text-xs flex items-center gap-2 transition-colors shadow-sm"
              >
                <RotateCcw className="w-3.5 h-3.5" />
                <span>Reload Application</span>
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
