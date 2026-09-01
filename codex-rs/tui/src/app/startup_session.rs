use super::App;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::AppServerStartedThread;
use crate::app_server_session::HistoryHydrationScope;
use crate::app_server_session::ResumeModelSettings;
use crate::app_server_session::is_active_writer_resume_error;
use crate::legacy_core::config::Config;
use crate::resume_picker::SessionTarget;
use codex_app_server_protocol::Thread;
use codex_protocol::ThreadId;
use color_eyre::Result;
use color_eyre::eyre::WrapErr;

pub(super) enum InitialSession {
    Live(AppServerStartedThread),
    ReadOnly(ReadOnlySession),
}

pub(super) struct ReadOnlySession {
    thread: Thread,
    requested_model: String,
    model_settings: ResumeModelSettings,
}

pub(super) async fn resume_or_read_only(
    app_server: &mut AppServerSession,
    config: Config,
    target_session: &SessionTarget,
    model_settings: ResumeModelSettings,
    fallback_model: String,
) -> Result<InitialSession> {
    match app_server
        .resume_thread(config.clone(), target_session.thread_id, model_settings)
        .await
    {
        Ok(started) => Ok(InitialSession::Live(started)),
        Err(err) if is_active_writer_resume_error(&err, target_session.thread_id) => {
            let mut thread = app_server
                .thread_read(target_session.thread_id, /*include_turns*/ false)
                .await
                .wrap_err("failed to read session held by another writer")?;
            app_server
                .hydrate_initial_thread_history(
                    &mut thread,
                    /*turn_cursor*/ None,
                    /*item_cursor*/ None,
                    Some(&config),
                    HistoryHydrationScope::Initial,
                )
                .await
                .wrap_err("failed to hydrate read-only session history")?;
            let requested_model = config.model.clone().unwrap_or(fallback_model);
            Ok(InitialSession::ReadOnly(ReadOnlySession {
                thread,
                requested_model,
                model_settings,
            }))
        }
        Err(err) => Err(err),
    }
}

impl App {
    pub(super) async fn attach_read_only_session(
        &mut self,
        read_only: ReadOnlySession,
    ) -> Result<()> {
        let ReadOnlySession {
            mut thread,
            requested_model,
            model_settings,
        } = read_only;
        let thread_id =
            ThreadId::from_string(&thread.id).wrap_err("invalid read-only thread id")?;
        let mut session = self.session_state_for_thread_read(thread_id, &thread).await;
        if model_settings == ResumeModelSettings::OverrideFromCurrentConfig
            || session.model.is_empty()
        {
            session.model = requested_model;
        }
        let turns = std::mem::take(&mut thread.turns);
        self.chat_widget.set_read_only_thread(model_settings);
        self.enqueue_primary_thread_session(session, turns).await?;
        if let Some(channel) = self.thread_event_channels.get_mut(&thread_id) {
            channel.mark_replay_only();
        }
        self.chat_widget.add_info_message(
            format!(
                "Thread {thread_id} is open in another Codex client. Replaying its saved transcript in read-only mode; use /fork to continue in a new thread."
            ),
            /*hint*/ None,
        );
        Ok(())
    }

    pub(super) async fn replace_with_read_only_session(
        &mut self,
        tui: &mut crate::tui::Tui,
        read_only: ReadOnlySession,
    ) -> Result<()> {
        self.reset_thread_event_state();
        let init = self.chatwidget_init_for_forked_or_resumed_thread(
            tui,
            self.config.clone(),
            /*initial_user_message*/ None,
        );
        self.replace_chat_widget(crate::chatwidget::ChatWidget::new_with_app_event(init));
        self.attach_read_only_session(read_only).await
    }
}
