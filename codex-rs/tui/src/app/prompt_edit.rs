//! Editing earlier prompts by overwriting the active thread or creating a branch.

use super::session_lifecycle::ThreadAttachPresentation;
use super::*;
use crate::app_server_session::ForkGoalContinuation;
use crate::chatwidget::UserMessage;
use codex_config::types::PromptEditMode;

impl App {
    pub(super) async fn edit_earlier_prompt(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        nth_user_message: usize,
        mut prompt: UserMessage,
    ) {
        if self.chat_widget.thread_id() != Some(thread_id) {
            return;
        }

        self.refresh_in_memory_config_from_disk_best_effort("editing an earlier prompt")
            .await;
        let config = self.fresh_session_config();
        let turns = match self.prompt_edit_turns(thread_id).await {
            Some(turns) => turns,
            None => {
                self.restore_backtrack_prompt_after_edit_error(
                    prompt,
                    "the selected thread is no longer available for prompt editing",
                );
                return;
            }
        };
        let target = match crate::app_backtrack::resolve_prompt_edit_target(
            &turns,
            nth_user_message,
            &mut prompt,
        ) {
            Ok(target) => target,
            Err(err) => {
                self.restore_backtrack_prompt_after_edit_error(prompt, err);
                return;
            }
        };

        match config.tui_prompt_edit_mode {
            PromptEditMode::Overwrite => {
                let num_turns = turns.len().saturating_sub(target.selected_turn_index);
                match app_server
                    .overwrite_thread_at(&config, thread_id, target.before_turn_id, num_turns)
                    .await
                {
                    Ok(thread) => {
                        if let Some(channel) = self.thread_event_channels.get(&thread_id) {
                            let mut store = channel.store.lock().await;
                            store.set_turns(thread.turns);
                            store.rebase_buffer_after_session_refresh();
                        }
                        self.apply_prompt_edit_overwrite(nth_user_message, prompt);
                    }
                    Err(err) => self.restore_backtrack_prompt_after_edit_error(prompt, err),
                }
            }
            PromptEditMode::Branch => {
                self.session_telemetry.counter(
                    "codex.thread.fork",
                    /*inc*/ 1,
                    &[("source", "transcript")],
                );
                let started = if target.selected_turn_index == 0
                    && !app_server.has_older_history(thread_id)
                {
                    app_server
                        .start_thread_with_session_start_source(
                            &config, /*session_start_source*/ None,
                            /*remote_cwd_override*/ None,
                        )
                        .await
                } else {
                    app_server
                        .fork_thread_at(
                            config,
                            thread_id,
                            /*last_turn_id*/ None,
                            Some(target.before_turn_id),
                            ForkGoalContinuation::StartIfIdle,
                        )
                        .await
                };
                match started {
                    Ok(forked) => {
                        self.shutdown_current_thread(app_server).await;
                        if let Err(err) = self
                            .replace_chat_widget_with_app_server_thread(
                                tui,
                                forked,
                                ThreadAttachPresentation::PromptEdit,
                                /*initial_user_message*/ None,
                            )
                            .await
                        {
                            self.restore_backtrack_prompt_after_edit_error(prompt, err);
                        } else {
                            self.chat_widget.restore_user_message_to_composer(prompt);
                        }
                    }
                    Err(err) => self.restore_backtrack_prompt_after_edit_error(prompt, err),
                }
            }
        }
    }

    async fn prompt_edit_turns(&self, thread_id: ThreadId) -> Option<Vec<Turn>> {
        let channel = self.thread_event_channels.get(&thread_id)?;
        let store = channel.store.lock().await;
        let mut turns = store.turns.clone();
        // Loaded snapshots can lag live notifications. Reconstruct only the item types used by
        // the prompt projection so target resolution sees the same history as the transcript.
        for event in &store.buffer {
            let ThreadBufferedEvent::Notification(notification) = event else {
                continue;
            };
            match notification.as_ref() {
                ServerNotification::TurnStarted(notification)
                    if !turns.iter().any(|turn| turn.id == notification.turn.id) =>
                {
                    turns.push(notification.turn.clone());
                }
                ServerNotification::ItemCompleted(notification) => {
                    if matches!(
                        notification.item,
                        ThreadItem::UserMessage { .. }
                            | ThreadItem::EnteredReviewMode { .. }
                            | ThreadItem::ExitedReviewMode { .. }
                    ) && let Some(turn) = turns
                        .iter_mut()
                        .find(|turn| turn.id == notification.turn_id)
                        && !turn
                            .items
                            .iter()
                            .any(|item| item.id() == notification.item.id())
                    {
                        turn.items.push(notification.item.clone());
                    }
                }
                ServerNotification::TurnCompleted(notification) => {
                    if let Some(turn) = turns
                        .iter_mut()
                        .find(|turn| turn.id == notification.turn.id)
                    {
                        turn.status = notification.turn.status.clone();
                        turn.error = notification.turn.error.clone();
                        turn.started_at = notification.turn.started_at;
                        turn.completed_at = notification.turn.completed_at;
                        turn.duration_ms = notification.turn.duration_ms;
                    }
                }
                _ => {}
            }
        }
        Some(turns)
    }
}
