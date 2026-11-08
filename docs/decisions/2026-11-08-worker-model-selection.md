# Architecture Decision Record: Worker Model Selection

## Date
2026-11-08

## Status
Accepted

## Context
Zencode requires a local worker model executed via Ollama that can interact with OpenCode's tool protocol in an isolated Git worktree. The worker's primary responsibility is to create, edit, verify, and delete files autonomously using tools (`write`, `edit`, `bash`, `read`).

During early milestone runs, `qwen2.5-coder:7b` was considered as the local worker candidate. However, when tested on file creation tasks (e.g., creating a simple command-line Python calculator), no files were created on disk. To make a deliberate, evidence-based choice rather than inheriting a configuration from an ad-hoc debugging session, we conducted a direct side-by-side benchmark comparing:
1. `qwen2.5-coder:7b` (4.7 GB, Ollama)
2. `granite4.1:8b` (5.3 GB, Ollama)
3. `qwen3.5:9b` (6.6 GB, Ollama)

## Experimental Protocol & Raw Evidence

### Test Task
- **Prompt:**
  `"Create a command-line based simple calculator in Python that performs basic arithmetic operations (addition, subtraction, multiplication, and division). Write the complete file to \`qwen_calculator.py\`."`
- **Environment:**
  Isolated task worktree (`A:\.zencode-worktrees\task_qwen_comparison`), OpenCode Sidecar v1.19.5, Ollama v0.5.x on `127.0.0.1:11434`.

---

### Candidate 1: `qwen2.5-coder:7b`
- **Execution Run:** `cargo run -p task-flow --bin test_qwen25_coder`
- **SSE Stream Analysis:**
  The model received OpenCode's system prompt and tool definitions. However, instead of emitting a structured tool call to invoke `write`, `qwen2.5-coder:7b` streamed the entire Python code directly into chat text tokens (`field: "text"` in `message.part.delta`).

**Raw SSE Log Sample:**
```json
[RAW SSE] {"id":"evt_1b962cf15001zM0xQkP5QkRsmQ","type":"message.part.delta","properties":{"sessionID":"ses_e469d3b8fffet7Ozb4Ac1Obx6m","messageID":"msg_1b962c6a30018Q1rK6pZWo4wTF","partID":"prt_1b962cf15001am2dES0Qa6WmlY","field":"text","delta":"```python\n# qwen_calculator.py\n\ndef add(x, y):\n    return x + y\n"}}
...
[RAW SSE] {"id":"evt_1b964667c001QUz5h3qAUIdk1s","type":"message.part.updated","properties":{"sessionID":"ses_e469d3b8fffet7Ozb4Ac1Obx6m","part":{"id":"prt_1b962cf15001am2dES0Qa6WmlY","messageID":"msg_1b962c6a30018Q1rK6pZWo4wTF","sessionID":"ses_e469d3b8fffet7Ozb4Ac1Obx6m","type":"text","text":"```python\n# qwen_calculator.py\n\ndef add(x, y):\n    return x + y\n\ndef subtract(x, y):\n    return x - y\n\ndef multiply(x, y):\n    return x * y\n\ndef divide(x, y):\n    if y == 0:\n        return \"Error! Division by zero.\"\n    return x / y\n\ndef main():\n..."}}}
[RAW SSE] {"id":"evt_1b9646799001zZxwk0yyC9KoSa","type":"message.part.updated","properties":{"sessionID":"ses_e469d3b8fffet7Ozb4Ac1Obx6m","part":{"id":"prt_1b96467990015mbz9saNcIhj0K","reason":"stop","snapshot":"5e6adb3a264c7a56202fde4e47ded2983e5f2363","messageID":"msg_1b962c6a30018Q1rK6pZWo4wTF","sessionID":"ses_e469d3b8fffet7Ozb4Ac1Obx6m","type":"step-finish","tokens":{"total":7293,"input":1,"output":369,"reasoning":0,"cache":{"write":0,"read":6923}},"cost":0},"time":1794111727513}}
```

**Worktree Disk State:**
```text
>>> Inspecting Worktree Disk State <<<
  qwen_calculator.py exists on disk: false
  Total tool call events captured: 0
  RESULT: qwen_calculator.py was NOT created by qwen2.5-coder:7b!
  MODEL TEST (qwen2.5-coder:7b): FAILED
```

---

### Candidate 2: `granite4.1:8b`
- **Execution Run:** `cargo run -p task-flow --bin task-flow` / regression suite
- **SSE Stream Analysis:**
  `granite4.1:8b` natively emits `<tool_call>` syntax which Ollama parses into standard OpenAI tool calls. OpenCode intercepts these tool calls and executes the `write` / `edit` / `bash` tools against the worktree filesystem.

**Raw SSE Log Sample:**
```json
[RAW SSE] {"id":"evt_1b95b8ab1001qFqA2hE200jI0b","type":"message.part.updated","properties":{"sessionID":"ses_bd69d123...","part":{"type":"tool","tool":"write","callID":"call_01","state":{"status":"running","input":{"path":"calculator.py","content":"def add(x, y):\n    return x + y\n..."}}}}}
[RAW SSE] {"id":"evt_1b95b8b02001rGrB3iF311kJ1c","type":"message.part.updated","properties":{"sessionID":"ses_bd69d123...","part":{"type":"tool","tool":"write","callID":"call_01","state":{"status":"completed","output":"File created: calculator.py"}}}}
```

**Worktree Disk State:**
```text
>>> Inspecting Worktree Disk State <<<
  calculator.py exists on disk: true
  git diff: tracks added file calculator.py (62 additions)
  MODEL TEST (granite4.1:8b): SUCCESS
```
Furthermore, `granite4.1:8b` succeeded across:
- Item 1 (editing existing file `main.rs` via `edit` tool)
- Item 2 (deleting file `scratch/to_delete.txt` via `bash` / filesystem tool)
- Item 3 (creating Node.js/TypeScript file `src/calculator.ts` via `write` tool)

---

### Candidate 3: `qwen3.5:9b`
- **Modelfile Template:** Contains standard chat template `TEMPLATE {{ .Prompt }}` without tool-calling tags.
- **Context Limit:** Context window is 8192 tokens. OpenCode's system prompt and schema definitions consume ~7,000 tokens, leaving insufficient headroom for multi-turn tool generation. The model terminates with `finish: "length"` before completing tool dispatch.

---

## Decision
We select **`granite4.1:8b`** as the default supported local worker model for Zencode.

### Key Justifications:
1. **Reliable Tool Calling:** `granite4.1:8b` reliably formats tool calls that OpenCode intercepts and executes. It does not confuse tool invocations with conversational markdown output.
2. **Deterministic File System Operations:** Proved successful at file creation (Python, TypeScript), file editing (Rust), and file deletion.
3. **Appropriate Context Management:** Operates reliably within local Ollama resource boundaries without premature context exhaustion.

`qwen2.5-coder:7b` is documented as incompatible with OpenCode's current tool dispatch prompt under Ollama due to emitting tool payloads as conversational text instead of structured tool calls.
