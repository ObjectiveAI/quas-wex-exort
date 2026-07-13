//! The in-process task engine.
//!
//! A **task** is a background invocation of another MCP tool (run via the
//! ObjectiveAI CLI `agents mcp tools call`). Tasks live in a map keyed by
//! `agent_instance_hierarchy` (AIH); ids are scoped per-AIH, so `wait`/`cancel`
//! only see tasks created under the same AIH.
//!
//! Each task is a spawned worker. On completion it either hands its result to a
//! waiting `wait`, or — if no one waited first — wakes the agent with an
//! `agents message`. Cancellation is immediate and silent.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use dashmap::DashMap;
use objectiveai_sdk::cli::command::plugin::PluginExecutor;
use rmcp::model::{CallToolResult, Content, Meta};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::mcp::common::{call_tool, gen_id, send_message};

/// The terminal state of a task.
enum Outcome {
    /// The underlying tool ran; carries its (already-converted) result, whose
    /// `is_error` flag distinguishes a successful call from a tool error.
    Completed(CallToolResult),
    /// The call could not be made (executor / argument failure).
    Failed(String),
    /// The task was cancelled before it finished.
    Cancelled,
}

/// A live task's shared handles. Cheap to clone — stored in the map and cloned
/// out for `wait`/`cancel`/`list`.
#[derive(Clone)]
struct TaskHandle {
    cancel: CancellationToken,
    /// Suppresses the completion nudge. Set by `wait` (the agent is collecting
    /// the result) or `cancel` (the agent is discarding the task).
    waited: Arc<AtomicBool>,
    /// `Some` once the worker finishes.
    outcome: watch::Receiver<Option<Arc<Outcome>>>,
}

/// The per-process task registry, keyed by AIH then task id.
pub struct TaskRegistry {
    executor: PluginExecutor,
    by_aih: Arc<DashMap<String, DashMap<String, TaskHandle>>>,
}

impl TaskRegistry {
    pub fn new(executor: PluginExecutor) -> Self {
        Self {
            executor,
            by_aih: Arc::new(DashMap::new()),
        }
    }

    /// Spawn a task that invokes `tool` with `arguments` (scoped to
    /// `response_id`) in the background, and return its id immediately.
    pub fn create(
        &self,
        aih: String,
        response_id: String,
        tool: String,
        arguments: serde_json::Value,
    ) -> String {
        let id = gen_id();
        let cancel = CancellationToken::new();
        let waited = Arc::new(AtomicBool::new(false));
        let (tx, rx) = watch::channel(None);

        self.by_aih.entry(aih.clone()).or_default().insert(
            id.clone(),
            TaskHandle {
                cancel: cancel.clone(),
                waited: waited.clone(),
                outcome: rx,
            },
        );

        tokio::spawn(worker(
            self.executor.clone(),
            aih,
            id.clone(),
            response_id,
            tool,
            arguments,
            cancel,
            waited,
            tx,
        ));

        id
    }

    /// Wait for a task to complete and return the underlying tool's result.
    /// Claims (removes) the task up front, so it immediately disappears from
    /// `list` and any later `wait`/`cancel` for that id gets "not found"; the
    /// cloned handle is kept to await the result. Marks the task "waited" so
    /// the completion message is not sent.
    pub async fn wait(&self, aih: &str, id: &str) -> CallToolResult {
        let handle = match self.take(aih, id) {
            Some(h) => h,
            None => return CallToolResult::error(vec![Content::text("task not found")]),
        };
        handle.waited.store(true, Ordering::Release);

        let mut rx = handle.outcome.clone();
        let outcome = match rx.wait_for(|v| v.is_some()).await {
            Ok(guard) => guard.clone().expect("wait_for guaranteed Some"),
            Err(_) => return CallToolResult::error(vec![Content::text("task ended unexpectedly")]),
        };

        match &*outcome {
            Outcome::Completed(result) => result.clone(),
            Outcome::Failed(e) => {
                CallToolResult::error(vec![Content::text(format!("task failed: {e}"))])
            }
            Outcome::Cancelled => {
                CallToolResult::error(vec![Content::text("task was cancelled")])
            }
        }
    }

    /// Cancel a running task immediately. Claims (removes) the task so it
    /// disappears from `list` and any later `wait`/`cancel` gets "not found".
    /// No completion message is sent.
    pub fn cancel(&self, aih: &str, id: &str) -> CallToolResult {
        match self.take(aih, id) {
            Some(handle) => {
                // Suppress the completion nudge, so a task that finishes in the
                // same instant (completion-vs-cancel race) still stays silent —
                // cancel means the agent is resolving it.
                handle.waited.store(true, Ordering::Release);
                handle.cancel.cancel();
                CallToolResult::success(vec![Content::text("cancelled")])
            }
            None => CallToolResult::error(vec![Content::text("task not found")]),
        }
    }

    /// List the AIH's tasks and their status.
    pub fn list(&self, aih: &str) -> CallToolResult {
        let mut items: Vec<serde_json::Value> = Vec::new();
        if let Some(inner) = self.by_aih.get(aih) {
            for entry in inner.iter() {
                let status = match entry.value().outcome.borrow().as_deref() {
                    None => "running",
                    Some(Outcome::Cancelled) => "cancelled",
                    Some(Outcome::Failed(_)) => "error",
                    Some(Outcome::Completed(r)) => {
                        if r.is_error == Some(true) {
                            "error"
                        } else {
                            "completed"
                        }
                    }
                };
                items.push(serde_json::json!({ "task_id": entry.key(), "status": status }));
            }
        }
        let body = serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string());
        CallToolResult::success(vec![Content::text(body)])
    }

    /// Atomically remove a task and return its handle, claiming it. `None` if
    /// the AIH or id is unknown. This is how `wait`/`cancel` delete a task:
    /// once claimed it no longer appears in `list`, and any later `wait`/`cancel`
    /// for that id gets "not found".
    fn take(&self, aih: &str, id: &str) -> Option<TaskHandle> {
        // The `get` read-guard is dropped at the end of this statement, before
        // we take the outer write lock below.
        let handle = self
            .by_aih
            .get(aih)
            .and_then(|inner| inner.remove(id).map(|(_, handle)| handle))?;
        // Drop the AIH's inner map if this was its last task. `remove_if`
        // evaluates `is_empty` under the outer shard's write lock, and
        // `create`'s `entry().or_default().insert()` holds that same lock for
        // its whole duration — so this can never delete a map that a concurrent
        // `create` is inserting into (the insert either completes first, leaving
        // it non-empty, or runs after and recreates the entry).
        self.by_aih.remove_if(aih, |_, inner| inner.is_empty());
        Some(handle)
    }
}

/// The spawned task body: run the tool call (or get cancelled), publish the
/// outcome to any waiter, and — unless `wait` claimed it first — nudge the agent.
#[allow(clippy::too_many_arguments)]
async fn worker(
    executor: PluginExecutor,
    aih: String,
    id: String,
    response_id: String,
    tool: String,
    arguments: serde_json::Value,
    cancel: CancellationToken,
    waited: Arc<AtomicBool>,
    tx: watch::Sender<Option<Arc<Outcome>>>,
) {
    let outcome = tokio::select! {
        _ = cancel.cancelled() => Outcome::Cancelled,
        result = call_tool(&executor, &response_id, &tool, arguments) => match result {
            Ok(native) => Outcome::Completed(to_rmcp(native)),
            Err(e) => Outcome::Failed(e),
        },
    };

    // The completion-message wording, or `None` to stay silent (cancelled).
    let kind: Option<&str> = match &outcome {
        Outcome::Cancelled => None,
        Outcome::Failed(_) => Some("with an error"),
        Outcome::Completed(r) => Some(if r.is_error == Some(true) {
            "with an error"
        } else {
            "successfully"
        }),
    };

    // Publish to any waiter before deciding on the message.
    let _ = tx.send(Some(Arc::new(outcome)));

    // If no one waited before completion, nudge the agent to resolve the task:
    // `task_wait` to collect the result, or `task_cancel` to discard it. The
    // entry is intentionally NOT removed here — the message carries no result,
    // so the task must stay retrievable until the agent resolves it explicitly
    // (removal happens only in `wait`/`cancel`).
    if !waited.load(Ordering::Acquire) {
        if let Some(kind) = kind {
            let text =
                format!("<quas-wex-exort>\nTask '{id}' has completed {kind}.\n</quas-wex-exort>");
            let _ = send_message(&executor, &aih, &text).await;
        }
    }
}

/// Convert the SDK's native tool result into an rmcp `CallToolResult`. The
/// SDK's `rmcp_bridge` only converts at the content-block level, so we map the
/// blocks and reconstruct the result envelope ourselves.
fn to_rmcp(native: objectiveai_sdk::mcp::tool::CallToolResult) -> CallToolResult {
    let content: Vec<Content> = native.content.into_iter().map(Into::into).collect();
    let mut result = if native.is_error == Some(true) {
        CallToolResult::error(content)
    } else {
        CallToolResult::success(content)
    };
    if let Some(structured) = native.structured_content {
        result.structured_content =
            Some(serde_json::Value::Object(structured.into_iter().collect()));
    }
    if let Some(meta) = native._meta {
        result.meta = Some(Meta(meta.into_iter().collect()));
    }
    result
}
