use super::*;
use pretty_assertions::assert_eq;

fn is_user_turn_event(event: &AppEvent) -> bool {
    matches!(
        event,
        AppEvent::CodexOp(Op::UserTurn { .. })
            | AppEvent::SubmitThreadOp {
                op: Op::UserTurn { .. },
                ..
            }
    )
}

#[tokio::test]
async fn active_writer_resume_replays_read_only_and_can_fork() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let config = app.chat_widget.config_ref().clone();
    let filename_timestamp = "2025-01-05T12-00-00";
    let thread_id = app_test_support::create_fake_rollout(
        config.codex_home.as_path(),
        filename_timestamp,
        "2025-01-05T12:00:00Z",
        "saved active-writer prompt",
        Some(&config.model_provider_id),
        /*git_info*/ None,
    )
    .expect("materialized rollout should be created");
    let thread_id = ThreadId::from_string(&thread_id)?;
    let mut writer = crate::start_embedded_app_server_for_picker(&config).await?;
    writer
        .resume_thread(
            config.clone(),
            thread_id,
            crate::app_server_session::ResumeModelSettings::OverrideFromCurrentConfig,
        )
        .await?;
    std::fs::write(
        app.config.codex_home.join("config.toml"),
        "[tui]\nresume_cwd = \"current\"\n",
    )?;
    app.harness_overrides.model = Some("gpt-5".to_string());
    let mut reader = crate::start_embedded_app_server_for_picker(&config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;

    app.chat_widget
        .restore_user_message_to_composer(format!("/resume {thread_id}").into());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let resume_event = std::iter::from_fn(|| app_event_rx.try_recv().ok())
        .find(|event| matches!(event, AppEvent::ResumeSessionByIdOrName(_)))
        .expect("inline /resume should dispatch an app event");
    let control = Box::pin(app.handle_event(&mut tui, &mut reader, resume_event)).await?;

    assert!(matches!(control, AppRunControl::Continue));
    assert_eq!(app.chat_widget.thread_id(), Some(thread_id));
    assert_eq!(app.chat_widget.current_model(), "gpt-5");
    assert_eq!(app.config.model.as_deref(), Some("gpt-5"));
    assert_eq!(
        app.thread_event_channels
            .get(&thread_id)
            .map(ThreadEventChannel::attachment),
        Some(ThreadEventAttachment::ReplayOnly)
    );

    let events = std::iter::from_fn(|| app_event_rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(events.iter().any(|event| {
        matches!(event, AppEvent::InsertHistoryCell(cell)
            if lines_to_single_string(&cell.display_lines(/*width*/ 100))
                .contains("saved active-writer prompt"))
    }));
    let banner = events
        .iter()
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => {
                let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 120));
                rendered
                    .contains("open in another Codex client")
                    .then_some(rendered)
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
        .replace(&thread_id.to_string(), "[thread-id]");
    assert_app_snapshot!("active_writer_read_only_banner", banner);

    app.chat_widget
        .restore_user_message_to_composer("blocked direct input".into());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let blocked_events = std::iter::from_fn(|| app_event_rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        blocked_events
            .iter()
            .all(|event| !is_user_turn_event(event)),
        "read-only input must not submit"
    );
    assert!(blocked_events.iter().any(|event| {
        matches!(event, AppEvent::InsertHistoryCell(cell)
            if lines_to_single_string(&cell.display_lines(/*width*/ 100))
                .contains("Direct input is disabled"))
    }));

    let control = Box::pin(app.handle_event(
        &mut tui,
        &mut reader,
        AppEvent::ForkCurrentSession { name: None },
    ))
    .await?;

    assert!(matches!(control, AppRunControl::Continue));
    assert_ne!(app.chat_widget.thread_id(), Some(thread_id));
    reader.shutdown().await?;
    writer.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn active_writer_fork_submits_preserved_startup_prompt() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let config = app.chat_widget.config_ref().clone();
    let filename_timestamp = "2025-01-06T12-00-00";
    let thread_id = app_test_support::create_fake_rollout(
        config.codex_home.as_path(),
        filename_timestamp,
        "2025-01-06T12:00:00Z",
        "saved prompt",
        Some(&config.model_provider_id),
        /*git_info*/ None,
    )
    .expect("materialized rollout should be created");
    let rollout_path =
        app_test_support::rollout_path(config.codex_home.as_path(), filename_timestamp, &thread_id);
    let thread_id = ThreadId::from_string(&thread_id)?;
    let mut writer = crate::start_embedded_app_server_for_picker(&config).await?;
    writer
        .resume_thread(
            config.clone(),
            thread_id,
            crate::app_server_session::ResumeModelSettings::OverrideFromCurrentConfig,
        )
        .await?;
    let mut reader = crate::start_embedded_app_server_for_picker(&config).await?;
    let pending_prompt = "continue in the fork".to_string();
    let model = get_model_offline_for_tests(config.model.as_deref());
    app.chat_widget = ChatWidget::new_with_app_event(ChatWidgetInit {
        config: config.clone(),
        frame_requester: crate::tui::FrameRequester::test_dummy(),
        app_event_tx: app.app_event_tx.clone(),
        workspace_command_runner: None,
        initial_user_message: create_initial_user_message(
            Some(pending_prompt.clone()),
            Vec::new(),
            Vec::new(),
        ),
        enhanced_keys_supported: false,
        has_chatgpt_account: false,
        has_codex_backend_auth: false,
        model_catalog: app.model_catalog.clone(),
        feedback: codex_feedback::CodexFeedback::new(),
        is_first_run: false,
        status_account_display: None,
        runtime_model_provider_base_url: None,
        initial_plan_type: None,
        model: Some(model.clone()),
        startup_tooltip_override: None,
        status_line_invalid_items_warned: app.status_line_invalid_items_warned.clone(),
        terminal_title_invalid_items_warned: app.terminal_title_invalid_items_warned.clone(),
        session_telemetry: app.session_telemetry.clone(),
    });
    let initial_session = startup_session::resume_or_read_only(
        &mut reader,
        config,
        &crate::resume_picker::SessionTarget {
            path: Some(rollout_path),
            thread_id,
            history_mode: None,
        },
        crate::app_server_session::ResumeModelSettings::OverrideFromCurrentConfig,
        model,
    )
    .await?;
    let startup_session::InitialSession::ReadOnly(read_only) = initial_session else {
        panic!("active writer should fall back to read-only thread replay");
    };
    app.attach_read_only_session(read_only).await?;
    let before_fork = std::iter::from_fn(|| app_event_rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(before_fork.iter().all(|event| !is_user_turn_event(event)));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let control = Box::pin(app.handle_event(
        &mut tui,
        &mut reader,
        AppEvent::ForkCurrentSession { name: None },
    ))
    .await?;

    assert!(matches!(control, AppRunControl::Continue));
    let forked_thread_id = app
        .chat_widget
        .thread_id()
        .expect("fork should attach a writable thread");
    assert_ne!(forked_thread_id, thread_id);
    let submitted_items =
        std::iter::from_fn(|| app_event_rx.try_recv().ok()).find_map(|event| match event {
            AppEvent::SubmitThreadOp {
                thread_id,
                op: Op::UserTurn { items, .. },
            } if thread_id == forked_thread_id => Some(items),
            AppEvent::CodexOp(Op::UserTurn { items, .. }) => Some(items),
            _ => None,
        });
    assert_eq!(
        submitted_items,
        Some(vec![UserInput::Text {
            text: pending_prompt,
            text_elements: Vec::new(),
        }])
    );
    reader.shutdown().await?;
    writer.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn replay_only_thread_stays_read_only_after_thread_switch() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let read_only_thread_id = ThreadId::new();
    let writable_thread_id = ThreadId::new();
    let mut read_only_channel = ThreadEventChannel::new_with_session(
        /*capacity*/ 4,
        test_thread_session(read_only_thread_id, test_path_buf("/tmp/read-only")),
        Vec::new(),
    );
    read_only_channel.mark_replay_only();
    app.thread_event_channels
        .insert(read_only_thread_id, read_only_channel);
    app.thread_event_channels.insert(
        writable_thread_id,
        ThreadEventChannel::new_with_session(
            /*capacity*/ 4,
            test_thread_session(writable_thread_id, test_path_buf("/tmp/writable")),
            Vec::new(),
        ),
    );
    app.agent_navigation.upsert(
        read_only_thread_id,
        Some("read-only".to_string()),
        Some("worker".to_string()),
        /*is_closed*/ false,
    );
    app.agent_navigation.upsert(
        writable_thread_id,
        Some("writable".to_string()),
        Some("worker".to_string()),
        /*is_closed*/ false,
    );
    app.side_threads.insert(
        read_only_thread_id,
        crate::app::side::SideThreadState::new(writable_thread_id),
    );
    app.side_threads.insert(
        writable_thread_id,
        crate::app::side::SideThreadState::new(read_only_thread_id),
    );
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;

    app.select_agent_thread(&mut tui, &mut app_server, read_only_thread_id)
        .await?;
    app.chat_widget.set_read_only_thread(
        crate::app_server_session::ResumeModelSettings::OverrideFromCurrentConfig,
    );
    app.select_agent_thread(&mut tui, &mut app_server, writable_thread_id)
        .await?;
    app.select_agent_thread(&mut tui, &mut app_server, read_only_thread_id)
        .await?;
    assert_eq!(
        app.chat_widget.fork_model_settings(),
        crate::app_server_session::ResumeModelSettings::OverrideFromCurrentConfig
    );
    app.chat_widget
        .restore_user_message_to_composer("must stay blocked".into());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(
        std::iter::from_fn(|| app_event_rx.try_recv().ok())
            .all(|event| !is_user_turn_event(&event)),
        "replay-only thread must stay read-only"
    );
    app_server.shutdown().await?;
    Ok(())
}
