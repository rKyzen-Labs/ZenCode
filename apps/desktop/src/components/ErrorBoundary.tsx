import { Component, type ReactNode, type ErrorInfo } from 'react';

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
        <div className="flex h-screen w-screen flex-col items-center justify-center bg-[#09090b] p-8 text-zinc-100 select-none font-mono text-xs">
          <div className="max-w-lg w-full bg-[#121214] border border-[#3f3f46] rounded p-5 space-y-4 shadow-2xl">
            <div className="flex items-center justify-between border-b border-[#27272a] pb-2">
              <span className="font-semibold text-zinc-100">[ui_error: crashed]</span>
              <span className="text-[10px] text-zinc-500">error_boundary</span>
            </div>

            <div className="p-2.5 bg-[#09090b] border border-[#27272a] text-zinc-300 overflow-x-auto max-h-40 whitespace-pre-wrap text-[11px]">
              {this.state.error?.message || 'Unknown UI Error'}
            </div>

            <div className="flex justify-end pt-2 border-t border-[#27272a]">
              <button
                type="button"
                onClick={this.handleReset}
                className="px-3 py-1.5 rounded bg-zinc-200 hover:bg-white text-black font-medium transition-colors"
              >
                [reload_application]
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
