//! The in-process loop engine.
//!
//! A **loop** is a spawned worker that delivers a fixed message to its agent
//! (via `agents message`) every interval. Loops live in a map keyed by
//! `agent_instance_hierarchy` (AIH); ids are scoped per-AIH, so `end_loop`
//! only sees loops begun under the same AIH.
//!
//! There is no persistence and no auto-cleanup: a never-ended loop runs until
//! the daemon process exits (matching how unresolved task entries live for the
//! process lifetime).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use dashmap::DashMap;
use objectiveai_sdk::cli::command::plugin::PluginExecutor;
use rmcp::model::{CallToolResult, Content};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::mcp::common::{gen_id, send_message};

/// A live loop's shared handle. Cheap to clone — stored in the map and cloned
/// out for `end`/`list`.
#[derive(Clone)]
struct LoopHandle {
    cancel: CancellationToken,
    /// When the loop next delivers its message; refreshed by the worker at the
    /// start of every cycle. `list` reads it to report the time remaining.
    next_at: Arc<Mutex<Instant>>,
}

/// The per-process loop registry, keyed by AIH then loop id.
pub struct LoopRegistry {
    executor: PluginExecutor,
    by_aih: Arc<DashMap<String, DashMap<String, LoopHandle>>>,
}

impl LoopRegistry {
    pub fn new(executor: PluginExecutor) -> Self {
        Self {
            executor,
            by_aih: Arc::new(DashMap::new()),
        }
    }

    /// Spawn a loop that messages `aih` with `message` every
    /// `interval_seconds`, and return its id immediately. The first message
    /// arrives after one full interval.
    pub fn begin(&self, aih: String, interval_seconds: u64, message: String) -> String {
        let id = gen_id();
        let cancel = CancellationToken::new();
        // Seeded here so the entry is listable before the worker's first cycle;
        // the worker refreshes it (to essentially the same deadline) on start.
        let next_at = Arc::new(Mutex::new(
            Instant::now() + Duration::from_secs(interval_seconds),
        ));

        self.by_aih.entry(aih.clone()).or_default().insert(
            id.clone(),
            LoopHandle {
                cancel: cancel.clone(),
                next_at: next_at.clone(),
            },
        );

        tokio::spawn(worker(
            self.executor.clone(),
            aih,
            id.clone(),
            message,
            interval_seconds,
            cancel,
            next_at,
        ));

        id
    }

    /// End a loop immediately. Claims (removes) the loop so any later `end`
    /// for that id gets "not found".
    pub fn end(&self, aih: &str, id: &str) -> CallToolResult {
        match self.take(aih, id) {
            Some(handle) => {
                handle.cancel.cancel();
                CallToolResult::success(vec![Content::text("ended")])
            }
            None => CallToolResult::error(vec![Content::text("loop not found")]),
        }
    }

    /// List the AIH's loops and how long until each next delivers its message.
    pub fn list(&self, aih: &str) -> CallToolResult {
        let now = Instant::now();
        let mut items: Vec<serde_json::Value> = Vec::new();
        if let Some(inner) = self.by_aih.get(aih) {
            for entry in inner.iter() {
                // Saturates to 0 while a due tick's send is still in flight
                // (the worker refreshes the deadline on its next cycle).
                let next_at = *entry.value().next_at.lock().expect("next_at lock");
                let seconds = next_at.saturating_duration_since(now).as_secs();
                items.push(serde_json::json!({
                    "loop_id": entry.key(),
                    "seconds_until_next_run": seconds,
                }));
            }
        }
        let body = serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string());
        CallToolResult::success(vec![Content::text(body)])
    }

    /// Atomically remove a loop and return its handle, claiming it. `None` if
    /// the AIH or id is unknown.
    fn take(&self, aih: &str, id: &str) -> Option<LoopHandle> {
        // The `get` read-guard is dropped at the end of this statement, before
        // we take the outer write lock below.
        let handle = self
            .by_aih
            .get(aih)
            .and_then(|inner| inner.remove(id).map(|(_, handle)| handle))?;
        // Drop the AIH's inner map if this was its last loop. `remove_if`
        // evaluates `is_empty` under the outer shard's write lock, and
        // `begin`'s `entry().or_default().insert()` holds that same lock for
        // its whole duration — so this can never delete a map that a concurrent
        // `begin` is inserting into (the insert either completes first, leaving
        // it non-empty, or runs after and recreates the entry).
        self.by_aih.remove_if(aih, |_, inner| inner.is_empty());
        Some(handle)
    }
}

/// The spawned loop body: sleep one interval, deliver the message, repeat —
/// until cancelled. A `sleep` loop (rather than `tokio::time::interval`) so the
/// first message lands only after a full interval and sends never burst:
/// the next delay starts after the previous send completes. Send errors are
/// ignored; the loop keeps ticking.
async fn worker(
    executor: PluginExecutor,
    aih: String,
    id: String,
    message: String,
    interval_seconds: u64,
    cancel: CancellationToken,
    next_at: Arc<Mutex<Instant>>,
) {
    let period = Duration::from_secs(interval_seconds);
    loop {
        let deadline = Instant::now() + period;
        *next_at.lock().expect("next_at lock") = deadline;
        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep_until(deadline) => {
                let text = format!("<quas-wex-exort loop-id=\"{id}\">\n{message}\n</quas-wex-exort>");
                let _ = send_message(&executor, &aih, &text).await;
            }
        }
    }
}
