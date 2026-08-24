use codex_protocol::ThreadId;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_rollout::RolloutConfig;
use codex_rollout::RolloutRecorder;
use codex_rollout::RolloutRecorderParams;

use super::LocalThreadStore;
use super::paginated_fork;
use super::thread_rollout_resolver;
use crate::ForkBoundary;
use crate::RevertLiveThreadParams;
use crate::RevertThreadParams;
use crate::RevertedLiveThread;
use crate::StoredModelContext;
use crate::ThreadPersistenceMetadata;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

/// Revert an unloaded paginated thread by creating a new immutable rollout file.
///
/// Old rollouts stay intact. The new file references the retained prefix, and the only mutable
/// cutover is the existing SQLite rollout-path pointer for the thread.
pub(super) async fn revert(
    store: &LocalThreadStore,
    params: RevertThreadParams,
) -> ThreadStoreResult<()> {
    let RevertThreadParams {
        thread_id,
        before_turn_id,
    } = params;
    let state_db = store
        .state_db()
        .await
        .ok_or(ThreadStoreError::Unsupported {
            operation: "revert_thread",
        })?;
    let _lifecycle_guard = store.live_writer_locks.lock_lifecycle(thread_id).await;
    let _live_writer_guard = store.live_writer_locks.lock(thread_id).await;
    store.ensure_live_recorder_absent(thread_id).await?;
    let _writer_lock = store.writer_lock_coordinator.acquire(thread_id)?;

    // Resolution may return a compressed sibling. Keep SQLite's exact stored path for the CAS.
    let expected_sqlite_path = state_db
        .get_thread(thread_id)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to read thread metadata for {thread_id}: {err}"),
        })?
        .ok_or(ThreadStoreError::ThreadNotFound { thread_id })?
        .rollout_path;
    let (_rollout_id, recorder, _model_context) =
        create_reverted_recorder(store, thread_id, before_turn_id, None).await?;
    let replacement_path = recorder.rollout_path().to_path_buf();
    recorder.persist().await.map_err(thread_store_io_error)?;
    recorder.shutdown().await.map_err(thread_store_io_error)?;

    let replaced = state_db
        .replace_rollout_path_if_current(
            thread_id,
            expected_sqlite_path.as_path(),
            replacement_path.as_path(),
        )
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to switch thread {thread_id} to reverted rollout: {err}"),
        })?;
    if !replaced {
        let _ = tokio::fs::remove_file(replacement_path.as_path()).await;
        return Err(ThreadStoreError::Conflict {
            message: format!("thread {thread_id} changed while it was being reverted"),
        });
    }
    Ok(())
}

/// Revert a paginated thread without relinquishing its process-wide writer lease.
///
/// The old recorder remains installed until the replacement rollout is durable and SQLite accepts
/// the compare-and-swap. Once SQLite points at the replacement, swapping the in-memory recorder is
/// infallible while the per-thread writer lock is held.
pub(super) async fn revert_live(
    store: &LocalThreadStore,
    params: RevertLiveThreadParams,
) -> ThreadStoreResult<RevertedLiveThread> {
    let RevertLiveThreadParams {
        thread_id,
        before_turn_id,
        metadata,
    } = params;
    let state_db = store
        .state_db()
        .await
        .ok_or(ThreadStoreError::Unsupported {
            operation: "revert_live_thread",
        })?;
    let _lifecycle_guard = store.live_writer_locks.lock_lifecycle(thread_id).await;
    let _live_writer_guard = store.live_writer_locks.lock(thread_id).await;
    let (old_recorder, old_rollout_id, history_mode) =
        super::live_writer::live_writer_parts(store, thread_id).await?;
    if history_mode != ThreadHistoryMode::Paginated {
        return Err(ThreadStoreError::InvalidRequest {
            message: format!("thread {thread_id} does not use paginated history"),
        });
    }
    old_recorder.flush().await.map_err(thread_store_io_error)?;
    super::thread_history_materialization::materialize_to_sqlite(
        store,
        old_rollout_id,
        old_recorder.rollout_path(),
    )
    .await?;

    let expected_sqlite_path = state_db
        .get_thread(thread_id)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to read thread metadata for {thread_id}: {err}"),
        })?
        .ok_or(ThreadStoreError::ThreadNotFound { thread_id })?
        .rollout_path;
    let (rollout_id, replacement, model_context) =
        create_reverted_recorder(store, thread_id, before_turn_id, Some(&metadata)).await?;
    let replacement_path = replacement.rollout_path().to_path_buf();
    replacement.persist().await.map_err(thread_store_io_error)?;
    super::thread_history_materialization::materialize_to_sqlite(
        store,
        rollout_id,
        replacement_path.as_path(),
    )
    .await?;

    let replaced = state_db
        .replace_rollout_path_if_current(
            thread_id,
            expected_sqlite_path.as_path(),
            replacement_path.as_path(),
        )
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to switch thread {thread_id} to reverted rollout: {err}"),
        })?;
    if !replaced {
        replacement
            .shutdown()
            .await
            .map_err(thread_store_io_error)?;
        let _ = tokio::fs::remove_file(replacement_path.as_path()).await;
        return Err(ThreadStoreError::Conflict {
            message: format!("thread {thread_id} changed while it was being reverted"),
        });
    }

    let old_entry = {
        let mut live_recorders = store.live_recorders.lock().await;
        let Some(old_entry) = live_recorders.remove(&thread_id) else {
            return Err(ThreadStoreError::Internal {
                message: format!(
                    "live writer for {thread_id} disappeared while its coordination lock was held"
                ),
            });
        };
        live_recorders.insert(
            thread_id,
            super::LiveRecorderEntry {
                recorder: replacement,
                rollout_id,
                history_mode: ThreadHistoryMode::Paginated,
                writer_lock: old_entry.writer_lock,
            },
        );
        old_entry.recorder
    };
    if let Err(err) = old_entry.shutdown().await {
        tracing::warn!("failed to close superseded rollout writer for {thread_id}: {err}");
    }
    Ok(RevertedLiveThread {
        rollout_path: replacement_path,
        model_context,
    })
}

async fn create_reverted_recorder(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    before_turn_id: String,
    metadata: Option<&ThreadPersistenceMetadata>,
) -> ThreadStoreResult<(ThreadId, RolloutRecorder, StoredModelContext)> {
    let current_rollout = thread_rollout_resolver::resolve_current(store, thread_id)
        .await?
        .ok_or(ThreadStoreError::ThreadNotFound { thread_id })?;
    let source_path = current_rollout.path;
    let source_meta = codex_rollout::read_session_meta_line(source_path.as_path())
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!(
                "failed to read current paginated rollout {}: {err}",
                source_path.display()
            ),
        })?
        .meta;
    if source_meta.id != current_rollout.thread_id {
        return Err(ThreadStoreError::InvalidRequest {
            message: format!("current rollout for {thread_id} belongs to another thread"),
        });
    }
    if source_meta.history_mode != ThreadHistoryMode::Paginated {
        return Err(ThreadStoreError::InvalidRequest {
            message: format!("thread {thread_id} does not use paginated history"),
        });
    }

    // HistoryPosition stores byte offsets in plain JSONL, so materialize any compressed
    // immutable lineage before deriving the retained boundary.
    let mut lineage = store.resolve_rollout_lineage(thread_id).await?;
    for segment in &mut lineage.segments {
        segment.rollout_path =
            codex_rollout::materialize_rollout_for_reference(segment.rollout_path.as_path())
                .await
                .map_err(|err| ThreadStoreError::Internal {
                    message: format!(
                        "failed to materialize rollout {} for revert: {err}",
                        segment.rollout_path.display()
                    ),
                })?;
        super::thread_history_materialization::materialize_to_sqlite(
            store,
            segment.rollout_id(),
            segment.rollout_path.as_path(),
        )
        .await?;
    }
    let history_base = paginated_fork::history_base_at_boundary(
        store,
        thread_id,
        ForkBoundary::BeforeTurn(before_turn_id),
        &lineage,
    )
    .await?;
    let model_context = StoredModelContext {
        thread_id,
        items: super::model_context::load_for_fork(lineage, history_base).await?,
    };

    let rollout_id = ThreadId::new();
    let recorder =
        create_replacement_recorder(store, source_meta, rollout_id, history_base, metadata).await?;
    Ok((rollout_id, recorder, model_context))
}

async fn create_replacement_recorder(
    store: &LocalThreadStore,
    source_meta: codex_rollout::SessionMeta,
    rollout_id: ThreadId,
    history_base: Option<codex_protocol::protocol::HistoryPosition>,
    metadata: Option<&ThreadPersistenceMetadata>,
) -> ThreadStoreResult<RolloutRecorder> {
    let config = RolloutConfig {
        codex_home: store.config.codex_home.clone(),
        sqlite: store.config.sqlite.clone(),
        cwd: metadata
            .and_then(|metadata| metadata.cwd.clone())
            .unwrap_or_else(|| source_meta.cwd.clone()),
        model_provider_id: metadata.map_or_else(
            || {
                source_meta
                    .model_provider
                    .clone()
                    .unwrap_or_else(|| store.config.default_model_provider_id.clone())
            },
            |metadata| metadata.model_provider.clone(),
        ),
        generate_memories: metadata.map_or_else(
            || source_meta.memory_mode.as_deref() != Some("disabled"),
            |metadata| {
                matches!(
                    metadata.memory_mode,
                    codex_protocol::protocol::ThreadMemoryMode::Enabled
                )
            },
        ),
    };
    let mut params = RolloutRecorderParams::new(
        source_meta.id,
        source_meta.forked_from_id,
        source_meta.parent_thread_id,
        source_meta.source,
        source_meta.thread_source,
        source_meta.originator,
        source_meta.base_instructions.unwrap_or_default(),
        source_meta.dynamic_tools.unwrap_or_default(),
    )
    .with_session_id(source_meta.session_id)
    .with_rollout_id(rollout_id)
    .with_selected_capability_roots(source_meta.selected_capability_roots)
    .with_multi_agent_version(source_meta.multi_agent_version)
    .with_history_mode(ThreadHistoryMode::Paginated)
    .with_history_base(history_base)
    .with_subagent_history_start_ordinal(source_meta.subagent_history_start_ordinal);
    if let Some(context_window) = source_meta.context_window {
        params = params.with_initial_window_id(context_window.window_id);
    }
    RolloutRecorder::new(&config, params)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to create reverted rollout: {err}"),
        })
}

fn thread_store_io_error(err: std::io::Error) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: err.to_string(),
    }
}

#[cfg(test)]
#[path = "revert_thread_tests.rs"]
mod tests;
