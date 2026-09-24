use super::App;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::HistoryHydrationScope;
use crate::app_server_session::ResumeModelSettings;
use crate::app_server_session::is_active_writer_error;
use crate::legacy_core::config::Config;
use crate::local_settings::LocalSettings;
use crate::resume_picker::SessionTarget;
use codex_app_server_protocol::Thread;
use codex_protocol::ThreadId;
use color_eyre::Result;
use color_eyre::eyre::WrapErr;

pub(super) struct ReadOnlySession {
    thread: Thread,
    requested_model: String,
    model_settings: ResumeModelSettings,
}

pub(super) async fn read_only_session_for_active_writer(
    app_server: &mut AppServerSession,
    config: Config,
    target_session: &SessionTarget,
    model_settings: ResumeModelSettings,
    fallback_model: String,
) -> Result<ReadOnlySession> {
    match app_server
        .resume_thread(
            &LocalSettings::from(&config),
            config.clone(),
            target_session.thread_id,
            model_settings,
        )
        .await
    {
        Ok(_) => color_eyre::eyre::bail!(
            "expected thread {} to have an active writer",
            target_session.thread_id
        ),
        Err(err) if is_active_writer_error(&err) => {
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
                    Some(&LocalSettings::from(&config)),
                    HistoryHydrationScope::Initial,
                )
                .await
                .wrap_err("failed to hydrate read-only session history")?;
            let requested_model = config.model.clone().unwrap_or(fallback_model);
            Ok(ReadOnlySession {
                thread,
                requested_model,
                model_settings,
            })
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
            super::active_writer_read_only_message(thread_id),
            /*hint*/ None,
        );
        Ok(())
    }
}
