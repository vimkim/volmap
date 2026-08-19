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
const DETAIL_TOP: u16 = GRID_TOP + 9;

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
            Event::Resize(width, height) => {
                state.status = format!("layout recomputed for {width}x{height}");
            }
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
    About,
}

impl Tab {
    const fn label(self) -> &'static str {
        match self {
            Self::Structure => "Structure",
            Self::Slots => "Slots",
            Self::Chain => "Chain",
            Self::Findings => "Findings",
            Self::Coverage => "Coverage",
            Self::About => "About",
        }
    }
}

struct State {
    volume_index: usize,
    sector: u32,
    cell: u8,
    tab: Tab,
    prompt: Option<Prompt>,
    filter: Option<String>,
    status: String,
    detail_scroll: usize,
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
            filter: None,
            status: "q quit · arrows move · [ ] sector · / jump · 1-6 tabs · ? about".to_owned(),
            detail_scroll: 0,
        })
    }
}

#[derive(Clone, Copy)]
enum PromptKind {
    Search,
    Jump,
    Filter,
}

impl PromptKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Jump => "jump",
            Self::Filter => "filter",
        }
    }
}

struct Prompt {
    kind: PromptKind,
    input: String,
}

#[allow(clippy::too_many_lines)]
fn handle_key(view: &GraphView, state: &mut State, key: KeyEvent) -> Result<bool, TuiError> {
    if let Some(prompt) = state.prompt.as_mut() {
        match key.code {
            KeyCode::Esc => state.prompt = None,
            KeyCode::Backspace => {
                prompt.input.pop();
            }
            KeyCode::Enter => {
                let Some(prompt) = state.prompt.take() else {
                    return Ok(false);
                };
                match prompt.kind {
                    PromptKind::Search | PromptKind::Jump => {
                        if jump(view, state, &prompt.input) {
                            state.status = format!("selected {}", prompt.input);
                        } else {
                            "selector not found; use volume:V, sector:V:S, or page:V:P"
                                .clone_into(&mut state.status);
                        }
                    }
                    PromptKind::Filter => match normalize_filter(&prompt.input) {
                        Some(FilterUpdate::Clear) => {
                            state.filter = None;
                            "filter cleared".clone_into(&mut state.status);
                        }
                        Some(FilterUpdate::Set(filter)) => {
                            state.status = format!("filter {filter}");
                            state.filter = Some(filter);
                        }
                        None => "invalid normalized filter".clone_into(&mut state.status),
                    },
                }
            }
            KeyCode::Char(value)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && prompt.input.len() < 64
                    && value.is_ascii()
                    && !value.is_ascii_control() =>
            {
                prompt.input.push(value);
            }
            _ => {}
        }
        return Ok(false);
    }
    match key.code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('/') => {
            state.prompt = Some(Prompt {
                kind: PromptKind::Search,
                input: String::new(),
            });
        }
        KeyCode::Char('g') => {
            state.prompt = Some(Prompt {
                kind: PromptKind::Jump,
                input: String::new(),
            });
        }
        KeyCode::Char('f') => {
            state.prompt = Some(Prompt {
                kind: PromptKind::Filter,
                input: state.filter.clone().unwrap_or_else(|| "all".to_owned()),
            });
        }
        KeyCode::Char('n') => move_finding(view, state, true),
        KeyCode::Char('N') => move_finding(view, state, false),
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
            state.detail_scroll = 0;
        }
        KeyCode::Char('1') => {
            state.tab = Tab::Structure;
            state.detail_scroll = 0;
        }
        KeyCode::Char('2') => {
            state.tab = Tab::Slots;
            state.detail_scroll = 0;
        }
        KeyCode::Char('3') => {
            state.tab = Tab::Chain;
            state.detail_scroll = 0;
        }
        KeyCode::Char('4') => {
            state.tab = Tab::Findings;
            state.detail_scroll = 0;
        }
        KeyCode::Char('5') => {
            state.tab = Tab::Coverage;
            state.detail_scroll = 0;
        }
        KeyCode::Char('6') => {
            state.tab = Tab::About;
            state.detail_scroll = 0;
        }
        KeyCode::Char('j') => {
            state.detail_scroll = state.detail_scroll.saturating_add(1);
        }
        KeyCode::Char('k') => {
            state.detail_scroll = state.detail_scroll.saturating_sub(1);
        }
        KeyCode::Enter => {
            state.status = format!(
                "selected page:{}:{}",
                view.volumes()[state.volume_index].vol_id.get(),
                state.sector.saturating_mul(64) + u32::from(state.cell)
            );
        }
        KeyCode::Esc => "no active overlay".clone_into(&mut state.status),
        KeyCode::Char('?') => {
            state.tab = Tab::About;
            state.detail_scroll = 0;
            "About/licenses · j/k scroll · Tab returns to inspection · q quit"
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
        MouseEventKind::Down(MouseButton::Left) if row == DETAIL_TOP => {
            if let Some(tab) = tab_at_column(column) {
                state.tab = tab;
                state.detail_scroll = 0;
            }
        }
        MouseEventKind::ScrollUp if row > DETAIL_TOP => {
            state.detail_scroll = state.detail_scroll.saturating_sub(1);
        }
        MouseEventKind::ScrollDown if row > DETAIL_TOP => {
            state.detail_scroll = state.detail_scroll.saturating_add(1);
        }
        MouseEventKind::ScrollUp => state.sector = state.sector.saturating_sub(1),
        MouseEventKind::ScrollDown => move_sector(view, state, 1)?,
        _ => {}
    }
    Ok(())
}

const fn tab_at_column(column: u16) -> Option<Tab> {
    match column {
        0..=13 => Some(Tab::Structure),
        14..=23 => Some(Tab::Slots),
        24..=33 => Some(Tab::Chain),
        34..=46 => Some(Tab::Findings),
        47..=60 => Some(Tab::Coverage),
        61..=71 => Some(Tab::About),
        _ => None,
    }
}

fn move_finding(view: &GraphView, state: &mut State, forward: bool) {
    let volumes = view.volumes();
    let mut positions = view
        .overview()
        .diagnostics
        .iter()
        .filter_map(|diagnostic| finding_position(&volumes, &diagnostic.subject))
        .collect::<Vec<_>>();
    positions.sort_unstable();
    positions.dedup();
    let current = (state.volume_index, state.sector, state.cell);
    let Some((volume_index, sector, cell)) = select_finding(&positions, current, forward) else {
        "no page findings in this revision".clone_into(&mut state.status);
        return;
    };
    state.volume_index = volume_index;
    state.sector = sector;
    state.cell = cell;
    state.tab = Tab::Findings;
    state.detail_scroll = 0;
    if forward {
        "next finding"
    } else {
        "previous finding"
    }
    .clone_into(&mut state.status);
}

fn finding_position(
    volumes: &[crate::inspection::VolumeView],
    subject: &str,
) -> Option<(usize, u32, u8)> {
    let fields = subject.split(':').collect::<Vec<_>>();
    let ["page", volume, page] = fields.as_slice() else {
        return None;
    };
    let volume = volume.parse::<i16>().ok()?;
    let page = page.parse::<u32>().ok()?;
    let index = volumes
        .iter()
        .position(|candidate| candidate.vol_id.get() == volume)?;
    (page < volumes[index].total_sectors.checked_mul(64)?).then_some((
        index,
        page / 64,
        u8::try_from(page % 64).ok()?,
    ))
}

fn select_finding(
    positions: &[(usize, u32, u8)],
    current: (usize, u32, u8),
    forward: bool,
) -> Option<(usize, u32, u8)> {
    if forward {
        positions
            .iter()
            .copied()
            .find(|position| *position > current)
            .or_else(|| positions.first().copied())
    } else {
        positions
            .iter()
            .rev()
            .copied()
            .find(|position| *position < current)
            .or_else(|| positions.last().copied())
    }
}

#[derive(Debug, Eq, PartialEq)]
enum FilterUpdate {
    Clear,
    Set(String),
}

fn normalize_filter(input: &str) -> Option<FilterUpdate> {
    if input == "all" || input.is_empty() {
        return Some(FilterUpdate::Clear);
    }
    let (field, value) = input.split_once(':')?;
    if value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-._".contains(&byte)
        })
    {
        return None;
    }
    match field {
        "volume" | "allocation" | "type" | "detail" | "tde" | "diagnostic" => {
            Some(FilterUpdate::Set(format!("{field}:{value}")))
        }
        _ => None,
    }
}

fn matches_filter(page: &PageProjection, filter: Option<&str>) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    let Some((field, value)) = filter.split_once(':') else {
        return false;
    };
    match field {
        "volume" => page.vol_id.to_string() == value,
        "allocation" => page.allocation == value,
        "type" => optional_text(&page.page_type) == value,
        "detail" => optional_text(&page.detail_support) == value,
        "tde" => page.tde_state == value,
        "diagnostic" => optional_text(&page.diagnostic) == value,
        _ => false,
    }
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
        (Tab::Findings, false) | (Tab::About, true) => Tab::Coverage,
        (Tab::Coverage, false) | (Tab::Structure, true) => Tab::About,
        (Tab::About, false) | (Tab::Slots, true) => Tab::Structure,
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
        " VOLMAP  snapshot {}  r{}  {}  filter:{} ",
        &fingerprint[..12],
        overview.revision.get(),
        crate::projection::outcome_name(overview.outcome),
        state.filter.as_deref().unwrap_or("all")
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
        if !matches_filter(&projected, state.filter.as_deref()) {
            queue!(stdout, SetAttribute(Attribute::Dim))?;
        }
        queue!(
            stdout,
            Print(truncate(&label, cell_width)),
            SetAttribute(Attribute::Reset)
        )?;
    }

    line(
        stdout,
        DETAIL_TOP,
        width,
        &format!(
            "[1 Structure] [2 Slots] [3 Chain] [4 Findings] [5 Coverage] [6 About]  active: {}",
            state.tab.label()
        ),
    )?;
    let lines = detail_lines(view, state, &page);
    let available = height.saturating_sub(DETAIL_TOP + 3);
    for (offset, text) in lines
        .into_iter()
        .skip(state.detail_scroll)
        .take(usize::from(available))
        .enumerate()
    {
        line(
            stdout,
            DETAIL_TOP + 1 + u16::try_from(offset).unwrap_or(0),
            width,
            &text,
        )?;
    }
    let footer = if let Some(prompt) = &state.prompt {
        format!("{}> {}", prompt.kind.label(), prompt.input)
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
        Tab::About => crate::notices::THIRD_PARTY_NOTICES
            .lines()
            .map(str::to_owned)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_filters_reject_unbounded_or_payload_searches() {
        assert_eq!(normalize_filter("all"), Some(FilterUpdate::Clear));
        assert_eq!(
            normalize_filter("allocation:allocated"),
            Some(FilterUpdate::Set("allocation:allocated".to_owned()))
        );
        assert_eq!(
            normalize_filter("diagnostic:page.envelope.invalid"),
            Some(FilterUpdate::Set(
                "diagnostic:page.envelope.invalid".to_owned()
            ))
        );
        assert_eq!(normalize_filter("payload:secret"), None);
        assert_eq!(normalize_filter("type:heap value"), None);
        assert_eq!(normalize_filter("type:*"), None);
    }

    #[test]
    fn keyboard_and_mouse_tab_orders_are_equivalent() {
        let mut tab = Tab::Structure;
        for expected in [
            Tab::Slots,
            Tab::Chain,
            Tab::Findings,
            Tab::Coverage,
            Tab::About,
        ] {
            tab = next_tab(tab, false);
            assert_eq!(tab.label(), expected.label());
        }
        for (column, expected) in [
            (0, Tab::Structure),
            (14, Tab::Slots),
            (24, Tab::Chain),
            (34, Tab::Findings),
            (47, Tab::Coverage),
            (61, Tab::About),
        ] {
            assert_eq!(
                tab_at_column(column).map(Tab::label),
                Some(expected.label())
            );
        }
        assert!(tab_at_column(72).is_none());
    }

    #[test]
    fn finding_navigation_uses_sparse_diagnostics_and_wraps() {
        use crate::format::{VolumePurpose, VolumeType};
        use crate::model::{PageId, VolId};

        let volumes = [
            crate::inspection::VolumeView {
                vol_id: VolId::new(0).unwrap(),
                purpose: VolumePurpose::PermanentData,
                volume_type: VolumeType::Permanent,
                total_sectors: 2,
                maximum_sectors: 2,
                system_last_page: PageId::new(1).unwrap(),
                reserved_sectors: 1,
            },
            crate::inspection::VolumeView {
                vol_id: VolId::new(1).unwrap(),
                purpose: VolumePurpose::PermanentData,
                volume_type: VolumeType::Permanent,
                total_sectors: 1,
                maximum_sectors: 1,
                system_last_page: PageId::new(1).unwrap(),
                reserved_sectors: 1,
            },
        ];
        assert_eq!(finding_position(&volumes, "page:0:0"), Some((0, 0, 0)));
        assert_eq!(finding_position(&volumes, "page:0:127"), Some((0, 1, 63)));
        assert_eq!(finding_position(&volumes, "page:1:63"), Some((1, 0, 63)));
        assert_eq!(finding_position(&volumes, "page:1:64"), None);
        assert_eq!(finding_position(&volumes, "snapshot"), None);

        let positions = [(0, 0, 2), (0, 1, 3), (1, 0, 4)];
        assert_eq!(select_finding(&positions, (0, 0, 2), true), Some((0, 1, 3)));
        assert_eq!(select_finding(&positions, (1, 0, 4), true), Some((0, 0, 2)));
        assert_eq!(
            select_finding(&positions, (0, 0, 2), false),
            Some((1, 0, 4))
        );
    }

    #[test]
    fn truncation_is_character_bounded_and_never_adds_terminal_control() {
        assert_eq!(truncate("abcdef", 3), "abc");
        assert_eq!(truncate("가나다", 2), "가나");
        assert!(!truncate("plain", 20).contains('\u{1b}'));
    }
}
