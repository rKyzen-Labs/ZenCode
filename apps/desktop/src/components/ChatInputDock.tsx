import React, { useState, useRef, useEffect } from 'react';

interface ChatInputDockProps {
  onSubmit: (prompt: string) => void;
  isLoading: boolean;
  disabled?: boolean;
  activeModelName: string;
  isPinned: boolean;
  onSelectModel?: (modelId: string | null) => void;
  placeholder?: string;
}

export const ChatInputDock: React.FC<ChatInputDockProps> = ({
  onSubmit,
  isLoading,
  disabled = false,
  activeModelName,
  isPinned,
  onSelectModel,
  placeholder = 'Ask anything, describe a task, or run checks...',
}) => {
  const [prompt, setPrompt] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 160)}px`;
    }
  }, [prompt]);

  const handleSubmit = () => {
    if (prompt.trim() && !isLoading) {
      onSubmit(prompt.trim());
      setPrompt('');
      if (textareaRef.current) {
        textareaRef.current.style.height = 'auto';
      }
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <div className="p-3 border-t border-zinc-800/80 bg-[#0c0c0e] font-sans shrink-0">
      <div className="max-w-4xl mx-auto rounded-xl border border-zinc-700/80 bg-[#121214] shadow-md focus-within:border-zinc-500 transition-colors p-2.5 space-y-2">
        <textarea
          ref={textareaRef}
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          rows={1}
          disabled={isLoading || disabled}
          className="w-full bg-transparent text-xs text-zinc-100 placeholder-zinc-500 focus:outline-none resize-none font-sans leading-relaxed selection:bg-zinc-700 max-h-40 overflow-y-auto disabled:opacity-50"
        />

        <div className="flex items-center justify-between pt-1 border-t border-zinc-800/60">
          {/* Left Model Pill */}
          <button
            type="button"
            onClick={() => onSelectModel?.(isPinned ? null : 'gemini-3.8-flash')}
            title="Toggle between 3.5 Flash-Lite and 3.8 Flash"
            className="flex items-center gap-1.5 text-[11px] text-zinc-400 font-mono hover:text-zinc-200 transition-colors"
          >
            <span className="w-1.5 h-1.5 rounded-full bg-accent" />
            <span>{isPinned ? 'pinned: 3.8-flash' : activeModelName}</span>
          </button>

          {/* Right Submit Button */}
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handleSubmit}
              disabled={isLoading || disabled || !prompt.trim()}
              className="px-3 py-1.5 rounded-lg bg-zinc-100 hover:bg-white text-zinc-950 font-medium text-xs flex items-center gap-1 transition-colors disabled:opacity-30 shadow-sm"
            >
              <span>Send</span>
              <span className="font-mono text-[11px]">&uarr;</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};