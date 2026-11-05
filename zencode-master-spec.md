# Zencode — Master Product & Engineering Specification

**Document status:** Free-first hybrid architecture locked for V1; implementation not yet started  
**Document version:** 2.0  
**Prepared:** 4 November 2026  
**Product name:** Zencode (working name; perform name, trademark, and domain checks before public launch)  
**Initial target:** Windows 11 desktop  
**Planned distribution:** Standalone desktop application, not a browser app and not a VS Code extension

---

## 0. Executive decision

Zencode is a standalone, free-first, local-first AI development workspace. Its defining workflow is deliberately hybrid:

1. **Gemini plans.** A free-tier-eligible Gemini Flash-Lite model receives one deliberately compact planning request for a new task and returns a structured implementation plan.
2. **A local model builds.** An Ollama-hosted coding model, driven through the OpenCode agent engine, performs project exploration, code edits, tool calls, build/test iterations, and debugging locally.
3. **Rust enforces the boundary.** The Rust core owns routing, request limits, privacy filtering, credentials, permissions, task isolation, and verification. Compiler errors and repetitive coding iterations are never automatically sent to Gemini.
4. **The user reviews.** Zencode shows the complete diff and verification results before changes can enter the user's main project.

The central product promise is **useful AI-assisted coding without a paid AI subscription or per-edit API charges in the normal workflow**. The intended configuration uses the Google Gemini API free tier for planning and local Ollama inference for implementation. This can keep API spend at zero while the selected Gemini model remains available on the free tier and the user's project stays within the account's active quotas. It is not a promise of unlimited requests, zero electricity/hardware cost, or permanent free access from Google.

### Locked V1 model architecture

- **Planner:** Gemini 3.5 Flash-Lite (`gemini-3.5-flash-lite`) through Google's Gemini Interactions API, called directly by a Rust `GeminiPlannerClient`. Google currently lists this model's Standard free tier with free input and output tokens. Model availability, eligibility, and quotas can change; Zencode must verify availability and the user's active limits rather than hard-code a daily request count.
- **Coding worker:** Ollama local inference, with Qwen2.5-Coder 7B as the initial benchmark candidate, not an unconditional quality guarantee. The exact default is selected only after it passes tool-calling, edit, and multi-step task tests on the target machine.
- **Agent engine:** OpenCode sidecar, configured for the local Ollama worker only in the default free-first profile. Do not configure Gemini or any other hosted provider as a worker fallback.
- **Routing:** Rust calls Gemini only for an explicit planning operation. OpenCode performs the worker loop locally. No hidden cloud routing, no automatic paid-model fallback, and no provider switch after a quota error.

The Gemini planner is intentionally **outside the OpenCode worker loop**. This is a deliberate boundary: if OpenCode or the local model encounters a compiler error, the local worker receives it. The Rust policy layer does not forward it to Gemini. A separate, user-triggered “Re-plan with Gemini” action may be considered later, with an explicit remote-context preview and a fresh quota/cost disclosure.

### The honest project verdict

Zencode is a **good engineering and portfolio project**. It combines desktop engineering, Rust process supervision, direct API integration, local inference, model-routing policy, Git automation, review UX, and software safety in a product-shaped problem.

It has a clearer product identity than a generic AI IDE: **free-first hybrid coding, with a cloud planner and a local implementation worker**. However, this is not a moat by itself. Other tools can adopt similar routing. The product earns a reason to exist only if the free-first workflow is easy to set up, works reliably on ordinary hardware, keeps paid traffic impossible by default, protects user changes, and makes every remote request understandable.

**Decision: build it. Prove the Gemini-planner-to-local-worker loop first, then the task-isolation and review workflow. Do not begin with the visual builder or a broad provider marketplace.**

# 1. Product definition

## 1.1 What Zencode is

Zencode is a free-first AI-assisted development workspace for solo developers, students learning software development, and indie builders who want capable planning from a hosted model while keeping repetitive coding and debugging on their own machine.

It is a desktop application that can:

- open and inspect a local software project;
- communicate with a coding agent through a locally supervised engine process;
- connect the Gemini planning API and a local Ollama coding worker;
- prepare an isolated workspace for an AI task;
- stream the agent's reasoning summaries, tool activity, and response into the UI;
- show a complete review of proposed file changes;
- run approved build, test, lint, and verification commands;
- let the user accept, reject, or revisit a task's changes; and
- later support visual-reference-to-code workflows through a design canvas.

Zencode is not intended to be a general chatbot with a file picker. Its central object is a **development task** with a project, agent session, isolated workspace, change set, verification result, and user decision.

## 1.2 Product promise

**Use a free-tier cloud model for planning and a local model for the actual coding loop, with every change reviewable.**

The promise is not unlimited free AI or guaranteed correctness. It is a carefully controlled default workflow: Gemini is used sparingly for planning; local inference handles the repetitive, token-heavy coding work; Rust prevents accidental cloud routing; and users inspect changes before integration.

## 1.3 Who it is for

Primary initial audience:

- solo developers and indie builders;
- developers who want to avoid recurring AI subscriptions and per-edit API charges;
- users who want a hosted planning model but local code execution and debugging;
- users who prefer a standalone desktop workspace over an editor extension; and
- people who want an explicit review step before agent changes are applied.

Not the initial target:

- enterprise teams needing organization-wide identity, audit, policy, and compliance controls;
- users expecting a full replacement for every feature in Visual Studio, IntelliJ, or VS Code;
- teams requiring simultaneous collaborative editing;
- people expecting Zencode to provide its own hosted model subscription or cloud compute service.

## 1.4 Main user journey

1. The user launches Zencode and opens a local project.
2. Zencode checks project state, Git availability, Ollama availability, and OpenCode engine availability.
3. The user describes a task. Zencode builds a compact project-context package and shows exactly what will be sent to Gemini.
4. After the user confirms remote planning, Rust sends one structured planning request to the configured free-tier Gemini model.
5. Rust validates the returned JSON plan and presents it to the user.
6. Zencode creates an isolated Git worktree for the implementation task.
7. The OpenCode worker runs inside that worktree with Ollama as its only configured model provider.
8. Project exploration, file edits, shell/tool activity, compiler errors, build/test iterations, and repair loops remain local.
9. Zencode collects a complete file-change inventory and verification results.
10. The user accepts, rejects, or keeps the task for further work. Integration is never automatic.

## 1.5 Product principles

- **Free-first:** a Gemini free-tier planner plus a local worker is the default architecture; no paid API provider is required for the normal workflow.
- **Local implementation:** all code editing, tool execution, compiler feedback, and repair loops run through the local Ollama worker.
- **No paid fallback:** no automatic switch to a paid model, a paid API tier, Groq, or another provider. A quota failure pauses planning; the user may continue in local-only mode.
- **Remote planning is explicit:** the user sees and approves the context sent to Google before each planning request.
- **No Zencode account in V1:** no mandatory sign-in, hosted backend, sync account, or remote project storage.
- **Provider transparency:** the user can see exactly which provider/model receives a request and why.
- **Review before integration:** agent changes are not silently merged into the user's main project.
- **No false security claims:** a Git worktree is a source-control isolation mechanism, not an operating-system sandbox.
- **Deterministic verification first:** a real build or test result is more useful than an AI model claiming that code is correct.
- **Small, testable releases:** reliability before feature count.

---

# 2. V1 scope and non-goals

## 2.1 V1 must include

- Windows 11 desktop build.
- Tauri 2 desktop shell.
- React + TypeScript frontend.
- Rust application backend.
- Supervised OpenCode sidecar using the pinned v1 HTTP/SSE API.
- Project open/reopen and local project metadata.
- One active project at a time for the first usable release.
- Agent chat/task view with streaming updates.
- Gemini Planner setup and one explicit planning request per new task.
- OpenCode worker configured for Ollama local inference only.
- A strict routing policy that blocks hosted providers from the coding-worker path.
- Local-only continuation when Gemini is unavailable or its free quota is exhausted.
- A visible planner request counter and token/usage details where supplied by the API.
- Git availability and working-tree preflight.
- One isolated Git worktree per write task.
- File-change inventory that includes tracked and untracked files.
- Diff review for additions, modifications, deletions, and renames; clear metadata for binary files.
- Explicit accept/reject workflow.
- Approved build/test command execution with visible output.
- Stop/cancel action and clean process shutdown.
- Local error logs that do not expose secrets.
- Clear Gemini Planner, Ollama Worker, engine, Git, and task status indicators.

## 2.2 V1 may include if the core remains stable

- Basic file explorer.
- Basic code viewing and editing.
- Recent projects list.
- Persistent task history stored locally.
- A simple settings page for model and privacy choices.

## 2.3 Explicitly out of scope for V1

Do not start these before the core workflow is reliable:

- full VS Code feature parity;
- plugin marketplace;
- team accounts, collaboration, cloud sync, or shared workspaces;
- hosted Zencode backend;
- Groq or other hosted coding-worker integrations in V1;
- mobile apps;
- automatic commits without user confirmation;
- any automatic provider fallback, including a move from the free Gemini tier to a paid tier;
- always-on autonomous agents;
- multi-agent swarms or unlimited parallel tasks;
- arbitrary MCP server marketplace;
- background indexing of every file on the machine;
- automatic dependency installation;
- remote code execution infrastructure;
- built-in deployment or hosting platform;
- design canvas and screenshot-to-code as a launch blocker.

These are not bad ideas individually. They are bad first milestones because they multiply the number of failure modes before Zencode has proven its core value.

---

# 3. Locked technology decisions

| Area | Decision | Reason / constraint |
|---|---|---|
| Desktop shell | Tauri 2 | Native desktop packaging with a web UI and Rust backend. |
| Frontend | React + TypeScript + Vite | Familiar UI stack, fast iteration, strong component ecosystem. |
| Backend | Rust | Owns process lifecycle, filesystem operations, Git orchestration, credentials, and policy enforcement. |
| Async runtime | Tokio | Process, HTTP, streaming, and task orchestration. |
| Coding engine | OpenCode sidecar | Reuse a real coding-agent engine for the local worker instead of reinventing tool execution. |
| Planner client | Rust `GeminiPlannerClient` using Gemini Interactions API | Planner traffic stays separate from OpenCode worker sessions and can be explicitly gated and counted. |
| Planner model | Gemini 3.5 Flash-Lite (`gemini-3.5-flash-lite`) | Currently listed with free Standard input/output pricing; verify active project eligibility and quotas. |
| Local worker provider | Ollama only | Ensures the repetitive coding/tool loop has no cloud-model route in the default profile. |
| Initial worker candidate | Qwen2.5-Coder 7B | Benchmark for tool calling and coding quality before locking the installed default. |
| Engine integration | OpenCode v1 HTTP API + SSE | Explicit headless server contract for the first adapter. Pin a release and verify `/doc` against it. |
| UI/backend bridge | Tauri commands and events | React does not directly control local processes or hold provider credentials. |
| HTTP client | Rust `reqwest` | Rust-side communication with Gemini Interactions API, the local Ollama service when needed, and OpenCode server. |
| Serialization | `serde` | Typed request/response models. |
| Local database | SQLite | Project records, task metadata, preferences, and history. Not a replacement for Git. |
| Database access | `rusqlite` behind a controlled DB service, or an equivalent single-writer layer | Keep writes serialized and predictable. Avoid letting UI components write directly to the DB. |
| Git integration | Git CLI invoked with argument arrays | Supports normal Git installations and worktree commands. Never build shell command strings from user or model text. |
| File watching | Rust `notify` crate | Detect external changes and refresh project state. |
| Code editor surface | CodeMirror 6 initially | Modular, lighter embedded editor surface. Do not attempt to build an entire IDE editor engine. |
| Styling | CSS variables + a restrained component system; Tailwind may be used for layout utilities | Consistent, themeable UI without coupling the entire design to utility classes. |
| Local models | Ollama required for default coding workflow | Detect the local service; guide user setup; never bundle multi-GB model weights. |
| Initial OS | Windows 11 | One platform first. |
| Later packaging | Linux AppImage/deb, then macOS DMG | Only after Windows release flow is repeatable. |

### Important OpenCode version rule

OpenCode's current documentation distinguishes **OpenCode 1** and **OpenCode 2**. Both use the `opencode` command name and are not designed to be installed side-by-side under that default command. OpenCode v1 documents a headless HTTP server (`opencode serve`) and OpenAPI/SSE interface. OpenCode v2 documents an ACP subprocess interface (`opencode acp`) using JSON-RPC over stdin/stdout.

Zencode V1 will target **one pinned OpenCode v1 release** because the selected architecture uses the documented HTTP/SSE server. The engine adapter must check the version and expected API contract. Do not silently accept OpenCode v2 as a compatible replacement. A future v2 adapter can use ACP, but that is a deliberate engineering change with its own tests.

---

# 4. High-level architecture

## 4.1 Component map

```text
┌───────────────────────────────────────────────────────────────┐
│                         ZENCODE DESKTOP                        │
│                                                               │
│  ┌──────────────────────── React + TypeScript ──────────────┐  │
│  │  Workspace UI   Chat / Tasks   Diff Review   Settings    │  │
│  │  File Explorer  Editor        Build Output   Status      │  │
│  └───────────────────────────┬──────────────────────────────┘  │
│                              │ Tauri IPC                       │
│  ┌───────────────────────────▼──────────────────────────────┐  │
│  │                       RUST CORE                           │  │
│  │                                                          │  │
│  │  App State / Command Router                              │  │
│  │       │                                                  │  │
│  │       ├── Project & Workspace Manager                    │  │
│  │       ├── Git / Worktree Manager                          │  │
│  │       ├── OpenCode Engine Adapter                         │  │
│  │       ├── Process Supervisor                              │  │
│  │       ├── Gemini Planner Client                            │  │
│       ├── Local Worker Policy Router                       │  │
│  │       ├── Permission & Approval Manager                   │  │
│  │       ├── Change Inventory & Diff Service                 │  │
│  │       ├── Build / Test Runner                             │  │
│  │       ├── Credential Service                              │  │
│  │       ├── Local Database                                  │  │
│  │       └── Logging / Diagnostics                           │  │
│  └─────────────┬───────────────────┬────────────────────────┘  │
│                │                   │                           │
│          Local HTTP + SSE             Git / tools              │
│                  │                       │                      │
│  ┌───────────────▼──────────┐  ┌────────▼───────────────────┐   │
│  │ OpenCode v1 Sidecar      │  │ Local Project + Git        │   │
│  │ Local worker profile    │  │ Worktrees                  │   │
│  │ Ollama provider ONLY    │  │ User tree remains separate │   │
│  └───────────────┬──────────┘  └────────────────────────────┘   │
│                  │                                               │
│          ┌───────▼────────┐                                      │
│          │ Ollama local   │                                      │
│          │ Qwen coder     │                                      │
│          └────────────────┘                                      │
└─────────────────────────────────────────────────────────────────┘

          Rust makes an explicit planning request only
                              │
                  ┌───────────▼───────────┐
                  │ Google Gemini API     │
                  │ Gemini 3.5 Flash-Lite │
                  │ Planner only          │
                  └───────────────────────┘
```

The UI talks to Rust through Tauri IPC. Rust is the authority for process control, filesystem changes, Git operations, credentials, Gemini planning requests, model-routing policy, and permission decisions. React must not launch processes, hold provider credentials, or make unrestricted requests to Gemini, Ollama, or the local OpenCode server.

## 4.2 Responsibility boundaries

### React frontend

Owns:

- rendering the desktop experience;
- local UI state such as selected tab, open panel, and current selection;
- displaying messages and streamed task events;
- displaying file inventory and diffs;
- gathering user approval and settings input;
- sending typed commands to Rust;
- receiving typed events from Rust.

Does not own:

- API keys or provider credentials;
- direct process execution;
- unrestricted filesystem access;
- Git commands;
- OpenCode process lifecycle;
- approval policy enforcement;
- final change application.

### Rust core

Owns:

- application lifecycle;
- project discovery and validation;
- local database access;
- engine process supervision;
- local server authentication;
- all requests to OpenCode;
- model selection policy and provider fallback rules;
- Gemini Planner client, context minimisation, explicit remote-request confirmation, and planner-call accounting;
- enforcement that the OpenCode worker uses only the local Ollama provider in the default profile;
- Git preflight and worktree lifecycle;
- change inventory and review data;
- approved build/test execution;
- credential retrieval and injection;
- safe logging, cancellation, and cleanup.

### OpenCode sidecar

Owns:

- coding-agent session execution;
- its built-in code-reading/search/edit tools;
- local model invocation through the configured Ollama provider;
- agent session and event state;
- tool permission requests supported by the selected release.

OpenCode does **not** own Zencode's Gemini planning calls, user experience, project-level approval workflow, target-branch policy, provider policy, or the final decision to integrate changes.

---

# 5. OpenCode engine integration

## 5.1 Why OpenCode

Building a capable coding agent from zero would require implementing tool calling, codebase exploration, patch editing, provider integrations, session handling, permissions, retries, and many edge cases. That is not the product's differentiator.

OpenCode already provides a coding-agent engine and documents a headless server interface. Zencode should integrate with that interface and concentrate its original engineering effort on orchestration, isolation, review, verification, and UX.

**Do not fork OpenCode for V1.** Forking creates a permanent maintenance burden and makes upstream improvements harder to adopt. Only consider a fork after a reproducible limitation in the documented API blocks a required Zencode feature.

## 5.2 Sidecar startup lifecycle

Rust should:

1. Locate the pinned development binary or bundled sidecar.
2. Verify the binary version before using it.
3. Create a random per-process server password.
4. Select a free loopback port, with retry handling for port collisions.
5. Spawn the process with `opencode serve`, explicit loopback hostname, and the chosen port.
6. Set the documented server password environment variable for that child process.
7. Set the working directory to the task's isolated worktree, not the user's main project directory.
8. Capture stdout and stderr continuously with bounded log buffers.
9. Poll the documented health endpoint until ready, process exit, or timeout.
10. Verify the server version and expected API contract.
11. Connect to the documented event stream and create/load a session.
12. On app shutdown or task termination, request a graceful stop and then terminate the process tree if it does not exit.

The documented OpenCode v1 interface includes:

- `GET /global/health` for readiness and version;
- `GET /doc` for the OpenAPI specification;
- session endpoints such as `POST /session` and `GET /session`;
- message endpoints such as `POST /session/:id/message` or `POST /session/:id/prompt_async`;
- session diff and abort endpoints;
- a server-sent events endpoint for event updates; and
- provider/authentication endpoints.

The exact event endpoint, request schema, and event payloads must be read from the pinned release's OpenAPI document and covered by tests. Do not hand-wave API response shapes or rely on undocumented event names.

## 5.3 Authentication and network boundary

- Bind the OpenCode server to `127.0.0.1` only.
- Enable the documented HTTP Basic Authentication password for the server process.
- Generate a fresh unpredictable password for the process lifetime.
- Keep the password in Rust memory only where possible; never print it to logs.
- Do not enable mDNS.
- Do not expose the server on `0.0.0.0`.
- Do not add CORS origins unless a tested architecture genuinely requires them. The normal UI-to-Rust-to-server path does not require the browser frontend to access the API directly.
- Do not expose the port or password in general UI logs.

Loopback binding and Basic Auth reduce accidental exposure. They do not defend against malware or another process already running with the same user's privileges. Do not market this as a complete security boundary.

## 5.4 Process supervision

Use Tokio process management and argument arrays. Never concatenate user input or AI-generated text into a shell command.

On Windows, supervise the process tree with a Windows Job Object or equivalent mechanism so child processes do not remain orphaned after cancellation or app exit. Capture stdout and stderr concurrently to prevent pipe deadlocks. Apply output limits and preserve a useful tail for diagnostics.

Process states should be explicit:

- `NotStarted`
- `Starting`
- `Ready`
- `Busy`
- `Stopping`
- `Stopped`
- `Failed`

A failed health check, unexpected process exit, invalid API version, or authentication failure must be shown as an engine error. Do not keep sending requests to a dead or unverified process.

## 5.5 Sidecar packaging

For development, the first prototype may use an already installed, version-pinned OpenCode v1 binary. Production packaging should bundle a known binary rather than depend on a user's global `PATH`.

Tauri supports external sidecar binaries through `bundle.externalBin`, with platform target-triple suffixes. The packaging pipeline must:

- pin the OpenCode release;
- verify the binary checksum or signature where available;
- record the source and exact version;
- include required third-party license notices;
- build and test the correct binary for each target architecture;
- avoid silently downloading or updating an engine at runtime; and
- provide an explicit engine update process.

Before distribution, audit the exact OpenCode release license, bundled dependencies, trademarks, and redistribution conditions. Do not assume the license from a previous release applies forever.

---

# 6. Project and Git workspace system

## 6.1 Why worktrees are required

The agent must not casually make changes in the user's active working directory. Each write task should receive its own Git worktree and task branch.

A Git worktree creates a separate working directory associated with the same repository. It lets an agent modify files without directly editing the user's primary working tree.

However:

> **A Git worktree is not a security sandbox.**

A process launched by the agent can still have the privileges of the logged-in user and may access paths outside the worktree. Worktrees protect normal project edits from directly colliding with the primary checkout; they do not confine arbitrary commands or malicious code.

## 6.2 Write-task preflight

Before a write task starts, Zencode must check:

- Git is installed and callable;
- the selected folder is inside a valid repository;
- the target branch and HEAD are known;
- the primary working tree's tracked and untracked state is understood;
- submodules and nested repositories are detected or reported;
- the worktree destination does not already exist;
- required disk space is available where practical; and
- the project is not already being modified by another Zencode task in a conflicting way.

### Dirty working tree policy

V1 must not silently stash, reset, overwrite, or commit a user's existing work.

If the primary tree has changes, Zencode should explain that the current task cannot safely create its standard isolated baseline. The user can:

- commit their existing work themselves;
- explicitly create a named local checkpoint through a clearly explained action; or
- use read-only analysis mode until the project is in a safe state.

Do not use `git reset --hard` as a normal cleanup or rollback strategy.

### Non-Git projects

Read-only project exploration can be supported without Git. For agent write tasks, V1 should ask the user to initialize or connect a Git repository. Zencode must not silently initialize Git or create commits in an existing project.

## 6.3 Task worktree lifecycle

For a write task:

1. Capture the target branch and baseline commit.
2. Create a uniquely named task branch and linked worktree.
3. Start OpenCode with the worktree as its project directory.
4. Keep the primary working tree untouched while the agent works.
5. When the task ends, collect a full status inventory.
6. Compare the worktree with the recorded baseline.
7. Present the complete change set for review.
8. If accepted, integrate the task branch only after checking that the target branch and primary working tree have not changed unexpectedly.
9. If conflicts occur, stop and present a conflict workflow. Never overwrite the user's version automatically.
10. Remove the task worktree only after the user accepts/rejects and after confirming no unsaved task data will be lost.

## 6.4 Complete change inventory

The review system must not rely only on a normal tracked-file diff.

It must account for:

- modified tracked files;
- added tracked files;
- untracked new files;
- deleted files;
- renamed files;
- copied files where Git detects them;
- binary files;
- executable-bit or file-mode changes;
- symlinks where supported;
- submodule changes; and
- generated artifacts.

The UI must clearly indicate files that cannot be shown as ordinary text diffs. The user still needs to know that they changed.

The review inventory should be derived from Git status and diff operations, not only from OpenCode's session diff endpoint. OpenCode's session diff is useful supporting information, not the sole source of truth for workspace state.

## 6.5 Accept and reject semantics

**Accept** means the user has reviewed the change set and requests integration into the selected target branch. It does not mean the code is guaranteed correct.

Before integration:

- verify the target branch is still the expected branch;
- verify the primary tree is still clean;
- verify the baseline has not changed unexpectedly;
- show the final file inventory again;
- explain any merge/cherry-pick conflict;
- require a separate confirmation for irreversible or broad actions.

**Reject** means the task branch will not be integrated. Keep the task available until the user confirms deletion or cleanup. Do not immediately delete the only copy of an agent's work after a single accidental click.

Git is not a magic transaction system. Avoid destructive cleanup commands and preserve recoverable task state whenever possible.

---

# 7. AI orchestration and model strategy

## 7.1 Core design: Gemini plans, local AI codes

The AI architecture is not a generic multi-provider router. It is a deliberately separated two-model workflow:

- **Gemini Planner:** a small number of explicit cloud requests used to understand the task and produce a structured implementation plan.
- **Local Coding Worker:** an Ollama model invoked by OpenCode. It performs the ongoing project work locally, including exploration, code edits, tool use, builds, tests, compiler-error analysis, and repair attempts.
- **Rust Policy Router:** the only component allowed to call Gemini or start the worker. It enforces the boundary in code, not merely through system prompts.

### Why the Gemini call is outside OpenCode

Rust should call the Gemini API directly for planning. OpenCode should be the local coding-agent engine only. Do not configure the Gemini model as an OpenCode worker or fallback model in the default profile.

This is stricter than asking an AI agent to “use local models for coding.” If the local worker encounters an error, its next tool/LLM iteration is still the local Ollama model. There is no path in the normal worker loop that forwards a compiler log to Gemini.

The default OpenCode configuration must expose only the local Ollama provider for worker sessions. The exact allowlist/config syntax must be verified against the pinned OpenCode release's schema and covered by an integration test. If this restriction cannot be reliably enforced for that engine version, do not ship the worker integration until it can.

## 7.2 Gemini Planner

### Initial model candidate

Use Gemini 3.5 Flash-Lite (`gemini-3.5-flash-lite`) as the first planner candidate. Google's current official pricing page lists free Standard input and output for this model. Free-tier eligibility and model availability can change; the app must confirm that the model is available to the user's API project and must never assume a fixed 500 requests/day quota.

Google's Interactions API is currently recommended for new Gemini integrations. Use it through a Rust client, with structured output enabled for the plan schema and `store=false` where supported by the chosen request mode. If Google changes the recommended API or the selected model's support, isolate the integration behind `GeminiPlannerClient` so it can be updated without changing the worker architecture.

### Planner responsibilities

The Planner may:

- interpret the user's requested change;
- receive a compact repository overview and explicitly approved source snippets;
- identify dependencies, risks, likely files, and implementation order;
- produce a machine-readable plan and acceptance criteria; and
- ask for missing information rather than invent project details.

The Planner must not:

- edit files;
- run shell commands;
- call the OpenCode worker directly;
- receive compiler errors or full build logs automatically;
- receive secrets, `.env` values, private keys, tokens, or credential files; or
- trigger additional Gemini calls without a user-visible action.

### Planning request frequency

- Default: one Gemini request when the user starts a new task and selects **Create plan**.
- No Gemini request for every coding iteration, file edit, shell action, build, test, or compiler error.
- No automatic re-planning loop in V1.
- A future **Re-plan with Gemini** action must show the exact context to be sent, explain that another remote request will be made, and require confirmation.
- If Gemini is rate-limited or unavailable, Zencode pauses cloud planning and lets the user continue in local-only mode. It never switches to a paid provider.

“One request per task” is a design target for the initial plan, not a universal guarantee for every feature or future workflow. The app must display actual Zencode-made request counts and usage metadata when available.

## 7.3 Planner context assembly and JSON contract

Rust assembles the planning context locally. The default payload should be compact and minimise remote source disclosure:

1. User's task description.
2. Project type and detected framework/language.
3. A shallow project tree with ignored/generated directories removed.
4. Relevant manifest/lockfile names and dependency summaries, not secret values.
5. Current Git branch and status summary.
6. Optional source excerpts selected by a local relevance pass. These are not uploaded by default; require the user to preview and approve them.
7. Explicit constraints such as target platform, existing architecture, and acceptance goals.

Never upload `.env`, credential stores, private keys, access tokens, personal documents, or other configured secret paths. Apply both built-in exclusions and `.gitignore`/Zencode ignore rules. Redact likely secrets before the user-facing preview. Do not promise perfect secret detection.

The Planner should return a strict JSON object. Use the Interactions API structured-output feature with the JSON schema supported by the selected model. Set `store=false` for stateless requests; if the selected API path cannot enforce the required stateless setting, do not silently fall back to server-stored interactions. A first schema should contain:

```json
{
  "task_summary": "string",
  "assumptions": ["string"],
  "questions": ["string"],
  "project_findings": ["string"],
  "steps": [
    {
      "id": "step-1",
      "goal": "string",
      "likely_files": ["relative/path"],
      "instructions": ["string"],
      "depends_on": [],
      "acceptance_checks": ["string"]
    }
  ],
  "risks": ["string"],
  "verification_suggestions": ["string"]
}
```

Rust must validate the response against a local schema, reject unknown or malformed structure safely, and show the user the plan before dispatching the worker. Treat all model output as untrusted data. The plan is guidance, not permission to execute commands or access files.

## 7.4 Local Coding Worker

### Initial model candidate

Use Qwen2.5-Coder 7B through Ollama as the first benchmark candidate because it is a plausible fit for a 32 GB consumer PC. Do not assume it is the final winner. It must pass real tests for:

- tool/function calling through the exact OpenCode version;
- reading/searching relevant files;
- editing more than one file coherently;
- following a multi-step plan;
- interpreting compiler and test output;
- repairing its own changes without looping; and
- respecting the task worktree and permissions.

If it fails, benchmark another quantized local model appropriate to the hardware. Do not quietly fall back to a cloud model. Do not download model weights without explicit user action and a clear storage/network notice.

### Worker responsibilities

The local worker performs:

- project exploration;
- source-file retrieval;
- code generation and edits;
- tool calls supported by OpenCode;
- approved build and test execution;
- compiler-error and test-failure analysis;
- local repair iterations; and
- a final local summary of changes.

All of these remain local to the user's machine at the model-inference layer. Project tools such as package managers may access the network when explicitly approved; local inference does not mean every build tool is automatically offline.

## 7.5 Strict routing matrix

| Operation | Route | Can call Gemini? |
|---|---|---:|
| Create initial implementation plan | Gemini Planner | Yes, one explicit request |
| Inspect project tree | Rust/local filesystem | No |
| Search/retrieve source files | Local tools | No |
| Write or edit code | Ollama Worker through OpenCode | No |
| Run shell/build/test/lint | Local approved process | No |
| Read compiler errors | Local worker | No |
| Fix compiler/test failures | Ollama Worker through OpenCode | No |
| Summarise local task result | Local worker or Rust | No |
| Re-plan after major scope change | User-triggered Gemini action only | Yes, after preview/confirmation |
| Screenshot/design analysis | Post-V1 feature; explicit Gemini request and upload consent | Only with consent |

The router must encode these routes as typed operations/enums. The Gemini key belongs only to the Rust planner client; it is not included in the OpenCode child environment. The OpenCode worker must use a dedicated configuration with Ollama as the only enabled provider, validated against the exact pinned release. Do not let a model choose its own provider. Do not route by parsing natural-language statements such as “please use local mode.” Do not rely only on system prompts.

## 7.6 No-paid-model routing and zero-spend safeguards

The default Zencode profile must prevent paid-model and alternative paid-provider routing through its own logic:

- Gemini is the only remote model endpoint in V1 and is used only for the planner role.
- The planner model ID is allowlisted; paid-only models are rejected.
- OpenCode worker configuration is restricted to the local Ollama provider.
- Groq, OpenAI, Anthropic, and other hosted coding providers are not configured in V1.
- No automatic provider fallback exists.
- No paid tier, paid model, or alternative service is selected after a free quota error.
- If planning is unavailable, the user can continue local-only or stop.
- Any future paid integration must be a separate opt-in feature, disabled by default, with clear cost and data-destination disclosure.

Important limitation: a model being listed with a free tier does not let Zencode prove which billing tier a user-owned Google API project is actually using. Zencode cannot inspect or control the project's billing state. For a strict no-spend setup, the user must keep the API project on the free tier and avoid enabling paid billing/capacity. Zencode can guarantee that its own router will not intentionally select paid-only models or other hosted providers; it cannot guarantee Google account-level billing behaviour. The UI must say this plainly during API-key setup.

## 7.7 Free-tier quota and usage handling

Google rate limits are model- and project-specific and are measured across dimensions such as requests per minute, input tokens per minute, and requests per day. They can change. Do not hard-code “500 requests per day” or display a fabricated remaining-quota number.

Zencode should:

- count the planner requests it sends;
- display returned token/usage metadata if supplied;
- display quota/rate-limit errors clearly;
- respect `Retry-After` or provider retry metadata when available;
- avoid automatic repeated retries;
- provide a link/instruction to check active quotas in Google AI Studio; and
- continue with local-only mode when the free planner is unavailable.

A Zencode-local request counter is not the same thing as Google's account-wide remaining quota. State that distinction in the UI.

## 7.8 Local model and hardware behaviour

- Detect an existing Ollama service at its normal local endpoint; allow explicit custom host configuration only with a warning if it is not loopback.
- Query installed models; do not assume Qwen is installed.
- Show model name, quantisation if discoverable, context window if known, and local readiness.
- Provide an optional first-run tool-call smoke test before enabling the worker.
- Do not bundle model weights.
- Warn that CPU inference may be slow and consume RAM/CPU.
- Keep context sizes realistic for the target PC and allow users to lower them.

## 7.9 Retry, stop, and recovery policy

- Provide a real stop/cancel button.
- Stop dispatching new worker actions after cancellation.
- Abort the active OpenCode session where supported.
- Terminate the process tree only when required.
- Limit local automatic repair retries to a small fixed cap, such as two or three attempts.
- Never send a failed local repair transcript to Gemini automatically.
- On repeated failure, preserve the worktree and diff, then summarize the problem locally.
- A user-triggered re-plan is a new explicit Gemini request, not an automatic error handler.

## 7.10 Why Groq and multi-provider routing are not in V1

Groq and other hosted APIs add provider credentials, rate-limit handling, changing data destinations, and a path to accidental paid usage. They are not required to prove the central idea. Keep the provider abstraction modular, but do not implement more hosted providers until the Gemini-planner/local-worker loop is reliable and users have demonstrated a need.

# 8. Permissions, security, and privacy

## 8.1 Threat model

Zencode is a local desktop app that may launch an AI agent and project tools with the user's operating-system permissions. Risks include:

- accidental edits to the wrong project;
- destructive shell commands;
- dependency install scripts with side effects;
- malicious instructions embedded in a repository or document;
- accidental exposure of secrets to an AI provider;
- a compromised or misconfigured local server;
- incomplete cleanup leaving processes running; and
- incorrect change inventory causing the user to approve an incomplete patch.

V1 should reduce common accidental risks. It must not claim to be a hardened hostile-code sandbox.

## 8.2 Permission defaults

Recommended defaults:

| Capability | Default |
|---|---|
| Read project files | Allow within selected project/worktree |
| Search project | Allow within selected project/worktree |
| Edit files | Only inside the task worktree; permission configuration must be tested against the pinned engine release |
| Run shell commands | Ask the user |
| Install dependencies | Ask the user, show exact command and working directory |
| Network access from tools | Ask or deny by default |
| Add MCP servers | Disabled until explicitly configured |
| Access files outside project | Deny by default where the engine supports it; otherwise warn that OS-level confinement is not guaranteed |
| Apply changes to target branch | Explicit Zencode approval required |
| Delete task worktree | Explicit confirmation when task data exists |

Do not start OpenCode in blanket auto-approve mode. Do not assume that disabling a Tauri frontend permission also confines a child process launched by Rust.

## 8.3 Command execution

- Invoke executable plus argument array; do not use shell-string concatenation.
- Display the exact command and working directory before asking approval.
- Make command output visible.
- Apply timeout and cancellation controls.
- Bound captured output to avoid memory exhaustion.
- On Windows, terminate child process trees using a Job Object or equivalent.
- Do not silently run package-manager install scripts.
- Do not assume a successful build proves behavioural correctness.

## 8.4 Credential handling

- Prefer OS-backed credential storage for keys managed directly by Zencode.
- Never store provider keys in frontend state, browser local storage, logs, crash reports, or project files.
- Never commit credentials into generated `opencode.json` files or repository settings.
- The Gemini API key must remain in the Rust credential service and must never be passed to the OpenCode worker process, Ollama, or project environment.
- If an OpenCode auth flow persists credentials in its own auth store, inspect the exact release's storage behaviour and disclose it. Do not describe that store as OS-encrypted unless verified.
- Keep credential access inside Rust and pass only what the child process needs.
- Redact credentials from process logs, error messages, and diagnostic exports.
- API keys are the first supported V1 path. OAuth can be added after its callback and token-storage behaviour are properly designed.

## 8.5 Provider privacy and free-tier disclosure

“Local-first” does not mean “all processing is local.” The default coding worker is local, but an explicit planning request sends its approved context to Google Gemini.

The current Gemini API pricing documentation states that free-tier submitted content may be used to improve Google's products; paid-tier data handling is different. This is a material trade-off for source-code planning and must be visible, not buried in a footer.

Zencode must:

- explain that the task description and approved context are sent to Google for planning;
- show the exact context package before each request;
- default to metadata/tree/manifest summaries rather than full source files;
- require approval before uploading source snippets or images;
- exclude secrets and private credential files by default;
- set `store=false` on Gemini Interactions requests so Zencode does not opt into server-side conversation history; if the selected API path cannot enforce this, fail closed rather than silently storing interaction state;
- explain that stateless API storage settings do not change Google's free-tier data-use terms;
- offer a local-only mode that makes no Gemini calls; and
- show that all implementation, compiler feedback, and repair loops stay on the local worker path.

V1 has no Zencode cloud backend, no account requirement, no advertising SDK, and no telemetry by default. If crash diagnostics or analytics are ever introduced, they must be opt-in and must not contain source code or secrets.

# 9. User experience and visual direction

## 9.1 Design direction

The UI should feel like a polished desktop development tool, not a generic chatbot and not a neon science-fiction dashboard.

Design qualities:

- clean, confident, restrained;
- dark theme first, with a light theme considered after layout maturity;
- clear typography and consistent spacing;
- subtle surfaces and depth rather than heavy glass effects;
- strong hierarchy between project, task, agent activity, and review;
- visible destructive-action boundaries;
- clear empty, loading, error, offline, and permission states; and
- responsive panel sizing for normal desktop windows.

Visual quality matters, but decoration must not hide important state. A beautifully animated button is useless if the user cannot tell which provider is receiving their source code.

## 9.2 Main application layout

Suggested desktop structure:

```text
┌────────────────────────────────────────────────────────────────────┐
│ Zencode | Project: Example App | Planner: Gemini | Worker: Ollama ready             │
├────────────┬───────────────────────────────┬───────────────────────┤
│ Navigation │ Main workspace                │ Context / Details     │
│            │                               │                       │
│ Projects   │ Editor / Agent conversation   │ Task plan             │
│ Tasks      │                               │ Changed files         │
│ Review     │                               │ Verification           │
│ Settings   │                               │ Permissions            │
│            │                               │                       │
├────────────┴───────────────────────────────┴───────────────────────┤
│ Branch · Worktree · Planner usage · Worker model · Git · Build status               │
└────────────────────────────────────────────────────────────────────┘
```

## 9.3 Core screens

### Welcome / project picker

- open folder;
- recent projects;
- engine availability;
- Git availability;
- Gemini Planner setup status;
- Ollama Worker readiness;
- local model detection.

### Workspace

- project file tree;
- editor or file viewer;
- active task conversation;
- Planner model and local worker selector;
- task status and stop control.

### Task view

- user request;
- Gemini implementation plan and approval state;
- agent activity and tool status;
- permission requests;
- logs and errors;
- task worktree and branch details.

### Review view

- complete changed-file inventory;
- unified or side-by-side text diff;
- added/deleted/renamed/binary state;
- verification results;
- accept, reject, and keep-working actions;
- clear warning if the target branch changed during the task.

### Settings

- Gemini API key and planner connection status;
- current planner model and last known quota/rate-limit status (never fabricated remaining quota);
- planner request count and usage metadata;
- local Ollama model selection and worker compatibility status;
- local-only mode;
- privacy and context-upload rules;
- hard no-paid-fallback policy status;
- engine version;
- theme and appearance;
- log export with secret redaction.

---

# 10. Build and verification system

## 10.1 Project detection

Detect project types from manifests and lockfiles. Initial detection should cover common ecosystems such as:

- Node.js projects using npm, pnpm, Yarn, or Bun;
- Rust projects using Cargo;
- Python projects using common project metadata and environment tools;
- Android/Gradle projects;
- common web frameworks; and
- other projects where a clear build/test command can be discovered.

Respect the package manager indicated by the project's lockfile. Do not replace a user's package manager or rewrite lockfiles merely because Zencode prefers another tool.

## 10.2 Verification flow

1. Detect candidate build/test/lint commands.
2. Explain why each command was selected.
3. Show the exact command and working directory.
4. Ask for approval where required.
5. Run with timeout, cancellation, and bounded output.
6. Capture exit code and duration.
7. Show output and classify the result as passed, failed, cancelled, or not run.
8. Attach the result to the task review.

A build that passes does not prove that the feature behaves correctly. Zencode should say “build passed” rather than “the code is correct.”

## 10.3 Build safety

- Never run project scripts automatically on opening a folder.
- Never auto-install dependencies without explicit approval.
- Warn when a command can execute arbitrary scripts.
- Use the task worktree as the working directory.
- Keep the user informed if a command accesses the network.
- Preserve build logs for the task without storing secrets.

---

# 11. Future feature: Design Canvas

The Design Canvas is a later differentiator, not a V1 launch dependency.

## 11.1 Purpose

Allow a user to provide a screenshot, mockup, or visual reference and ask Zencode to create or adjust a working UI implementation inside the selected project.

## 11.2 Workflow

1. User opens the Design Canvas.
2. User imports a reference image or selects an existing app screen.
3. Zencode identifies the frontend framework and relevant project files.
4. With explicit image-upload consent, a vision-capable Gemini Planner analyses layout, hierarchy, spacing, typography, and components.
5. Gemini returns a structured design implementation plan; the coding worker remains local.
6. The Worker implements the plan in a task worktree.
7. Zencode starts a local preview server only after user approval of the command.
8. The user compares the implementation and reference.
9. The user requests adjustments.
10. All code changes return through the same diff-review and approval system.

## 11.3 Non-negotiable design-canvas rules

- No separate, unreviewed write path.
- No automatic merge of generated UI code.
- Preview server should bind to loopback by default.
- Model vision capability must be checked before sending an image.
- Show the provider before uploading the image.
- Preserve the original project and reference asset.
- Do not claim pixel-perfect output as a guarantee.

## 11.4 Why it comes later

Screenshot-to-code looks impressive in a demo, but it is easy to produce a visually plausible yet structurally poor implementation. Zencode must first prove that it can safely create, inspect, and integrate ordinary code changes. The Design Canvas can then use that mature workflow.

---

# 12. Local data and storage

## 12.1 What Zencode stores

Local SQLite database may store:

- project records and last-opened paths;
- project/task IDs;
- task titles and state;
- mapping between Zencode tasks and OpenCode sessions;
- worktree paths and branch names;
- selected model/provider IDs;
- verification commands and result metadata;
- user preferences; and
- non-sensitive diagnostics.

Do not duplicate entire source files or transcripts in the Zencode database unless a specific feature requires it. OpenCode maintains its own engine/session data; Zencode should reference the relevant session and store only the information needed for its own workflow.

## 12.2 Storage rules

- Use the operating system's application-data directory.
- Keep project metadata separate from project source code.
- Do not add hidden files to user projects without telling the user.
- Use database migrations.
- Use one controlled writer path.
- Handle interrupted writes and app crashes.
- Provide an option to clear local task history and clean orphaned worktrees.
- Never interpret deleting task history as permission to delete the user's source project.

---

# 13. Error handling and recovery

Every major subsystem should produce typed, actionable errors.

Examples:

| Failure | Required behaviour |
|---|---|
| OpenCode binary missing | Explain how to install or repair the pinned engine; do not silently fetch an arbitrary binary. |
| Wrong OpenCode major version | Refuse to start that adapter and explain the version mismatch. |
| Server startup timeout | Show bounded stderr/log tail and allow retry. |
| Authentication failure | Stop API calls and report engine connection failure without revealing password. |
| Gemini planner unavailable or free quota exhausted | Do not switch providers; offer retry later or continue in local-only mode. |
| Local Ollama unavailable | Do not switch to cloud coding; show setup/repair steps and allow read-only project use. |
| Malformed Gemini plan | Reject the response, show a safe error, and allow one user-triggered retry; do not dispatch malformed instructions. |
| Rate limit | Respect provider retry metadata, avoid repeated retries, and show the wait/queue state. |
| Agent cancelled | Preserve the worktree and partial changes for review. |
| Git worktree creation failure | Do not fall back to editing the primary tree silently. |
| Main branch changed during task | Stop integration and present a conflict/rebase/refresh choice. |
| Diff inventory incomplete | Block acceptance until the inventory is complete or clearly marked as an explicit user override. |
| Build command fails | Preserve output and worktree; do not auto-revert code. |
| App crashes during task | On next launch, detect the task worktree and orphaned process state; offer safe recovery. |

Never hide a failed verification behind a green success state.

---

# 14. Repository layout proposal

A monorepo is appropriate for the first version.

```text
zencode/
├─ apps/
│  └─ desktop/
│     ├─ src/
│     │  ├─ app/
│     │  ├─ components/
│     │  ├─ features/
│     │  │  ├─ projects/
│     │  │  ├─ tasks/
│     │  │  ├─ chat/
│     │  │  ├─ review/
│     │  │  ├─ models/
│     │  │  └─ settings/
│     │  ├─ hooks/
│     │  ├─ lib/
│     │  ├─ stores/
│     │  └─ styles/
│     ├─ src-tauri/
│     │  ├─ src/
│     │  │  ├─ commands/
│     │  │  ├─ engine/
│     │  │  ├─ process/
│     │  │  ├─ project/
│     │  │  ├─ git/
│     │  │  ├─ planner/
│     │  │  ├─ worker/
│     │  │  ├─ models/
│     │  │  ├─ permissions/
│     │  │  ├─ review/
│     │  │  ├─ verification/
│     │  │  ├─ storage/
│     │  │  ├─ security/
│     │  │  └─ logging/
│     │  ├─ capabilities/
│     │  ├─ binaries/
│     │  └─ tauri.conf.json
│     └─ package.json
├─ crates/
│  └─ zencode-core/        # Shared Rust domain types and business rules
├─ docs/
│  ├─ architecture/
│  ├─ decisions/
│  └─ testing/
├─ scripts/
│  ├─ package-sidecar/
│  └─ release/
├─ tests/
│  ├─ engine-contract/
│  ├─ git-worktree/
│  └─ end-to-end/
├─ Cargo.toml
├─ package.json
└─ README.md
```

Do not create every folder immediately. Add modules when the relevant milestone begins. This layout is a destination map, not a demand to create empty directories for decoration.

---

# 15. Implementation roadmap

The build order must prove the original free-first idea before investing in UI polish.

## Milestone 0 — Prove the hybrid AI loop

**Goal:** Prove that one explicit Gemini planning request can produce a valid structured plan, and that OpenCode can execute the work locally through Ollama without sending coding iterations to a cloud provider.

Build a small Rust prototype before the full desktop app. It should:

1. Verify the pinned OpenCode v1 binary and launch its HTTP server.
2. Detect readiness through the documented health endpoint.
3. Connect to the local Ollama service and confirm the selected coding model is installed.
4. Create an OpenCode session using a dedicated worker configuration restricted to Ollama only.
5. Send one harmless worker prompt and receive the response/events.
6. Call Gemini 3.5 Flash-Lite through the current documented Gemini API with a small test task and request structured JSON output.
7. Validate the plan against a Rust schema.
8. Record planner request count and response usage metadata where available.
9. Prove that local worker tool calls, compiler errors, and repair messages do not invoke Gemini.
10. Shut down OpenCode cleanly without leaving processes running.

Test failure paths:

- missing or incompatible OpenCode binary;
- Ollama is not running;
- selected local model is missing;
- local worker tool calling fails;
- Gemini key invalid;
- Gemini rate limit or quota exhaustion;
- Gemini returns malformed structured output;
- server port collision and readiness timeout;
- cancellation during a worker request; and
- app shutdown while the engine process is active.

**Milestone 0 is complete only when the separation is demonstrated in logs/tests:** one explicit planning request can reach Gemini; all worker iterations remain on Ollama; a planner quota failure never triggers a paid provider; and the process shuts down cleanly.

## Milestone 1 — Desktop foundation

- Create Tauri 2 + React + TypeScript project.
- Add Rust application state and typed Tauri commands.
- Add OpenCode engine adapter and process supervisor.
- Add Gemini Planner client behind a Rust-only interface.
- Add project picker, basic task input, planner consent preview, and plan display.
- Add local worker status and local-only mode.
- Add a plain task/chat view with streaming worker output.

Acceptance criteria:

- App launches on Windows 11.
- User can select a project.
- User can preview the planning context before sending it to Gemini.
- A Gemini plan is returned and validated.
- User can send that plan to the local OpenCode/Ollama worker.
- Worker requests do not reach Gemini.
- Closing the app leaves no unintended OpenCode process running.

## Milestone 2 — Git isolation and change review

- Detect Git and project repository state.
- Implement clean/dirty preflight.
- Create and track task-specific worktrees.
- Run the local worker only inside the task worktree.
- Collect complete Git status and diff inventory.
- Build file-level and line-level diff UI.
- Support accept/reject and cleanup workflows.
- Detect concurrent changes in the primary tree.

Acceptance criteria:

- Agent changes do not appear in the primary worktree before acceptance.
- New, modified, deleted, renamed, and binary files are represented.
- Dirty primary tree is never silently overwritten.
- Rejecting a task does not delete it without confirmation.
- Accepting a task checks the target branch and handles conflicts safely.

## Milestone 3 — Permissions and verification

- Add permission prompts and explicit command approval.
- Add project/build-system detection.
- Add approved build/test/lint execution.
- Add process cancellation, timeout, and output limits.
- Add safe logging and secret redaction.
- Add crash recovery and orphan-worktree detection.

Acceptance criteria:

- No build/install command runs without the configured approval path.
- The exact command and working directory are visible.
- Cancellation stops the relevant process tree.
- Failures preserve task changes and logs.
- No credentials appear in logs or UI diagnostics.

## Milestone 4 — Free-first policy and quota controls

- Add Gemini key setup with clear free-tier and data-use disclosure.
- Add explicit context preview and source-snippet approval.
- Add model allowlist and no-paid-provider policy.
- Add local-only mode and quota-exhaustion handling.
- Add planner request counts and usage metadata display.
- Add settings to inspect/clear stored task history.
- Add tests proving compiler/build errors cannot be routed to Gemini.

Acceptance criteria:

- The only outbound model requests in the default profile are explicit Gemini planning requests.
- The OpenCode worker can use only Ollama in the default profile.
- There is no automatic provider fallback.
- A free-quota error leaves the user with a local-only path.
- The app never claims to know account-wide remaining quota unless Google supplies authoritative data.

## Milestone 5 — Workspace polish

- Improve project navigation and editor surface.
- Add task history and recovery.
- Improve keyboard shortcuts and panel layout.
- Add settings and themes.
- Add onboarding and troubleshooting diagnostics.
- Improve accessibility and desktop behaviour.

## Milestone 6 — Design Canvas

- Add image import and preview.
- Add vision-capability checks and explicit upload consent.
- Add Gemini design planning output.
- Add local preview-server control.
- Add iteration workflow through local worker.
- Reuse the existing worktree, diff, verification, and approval pipeline.

## Milestone 7 — Packaging and release

- Bundle pinned OpenCode sidecar for Windows.
- Add installer and upgrade/uninstall behaviour.
- Verify license notices and binary checksums.
- Test clean install, upgrade, and uninstall.
- Test on a clean Windows machine without development toolchains.
- Add crash recovery and diagnostics documentation.
- Only then plan Linux packaging; macOS follows later.

# 16. Testing strategy and release gates

## 16.1 Unit tests

Cover:

- engine state transitions;
- API request/response parsing;
- server readiness and version checks;
- port selection and retry;
- permission decision logic;
- model capability filtering;
- provider fallback policy;
- Gemini planner allowlist and explicit-call policy;
- no-paid-fallback invariant;
- Ollama-only worker provider policy;
- planner-context secret filtering and preview metadata;
- structured plan schema validation;
- secret redaction;
- worktree naming and lifecycle state;
- file inventory classification;
- timeout/cancellation behaviour; and
- database migrations.

## 16.2 Integration tests

Use a pinned OpenCode test binary or a controlled local fixture to test:

- launch and readiness;
- health/version validation;
- local Ollama worker connection and tool-call smoke tests;
- Gemini planner request, structured response parsing, and quota-error handling;
- proof that local build/compiler-error loops do not call Gemini;
- session creation;
- prompt submission;
- SSE stream reconnect;
- permission request handling;
- abort;
- server failure and restart; and
- clean shutdown.

Git integration tests should use temporary repositories containing:

- tracked modified files;
- new untracked files;
- deleted files;
- renamed files;
- binary assets;
- file-mode changes;
- dirty primary working trees; and
- concurrent primary-branch changes.

## 16.3 End-to-end tests

Test a complete user task from project selection to review and acceptance. Include failure paths, not only the happy path.

## 16.4 Non-negotiable release gates

Do not call a build “ready” until:

- no primary-project changes are lost in testing;
- no accepted patch is missing a changed file from its inventory;
- no cancelled task leaves an uncontrolled child process;
- the Gemini context package is previewed before source context is sent;
- Gemini is called only for explicit planning/re-planning operations;
- the coding worker is demonstrably Ollama-only;
- no automatic paid-provider or cross-provider fallback occurs;
- the engine version is checked;
- the app can recover from a forced process crash; and
- installer/uninstaller behaviour is tested on a clean machine.

---

# 17. Product viability and differentiation

## 17.1 The competitive reality

Zencode enters a crowded AI coding market. Existing tools already provide agentic code changes, model integrations, local model support, and review workflows. A standalone Tauri UI around OpenCode is not enough.

The original Zencode premise is more specific:

> **Use a free-tier cloud model for planning, then use local AI for the repetitive coding and debugging loop, so normal use does not require paid AI API calls.**

That is a clearer product hypothesis than “another AI IDE.” It is still not a guaranteed moat: competitors can adopt the same model split, and local models vary in quality.

## 17.2 What Zencode must do better

1. **Free-first by architecture:** Gemini is a planner, not a coding worker; local Ollama performs the long coding loop.
2. **No accidental paid traffic:** typed routing rules, an Ollama-only OpenCode worker profile, no automatic fallback, and visible Gemini call accounting.
3. **Simple hybrid setup:** explain how to connect a free-tier Gemini key and an installed local model without requiring a paid subscription or a hosted Zencode account.
4. **Local change control:** every write task uses a worktree, complete change inventory, explicit review, and safe integration.
5. **Honest privacy:** show which context leaves the machine and explain the free-tier data-use trade-off.
6. **Hardware realism:** do not promise that every local model will run quickly on every computer; test and display compatibility.

## 17.3 What is not a differentiator

These alone do not make Zencode special:

- AI chat;
- code generation;
- a dark interface;
- Rust or Tauri;
- support for a popular model;
- a generic multi-agent diagram; or
- a claim that the app is “100% free forever.”

## 17.4 Can it get traction?

Possibly, if it solves a specific pain for students, solo developers, and indie builders who want to experiment with AI coding without paying for a coding subscription. The strongest message is not “better than every AI IDE.” It is:

> **A free-first AI coding workspace: Gemini plans, your local model codes, and you review every change.**

Do not market it as unlimited free AI. Google owns the API quota, model availability, and free-tier terms. Users also need hardware capable of running their chosen local model.

## 17.5 Validation plan

Before adding advanced features:

1. Run at least 20 real tasks across more than one project type.
2. Measure planner requests per task, local worker success, number of repair loops, task completion rate, and time spent reviewing.
3. Record whether normal tasks complete with exactly one Gemini planning call and local-only implementation. Treat extra user-triggered re-plans separately.
4. Test on the target PC and at least one lower-spec or different-spec machine before making broad hardware claims.
5. Ask independent developers to install and use the app without personal coaching.
6. Compare it with their existing tools, not with an imaginary competitor.
7. Check whether the free-first approach is actually enough for their workloads and whether they trust the context disclosure.

Suggested go/no-go signals:

- zero loss of user changes in tested scenarios;
- every changed file represented in review;
- local worker handles ordinary edits/build errors without cloud escalation;
- Gemini is used only for user-authorised planning;
- no paid API usage is initiated by Zencode's default router;
- setup is understandable to a new user; and
- some testers continue using the workflow after the novelty wears off.

## 17.6 Is the work worth doing?

**As a learning and portfolio project: yes.** The hybrid routing, local model integration, process supervision, Git isolation, and safe review system are substantial engineering achievements even if the app never becomes a business.

**As a product: unproven, but worth validating.** The free-first architecture gives it a clearer identity. The biggest risks are local model quality, setup friction, hardware performance, Gemini quota/policy changes, and competition copying the idea. Build the smallest useful loop, test it with real projects, and earn the next milestone through evidence.

# 18. Risks and mitigations

| Risk | Why it matters | Mitigation |
|---|---|---|
| OpenCode API changes | Sidecar integration could break after an update. | Pin version, verify `/doc`, contract-test adapter, separate engine interface. |
| OpenCode v1/v2 confusion | Same command name, different integration generations. | Bundle pinned v1 binary; check version; do not auto-detect arbitrary global binary. |
| Thin-wrapper problem | Users can use OpenCode or another AI editor directly. | Invest in task isolation, review completeness, transparency, and workflow quality. |
| Git worktree misunderstood as sandbox | Agent-launched processes may reach outside the worktree. | State limits honestly; ask before shell/network actions; plan stronger OS sandboxing later. |
| Incomplete change review | User may approve a patch while missing an untracked or binary change. | Build inventory from Git status and filesystem-aware checks; block approval if incomplete. |
| Dirty primary tree | Merge/apply could damage user work. | Strict preflight, explicit checkpoint action, no automatic stash/reset. |
| Free-tier quota/model changes | Planner may become unavailable or its limits may change. | Verify current model/quotas, show failures honestly, continue local-only; never paid-fallback. |
| Free-tier data use | Planning context may be used to improve Google products. | Explicit context preview, minimise data, exclude secrets, offer local-only mode. |
| Local worker tool-use weakness | A code model may generate text but fail to use tools reliably. | Benchmark Qwen candidate; require tool-call smoke tests; never silently use cloud worker. |
| Billing configuration | A user's Google API project may have paid billing enabled. | Explain account-level billing limitation; enforce model allowlist and no paid-provider route in Zencode. |
| Local model performance | Consumer hardware varies widely. | Benchmark quantised local models on representative hardware; no speed guarantees. |
| Scope creep | IDE, canvas, indexing, plugins, and multi-agent work multiply effort. | Milestones and release gates; no later feature before the core loop passes. |
| Packaging/licensing | Bundled third-party binaries have obligations. | Pin releases, audit licenses, preserve notices, verify checksums. |
| Weak demand | Technically good software may still have no users. | Validate with real users before expanding. |

---

# 19. Architecture decisions that are frozen

Unless implementation evidence proves a decision wrong, V1 is locked to:

- Product: Zencode, free-first hybrid AI development workspace.
- Core promise: Gemini plans; a local Ollama model performs coding and debugging; normal workflow does not require paid AI API calls.
- Application type: installable desktop application.
- Initial operating system: Windows 11.
- Desktop framework: Tauri 2.
- Frontend: React + TypeScript + Vite.
- Backend: Rust.
- Planner: direct Gemini API calls from Rust, using Gemini 3.5 Flash-Lite (`gemini-3.5-flash-lite`) as the initial free-tier candidate, subject to current account/model availability.
- Planner API: Gemini Interactions API as the current recommended starting point; isolate behind a client adapter.
- Planner call frequency: one explicit planning request per new task by default; no automatic replanning.
- Coding worker: Ollama local inference.
- Initial local model candidate: Qwen2.5-Coder 7B, pending tool-use and quality tests on target hardware.
- Agent engine: OpenCode sidecar, not a fork.
- Engine protocol: pinned OpenCode v1 HTTP API + SSE.
- OpenCode worker provider: Ollama only in the default profile.
- Routing: Rust owns all provider/model routing; compiler errors and coding iterations remain local.
- No paid fallback: no Groq/paid cloud coding provider or automatic tier upgrade in V1.
- Frontend communication: Tauri IPC only.
- Local service binding: loopback only, authenticated.
- Task isolation: Git worktrees for write tasks.
- Review: complete change inventory plus explicit user acceptance.
- Verification: user-approved build/test commands with recorded results.
- Privacy: preview all remote planning context; secrets excluded; local-only mode available.
- Backend service: no Zencode-hosted backend in V1.
- Telemetry: off by default; no source-code collection.
- Design Canvas: post-core-workflow milestone.
- Forking OpenCode: not planned unless a specific required API limitation is proven.

Reopen an architecture decision only when a concrete test or implementation limitation demonstrates the need. Do not reopen it because another framework appeared in a social-media thread.

---

# 20. Immediate next action

Do not start by building the dashboard, design canvas, custom indexer, or provider marketplace.

Start with **Milestone 0: prove the hybrid model boundary** in a small Rust prototype.

The first implementation must answer these questions:

1. Can Rust make one explicit Gemini planning request and validate the returned structured plan?
2. Can Rust launch the pinned OpenCode worker and connect it to Ollama?
3. Can the local model reliably use the tools needed for real coding tasks?
4. Can the worker complete multiple edit/build/error-repair cycles without any Gemini calls?
5. Does quota exhaustion stop cloud planning without triggering another provider or paid tier?
6. Can the engine shut down cleanly without leaving child processes?

The first successful output should be a repeatable test log showing:

- exact OpenCode version;
- local Ollama endpoint and selected model;
- Gemini planner model ID and explicit request count;
- plan response validated against the local JSON schema;
- worker prompt handled by Ollama;
- local tool/build/compiler-error loop completed without a Gemini request;
- quota-error test resulted in local-only mode;
- process shut down; and
- no child process remained.

Only after this proof should the full Tauri desktop shell begin. The project is built around this model split; prove it before polishing anything around it.

# 21. Reference documentation

These references were checked on 4 November 2026. Google currently lists Gemini 3.5 Flash-Lite as free for Standard input/output use, while free-tier requests are subject to project/model-specific limits and free-tier data-use terms. API details, model availability, and quotas can change; implementation must re-check the documentation and the user's active AI Studio project before release.

- [OpenCode v1 Server API](https://opencode.ai/docs/server/) — headless server, authentication, OpenAPI document, sessions, messages, events, provider and auth endpoints.
- [OpenCode CLI documentation](https://opencode.ai/docs/cli/) — server command and environment variables.
- [OpenCode Providers](https://opencode.ai/docs/providers/) — provider connections and credential handling.
- [OpenCode Agents](https://opencode.ai/docs/agents/) — per-agent models and permissions; verify the exact schema for the pinned release.
- [Ollama + OpenCode integration](https://github.com/ollama/ollama/blob/main/docs/integrations/opencode.mdx) — local provider configuration.
- [OpenCode Tools](https://opencode.ai/docs/tools/) — coding tools and permission model.
- [OpenCode v2 ACP documentation](https://opencode.ai/v2/docs/cli/acp) — separate JSON-RPC-over-stdio integration generation; not the V1 adapter target.
- [Tauri 2 sidecar documentation](https://v2.tauri.app/develop/sidecar/) — bundling external binaries by target triple.
- [Tauri 2 capabilities documentation](https://v2.tauri.app/security/capabilities/) — frontend capability and permission boundaries.
- [Git worktree documentation](https://git-scm.com/docs/git-worktree) — linked working trees and lifecycle.
- [Gemini API models](https://ai.google.dev/gemini-api/docs/models) — current model IDs and capabilities.
- [Gemini 3.5 Flash-Lite model](https://ai.google.dev/gemini-api/docs/models/gemini-3.5-flash-lite) — candidate planner model.
- [Gemini API pricing](https://ai.google.dev/gemini-api/docs/pricing) — free/paid tier availability and data-use disclosures.
- [Gemini API rate limits](https://ai.google.dev/gemini-api/docs/rate-limits) — model/project-specific quota dimensions.
- [Gemini Interactions API](https://ai.google.dev/gemini-api/docs/interactions-overview) — current recommended API for new integrations.
- [Gemini structured outputs](https://ai.google.dev/gemini-api/docs/structured-output) — JSON-schema constrained output.
- [Zed Agent Panel documentation](https://zed.dev/docs/ai/agent-panel) — existing agent editing, review, model, and permission capabilities relevant to competitive analysis.
- [Zed AI overview](https://zed.dev/docs/ai/overview) — model access and existing AI workflows.

---

**Final engineering rule:** Zencode is not successful because it can make an AI edit a file. It is successful when it can make that edit in the right place, show every change, explain what was verified, preserve the user's existing work, and let the user decide what happens next.
