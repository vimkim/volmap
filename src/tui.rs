//! Responsive stacked terminal projection over an immutable graph revision.

use std::fmt;
use std::io::{self, IsTerminal, Stdout, Write};
use std::time::Duration;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use crossterm::{execute, queue};

use crate::inspection::{GraphView, QueryError};
use crate::model::SectorId;
use crate::projection::{OptionalTextProjection, PageProjection, page_projection};

const GRID_TOP: u16 = 4;

#[derive(Debug)]
pub enum TuiError {
    NotTerminal,
    TerminalTooSmall,
    Query(QueryError),
    Io(io::Error),
}

impl fmt::Display for TuiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotTerminal => formatter.write_str("TUI requires terminal stdin and stdout"),
            Self::TerminalTooSmall => {
                formatter.write_str("TUI requires a terminal of at least 60 columns by 20 rows")
            }
            Self::Query(error) => write!(formatter, "TUI graph query failed: {error}"),
            Self::Io(error) => write!(formatter, "TUI terminal I/O failed: {error}"),
        }
    }
}

impl std::error::Error for TuiError {}

impl From<io::Error> for TuiError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<QueryError> for TuiError {
    fn from(value: QueryError) -> Self {
        Self::Query(value)
    }
}

pub fn run(view: &GraphView) -> Result<(), TuiError> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(TuiError::NotTerminal);
    }
    let mut terminal = TerminalGuard::enter()?;
    let mut state = State::new(view)?;
    draw(terminal.stdout(), view, &state)?;
    loop {
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if handle_key(view, &mut state, key)? => break,
            Event::Mouse(mouse) => {
                handle_mouse(view, &mut state, mouse.kind, mouse.column, mouse.row)?;
            }
            Event::Resize(_, _) => "layout recomputed".clone_into(&mut state.status),
            _ => {}
        }
        draw(terminal.stdout(), view, &state)?;
    }
    Ok(())
}

struct TerminalGuard {
    stdout: Stdout,
}

impl TerminalGuard {
    fn enter() -> Result<Self, TuiError> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide) {
            let _ = disable_raw_mode();
            return Err(TuiError::Io(error));
        }
        Ok(Self { stdout })
    }

    fn stdout(&mut self) -> &mut Stdout {
        &mut self.stdout
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, Show, DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

#[derive(Clone, Copy)]
enum Tab {
    Structure,
    Slots,
    Chain,
    Findings,
    Coverage,
}

impl Tab {
    const fn label(self) -> &'static str {
        match self {
            Self::Structure => "Structure",
            Self::Slots => "Slots",
            Self::Chain => "Chain",
            Self::Findings => "Findings",
            Self::Coverage => "Coverage",
        }
    }
}

struct State {
    volume_index: usize,
    sector: u32,
    cell: u8,
    tab: Tab,
    prompt: Option<String>,
    status: String,
}

impl State {
    fn new(view: &GraphView) -> Result<Self, TuiError> {
        if view.volumes().is_empty() {
            return Err(TuiError::Query(QueryError::EntityNotFound));
        }
        Ok(Self {
            volume_index: 0,
            sector: 0,
            cell: 0,
            tab: Tab::Structure,
            prompt: None,
            status: "q quit · arrows move · [ ] sector · / jump · 1-5 tabs · ? help".to_owned(),
        })
    }
}

fn handle_key(view: &GraphView, state: &mut State, key: KeyEvent) -> Result<bool, TuiError> {
    if let Some(prompt) = state.prompt.as_mut() {
        match key.code {
            KeyCode::Esc => state.prompt = None,
            KeyCode::Backspace => {
                prompt.pop();
            }
            KeyCode::Enter => {
                let selector = state.prompt.take().unwrap_or_default();
                if jump(view, state, &selector) {
                    state.status = format!("selected {selector}");
                } else {
                    "selector not found; use volume:V, sector:V:S, or page:V:P"
                        .clone_into(&mut state.status);
                }
            }
            KeyCode::Char(value)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && prompt.len() < 64
                    && value.is_ascii()
                    && !value.is_ascii_control() =>
            {
                prompt.push(value);
            }
            _ => {}
        }
        return Ok(false);
    }
    match key.code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('/' | 'g') => state.prompt = Some(String::new()),
        KeyCode::Left => state.cell = state.cell.saturating_sub(1),
        KeyCode::Right => state.cell = state.cell.saturating_add(1).min(63),
        KeyCode::Up => state.cell = state.cell.saturating_sub(8),
        KeyCode::Down => state.cell = state.cell.saturating_add(8).min(63),
        KeyCode::Char('[') => state.sector = state.sector.saturating_sub(1),
        KeyCode::Char(']') => move_sector(view, state, 1)?,
        KeyCode::PageUp => move_volume(view, state, false),
        KeyCode::PageDown => move_volume(view, state, true),
        KeyCode::Tab => {
            state.tab = next_tab(state.tab, key.modifiers.contains(KeyModifiers::SHIFT));
        }
        KeyCode::Char('1') => state.tab = Tab::Structure,
        KeyCode::Char('2') => state.tab = Tab::Slots,
        KeyCode::Char('3') => state.tab = Tab::Chain,
        KeyCode::Char('4') => state.tab = Tab::Findings,
        KeyCode::Char('5') => state.tab = Tab::Coverage,
        KeyCode::Char('?') => {
            "PgUp/PgDn volume · [/] sector · arrows page · / typed jump · tabs 1-5 · q quit"
                .clone_into(&mut state.status);
        }
        _ => {}
    }
    Ok(false)
}

fn handle_mouse(
    view: &GraphView,
    state: &mut State,
    kind: MouseEventKind,
    column: u16,
    row: u16,
) -> Result<(), TuiError> {
    match kind {
        MouseEventKind::Down(MouseButton::Left) if (GRID_TOP..GRID_TOP + 8).contains(&row) => {
            let (width, _) = terminal::size()?;
            let cell_width = (width / 8).max(1);
            let grid_column = (column / cell_width).min(7);
            let grid_row = row - GRID_TOP;
            state.cell = u8::try_from(grid_row * 8 + grid_column).unwrap_or(63);
        }
        MouseEventKind::ScrollUp => state.sector = state.sector.saturating_sub(1),
        MouseEventKind::ScrollDown => move_sector(view, state, 1)?,
        _ => {}
    }
    Ok(())
}

fn move_sector(view: &GraphView, state: &mut State, amount: u32) -> Result<(), TuiError> {
    let volume = view
        .volumes()
        .get(state.volume_index)
        .copied()
        .ok_or(QueryError::EntityNotFound)?;
    state.sector = state
        .sector
        .saturating_add(amount)
        .min(volume.total_sectors.saturating_sub(1));
    Ok(())
}

fn move_volume(view: &GraphView, state: &mut State, forward: bool) {
    let count = view.volumes().len();
    state.volume_index = if forward {
        (state.volume_index + 1).min(count.saturating_sub(1))
    } else {
        state.volume_index.saturating_sub(1)
    };
    state.sector = 0;
    state.cell = 0;
}

const fn next_tab(tab: Tab, reverse: bool) -> Tab {
    match (tab, reverse) {
        (Tab::Structure, false) | (Tab::Chain, true) => Tab::Slots,
        (Tab::Slots, false) | (Tab::Findings, true) => Tab::Chain,
        (Tab::Chain, false) | (Tab::Coverage, true) => Tab::Findings,
        (Tab::Findings, false) | (Tab::Structure, true) => Tab::Coverage,
        (Tab::Coverage, false) | (Tab::Slots, true) => Tab::Structure,
    }
}

fn jump(view: &GraphView, state: &mut State, selector: &str) -> bool {
    let fields = selector.split(':').collect::<Vec<_>>();
    let (raw_volume, sector, cell) = match fields.as_slice() {
        ["volume", volume] => (volume.parse::<i16>().ok(), Some(0_u32), Some(0_u8)),
        ["sector", volume, sector] => (
            volume.parse::<i16>().ok(),
            sector.parse::<u32>().ok(),
            Some(0),
        ),
        ["page", volume, page] => {
            let page = page.parse::<u32>().ok();
            (
                volume.parse::<i16>().ok(),
                page.map(|value| value / 64),
                page.and_then(|value| u8::try_from(value % 64).ok()),
            )
        }
        _ => return false,
    };
    let Some(raw_volume) = raw_volume else {
        return false;
    };
    let Some(index) = view
        .volumes()
        .iter()
        .position(|volume| volume.vol_id.get() == raw_volume)
    else {
        return false;
    };
    let Some(sector) = sector else {
        return false;
    };
    if sector >= view.volumes()[index].total_sectors {
        return false;
    }
    state.volume_index = index;
    state.sector = sector;
    state.cell = cell.unwrap_or(0);
    true
}

#[allow(clippy::too_many_lines)]
fn draw(stdout: &mut Stdout, view: &GraphView, state: &State) -> Result<(), TuiError> {
    let (width, height) = terminal::size()?;
    if width < 60 || height < 20 {
        return Err(TuiError::TerminalTooSmall);
    }
    let overview = view.overview();
    let volumes = view.volumes();
    let volume = volumes
        .get(state.volume_index)
        .copied()
        .ok_or(QueryError::EntityNotFound)?;
    let sector_id = SectorId::new(i32::try_from(state.sector).map_err(|_| QueryError::Arithmetic)?)
        .map_err(|_| QueryError::Arithmetic)?;
    let sector = view.sector(volume.vol_id, sector_id)?;
    let selected = sector
        .pages
        .get(usize::from(state.cell))
        .copied()
        .ok_or(QueryError::EntityNotFound)?;
    let page = page_projection(selected);
    let fingerprint = crate::projection::snapshot_id_hex(overview.snapshot_id);
    let title = format!(
        " VOLMAP  snapshot {}  r{}  {} ",
        &fingerprint[..12],
        overview.revision.get(),
        crate::projection::outcome_name(overview.outcome)
    );
    queue!(
        stdout,
        MoveTo(0, 0),
        Clear(ClearType::All),
        SetAttribute(Attribute::Bold),
        Print(truncate(&title, width)),
        SetAttribute(Attribute::Reset)
    )?;
    line(
        stdout,
        1,
        width,
        &format!(
            "volume {} ({}/{}) / sector {} ({}) / page {}",
            volume.vol_id.get(),
            state.volume_index + 1,
            volumes.len(),
            state.sector,
            if sector.reserved {
                "reserved"
            } else {
                "unreserved"
            },
            page.page_id
        ),
    )?;
    line(
        stdout,
        2,
        width,
        "legend: S system  r reserved  . unreserved  ! finding  [selected]",
    )?;

    let cell_width = width / 8;
    for (index, item) in sector.pages.iter().enumerate() {
        let row = GRID_TOP + u16::try_from(index / 8).unwrap_or(0);
        let column = u16::try_from(index % 8).unwrap_or(0) * cell_width;
        let projected = page_projection(*item);
        let marker = if matches!(projected.diagnostic, OptionalTextProjection::Unknown) {
            match projected.allocation {
                "system-metadata" => 'S',
                "reserved-unallocated" => 'r',
                _ => '.',
            }
        } else {
            '!'
        };
        let label = format!("{marker}{:>5}", projected.page_id);
        queue!(stdout, MoveTo(column, row))?;
        if index == usize::from(state.cell) {
            queue!(stdout, SetAttribute(Attribute::Reverse))?;
        }
        queue!(
            stdout,
            Print(truncate(&label, cell_width)),
            SetAttribute(Attribute::Reset)
        )?;
    }

    let detail_top = GRID_TOP + 9;
    line(
        stdout,
        detail_top,
        width,
        &format!(
            "[1 Structure] [2 Slots] [3 Chain] [4 Findings] [5 Coverage]  active: {}",
            state.tab.label()
        ),
    )?;
    let lines = detail_lines(view, state, &page);
    let available = height.saturating_sub(detail_top + 3);
    for (offset, text) in lines.into_iter().take(usize::from(available)).enumerate() {
        line(
            stdout,
            detail_top + 1 + u16::try_from(offset).unwrap_or(0),
            width,
            &text,
        )?;
    }
    let footer = if let Some(prompt) = &state.prompt {
        format!("jump> {prompt}")
    } else {
        state.status.clone()
    };
    line(stdout, height - 1, width, &footer)?;
    stdout.flush()?;
    Ok(())
}

fn detail_lines(view: &GraphView, state: &State, page: &PageProjection) -> Vec<String> {
    match state.tab {
        Tab::Structure => vec![
            format!(
                "identity page:{}:{}  sector:{}  allocation:{}",
                page.vol_id, page.page_id, page.sector_id, page.allocation
            ),
            format!(
                "physical-type:{}  availability:{}  detail:{}  TDE:{}",
                optional_text(&page.page_type),
                page.availability,
                optional_text(&page.detail_support),
                page.tde_state
            ),
            format!(
                "LSA:{}  evidence:page:{}:{}  structural ranges only · bytes withheld",
                optional_count(&page.lsa_word),
                page.vol_id,
                page.page_id
            ),
        ],
        Tab::Slots => vec![
            "slot directory: not present in revision 0".to_owned(),
            "Run an explicit deep enrichment when the selected page type supports slots."
                .to_owned(),
        ],
        Tab::Chain => vec![
            "relationships: no validated deep chain present in revision 0".to_owned(),
            "Payload fragments, ciphertext, and source bytes are never displayed.".to_owned(),
        ],
        Tab::Findings => vec![format!(
            "selected finding: {}",
            optional_text(&page.diagnostic)
        )],
        Tab::Coverage => view
            .overview()
            .coverage
            .into_iter()
            .map(|coverage| {
                format!(
                    "{}: {:?} conclusive={}/{} total={}",
                    coverage.facet,
                    coverage.coverage,
                    coverage.conclusive,
                    coverage.evaluated,
                    coverage
                        .trusted_total
                        .map_or_else(|| "unknown".to_owned(), |value| value.to_string())
                )
            })
            .collect(),
    }
}

fn optional_text(value: &OptionalTextProjection) -> &'static str {
    match value {
        OptionalTextProjection::Known(value) => value,
        OptionalTextProjection::Unknown => "unknown",
        OptionalTextProjection::Unsupported => "unsupported",
    }
}

fn optional_count(value: &crate::projection::OptionalCountProjection) -> &str {
    match value {
        crate::projection::OptionalCountProjection::Known(value) => value,
        crate::projection::OptionalCountProjection::Unknown => "unknown",
    }
}

fn line(stdout: &mut Stdout, row: u16, width: u16, value: &str) -> Result<(), io::Error> {
    queue!(stdout, MoveTo(0, row), Print(truncate(value, width)))
}

fn truncate(value: &str, width: u16) -> String {
    value.chars().take(usize::from(width)).collect()
}
