mod cmd_picker;
mod event_handler;
mod events;
pub(crate) mod input;
mod markdown;
mod permission_handler;
pub(crate) mod picker;
pub(crate) mod renderer;
mod slash;
mod status;
mod terminal;
pub(crate) mod utils;

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event;
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use crossterm::style::Color;
use tokio::sync::mpsc;

use crate::cli::Cli;
use crate::config::Config;
use crate::context::ContextFiles;
use crate::event::{AgentEvent, UserEvent};
#[cfg(feature = "mcp")]
use crate::extras::mcp::McpClientManager;
use crate::permission::ask::{AskReceiver, AskSender};
use crate::permission::checker::PermCheck;
use crate::provider::{AnyAgent, AnyClient};
use crate::sandbox::Sandbox;
use crate::session::{MessageRole, Session};
use crate::ui::event_handler::{ensure_agent, handle_agent_event};
use crate::ui::events::{render_session, sanitize_output};
use crate::ui::input::InputEditor;
use crate::ui::permission_handler::handle_permission_request;
use crate::ui::renderer::{Renderer, copy_to_clipboard};
use crate::ui::slash::{handle_compress, handle_slash};
use crate::ui::status::StatusLine;
use crate::ui::terminal::TerminalGuard;

use self::utils::parse_color;

pub(super) const C_AGENT: Color = Color::White;
pub(super) const C_ERROR: Color = Color::Red;
pub(super) const C_TOOL: Color = Color::Yellow;
pub(super) const C_PERM: Color = Color::Magenta;

fn refresh_display(
    renderer: &mut Renderer,
    input: &InputEditor,
    session: &Session,
    is_running: bool,
    loop_label: Option<&str>,
    prompt_name: Option<&str>,
    perm_mode: Option<&str>,
) -> io::Result<()> {
    renderer.render_viewport()?;
    let status = StatusLine::render(session, is_running, 0, loop_label, prompt_name, perm_mode);
    renderer.draw_bottom(&input.buffer, input.cursor, &status, is_running)?;
    if let Some(ref picker) = input.picker {
        picker.draw()?;
    }
    Ok(())
}

fn spawn_event_thread(
    user_tx: mpsc::Sender<UserEvent>,
    running: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            if let Ok(true) = event::poll(Duration::from_millis(50)) {
                match event::read() {
                    Ok(event::Event::Key(key)) => {
                        if key.kind == KeyEventKind::Press
                            && user_tx.blocking_send(UserEvent::Key(key)).is_err()
                        {
                            break;
                        }
                    }
                    Ok(event::Event::Mouse(m)) => match m.kind {
                        MouseEventKind::ScrollUp => {
                            if user_tx.blocking_send(UserEvent::ScrollUp).is_err() {
                                break;
                            }
                        }
                        MouseEventKind::ScrollDown => {
                            if user_tx.blocking_send(UserEvent::ScrollDown).is_err() {
                                break;
                            }
                        }
                        MouseEventKind::Down(MouseButton::Left) => {
                            let _ = user_tx.blocking_send(UserEvent::MouseDown {
                                row: m.row,
                                col: m.column,
                            });
                        }
                        MouseEventKind::Drag(MouseButton::Left) => {
                            let _ = user_tx.blocking_send(UserEvent::MouseDrag {
                                row: m.row,
                                col: m.column,
                            });
                        }
                        MouseEventKind::Up(MouseButton::Left) => {
                            let _ = user_tx.blocking_send(UserEvent::MouseUp {
                                row: m.row,
                                col: m.column,
                            });
                        }
                        _ => {}
                    },
                    Ok(event::Event::Resize(cols, rows)) => {
                        let _ = user_tx.blocking_send(UserEvent::Resize(cols, rows));
                    }
                    Ok(event::Event::Paste(data)) => {
                        let _ = user_tx.blocking_send(UserEvent::Paste(data));
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
        }
    })
}

/// Lazily initialise the MCP client manager (connects only on first use).
#[cfg(feature = "mcp")]
async fn ensure_mcp_manager<'a>(
    mcp: &'a mut Option<McpClientManager>,
    cfg: &'a Config,
) -> Option<&'a McpClientManager> {
    if mcp.is_none()
        && let Some(servers) = &cfg.mcp_servers
    {
        *mcp = Some(McpClientManager::connect_all(servers).await);
    }
    mcp.as_ref()
}

#[allow(clippy::too_many_arguments)]
pub async fn run_interactive(
    mut client: AnyClient,
    mut agent: Option<AnyAgent>,
    cli: &Cli,
    cfg: &Config,
    session: &mut Session,
    context: &mut ContextFiles,
    permission: Option<PermCheck>,
    ask_tx: Option<AskSender>,
    mut ask_rx: Option<AskReceiver>,
    sandbox: Sandbox,
) -> anyhow::Result<()> {
    let _guard = TerminalGuard::new()?;

    #[cfg(feature = "mcp")]
    let mut mcp_manager: Option<McpClientManager> = None;

    let mut renderer = Renderer::new()?;
    renderer.set_monochrome(cli.no_color);
    if let Some(ref theme_name) = context.current_theme_name {
        if let Some(content) = context.themes.get(theme_name.as_str()) {
            crate::context::themes::apply(content, &mut renderer);
        }
    } else if let Some(colors) = &cfg.colors {
        let chat_bg = colors.chat_background.as_deref().and_then(parse_color);
        let input_bg = colors.input_background.as_deref().and_then(parse_color);
        let status_bg = colors.status_background.as_deref().and_then(parse_color);
        renderer.set_background_colors(chat_bg, input_bg, status_bg);
    }
    let mut input = InputEditor::new();
    input.set_monochrome(cli.no_color);
    input.set_prompt_names(context.prompts.keys().cloned().collect());
    input.set_theme_names(context.themes.keys().cloned().collect());
    if let Some(editor) = &cfg.editor {
        input.set_editor(editor.clone());
    }
    input.set_quick_model_names(
        cfg.quick_models
            .as_ref()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default(),
    );
    input.load_global_history();
    let mut is_running = false;
    let mut agent_rx: Option<mpsc::Receiver<AgentEvent>> = None;
    let mut agent_line_started = false;
    let mut response_buf = String::new();
    let mut response_start_line: Option<usize> = None;
    let mut show_reasoning = true;
    let mut reasoning_enabled = true;
    let mut was_reasoning = false;
    let mut todo_tools_enabled = false;
    #[allow(unused_mut)]
    let mut loop_label: Option<String> = None;
    #[cfg(feature = "loop")]
    let mut loop_state: Option<crate::extras::r#loop::LoopState> = None;
    #[cfg(feature = "git-worktree")]
    let mut wt_return_path: Option<String> = None;

    let perm_mode = || -> Option<String> {
        permission.as_ref().map(|p| {
            p.lock()
                .unwrap_or_else(|e| e.into_inner())
                .mode()
                .to_string()
        })
    };

    render_session(&mut renderer, session, cli, cfg, context)?;
    refresh_display(
        &mut renderer,
        &input,
        session,
        false,
        None,
        context.current_prompt_name.as_deref(),
        perm_mode().as_deref(),
    )?;

    #[cfg(feature = "git-worktree")]
    if let Some(name) = &cli.worktree {
        match crate::extras::git_worktree::create(name) {
            Ok((path, _info)) => {
                std::env::set_current_dir(&path).ok();
                session.working_dir = compact_str::CompactString::new(path.to_string_lossy());
                context.reload();
                #[cfg(feature = "mcp")]
                let mcp_ref = ensure_mcp_manager(&mut mcp_manager, cfg).await;
                let model = client.completion_model(session.model.to_string());
                agent = Some(
                    crate::provider::build_agent(
                        model,
                        cli,
                        cfg,
                        context,
                        permission.clone(),
                        ask_tx.clone(),
                        sandbox.clone(),
                        reasoning_enabled,
                        #[cfg(feature = "mcp")]
                        mcp_ref,
                    )
                    .await,
                );
                let _ = render_session(&mut renderer, session, cli, cfg, context);
            }
            Err(e) => {
                let _ = renderer.write_line(&format!("worktree failed: {}", e), C_ERROR);
            }
        }
    }
    #[cfg(feature = "git-worktree")]
    if cli.parallel {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let name = ts.to_string();
        match crate::extras::git_worktree::create(&name) {
            Ok((path, _info)) => {
                std::env::set_current_dir(&path).ok();
                session.working_dir = compact_str::CompactString::new(path.to_string_lossy());
                context.reload();
                #[cfg(feature = "mcp")]
                let mcp_ref = ensure_mcp_manager(&mut mcp_manager, cfg).await;
                let model = client.completion_model(session.model.to_string());
                agent = Some(
                    crate::provider::build_agent(
                        model,
                        cli,
                        cfg,
                        context,
                        permission.clone(),
                        ask_tx.clone(),
                        sandbox.clone(),
                        reasoning_enabled,
                        #[cfg(feature = "mcp")]
                        mcp_ref,
                    )
                    .await,
                );
                let _ = render_session(&mut renderer, session, cli, cfg, context);
            }
            Err(e) => {
                let _ = renderer.write_line(&format!("worktree failed: {}", e), C_ERROR);
            }
        }
    }

    let (mut user_tx, mut user_rx) = mpsc::channel::<UserEvent>(64);
    let mut running = Arc::new(AtomicBool::new(true));
    let mut event_handle = Some(spawn_event_thread(user_tx.clone(), running.clone()));

    loop {
        tokio::select! {
            Some(ev) = user_rx.recv() => {
                match ev {
                    UserEvent::Resize(cols, rows) => {
                        let _ = (cols, rows);
                        renderer.resize();
                        refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                        continue;
                    }
                    UserEvent::ScrollUp => {
                        renderer.scroll_line_up();
                        refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                        continue;
                    }
                    UserEvent::ScrollDown => {
                        renderer.scroll_line_down();
                        refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                        continue;
                    }
                    UserEvent::MouseDown { row, col: _ } => {
                        if row < renderer.visible_lines() as u16
                            && let Some(idx) = renderer.buffer_line_at_row(row) {
                                renderer.selection_active = true;
                                renderer.selection_start = Some(idx);
                                renderer.selection_end = Some(idx);
                                refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                            }
                        continue;
                    }
                    UserEvent::MouseDrag { row, col: _ } => {
                        if renderer.selection_active
                            && let Some(idx) = renderer.buffer_line_at_row(row) {
                                renderer.selection_end = Some(idx);
                                refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                            }
                        continue;
                    }
                    UserEvent::MouseUp { row, col: _ } => {
                        if renderer.selection_active {
                            if let Some(idx) = renderer.buffer_line_at_row(row) {
                                renderer.selection_end = Some(idx);
                            }
                            if let Some(text) = renderer.selected_text() {
                                copy_to_clipboard(&text);
                            }
                            renderer.clear_selection();
                            refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                        }
                        continue;
                    }
                    UserEvent::Paste(data) => {
                        input.handle_paste(data);
                        refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                        continue;
                    }
                    UserEvent::Key(key) => {
                        let is_ctrl_c = key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL);
                        let is_ctrl_d = key.code == KeyCode::Char('d')
                            && key.modifiers.contains(KeyModifiers::CONTROL);
                        if is_ctrl_c || is_ctrl_d {
                            if is_running {
                                is_running = false;
                                agent_rx = None;
                                #[cfg(feature = "loop")]
                                if let Some(ref mut ls) = loop_state {
                                    ls.active = false;
                                    loop_label = None;
                                }
                                renderer.write_line("interrupted", C_ERROR)?;
                                refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                            } else {
                                break;
                            }
                            continue;
                        }

                        if renderer.selection_active && key.code == KeyCode::Char('y') {
                            if let Some(text) = renderer.selected_text() {
                                copy_to_clipboard(&text);
                                renderer.write_line("copied selection", Color::Green)?;
                            }
                            renderer.clear_selection();
                            refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                            continue;
                        }
                        if renderer.selection_active && key.code == KeyCode::Esc {
                            renderer.clear_selection();
                            refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                            continue;
                        }

                        let ctrl_r = key.code == KeyCode::Char('r')
                            && key.modifiers.contains(KeyModifiers::CONTROL);
                        if ctrl_r {
                            show_reasoning = !show_reasoning;
                            renderer.write_line(
                                &format!("reasoning visibility: {}", if show_reasoning { "on" } else { "off" }),
                                Color::White,
                            )?;
                            refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                            continue;
                        }

                        match key.code {
                            KeyCode::PageUp => {
                                renderer.scroll_page_up();
                                refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                                continue;
                            }
                            KeyCode::PageDown => {
                                renderer.scroll_page_down();
                                refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                                continue;
                            }
                            KeyCode::Home => {
                                renderer.scroll_to_top();
                                refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                                continue;
                            }
                            KeyCode::End => {
                                renderer.scroll_to_bottom()?;
                                refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                                continue;
                            }
                            _ => {}
                        }

                        if input.picker.as_ref().is_some_and(|p| p.active())
                            && input.handle_picker_key(key) {
                                refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                                continue;
                            }

                        if key.code == KeyCode::Char('g') && key.modifiers.contains(KeyModifiers::CONTROL) {
                            if let Some(h) = event_handle.take() {
                                running.store(false, Ordering::Relaxed);
                                let _ = h.join();
                            }
                            input.open_in_editor();
                            running = Arc::new(AtomicBool::new(true));
                            let (new_tx, new_rx) = mpsc::channel(64);
                            user_tx = new_tx;
                            user_rx = new_rx;
                            event_handle = Some(spawn_event_thread(user_tx.clone(), running.clone()));
                            refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                            continue;
                        }

                        if let Some(text) = input.handle_key(key) {
                            #[cfg(feature = "loop")]
                            if loop_state.as_ref().is_some_and(|ls| ls.active) && !text.starts_with('/') {
                                renderer.write_line("loop active: /loop stop to cancel", C_ERROR)?;
                                refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                                continue;
                            }
                            if renderer.is_scrolling() {
                                renderer.scroll_to_bottom()?;
                            }
                            if text.starts_with('/') {
                                for line in text.lines() {
                                    let safe_line = sanitize_output(line);
                                    renderer.write_line(&format!("> {}", safe_line), Color::Green)?;
                                }
                                renderer.write_line("", Color::White)?;
                                #[cfg(feature = "mcp")]
                                let mcp_ref = ensure_mcp_manager(&mut mcp_manager, cfg).await;
                                let result = handle_slash(&text, &mut agent, &mut client, &mut renderer, session, cli, cfg, context, &mut show_reasoning, &mut reasoning_enabled, &mut is_running, &mut input, &permission, &ask_tx, &mut todo_tools_enabled, &sandbox, #[cfg(feature = "loop")] &mut loop_state, #[cfg(feature = "mcp")] mcp_ref).await;
                                match result {
                                Err(e) if e.to_string().starts_with("DEFER_COMPRESS:") => {
                                    let err_msg = e.to_string();
                                    let instructions = err_msg.strip_prefix("DEFER_COMPRESS:").and_then(|s| {
                                        let s = s.trim();
                                        if s.is_empty() || s == "(none)" { None } else { Some(s.to_string()) }
                                    });
                                        #[cfg(feature = "mcp")]
                                        let mcp_ref = ensure_mcp_manager(&mut mcp_manager, cfg).await;
                                        let compress_result = handle_compress(
                                            instructions.as_deref(),
                                            &mut agent, &mut client, &mut renderer, session, cli, cfg, context,
                                            reasoning_enabled,
                                            &permission, &ask_tx, &sandbox,
                                            #[cfg(feature = "mcp")] mcp_ref,
                                        ).await;
                                        if let Err(e) = compress_result {
                                            renderer.write_line(&format!("compress error: {}", e), C_ERROR)?;
                                        }
                                        let _ = crate::session::storage::save_session(session);
                                    }
                                    #[cfg(feature = "git-worktree")]
                                    Err(e) if e.to_string().starts_with("DEFER_WT_MERGE:") => {
                                        let err_msg = e.to_string();
                                        let parts: Vec<&str> = err_msg.strip_prefix("DEFER_WT_MERGE:").unwrap_or("").splitn(5, ':').collect();
                                        if parts.len() == 5 {
                                            let branch = parts[0];
                                            let target = parts[1];
                                            let main_path = parts[2].to_string();
                                            let wt_path = parts[3];
                                            let _repo_name = parts[4];
                                            let prompt = format!(
                                                "I'm in a git worktree on branch '{}' at '{}'. \
                                                 Please merge branch '{}' into '{}' in the main repo at '{}', \
                                                 push the changes, and delete the worktree at '{}'. \
                                                 After merging, go back to the main repo at '{}'.",
                                                branch, wt_path, branch, target, main_path, wt_path, main_path
                                            );
                                            session.add_message(MessageRole::User, &prompt);
                                            let history = crate::agent::runner::convert_history(session);
                                            #[cfg(feature = "mcp")]
                                            let mcp_ref = ensure_mcp_manager(&mut mcp_manager, cfg).await;
                                            ensure_agent(
                                                &mut agent, &client, session, cli, cfg, context,
                                                &permission, &ask_tx, &sandbox, reasoning_enabled,
                                                #[cfg(feature = "mcp")] mcp_ref,
                                            ).await;
                                            let runner = agent.as_ref().unwrap().clone().spawn_runner(prompt, history);
                                            agent_rx = Some(runner.event_rx);
                                            is_running = true;
                                            wt_return_path = Some(main_path);
                                        }
                                    }
                                    #[cfg(feature = "git-worktree")]
                                    Err(e) if e.to_string().starts_with("DEFER_WT_EXIT:") => {
                                        let err_msg = e.to_string();
                                        let parts: Vec<&str> = err_msg.strip_prefix("DEFER_WT_EXIT:").unwrap_or("").splitn(2, ':').collect();
                                        if parts.len() == 2 {
                                            let main_path = parts[0];
                                            std::env::set_current_dir(main_path)
                                                .map_err(|e| anyhow::anyhow!("failed to change directory: {}", e))?;
                                            session.working_dir = compact_str::CompactString::new(main_path);
                                            context.reload();
                                            #[cfg(feature = "mcp")]
                                            let mcp_ref = ensure_mcp_manager(&mut mcp_manager, cfg).await;
                                            let model = client.completion_model(session.model.to_string());
                                            agent = Some(crate::provider::build_agent(
                                                model,
                                                cli,
                                                cfg,
                                                context,
                                                permission.clone(),
                                                ask_tx.clone(),
                                                sandbox.clone(),
                                                reasoning_enabled,
                                                #[cfg(feature = "mcp")] mcp_ref,
                                            ).await);
                                            render_session(&mut renderer, session, cli, cfg, context)?;
                                            renderer.write_line(
                                                &format!("returned to main repo at {}", main_path),
                                                C_AGENT,
                                            )?;
                                        }
                                    }
                                    Err(e) => {
                                        if e.downcast_ref::<std::io::Error>().is_some_and(|e: &std::io::Error| e.kind() == std::io::ErrorKind::Interrupted) {
                                            break;
                                        }
                                        renderer.write_line(&format!("error: {}", e), C_ERROR)?;
                                    }
                                    Ok(_) => {
                                        if !cli.no_session
                                            && let Err(e) = crate::session::storage::save_session(session)
                                        {
                                            renderer.write_line(
                                                &format!("warning: failed to save session: {}", e),
                                                C_ERROR,
                                            )?;
                                        }
                                        #[cfg(feature = "loop")]
                                        if let Some(ref mut ls) = loop_state
                                            && ls.active && ls.iteration == 0 && !is_running
                                        {
                                            ls.iteration = 1;
                                            let prompt = ls.build_prompt();
                                            #[cfg(feature = "mcp")]
                                            let mcp_ref = ensure_mcp_manager(&mut mcp_manager, cfg).await;
                                            ensure_agent(
                                                &mut agent, &client, session, cli, cfg, context,
                                                &permission, &ask_tx, &sandbox, reasoning_enabled,
                                                #[cfg(feature = "mcp")] mcp_ref,
                                            ).await;
                                            let runner = agent.as_ref().unwrap().clone().spawn_runner(prompt, Vec::new());
                                            agent_rx = Some(runner.event_rx);
                                            is_running = true;
                                            loop_label = Some(ls.iteration_label());
                                        }
                                    }
                                }
                                if !cli.no_session
                                    && let Err(e) = crate::session::storage::save_session(session)
                                {
                                    renderer.write_line(
                                        &format!("warning: failed to save session: {}", e),
                                        C_ERROR,
                                    )?;
                                }
                            } else {
                                for line in text.lines() {
                                    let safe_line = sanitize_output(line);
                                    renderer.write_line(&format!("> {}", safe_line), Color::Green)?;
                                }
                                renderer.write_line("", Color::White)?;

                                #[cfg(feature = "mcp")]
                                let mcp_ref = ensure_mcp_manager(&mut mcp_manager, cfg).await;
                                ensure_agent(
                                    &mut agent, &client, session, cli, cfg, context,
                                    &permission, &ask_tx, &sandbox, reasoning_enabled,
                                    #[cfg(feature = "mcp")] mcp_ref,
                                ).await;
                                let history = crate::agent::runner::convert_history(session);
                                let runner = agent.as_ref().unwrap().clone().spawn_runner(
                                    text.to_string(),
                                    history,
                                );
                                agent_rx = Some(runner.event_rx);
                                is_running = true;

                                session.add_message(MessageRole::User, &text);
                                if !cli.no_session {
                                    let _ = crate::session::chat_history::append_entry(
                                        &crate::session::chat_history::ChatHistoryEntry {
                                            content: text.to_string(),
                                            timestamp: session.updated_at.clone(),
                                        },
                                    );
                                }
                            }
                            refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                        } else if is_running {
                            refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
                        } else {
                            let status = StatusLine::render(session, is_running, 0, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref());
                            renderer.draw_bottom(&input.buffer, input.cursor, &status, is_running)?;
                            if let Some(ref picker) = input.picker {
                                picker.draw()?;
                            }
                        }
                    }
                }
            }
            Some(event) = async {
                agent_rx.as_mut()?.recv().await
            } => {
                #[cfg(feature = "mcp")]
                let mcp_ref = ensure_mcp_manager(&mut mcp_manager, cfg).await;
                handle_agent_event(
                    event, &mut renderer, session, cfg, cli, context,
                    &mut is_running, &mut agent_rx, &mut agent_line_started,
                    &mut response_buf, &mut response_start_line, &mut was_reasoning,
                    show_reasoning,
                    &mut agent, &mut client, &mut loop_label,
                    &permission, &ask_tx, &sandbox,
                    #[cfg(feature = "loop")] &mut loop_state,
                    #[cfg(feature = "git-worktree")] &mut wt_return_path,
                    #[cfg(feature = "mcp")] mcp_ref,
                ).await?;
                refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
            }
            Some(ask_req) = async {
                ask_rx.as_mut()?.recv().await
            } => {
                handle_permission_request(
                    ask_req, &mut renderer, session, cli,
                    &mut user_rx, &mut agent_line_started, &mut was_reasoning,
                ).await?;
                refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)), if is_running => {
                refresh_display(&mut renderer, &input, session, is_running, loop_label.as_deref(), context.current_prompt_name.as_deref(), perm_mode().as_deref())?;
            }
            else => {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }
    }

    #[cfg(feature = "git-worktree")]
    if cli.resolve_wt_auto_merge(cfg)
        && let Some(info) = crate::extras::git_worktree::detect()
    {
        let target = crate::extras::git_worktree::default_branch(&info.main_repo_path)
            .unwrap_or_else(|| "main".to_string());
        let _ = crate::extras::git_worktree::merge(&info, &target);
    }

    #[cfg(feature = "mcp")]
    if let Some(mgr) = mcp_manager {
        mgr.shutdown().await;
    }

    Ok(())
}
