# quas-wex-exort

Programmatic invocation of MCP tools and the ObjectiveAI CLI for ObjectiveAI agents.

An ObjectiveAI plugin that runs an agent-facing MCP server. Tools surface to
agents prefixed with the server name (e.g. `quas-wex-exort_multi_call`).

## Toolsets

Each gated toolset is enabled per-session by a boolean in the plugin's
`arguments` map (`tasks`, `multi`, `loops`, `python`), which the host bridges
into the `x-objectiveai-arguments` request header. A disabled toolset's tools
are hidden and uncallable.

### `tasks` — background tool invocations

- `create_task` — invoke another MCP tool in the arsenal as a background
  task; returns the task id immediately.
- `list_tasks` — list your tasks and their status.
- `wait_task` — wait for a task to complete and return its result.
- `cancel_task` — cancel a running task.

A task that completes without being waited on nudges the agent with a
completion message.

### `multi` — concurrent tool invocations

- `multi_call` — invoke several MCP tools concurrently and return all their
  results together.

### `loops` — recurring reminder messages

- `begin_loop` — begin a loop that messages the agent with a fixed message
  every `interval_seconds` (minimum 1; first message after one full interval);
  returns the loop id immediately. Messages are delivered wrapped in a
  `<quas-wex-exort loop-id="…">` envelope with the message text verbatim
  inside.
- `end_loop` — end a loop by id, stopping its messages.

### `python` — scripted tool orchestration (planned)

Run tools via Python scripts ([#4](https://github.com/ObjectiveAI/quas-wex-exort/issues/4)).
The `python` argument is accepted today but gates no tools yet.

### Ungated

- `list_tools` — paginated listing of the agent's arsenal, with optional
  field selection.

## Development

- `bash build.sh` — build the plugin and stage the local `.objectiveai/`
  sandbox (host binaries + unpacked plugin).
- `bash test.sh` — run the integration suite against the staged sandbox
  (requires cargo-nextest).
- `bash build-and-test.sh` — both.
