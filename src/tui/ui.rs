use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::Stylize;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table, Tabs};

use crate::player::queue::RepeatMode;

use super::app::{App, Tab};

/// Render the full TUI frame.
pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tabs
            Constraint::Min(5),    // content
            Constraint::Length(4), // now playing
            Constraint::Length(1), // help
        ])
        .split(f.area());

    render_tabs(f, chunks[0], app);
    render_content(f, chunks[1], app);
    render_player(f, chunks[2], app);
    render_help(f, chunks[3], app);
}

fn render_tabs(f: &mut Frame, area: Rect, app: &App) {
    let titles: Vec<&str> = Tab::ALL.iter().map(|t| t.label()).collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" SynoPlayer "))
        .select(app.active_tab.index())
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::Yellow).bold());
    f.render_widget(tabs, area);
}

fn render_content(f: &mut Frame, area: Rect, app: &mut App) {
    match app.active_tab {
        Tab::Library => render_songs_table(f, area, app),
        Tab::Folders => render_folders(f, area, app),
        Tab::Playlists => render_playlists(f, area, app),
        Tab::Queue => render_queue(f, area, app),
    }
}

fn render_songs_table(f: &mut Frame, area: Rect, app: &mut App) {
    let (title, items, state) = if let Some(ref mut detail) = app.playlist_detail {
        (
            format!(" {} ", detail.name),
            &detail.songs.items,
            &mut detail.songs.state,
        )
    } else {
        let count = app.songs.items.len();
        (
            format!(" Songs ({count}) "),
            &app.songs.items,
            &mut app.songs.state,
        )
    };
    let header = Row::new(vec!["Artist", "Title", "Album", "Duration"])
        .style(Style::default().fg(Color::DarkGray).bold())
        .bottom_margin(1);

    let playing_id = app
        .now_playing
        .as_ref()
        .map(|np| np.track.id.as_str())
        .unwrap_or("");

    let rows = items.iter().map(|song| {
        let (artist, title, album, dur) = extract_song_display(song);
        let style = if song.id == playing_id {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        let marker = if song.id == playing_id { "▶ " } else { "" };
        Row::new(vec![
            Cell::from(format!("{marker}{artist}")),
            Cell::from(title),
            Cell::from(album),
            Cell::from(dur),
        ])
        .style(style)
    });

    let widths = [
        Constraint::Percentage(28),
        Constraint::Percentage(35),
        Constraint::Percentage(25),
        Constraint::Percentage(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("► ");

    f.render_stateful_widget(table, area, state);
}

fn render_folders(f: &mut Frame, area: Rect, app: &mut App) {
    let header = Row::new(vec!["", "Name", "Artist", "Album", "Duration"])
        .style(Style::default().fg(Color::DarkGray).bold())
        .bottom_margin(1);

    let playing_id = app
        .now_playing
        .as_ref()
        .map(|np| np.track.id.as_str())
        .unwrap_or("");

    let rows = app.folders.items.iter().map(|item| {
        if item.is_dir {
            Row::new(vec![
                Cell::from("[DIR]"),
                Cell::from(item.title.clone()),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ])
            .style(Style::default().fg(Color::Yellow))
        } else {
            let song = item.to_song();
            let (artist, title, album, dur) = extract_song_display(&song);
            let is_playing = item.id == playing_id;
            let style = if is_playing {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            let marker = if is_playing { " ▶" } else { "" };
            Row::new(vec![
                Cell::from(format!("  {marker}")),
                Cell::from(title),
                Cell::from(artist),
                Cell::from(album),
                Cell::from(dur),
            ])
            .style(style)
        }
    });

    let widths = [
        Constraint::Length(6),
        Constraint::Percentage(30),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(12),
    ];

    // Build title from breadcrumb stack
    let title = if app.folder_stack.is_empty() {
        let count = app.folders.items.len();
        format!(" Folders ({count}) ")
    } else {
        let path: Vec<&str> = app.folder_stack.iter().map(|(_, name)| name.as_str()).collect();
        let count = app.folders.items.len();
        format!(" /{} ({count}) ", path.join("/"))
    };

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("► ");

    f.render_stateful_widget(table, area, &mut app.folders.state);
}

fn render_playlists(f: &mut Frame, area: Rect, app: &mut App) {
    if app.playlist_detail.is_some() {
        render_songs_table(f, area, app);
        return;
    }

    let header = Row::new(vec!["Name", "Songs", "Library"])
        .style(Style::default().fg(Color::DarkGray).bold())
        .bottom_margin(1);

    let rows = app.playlists.items.iter().map(|pl| {
        Row::new(vec![
            Cell::from(pl.name.clone()),
            Cell::from(format!("{}", pl.song_count())),
            Cell::from(pl.library.clone()),
        ])
    });

    let widths = [
        Constraint::Percentage(50),
        Constraint::Percentage(20),
        Constraint::Percentage(30),
    ];

    let count = app.playlists.items.len();
    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Playlists ({count}) ")),
        )
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("► ");

    f.render_stateful_widget(table, area, &mut app.playlists.state);
}

fn render_queue(f: &mut Frame, area: Rect, app: &mut App) {
    let playing_idx = app.now_playing.as_ref().map(|np| np.queue_index);

    let header = Row::new(vec!["#", "Artist", "Title", "Album"])
        .style(Style::default().fg(Color::DarkGray).bold())
        .bottom_margin(1);

    let rows = app.queue.iter().enumerate().map(|(i, song)| {
        let (artist, title, album, _) = extract_song_display(song);
        let is_playing = playing_idx == Some(i);
        let style = if is_playing {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        let marker = if is_playing { "▶" } else { "" };
        Row::new(vec![
            Cell::from(format!("{}{}", marker, i + 1)),
            Cell::from(artist),
            Cell::from(title),
            Cell::from(album),
        ])
        .style(style)
    });

    let widths = [
        Constraint::Length(5),
        Constraint::Percentage(30),
        Constraint::Percentage(35),
        Constraint::Percentage(30),
    ];

    let count = app.queue.len();
    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Queue ({count}) ")),
        )
        .row_highlight_style(Style::default().bg(Color::DarkGray));

    f.render_widget(table, area);
}

fn render_player(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    match &app.now_playing {
        Some(np) => {
            let elapsed = np.elapsed();
            let total = np.track.duration;
            let progress = np.progress();

            // Mode indicators
            let shuffle_ind = if app.shuffle { " [S]" } else { "" };
            let repeat_ind = match app.repeat_mode {
                RepeatMode::Off => "",
                RepeatMode::One => " [R:1]",
                RepeatMode::All => " [R:*]",
            };

            // Line 1: track info + time + mode
            let elapsed_str = format_dur(elapsed);
            let total_str = format_dur(total);
            let info = Line::from(vec![
                Span::styled("▶ ", Style::default().fg(Color::Green)),
                Span::styled(
                    format!("{} - {}", np.track.artist, np.track.title),
                    Style::default().fg(Color::White).bold(),
                ),
                Span::raw("  "),
                Span::styled(np.track.album.clone(), Style::default().fg(Color::DarkGray)),
                Span::raw(format!("  {elapsed_str} / {total_str}")),
                Span::styled(
                    format!("{shuffle_ind}{repeat_ind}"),
                    Style::default().fg(Color::Yellow),
                ),
            ]);
            f.render_widget(Paragraph::new(info), inner);

            // Line 2: progress bar
            if inner.height >= 2 {
                let gauge_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
                let gauge = Gauge::default()
                    .ratio(progress)
                    .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray));
                f.render_widget(gauge, gauge_area);
            }
        }
        None => {
            // Mode indicators even when stopped
            let shuffle_ind = if app.shuffle { " [S]" } else { "" };
            let repeat_ind = match app.repeat_mode {
                RepeatMode::Off => "",
                RepeatMode::One => " [R:1]",
                RepeatMode::All => " [R:*]",
            };
            let mode_str = format!("{shuffle_ind}{repeat_ind}");

            let status = Line::from(vec![
                Span::styled("■ ", Style::default().fg(Color::DarkGray)),
                Span::raw(&app.status),
                Span::styled(mode_str, Style::default().fg(Color::Yellow)),
            ]);
            f.render_widget(Paragraph::new(status), inner);
        }
    }
}

fn render_help(f: &mut Frame, area: Rect, app: &App) {
    let shuffle_label = if app.shuffle { "ON" } else { "off" };
    let repeat_label = match app.repeat_mode {
        RepeatMode::Off => "off",
        RepeatMode::One => "one",
        RepeatMode::All => "all",
    };

    let help = Line::from(vec![
        Span::styled(" ↑↓", Style::default().fg(Color::Yellow)),
        Span::raw(":Nav "),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(":Play "),
        Span::styled("Space", Style::default().fg(Color::Yellow)),
        Span::raw(":Stop "),
        Span::styled("n/p", Style::default().fg(Color::Yellow)),
        Span::raw(":Next/Prev "),
        Span::styled("s", Style::default().fg(Color::Yellow)),
        Span::raw(format!(":{shuffle_label} ")),
        Span::styled("r", Style::default().fg(Color::Yellow)),
        Span::raw(format!(":{repeat_label} ")),
        Span::styled("Tab", Style::default().fg(Color::Yellow)),
        Span::raw(":Switch "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(":Back "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(":Quit"),
    ]);
    f.render_widget(
        Paragraph::new(help).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}

/// Extract display strings from a Song (artist, title, album, duration).
fn extract_song_display(song: &crate::api::types::Song) -> (String, String, String, String) {
    if let Some(ref add) = song.additional {
        let tag = add.song_tag.as_ref();
        let audio = add.song_audio.as_ref();
        (
            tag.map(|t| t.artist.clone()).unwrap_or_default(),
            tag.map(|t| t.title.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| song.title.clone()),
            tag.map(|t| t.album.clone()).unwrap_or_default(),
            format_dur(Duration::from_secs(
                audio.map(|a| a.duration as u64).unwrap_or(0),
            )),
        )
    } else {
        (
            String::new(),
            song.title.clone(),
            String::new(),
            "0:00".to_string(),
        )
    }
}

fn format_dur(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}
