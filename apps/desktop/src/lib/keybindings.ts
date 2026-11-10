export interface KeybindingDef {
  id: string;
  label: string;
  description: string;
  category: 'Navigation' | 'Chat' | 'Workspace & Pane' | 'General';
  defaultCombo: string; // e.g. "ctrl+t"
}

export const DEFAULT_KEYBINDINGS: KeybindingDef[] = [
  {
    id: 'new_chat',
    label: 'New Chat Session / Tab',
    description: 'Create a new tab and chat session',
    category: 'Navigation',
    defaultCombo: 'ctrl+t',
  },
  {
    id: 'close_tab',
    label: 'Close Active Tab',
    description: 'Close the currently selected chat tab',
    category: 'Navigation',
    defaultCombo: 'ctrl+w',
  },
  {
    id: 'next_tab',
    label: 'Next Tab',
    description: 'Switch to the next tab on the right',
    category: 'Navigation',
    defaultCombo: 'ctrl+tab',
  },
  {
    id: 'prev_tab',
    label: 'Previous Tab',
    description: 'Switch to the previous tab on the left',
    category: 'Navigation',
    defaultCombo: 'ctrl+shift+tab',
  },
  {
    id: 'toggle_files_pane',
    label: 'Toggle Workspace Files Pane',
    description: 'Open or collapse the files & workspace tree side pane',
    category: 'Workspace & Pane',
    defaultCombo: 'ctrl+b',
  },
  {
    id: 'toggle_terminal',
    label: 'Open Terminal in Pane',
    description: 'Open the right pane and select the Terminal tab',
    category: 'Workspace & Pane',
    defaultCombo: 'ctrl+`',
  },
  {
    id: 'open_search',
    label: 'Search & History',
    description: 'Open the search and session history page',
    category: 'Navigation',
    defaultCombo: 'ctrl+k',
  },
  {
    id: 'open_settings',
    label: 'Settings',
    description: 'Navigate directly to application settings',
    category: 'Navigation',
    defaultCombo: 'ctrl+,',
  },
  {
    id: 'shortcuts_help',
    label: 'Keyboard Shortcuts Cheat Sheet',
    description: 'Show the popup with all active keyboard shortcuts',
    category: 'General',
    defaultCombo: 'ctrl+/',
  },
  {
    id: 'focus_prompt',
    label: 'Focus Chat Input',
    description: 'Jump cursor directly to the prompt input field',
    category: 'Chat',
    defaultCombo: 'ctrl+l',
  },
];

const STORAGE_KEY = 'zencode_custom_keybindings';

/** Loads custom keybindings merged with defaults */
export function loadKeybindings(): Record<string, string> {
  const map: Record<string, string> = {};
  for (const item of DEFAULT_KEYBINDINGS) {
    map[item.id] = item.defaultCombo;
  }

  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (typeof parsed === 'object' && parsed !== null) {
        for (const [k, v] of Object.entries(parsed)) {
          if (typeof v === 'string' && v.trim()) {
            map[k] = v.trim().toLowerCase();
          }
        }
      }
    }
  } catch (e) {
    console.error('Failed to load keybindings from localStorage:', e);
  }

  return map;
}

/** Saves custom keybindings to localStorage */
export function saveKeybinding(id: string, newCombo: string): Record<string, string> {
  const current = loadKeybindings();
  current[id] = newCombo.trim().toLowerCase();
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(current));
  } catch (e) {
    console.error('Failed to save keybinding:', e);
  }
  return current;
}

/** Resets all keybindings to defaults */
export function resetAllKeybindings(): Record<string, string> {
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch (e) {
    console.error('Failed to clear keybindings:', e);
  }
  return loadKeybindings();
}

/** Converts a serialized combo (e.g. "ctrl+shift+t") into display pills (e.g. ["Ctrl", "Shift", "T"]) */
export function formatKeyDisplay(combo: string): string[] {
  const isMac = typeof navigator !== 'undefined' && /Mac|iPod|iPhone|iPad/.test(navigator.platform);
  const parts = combo.split('+').map((p) => p.trim().toLowerCase());

  return parts.map((p) => {
    if (p === 'ctrl' || p === 'control') return isMac ? '⌘' : 'Ctrl';
    if (p === 'shift') return isMac ? '⇧' : 'Shift';
    if (p === 'alt') return isMac ? '⌥' : 'Alt';
    if (p === 'meta' || p === 'cmd') return '⌘';
    if (p === '`') return '` (Backtick)';
    if (p === ',') return ',';
    if (p === '/') return '/';
    if (p === 'tab') return 'Tab';
    if (p === 'enter') return 'Enter';
    if (p === 'backspace') return 'Backspace';
    if (p === 'escape' || p === 'esc') return 'Esc';
    return p.toUpperCase();
  });
}

/** Parses a browser KeyboardEvent into a normalized key combo string (e.g. "ctrl+t") */
export function parseKeyboardEventToCombo(e: KeyboardEvent): string | null {
  // Ignore lone modifier keys
  if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) {
    return null;
  }

  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push('ctrl');
  if (e.altKey) parts.push('alt');
  if (e.shiftKey) parts.push('shift');

  const key = e.key.toLowerCase();
  if (key === ' ') {
    parts.push('space');
  } else if (key === 'escape') {
    parts.push('esc');
  } else {
    parts.push(key);
  }

  return parts.join('+');
}

/** Checks whether a KeyboardEvent matches a given combo */
export function matchesKeybinding(e: KeyboardEvent, combo: string): boolean {
  if (!combo) return false;
  const parts = combo.toLowerCase().split('+').map((p) => p.trim());

  const requiresCtrl = parts.includes('ctrl') || parts.includes('cmd') || parts.includes('meta');
  const requiresShift = parts.includes('shift');
  const requiresAlt = parts.includes('alt');

  const hasCtrl = e.ctrlKey || e.metaKey;
  const hasShift = e.shiftKey;
  const hasAlt = e.altKey;

  if (requiresCtrl !== hasCtrl) return false;
  if (requiresShift !== hasShift) return false;
  if (requiresAlt !== hasAlt) return false;

  // The base key is the non-modifier token in combo
  const baseKey = parts.find((p) => !['ctrl', 'cmd', 'meta', 'shift', 'alt'].includes(p));
  if (!baseKey) return false;

  const eventKey = e.key.toLowerCase();

  if (baseKey === 'tab') return eventKey === 'tab';
  if (baseKey === 'enter') return eventKey === 'enter';
  if (baseKey === 'esc' || baseKey === 'escape') return eventKey === 'escape';
  if (baseKey === '`') return eventKey === '`' || eventKey === '~';
  if (baseKey === '/') return eventKey === '/' || eventKey === '?';
  if (baseKey === ',') return eventKey === '<' || eventKey === ',';

  return eventKey === baseKey;
}
