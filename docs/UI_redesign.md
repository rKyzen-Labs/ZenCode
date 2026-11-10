# ZenCode UI/UX Redesign Specification & Decisions

This document summarizes the architectural decisions and specifications for the complete ZenCode UI/UX redesign (Steps 1 through 14, including layout corrections).

---

## Step 1 — Application Shell: Three-Pane Layout & Custom Titlebar
- **Custom Frameless Titlebar**: Native OS window decorations are disabled (`"decorations": false`). The top titlebar manages window dragging (`data-tauri-drag-region`) and hosts window controls (Minimize, Maximize/Restore, Close).
- **Navigation Controls (Left of Titlebar)**: `Home` icon (routes to Landing page), `Search` icon (routes to dedicated search page), and `Settings` icon (routes to Settings page).
- **Inline Session Tabs**: Chat/task sessions render as tabs directly inside the titlebar with title truncation, message icon, and close button (`X`). A `+` button opens a new session.
- **Quota Ring & Popover**: Compact circular SVG ring sits to the left of window control buttons in the titlebar, indicating active model quota consumption (RPM, TPM, RPD) with an expandable popover.
- **Center Chat Pane**: Occupies the main central viewport (`flex-1 min-w-0`), hosting the Landing Page, Chat Thread, Search Page, or Settings Page.
- **Right File Tree Pane**: The workspace file tree and opened file reader tabs are placed on the **right** side of the screen rather than the left.
- **Pane Visibility Scope**: The right file pane and its open/close toggle are rendered **only** during an active chat session (`activeView === 'chat'`). They are completely hidden on the landing page, search page, and settings page.
- **Horizontal Resizability**: The right file pane features a draggable left-edge handle (`cursor-col-resize`), allowing users to widen or narrow it. Width is persisted per workspace in `localStorage`.
- **Top-Edge Pane Toggle**: The pane collapse/expand toggle button is located at the top edge of the right pane (`PanelRightClose`). A slim trigger on the far right allows reopening when collapsed.
- **CodeMirror 6 Reader**: Opened files display in closeable tabs within the right pane using a read-only CodeMirror 6 editor with line numbers and dark theme.

---

## Step 2 — Landing Page
- **Centered Wordmark & Input Box**: The ZenCode wordmark and prompt input box are centered together both vertically and horizontally in the window.
- **Prompt Input Dock**: Centered prompt box matching Image 3 with:
  - Add attachment button (`Paperclip`) on the bottom left.
  - Mode selector dropdown (`Build v`, `Turbo v`).
  - Planner model selector dropdown (`Gemini / gemini-3.5-flash-lite`, `gemini-3.8-flash`).
  - Send button with upward arrow (`ArrowUp`).
- **Workspace Selector Row**: Positioned directly beneath the input dock matching Image 3 (`[Z] Workspace / [Monitor] Local • [GitBranch] main`, `A:\Workspace`). Clicking opens a dropdown with previously opened workspaces and a native OS folder picker (`pickFolder`) to browse the filesystem.
- **Recent Chats List**: Centered beneath the workspace row, listing recent chat sessions with square initial badges and relative timestamps.

---

## Step 3 — Dedicated Search Page
- **Layout Matching Reference Image 1**:
  - **Left / Center Sessions Area**: Top centered search bar (`Search sessions`) with search icon and `New session` action on the right. Below, matching sessions listed under headers like `Older` with square badges (`[Z]`), session titles, and workspace name suffixes.
  - **Right Projects Panel**: Displays `Projects` header with an add button (`FolderPlus`) to browse for new workspaces via native folder picker, a list of projects with colored badges (`[S]`, `[Z]`), and `Settings` + `Help` links at the bottom.

---

## Step 4 — Settings Page
- **Sidebar Navigation**: Dedicated sections for Workspaces, Worktrees, API Key, Rate Limiter, Worker Model, and Execution Mode.
- **Workspaces Section**: Lists all known workspaces with path inspection and an option to remove workspaces from the known list (without deleting files on disk).
- **Worktrees Section**: Discovers active or orphaned Git worktrees across repositories, surfacing M2 "Review" and "Clean" actions.
- **API Key Section**: Displays masked key status (`AIza...xxxx`) stored securely in Windows Credential Manager. Allows replacing or saving a new Gemini API key.
- **Rate Limiter / Quota Dashboard**: Real-time RPM, TPM, and RPD telemetry progress meters for Gemini models with reset timers and model pinning controls.
- **Worker Model Selection**: Queries installed Ollama models live via `ollama list`. Sets active local worker model, marking `granite4.1:8b` as the recommended and verified benchmark model.
- **Default vs Turbo Mode**: Toggles between Default Mode (step-by-step confirmation on all actions) and Turbo Mode (auto-approval for allowlisted safe commands).

---

## Step 5 — Live Agent Activity Transparency & Continuous Pipeline
- **Context Preview — Trust Once Per Workspace**: The Gemini context preview asks only the first time per workspace. Once confirmed, subsequent tasks in that same workspace proceed straight to planning automatically without showing the preview again. Trust decisions are persisted per workspace in SQLite and can be reset from Settings.
- **Mandatory Consent Gate Rendering**: On untrusted/first-time prompts, the Context Preview card renders directly in the thread displaying the target workspace, git branch, indexed file count, preview excerpt, and an explicit "Confirm & Plan" button with loading state.
- **Active Planning & Execution Feedback**: Submitting a prompt immediately renders the user prompt along with a live pulsating `#38bdf8` planning indicator ("Planning with Gemini..."), ensuring zero silent waiting states before the plan arrives and worker execution starts.
- **Continuous Automated Execution**: Planning immediately and automatically transitions into local worker execution with zero manual clicks required.
- **Execution Terminal**: Real-time stdout/stderr stream with manual stop/cancel action for running processes.

---

## Step 6 — Collapsible Live Agent Activity & Inline Diff Viewer
- **Collapsible State Line (Collapsed by Default)**: While a task runs or completes, the chat displays a single collapsible line indicating the current state (e.g. `Working... (24s) ›`, `Thinking... ›`, `Running cargo test ›`, or `Worked for 3m ˇ`).
- **Hierarchical Unfolded Activity**: Clicking the activity line unfolds the detailed step-by-step log matching reference:
  - `Thought for 16s ›`
  - `Ran search for rfd or dialog ›`
  - `Analyzed Cargo.toml #L1-36`
  - `Edited mod.rs +14 -0`
- **Automatic Task Changes Application & Undo**: Changes from completed tasks are applied automatically to the primary branch. Manual "Accept All Changes / Reject Task" buttons are removed.
- **Undo via Reverting the Prompt**: Each user prompt card features an "Undo changes" action that resets the primary working tree back to the pre-task `base_commit` for that specific task.
- **Inline Changed Files Summary**: After task completion, a compact unfolded-by-default summary line shows `N files changed +X -Y`. Clicking expands/collapses the changed files list inline in the chat.
- **Integrated Diff Viewer in File Pane**: Standalone "Changes" and "Explore" panels are removed. Clicking any changed file in the chat list opens that file directly in the right-side `FileTreePane`'s CodeMirror 6 viewer, highlighted with line additions/deletions.

---

## In-Chat Session Interface
- **Chat Distinction**: User prompts are aligned to the right with subtle tone (`bg-[#18181b]`) and include the "Undo changes" action. Agent responses are aligned to the left with flat, natural spacing.
- **Workspace Selector — Home Page Only**: The workspace selector row is rendered strictly on the landing page; removed from the in-chat view.
- **No Standalone Sub-Bar**: Removed the secondary workspace header beneath the titlebar to maximize vertical chat real estate.
- **Minimalist Flat System**: Zero extraneous bordered cards or boxes; distinction through alignment, spacing, and subtle tones.

---

## Step 7 — Default vs Turbo Execution Modes
- **Default Mode**: Full gating. Context preview, planning request, and all worker tool commands require explicit confirmation.
- **Turbo Mode**: Autonomous execution for allowlisted safe read and verification commands.
- **Safety Invariant**: Context Preview before external AI calls and any dangerous/blacklisted command (e.g., shell redirects, destructive commands) **always** require explicit user confirmation regardless of mode.

---

## Step 8 — Desktop Notifications
- **OS Native Notifications**: Dispatches OS desktop notification when the local worker completes an isolated task session or when an execution requires user intervention.

---

## Step 9 — Visual Design System
- **Strict Monochrome Base**: Deep black `#0a0a0a`, `#0c0c0c`, `#121212`, and neutral `#262626` borders.
- **Single Accent Color**: Light blue `#38bdf8` reserved strictly for active states, focus outlines, send button, and the quota indicator.
- **Typography**: JetBrains Mono monospace font across all UI elements, labels, code, and chat threads.
- **Zero Placeholder Brackets**: Complete removal of bracket shorthand text (e.g. `[x]`, `[error]`, `[button]`) in favor of styled Lucide icons and proper UI components.

---

## Step 10 — Offer-Based Workspace Safety Cards
- **Git Repository Required**: Inline card offering explicit Git initialization for non-git folders; never run automatically.
- **Standard .gitignore Offer**: Detects missing `.gitignore` and offers project-tailored ignore rules (Cargo, Node, Python).
- **Initial Commit on Empty Repos**: Inline prompt offering an initial empty commit for 0-commit repositories to enable isolated worktree creation.

---

## Step 11 — Circular Quota Indicator & Popover
- **Compact SVG Ring**: Positioned in the custom titlebar, rendering live percentage stroke based on the active model's highest consumed limit (RPM or TPM).
- **Interactive Popover**: Detailed breakdown of RPM, TPM, RPD limits, consumed counts, remaining units, and cascade failover status.

---

## Step 12 — Right Pane Drag Resizing & State Persistence
- **Draggable Left Divider**: 4px hover-active handle (`cursor-col-resize`) allowing dynamic resizing between 200px and 60% of viewport width.
- **Workspace-Scoped Persistence**: Both width and open/collapsed state persist across sessions in `localStorage`.

---

## Step 13 — Minimalist High-Contrast Simplification
- **1px Clean Dividers**: Elimination of heavy drop shadows and rounded card nesting in favor of sharp, high-contrast borders (`#262626`, `#1c1c1c`).
- **Maximum Screen Real Estate**: Maximized viewport allocation for chat reasoning, code viewing, and diff analysis.

---

## Step 14 — Custom App Icon Set
- **High-Contrast Identity**: Multi-resolution app icon generated from the custom monochrome `Z` wordmark asset across all standard Tauri desktop icon sizes (`32x32`, `128x128`, `icon.ico`, `icon.png`).
