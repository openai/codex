/// A Ratatui example that demonstrates how to use the inlined viewport.
///
/// It shows a list of downloads in progress, with a progress bar for each download.
///
/// This example runs with the Ratatui library code in the branch that you are currently
/// reading. See the [`latest`] branch for the code which works with the most recent Ratatui
/// release.
///
/// [`latest`]: https://github.com/ratatui/ratatui/tree/latest
use std::{
    collections::{BTreeMap, VecDeque},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use color_eyre::Result;
use crossterm::event;
use rand::distr::{Distribution, Uniform};
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Gauge, LineGauge, List, ListItem, Paragraph, Widget};
use ratatui::{Frame, Terminal, TerminalOptions, Viewport, symbols};

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = ratatui::init_with_options(TerminalOptions {
        viewport: Viewport::Inline(8),
    });

    let (tx, rx) = mpsc::channel();
    input_handling(tx.clone());
    let workers = workers(tx);
    let mut downloads = downloads();

    for w in &workers {
        let d = downloads.next(w.id).unwrap();
        w.tx.send(d).unwrap();
    }

    let app_result = run(&mut terminal, workers, downloads, rx);

    ratatui::restore();

    app_result
}

const NUM_DOWNLOADS: usize = 10;

type DownloadId = usize;
type WorkerId = usize;
enum Event {
    Input(event::KeyEvent),
    Tick,
    Resize,
    DownloadUpdate(WorkerId, DownloadId, f64),
    DownloadDone(WorkerId, DownloadId),
}
struct Downloads {
    pending: VecDeque<Download>,
    in_progress: BTreeMap<WorkerId, DownloadInProgress>,
}

impl Downloads {
    fn next(&mut self, worker_id: WorkerId) -> Option<Download> {
        match self.pending.pop_front() {
            Some(d) => {
                self.in_progress.insert(
                    worker_id,
                    DownloadInProgress {
                        id: d.id,
                        started_at: Instant::now(),
                        progress: 0.0,
                    },
                );
                Some(d)
            }
            None => None,
        }
    }
}
struct DownloadInProgress {
    id: DownloadId,
    started_at: Instant,
    progress: f64,
}
struct Download {
    id: DownloadId,
    size: usize,
}
struct Worker {
    id: WorkerId,
    tx: mpsc::Sender<Download>,
}

fn input_handling(tx: mpsc::Sender<Event>) {
    let tick_rate = Duration::from_millis(200);
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            // poll for tick rate duration, if no events, sent tick event.
            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout).unwrap() {
                match event::read().unwrap() {
                    event::Event::Key(key) => tx.send(Event::Input(key)).unwrap(),
                    event::Event::Resize(_, _) => tx.send(Event::Resize).unwrap(),
                    _ => {}
                }
            }
            if last_tick.elapsed() >= tick_rate {
                tx.send(Event::Tick).unwrap();
                last_tick = Instant::now();
            }
        }
    });
}

#[expect(clippy::cast_precision_loss, clippy::needless_pass_by_value)]
fn workers(tx: mpsc::Sender<Event>) -> Vec<Worker> {
    (0..4)
        .map(|id| {
            let (worker_tx, worker_rx) = mpsc::channel::<Download>();
            let tx = tx.clone();
            thread::spawn(move || {
                while let Ok(download) = worker_rx.recv() {
                    let mut remaining = download.size;
                    while remaining > 0 {
                        let wait = (remaining as u64).min(10);
                        thread::sleep(Duration::from_millis(wait * 10));
                        remaining = remaining.saturating_sub(10);
                        let progress = (download.size - remaining) * 100 / download.size;
                        tx.send(Event::DownloadUpdate(id, download.id, progress as f64))
                            .unwrap();
                    }
                    tx.send(Event::DownloadDone(id, download.id)).unwrap();
                }
            });
            Worker { id, tx: worker_tx }
        })
        .collect()
}

fn downloads() -> Downloads {
    let distribution = Uniform::new(0, 1000).expect("invalid range");
    let mut rng = rand::rng();
    let pending = (0..NUM_DOWNLOADS)
        .map(|id| {
            let size = distribution.sample(&mut rng);
            Download { id, size }
        })
        .collect();
    Downloads {
        pending,
        in_progress: BTreeMap::new(),
    }
}

#[expect(clippy::needless_pass_by_value)]
fn run<B: Backend>(
    terminal: &mut Terminal<B>,
    workers: Vec<Worker>,
    mut downloads: Downloads,
    rx: mpsc::Receiver<Event>,
) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    let mut redraw = true;
    loop {
        if redraw {
            terminal.draw(|frame| render(frame, &downloads))?;
        }
        redraw = true;

        match rx.recv()? {
            Event::Input(event) => {
                if event.code == event::KeyCode::Char('q') {
                    break;
                }
            }
            Event::Resize => {
                terminal.autoresize()?;
            }
            Event::Tick => {}
            Event::DownloadUpdate(worker_id, _download_id, progress) => {
                let download = downloads.in_progress.get_mut(&worker_id).unwrap();
                download.progress = progress;
                redraw = false;
            }
            Event::DownloadDone(worker_id, download_id) => {
                let download = downloads.in_progress.remove(&worker_id).unwrap();
                terminal.insert_before(1, |buf| {
                    Paragraph::new(Line::from(vec![
                        Span::from("Finished "),
                        Span::styled(
                            format!("download {download_id}"),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::from(format!(
                            " in {}ms",
                            download.started_at.elapsed().as_millis()
                        )),
                    ]))
                    .render(buf.area, buf);
                })?;
                match downloads.next(worker_id) {
                    Some(d) => workers[worker_id].tx.send(d).unwrap(),
                    None => {
                        if downloads.in_progress.is_empty() {
                            terminal.insert_before(1, |buf| {
                                Paragraph::new("Done !").render(buf.area, buf);
                            })?;
                            break;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn render(frame: &mut Frame, downloads: &Downloads) {
    let area = frame.area();

    let block = Block::new().title(Line::from("Progress").centered());
    frame.render_widget(block, area);

    let vertical = Layout::vertical([Constraint::Length(2), Constraint::Length(4)]).margin(1);
    let horizontal = Layout::horizontal([Constraint::Percentage(20), Constraint::Percentage(80)]);
    let [progress_area, main] = area.layout(&vertical);
    let [list_area, gauge_area] = main.layout(&horizontal);

    // total progress
    let done = NUM_DOWNLOADS - downloads.pending.len() - downloads.in_progress.len();
    #[expect(clippy::cast_precision_loss)]
    let progress = LineGauge::default()
        .filled_style(Style::default().fg(Color::Blue))
        .label(format!("{done}/{NUM_DOWNLOADS}"))
        .ratio(done as f64 / NUM_DOWNLOADS as f64);
    frame.render_widget(progress, progress_area);

    // in progress downloads
    let items: Vec<ListItem> = downloads
        .in_progress
        .values()
        .map(|download| {
            ListItem::new(Line::from(vec![
                Span::raw(symbols::DOT),
                Span::styled(
                    format!(" download {:>2}", download.id),
                    Style::default()
                        .fg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(
                    " ({}ms)",
                    download.started_at.elapsed().as_millis()
                )),
            ]))
        })
        .collect();
    let list = List::new(items);
    frame.render_widget(list, list_area);

    #[expect(clippy::cast_possible_truncation)]
    for (i, (_, download)) in downloads.in_progress.iter().enumerate() {
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(Color::Yellow))
            .ratio(download.progress / 100.0);
        if gauge_area.top().saturating_add(i as u16) > area.bottom() {
            continue;
        }
        frame.render_widget(
            gauge,
            Rect {
                x: gauge_area.left(),
                y: gauge_area.top().saturating_add(i as u16),
                width: gauge_area.width,
                height: 1,
            },
        );
    }
}
