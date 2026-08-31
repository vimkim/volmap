//! Terminal lifecycle, event scheduling, and frame presentation for the
//! focused inspector.

use std::collections::VecDeque;
use std::env;
use std::fmt;
use std::io::{self, IsTerminal, Stdout, Write};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use crossterm::{execute, queue};

use crate::inspection::{GraphView, ResourcePolicy};

use super::{
    ColorProfile, DistributionHitRegion, FocusedAction, FocusedEnrichmentCompletion,
    FocusedEnrichmentRequest, FocusedError, FocusedMode, FocusedSession, GlyphProfile, HitRegion,
    PageHitRegion, PageRenderer, PointerInput, PresentationProfile, SectorRenderer, SemanticStyle,
    StructuralKey, Surface, VolumeFrame, VolumeLayout, VolumeRenderer, key_action, pointer_actions,
};

#[cfg(test)]
use super::FocusedState;

const INPUT_WAIT: Duration = Duration::from_millis(25);
const MAX_WIDTH: u16 = 240;
const MAX_HEIGHT: u16 = 80;

/// A completed focused terminal run carrying the final adopted view.
pub(crate) struct FocusedExit {
    view: GraphView,
    #[cfg(test)]
    state: FocusedState,
}

impl FocusedExit {
    pub(crate) fn into_view(self) -> GraphView {
        self.view
    }

    #[cfg(test)]
    pub(super) const fn state(&self) -> FocusedState {
        self.state
    }
}

#[derive(Debug)]
pub(crate) enum FocusedTerminalError {
    NotTerminal,
    Focused(FocusedError),
    Io(io::Error),
    WorkerBusy,
    WorkerDisconnected,
    WorkerPanicked,
    InvalidHitGeometry,
    GenerationOverflow,
}

impl fmt::Display for FocusedTerminalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotTerminal => {
                formatter.write_str("focused TUI requires terminal stdin and stdout")
            }
            Self::Focused(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "focused TUI terminal I/O failed: {error}"),
            Self::WorkerBusy => formatter.write_str("focused TUI enrichment worker is busy"),
            Self::WorkerDisconnected => {
                formatter.write_str("focused TUI enrichment worker disconnected")
            }
            Self::WorkerPanicked => formatter.write_str("focused TUI enrichment worker panicked"),
            Self::InvalidHitGeometry => {
                formatter.write_str("focused TUI renderer produced invalid hit geometry")
            }
            Self::GenerationOverflow => {
                formatter.write_str("focused TUI layout generation overflowed")
            }
        }
    }
}

impl std::error::Error for FocusedTerminalError {}

impl From<FocusedError> for FocusedTerminalError {
    fn from(value: FocusedError) -> Self {
        Self::Focused(value)
    }
}

impl From<io::Error> for FocusedTerminalError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TerminalCapabilities {
    pub ansi_color: bool,
    pub unicode: bool,
    pub mouse: bool,
}

impl TerminalCapabilities {
    const fn profile(self) -> PresentationProfile {
        if self.ansi_color && self.unicode {
            PresentationProfile::ANSI_UNICODE
        } else {
            PresentationProfile::MONO_ASCII
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PointerKind {
    Activate,
    Wheel(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HostEvent {
    Key(StructuralKey),
    Pointer {
        generation: Option<u64>,
        column: u16,
        row: u16,
        kind: PointerKind,
    },
    Resize {
        width: u16,
        height: u16,
    },
    Ignored,
}

pub(super) trait TerminalHost {
    fn is_terminal(&self) -> bool;
    fn capabilities(&self) -> TerminalCapabilities;
    fn size(&mut self) -> io::Result<(u16, u16)>;
    fn enable_raw(&mut self) -> io::Result<()>;
    fn disable_raw(&mut self) -> io::Result<()>;
    fn enter_alternate(&mut self) -> io::Result<()>;
    fn leave_alternate(&mut self) -> io::Result<()>;
    fn enable_mouse(&mut self) -> io::Result<()>;
    fn disable_mouse(&mut self) -> io::Result<()>;
    fn hide_cursor(&mut self) -> io::Result<()>;
    fn show_cursor(&mut self) -> io::Result<()>;
    fn poll_event(&mut self, timeout: Duration) -> io::Result<bool>;
    fn read_event(&mut self, generation: Option<u64>) -> io::Result<HostEvent>;
    fn present(
        &mut self,
        frame: &VolumeFrame,
        previous: Option<&VolumeFrame>,
        force_clear: bool,
    ) -> io::Result<()>;
}

struct CrosstermHost {
    stdout: Stdout,
    capabilities: TerminalCapabilities,
}

impl CrosstermHost {
    fn new() -> Self {
        let term = env::var("TERM").unwrap_or_default();
        let locale = env::var("LC_ALL")
            .or_else(|_| env::var("LC_CTYPE"))
            .or_else(|_| env::var("LANG"))
            .unwrap_or_default()
            .to_ascii_lowercase();
        let terminal_is_dumb = term.eq_ignore_ascii_case("dumb");
        let unicode = locale.contains("utf-8") || locale.contains("utf8");
        Self {
            stdout: io::stdout(),
            capabilities: TerminalCapabilities {
                ansi_color: !terminal_is_dumb && env::var_os("NO_COLOR").is_none(),
                unicode: !terminal_is_dumb && unicode,
                mouse: !terminal_is_dumb,
            },
        }
    }
}

impl TerminalHost for CrosstermHost {
    fn is_terminal(&self) -> bool {
        io::stdin().is_terminal() && self.stdout.is_terminal()
    }

    fn capabilities(&self) -> TerminalCapabilities {
        self.capabilities
    }

    fn size(&mut self) -> io::Result<(u16, u16)> {
        terminal::size()
    }

    fn enable_raw(&mut self) -> io::Result<()> {
        enable_raw_mode()
    }

    fn disable_raw(&mut self) -> io::Result<()> {
        disable_raw_mode()
    }

    fn enter_alternate(&mut self) -> io::Result<()> {
        execute!(self.stdout, EnterAlternateScreen)
    }

    fn leave_alternate(&mut self) -> io::Result<()> {
        execute!(self.stdout, LeaveAlternateScreen)
    }

    fn enable_mouse(&mut self) -> io::Result<()> {
        execute!(self.stdout, EnableMouseCapture)
    }

    fn disable_mouse(&mut self) -> io::Result<()> {
        execute!(self.stdout, DisableMouseCapture)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        execute!(self.stdout, Hide)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        execute!(self.stdout, Show)
    }

    fn poll_event(&mut self, timeout: Duration) -> io::Result<bool> {
        event::poll(timeout)
    }

    fn read_event(&mut self, generation: Option<u64>) -> io::Result<HostEvent> {
        Ok(match event::read()? {
            Event::Key(key)
                if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                key_event(key.code).map_or(HostEvent::Ignored, HostEvent::Key)
            }
            Event::Mouse(mouse) => {
                let kind = match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => Some(PointerKind::Activate),
                    MouseEventKind::ScrollUp => Some(PointerKind::Wheel(-1)),
                    MouseEventKind::ScrollDown => Some(PointerKind::Wheel(1)),
                    _ => None,
                };
                kind.map_or(HostEvent::Ignored, |kind| HostEvent::Pointer {
                    generation,
                    column: mouse.column,
                    row: mouse.row,
                    kind,
                })
            }
            Event::Resize(width, height) => HostEvent::Resize { width, height },
            Event::FocusGained | Event::FocusLost | Event::Key(_) => HostEvent::Ignored,
        })
    }

    fn present(
        &mut self,
        frame: &VolumeFrame,
        previous: Option<&VolumeFrame>,
        force_clear: bool,
    ) -> io::Result<()> {
        let can_diff = !force_clear
            && previous
                .is_some_and(|old| old.surface == frame.surface && old.profile == frame.profile);
        if !can_diff {
            queue!(self.stdout, Clear(ClearType::All))?;
        }
        let previous_cells = can_diff.then(|| &previous.expect("checked above").cells);
        for row in 0..frame.surface.height {
            for column in 0..frame.surface.width {
                let index =
                    usize::from(row) * usize::from(frame.surface.width) + usize::from(column);
                let cell = &frame.cells[index];
                if cell.continuation || previous_cells.is_some_and(|cells| cells[index] == *cell) {
                    continue;
                }
                queue!(self.stdout, MoveTo(column, row))?;
                queue_style(&mut self.stdout, cell.style, frame.profile)?;
                queue!(self.stdout, Print(cell.glyph.as_str()))?;
            }
        }
        queue!(
            self.stdout,
            SetAttribute(Attribute::Reset),
            ResetColor,
            MoveTo(0, frame.surface.height.saturating_sub(1))
        )?;
        self.stdout.flush()
    }
}

fn key_event(code: KeyCode) -> Option<StructuralKey> {
    match code {
        KeyCode::Left => Some(StructuralKey::Left),
        KeyCode::Right => Some(StructuralKey::Right),
        KeyCode::Up => Some(StructuralKey::Up),
        KeyCode::Down => Some(StructuralKey::Down),
        KeyCode::Enter => Some(StructuralKey::Enter),
        KeyCode::Esc => Some(StructuralKey::Escape),
        KeyCode::Backspace => Some(StructuralKey::Backspace),
        KeyCode::Char('[') => Some(StructuralKey::PreviousSector),
        KeyCode::Char(']') => Some(StructuralKey::NextSector),
        KeyCode::PageUp => Some(StructuralKey::PreviousVolume),
        KeyCode::PageDown => Some(StructuralKey::NextVolume),
        KeyCode::Char('?') => Some(StructuralKey::Help),
        KeyCode::Char('q') => Some(StructuralKey::Quit),
        _ => None,
    }
}

fn queue_style(
    stdout: &mut Stdout,
    style: SemanticStyle,
    profile: PresentationProfile,
) -> io::Result<()> {
    queue!(stdout, SetAttribute(Attribute::Reset), ResetColor)?;
    let attribute = match style {
        SemanticStyle::Header | SemanticStyle::Allocated | SemanticStyle::Occupancy => {
            Some(Attribute::Bold)
        }
        SemanticStyle::Focus | SemanticStyle::Finding => Some(Attribute::Reverse),
        SemanticStyle::Unknown => Some(Attribute::Underlined),
        SemanticStyle::Muted | SemanticStyle::Reserved => Some(Attribute::Dim),
        SemanticStyle::Plain | SemanticStyle::System | SemanticStyle::Unreserved => None,
    };
    if let Some(attribute) = attribute {
        queue!(stdout, SetAttribute(attribute))?;
    }
    if profile.colors == ColorProfile::Ansi {
        let color = match style {
            SemanticStyle::Header => Some(Color::Cyan),
            SemanticStyle::Focus => Some(Color::Black),
            SemanticStyle::System => Some(Color::Magenta),
            SemanticStyle::Allocated | SemanticStyle::Occupancy => Some(Color::Green),
            SemanticStyle::Reserved => Some(Color::Yellow),
            SemanticStyle::Unreserved | SemanticStyle::Muted => Some(Color::DarkGrey),
            SemanticStyle::Unknown => Some(Color::Blue),
            SemanticStyle::Finding => Some(Color::Red),
            SemanticStyle::Plain => None,
        };
        if let Some(color) = color {
            queue!(stdout, SetForegroundColor(color))?;
        }
    }
    Ok(())
}

struct TerminalLease<H: TerminalHost> {
    host: H,
    state: TerminalLifecycle,
}

#[derive(Clone, Copy)]
enum TerminalLifecycle {
    New,
    Raw,
    Alternate,
    Mouse,
    HiddenWithMouse,
    HiddenWithoutMouse,
    Closed,
}

impl<H: TerminalHost> TerminalLease<H> {
    fn enter(host: H) -> Result<Self, FocusedTerminalError> {
        if !host.is_terminal() {
            return Err(FocusedTerminalError::NotTerminal);
        }
        let capabilities = host.capabilities();
        let mut lease = Self {
            host,
            state: TerminalLifecycle::New,
        };
        lease.host.enable_raw()?;
        lease.state = TerminalLifecycle::Raw;
        lease.host.enter_alternate()?;
        lease.state = TerminalLifecycle::Alternate;
        if capabilities.mouse {
            lease.host.enable_mouse()?;
            lease.state = TerminalLifecycle::Mouse;
        }
        lease.host.hide_cursor()?;
        lease.state = if capabilities.mouse {
            TerminalLifecycle::HiddenWithMouse
        } else {
            TerminalLifecycle::HiddenWithoutMouse
        };
        Ok(lease)
    }

    fn close(&mut self) -> io::Result<()> {
        let mut first_error = None;
        let state = std::mem::replace(&mut self.state, TerminalLifecycle::Closed);
        if matches!(
            state,
            TerminalLifecycle::HiddenWithMouse | TerminalLifecycle::HiddenWithoutMouse
        ) && let Err(error) = self.host.show_cursor()
        {
            first_error = Some(error);
        }
        if matches!(
            state,
            TerminalLifecycle::Mouse | TerminalLifecycle::HiddenWithMouse
        ) && let Err(error) = self.host.disable_mouse()
            && first_error.is_none()
        {
            first_error = Some(error);
        }
        if matches!(
            state,
            TerminalLifecycle::Alternate
                | TerminalLifecycle::Mouse
                | TerminalLifecycle::HiddenWithMouse
                | TerminalLifecycle::HiddenWithoutMouse
        ) && let Err(error) = self.host.leave_alternate()
            && first_error.is_none()
        {
            first_error = Some(error);
        }
        if !matches!(state, TerminalLifecycle::New | TerminalLifecycle::Closed)
            && let Err(error) = self.host.disable_raw()
            && first_error.is_none()
        {
            first_error = Some(error);
        }
        first_error.map_or(Ok(()), Err)
    }
}

impl<H: TerminalHost> Drop for TerminalLease<H> {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

trait EnrichmentWorker {
    fn start(&mut self, request: FocusedEnrichmentRequest) -> Result<(), FocusedTerminalError>;
    fn try_completion(
        &mut self,
    ) -> Result<Option<FocusedEnrichmentCompletion>, FocusedTerminalError>;
    fn shutdown(&mut self) -> Result<(), FocusedTerminalError>;
}

#[derive(Default)]
struct ChannelWorker {
    receiver: Option<Receiver<FocusedEnrichmentCompletion>>,
    handle: Option<JoinHandle<()>>,
}

impl ChannelWorker {
    fn finish_thread(&mut self) -> Result<(), FocusedTerminalError> {
        if let Some(handle) = self.handle.take()
            && handle.join().is_err()
        {
            return Err(FocusedTerminalError::WorkerPanicked);
        }
        self.receiver = None;
        Ok(())
    }
}

impl EnrichmentWorker for ChannelWorker {
    fn start(&mut self, request: FocusedEnrichmentRequest) -> Result<(), FocusedTerminalError> {
        if self.receiver.is_some() || self.handle.is_some() {
            return Err(FocusedTerminalError::WorkerBusy);
        }
        let (sender, receiver): (
            SyncSender<FocusedEnrichmentCompletion>,
            Receiver<FocusedEnrichmentCompletion>,
        ) = mpsc::sync_channel(1);
        self.handle = Some(thread::spawn(move || {
            let _ = sender.send(request.execute());
        }));
        self.receiver = Some(receiver);
        Ok(())
    }

    fn try_completion(
        &mut self,
    ) -> Result<Option<FocusedEnrichmentCompletion>, FocusedTerminalError> {
        let Some(receiver) = self.receiver.as_ref() else {
            return Ok(None);
        };
        match receiver.try_recv() {
            Ok(completion) => {
                self.finish_thread()?;
                Ok(Some(completion))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                self.finish_thread()?;
                Err(FocusedTerminalError::WorkerDisconnected)
            }
        }
    }

    fn shutdown(&mut self) -> Result<(), FocusedTerminalError> {
        self.finish_thread()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HitTarget {
    Sector(u32),
    Page(u8),
    Distribution(super::PageDistributionItemId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommittedHit {
    target: HitTarget,
    left: u16,
    top: u16,
    right: u16,
    bottom: u16,
}

impl CommittedHit {
    const fn contains(self, column: u16, row: u16) -> bool {
        column >= self.left && column <= self.right && row >= self.top && row <= self.bottom
    }

    const fn overlaps(self, other: Self) -> bool {
        self.left <= other.right
            && other.left <= self.right
            && self.top <= other.bottom
            && other.top <= self.bottom
    }
}

#[derive(Clone, Debug)]
struct LayoutCommit {
    generation: u64,
    surface: Surface,
    mode: FocusedMode,
    hits: Vec<CommittedHit>,
}

impl LayoutCommit {
    fn from_frame(
        generation: u64,
        mode: FocusedMode,
        frame: &VolumeFrame,
    ) -> Result<Self, FocusedTerminalError> {
        let mut hits = Vec::with_capacity(
            frame.hits.len() + frame.page_hits.len() + frame.distribution_hits.len(),
        );
        for hit in &frame.hits {
            hits.push(sector_hit(*hit)?);
        }
        hits.extend(frame.page_hits.iter().map(|hit| page_hit(*hit)));
        hits.extend(
            frame
                .distribution_hits
                .iter()
                .map(|hit| distribution_hit(*hit)),
        );
        let commit = Self {
            generation,
            surface: frame.surface,
            mode,
            hits,
        };
        commit.validate()?;
        Ok(commit)
    }

    fn validate(&self) -> Result<(), FocusedTerminalError> {
        for (index, hit) in self.hits.iter().enumerate() {
            let target_matches_mode = matches!(
                (self.mode, hit.target),
                (FocusedMode::Volume, HitTarget::Sector(_))
                    | (FocusedMode::Sector, HitTarget::Page(_))
                    | (FocusedMode::Page, HitTarget::Distribution(_))
            );
            if hit.left > hit.right
                || hit.top > hit.bottom
                || hit.right >= self.surface.width
                || hit.bottom >= self.surface.height
                || !target_matches_mode
                || self.hits[index + 1..]
                    .iter()
                    .any(|other| hit.overlaps(*other))
            {
                return Err(FocusedTerminalError::InvalidHitGeometry);
            }
        }
        Ok(())
    }

    fn actions(self, column: u16, row: u16, kind: PointerKind) -> Vec<FocusedAction> {
        if let PointerKind::Wheel(rows) = kind {
            return pointer_actions(self.mode, PointerInput::WheelRows(rows));
        }
        let Some(hit) = self
            .hits
            .iter()
            .find(|hit| hit.contains(column, row))
            .copied()
        else {
            return Vec::new();
        };
        match hit.target {
            HitTarget::Sector(sector) => {
                pointer_actions(self.mode, PointerInput::ActivateSector(sector))
            }
            HitTarget::Page(page) => pointer_actions(self.mode, PointerInput::ActivatePage(page)),
            HitTarget::Distribution(item) => {
                pointer_actions(self.mode, PointerInput::FocusDistributionItem(item))
            }
        }
    }
}

fn sector_hit(hit: HitRegion) -> Result<CommittedHit, FocusedTerminalError> {
    Ok(CommittedHit {
        target: HitTarget::Sector(
            u32::try_from(hit.sector_id).map_err(|_| FocusedTerminalError::InvalidHitGeometry)?,
        ),
        left: hit.left,
        top: hit.top,
        right: hit.right,
        bottom: hit.bottom,
    })
}

const fn page_hit(hit: PageHitRegion) -> CommittedHit {
    CommittedHit {
        target: HitTarget::Page(hit.page_index),
        left: hit.left,
        top: hit.top,
        right: hit.right,
        bottom: hit.bottom,
    }
}

const fn distribution_hit(hit: DistributionHitRegion) -> CommittedHit {
    CommittedHit {
        target: HitTarget::Distribution(hit.item),
        left: hit.left,
        top: hit.top,
        right: hit.right,
        bottom: hit.bottom,
    }
}

pub(super) struct FocusedRuntime {
    session: FocusedSession,
    physical_width: u16,
    physical_height: u16,
    surface: Surface,
    profile: PresentationProfile,
    next_generation: u64,
    committed: Option<LayoutCommit>,
    presented: Option<VolumeFrame>,
}

impl FocusedRuntime {
    pub(super) fn new(
        session: FocusedSession,
        width: u16,
        height: u16,
        profile: PresentationProfile,
    ) -> Self {
        Self {
            session,
            physical_width: width,
            physical_height: height,
            surface: bounded_surface(width, height),
            profile,
            next_generation: 0,
            committed: None,
            presented: None,
        }
    }

    pub(super) fn begin_resize(&mut self) {
        self.committed = None;
    }

    pub(super) fn finish_resize(&mut self, width: u16, height: u16) {
        self.physical_width = width;
        self.physical_height = height;
        self.surface = bounded_surface(width, height);
    }

    pub(super) fn generation(&self) -> Option<u64> {
        self.committed.as_ref().map(|commit| commit.generation)
    }

    #[cfg(test)]
    pub(super) fn retained_frame_cells(&self) -> usize {
        self.presented.as_ref().map_or(0, |frame| frame.cells.len())
    }

    #[cfg(test)]
    pub(super) fn retained_frame_count(&self) -> usize {
        usize::from(self.presented.is_some())
    }

    fn is_too_small(&self) -> bool {
        self.physical_width < super::MIN_WIDTH || self.physical_height < super::MIN_HEIGHT
    }

    pub(super) fn handle_event(&mut self, event: HostEvent) -> Result<bool, FocusedTerminalError> {
        match event {
            HostEvent::Key(StructuralKey::Quit) if self.is_too_small() => {
                let transition = self.session.advance_focused(
                    FocusedAction::Quit,
                    Surface::new(super::MIN_WIDTH, super::MIN_HEIGHT),
                )?;
                Ok(transition.changed)
            }
            HostEvent::Key(_) | HostEvent::Pointer { .. } if self.is_too_small() => Ok(false),
            HostEvent::Key(key) => Ok(self
                .session
                .advance_focused(key_action(key), self.surface)?
                .changed),
            HostEvent::Pointer {
                generation,
                column,
                row,
                kind,
            } => self.handle_pointer(generation, column, row, kind),
            HostEvent::Resize { .. } | HostEvent::Ignored => Ok(false),
        }
    }

    fn handle_pointer(
        &mut self,
        generation: Option<u64>,
        column: u16,
        row: u16,
        kind: PointerKind,
    ) -> Result<bool, FocusedTerminalError> {
        let Some(commit) = self
            .committed
            .as_ref()
            .filter(|commit| Some(commit.generation) == generation)
            .cloned()
        else {
            return Ok(false);
        };
        let mut changed = false;
        for action in commit.actions(column, row, kind) {
            changed |= self.session.advance_focused(action, self.surface)?.changed;
        }
        Ok(changed)
    }

    fn complete(
        &mut self,
        completion: FocusedEnrichmentCompletion,
    ) -> Result<bool, FocusedTerminalError> {
        Ok(self.session.complete_enrichment(completion)?.changed)
    }

    pub(super) fn render<H: TerminalHost>(
        &mut self,
        host: &mut H,
        force_clear: bool,
    ) -> Result<(), FocusedTerminalError> {
        let frame = self.compose()?;
        let generation = self.next_generation;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(FocusedTerminalError::GenerationOverflow)?;
        let commit =
            LayoutCommit::from_frame(generation, self.session.focused_state().mode, &frame)?;
        host.present(&frame, self.presented.as_ref(), force_clear)?;
        self.presented = Some(frame);
        self.committed = Some(commit);
        Ok(())
    }

    fn compose(&self) -> Result<VolumeFrame, FocusedTerminalError> {
        if self.is_too_small() {
            return Ok(too_small_frame(
                self.surface,
                self.profile,
                self.physical_width,
                self.physical_height,
            ));
        }
        let mut frame = match self.session.focused_state().mode {
            FocusedMode::Volume => {
                let layout = VolumeLayout::for_surface(self.surface)?;
                VolumeRenderer::render(&self.session.scene(layout)?, self.surface, self.profile)?
            }
            FocusedMode::Sector => {
                SectorRenderer::render(&self.session.sector_scene()?, self.surface, self.profile)?
            }
            FocusedMode::Page => {
                PageRenderer::render(&self.session.page_scene()?, self.surface, self.profile)?
            }
        };
        if self.session.focused_state().help_visible {
            draw_help_overlay(&mut frame, self.session.focused_state().mode, self.profile);
        }
        Ok(frame)
    }

    fn take_request(&mut self) -> Option<FocusedEnrichmentRequest> {
        self.session.take_enrichment_request()
    }

    fn quit_requested(&self) -> bool {
        self.session.focused_state().quit_requested
    }

    fn exit(self) -> FocusedExit {
        FocusedExit {
            view: self.session.current_view(),
            #[cfg(test)]
            state: self.session.focused_state(),
        }
    }
}

fn bounded_surface(width: u16, height: u16) -> Surface {
    Surface::new(width.min(MAX_WIDTH), height.min(MAX_HEIGHT))
}

fn too_small_frame(
    surface: Surface,
    profile: PresentationProfile,
    physical_width: u16,
    physical_height: u16,
) -> VolumeFrame {
    let mut frame = VolumeFrame::new(surface, profile);
    frame.put_text(
        0,
        0,
        surface.width,
        "VOLMAP focused inspector paused",
        SemanticStyle::Header,
    );
    if surface.height > 1 {
        frame.put_text(
            0,
            1,
            surface.width,
            &format!(
                "terminal {physical_width}x{physical_height}; resize to at least {}x{}",
                super::MIN_WIDTH,
                super::MIN_HEIGHT,
            ),
            SemanticStyle::Finding,
        );
    }
    if surface.height > 2 {
        frame.put_text(
            0,
            2,
            surface.width,
            "inspection state and revision are retained; q quits",
            SemanticStyle::Muted,
        );
    }
    frame
}

fn draw_help_overlay(frame: &mut VolumeFrame, mode: FocusedMode, profile: PresentationProfile) {
    let separator = if profile.glyphs == GlyphProfile::Unicode {
        " · "
    } else {
        " | "
    };
    let first_row = frame.surface.height.saturating_sub(4);
    let blank = " ".repeat(usize::from(frame.surface.width));
    for row in first_row..frame.surface.height {
        frame.put_text(0, row, frame.surface.width, &blank, SemanticStyle::Plain);
    }
    frame.put_text(
        0,
        first_row,
        frame.surface.width,
        &format!("Focused {mode:?} keys{separator}?/Esc close help"),
        SemanticStyle::Header,
    );
    if first_row + 1 < frame.surface.height {
        frame.put_text(
            0,
            first_row + 1,
            frame.surface.width,
            &format!("arrows move{separator}Enter descend/open{separator}Esc/Backspace return"),
            SemanticStyle::Plain,
        );
    }
    if first_row + 2 < frame.surface.height {
        frame.put_text(
            0,
            first_row + 2,
            frame.surface.width,
            &format!("[ ] sibling Sector{separator}PgUp/PgDn Volume{separator}wheel scroll"),
            SemanticStyle::Plain,
        );
    }
    if first_row + 3 < frame.surface.height {
        frame.put_text(
            0,
            first_row + 3,
            frame.surface.width,
            "q quits; all core inspection actions are available from the keyboard",
            SemanticStyle::Muted,
        );
    }
    frame.hits.clear();
    frame.page_hits.clear();
    frame.distribution_hits.clear();
}

/// Run the focused terminal host.
pub(crate) fn run(
    view: GraphView,
    policy: ResourcePolicy,
) -> Result<FocusedExit, FocusedTerminalError> {
    run_with_host(view, policy, CrosstermHost::new(), ChannelWorker::default())
}

fn run_with_host<H: TerminalHost, W: EnrichmentWorker>(
    view: GraphView,
    policy: ResourcePolicy,
    host: H,
    mut worker: W,
) -> Result<FocusedExit, FocusedTerminalError> {
    let mut terminal = TerminalLease::enter(host)?;
    let capabilities = terminal.host.capabilities();
    let (width, height) = terminal.host.size()?;
    let session = FocusedSession::new(view, policy)?;
    let mut runtime = FocusedRuntime::new(session, width, height, capabilities.profile());
    let drive_error = drive(&mut terminal.host, &mut worker, &mut runtime).err();
    let exit = drive_error.is_none().then(|| runtime.exit());
    let worker_result = worker.shutdown();
    let cleanup_result = terminal.close().map_err(FocusedTerminalError::Io);
    if let Some(error) = drive_error {
        return Err(error);
    }
    worker_result?;
    cleanup_result?;
    Ok(exit.expect("a successful drive creates an exit"))
}

fn drive<H: TerminalHost, W: EnrichmentWorker>(
    host: &mut H,
    worker: &mut W,
    runtime: &mut FocusedRuntime,
) -> Result<(), FocusedTerminalError> {
    let mut pending_inputs = VecDeque::new();
    runtime.render(host, true)?;
    dispatch_request(runtime, worker)?;
    loop {
        let event = if let Some(event) = pending_inputs.pop_front() {
            Some(event)
        } else if host.poll_event(INPUT_WAIT)? {
            Some(host.read_event(runtime.generation())?)
        } else {
            None
        };

        if let Some(event) = event {
            if let HostEvent::Resize { width, height } = event {
                runtime.begin_resize();
                let (width, height) = coalesce_resize(
                    host,
                    runtime.generation(),
                    width,
                    height,
                    &mut pending_inputs,
                )?;
                runtime.finish_resize(width, height);
                runtime.render(host, true)?;
            } else {
                let changed = runtime.handle_event(event)?;
                if runtime.quit_requested() {
                    return Ok(());
                }
                if changed {
                    runtime.render(host, false)?;
                }
            }
            dispatch_request(runtime, worker)?;
            if runtime.quit_requested() {
                return Ok(());
            }
            continue;
        }

        if let Some(completion) = worker.try_completion()?
            && runtime.complete(completion)?
        {
            runtime.render(host, false)?;
            dispatch_request(runtime, worker)?;
        }
    }
}

fn dispatch_request<W: EnrichmentWorker>(
    runtime: &mut FocusedRuntime,
    worker: &mut W,
) -> Result<(), FocusedTerminalError> {
    if let Some(request) = runtime.take_request() {
        worker.start(request)?;
    }
    Ok(())
}

fn coalesce_resize<H: TerminalHost>(
    host: &mut H,
    generation: Option<u64>,
    mut width: u16,
    mut height: u16,
    pending_inputs: &mut VecDeque<HostEvent>,
) -> Result<(u16, u16), FocusedTerminalError> {
    while host.poll_event(Duration::ZERO)? {
        match host.read_event(generation)? {
            HostEvent::Resize {
                width: next_width,
                height: next_height,
            } => {
                width = next_width;
                height = next_height;
            }
            event => {
                pending_inputs.push_back(event);
                break;
            }
        }
    }
    Ok((width, height))
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HostOperation {
    EnableRaw,
    EnterAlternate,
    EnableMouse,
    HideCursor,
    Present,
    ShowCursor,
    DisableMouse,
    LeaveAlternate,
    DisableRaw,
}

#[cfg(test)]
#[derive(Clone)]
pub(super) struct ScriptObserver(std::sync::Arc<std::sync::Mutex<ScriptObservation>>);

#[cfg(test)]
#[derive(Default)]
struct ScriptObservation {
    operations: Vec<HostOperation>,
    presentations: Vec<String>,
}

#[cfg(test)]
impl ScriptObserver {
    pub(super) fn operations(&self) -> Vec<HostOperation> {
        self.0.lock().unwrap().operations.clone()
    }

    pub(super) fn presentations(&self) -> Vec<String> {
        self.0.lock().unwrap().presentations.clone()
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) enum ScriptPoll {
    Event(HostEvent),
    Idle,
}

#[cfg(test)]
pub(super) struct ScriptHost {
    observer: ScriptObserver,
    terminal: bool,
    capabilities: TerminalCapabilities,
    size: (u16, u16),
    polls: VecDeque<ScriptPoll>,
    fail_on: Option<(HostOperation, usize)>,
}

#[cfg(test)]
impl ScriptHost {
    pub(super) fn new(
        size: (u16, u16),
        capabilities: TerminalCapabilities,
        polls: impl IntoIterator<Item = ScriptPoll>,
    ) -> (Self, ScriptObserver) {
        let observer = ScriptObserver(std::sync::Arc::new(std::sync::Mutex::new(
            ScriptObservation::default(),
        )));
        (
            Self {
                observer: observer.clone(),
                terminal: true,
                capabilities,
                size,
                polls: polls.into_iter().collect(),
                fail_on: None,
            },
            observer,
        )
    }

    pub(super) fn fail_on(mut self, operation: HostOperation) -> Self {
        self.fail_on = Some((operation, 1));
        self
    }

    pub(super) fn fail_on_occurrence(
        mut self,
        operation: HostOperation,
        occurrence: usize,
    ) -> Self {
        self.fail_on = Some((operation, occurrence));
        self
    }

    pub(super) fn not_terminal(mut self) -> Self {
        self.terminal = false;
        self
    }

    fn operation(&mut self, operation: HostOperation) -> io::Result<()> {
        let occurrence = {
            let mut observation = self.observer.0.lock().unwrap();
            observation.operations.push(operation);
            observation
                .operations
                .iter()
                .filter(|candidate| **candidate == operation)
                .count()
        };
        if self.fail_on == Some((operation, occurrence)) {
            Err(io::Error::other("scripted terminal failure"))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
impl TerminalHost for ScriptHost {
    fn is_terminal(&self) -> bool {
        self.terminal
    }

    fn capabilities(&self) -> TerminalCapabilities {
        self.capabilities
    }

    fn size(&mut self) -> io::Result<(u16, u16)> {
        Ok(self.size)
    }

    fn enable_raw(&mut self) -> io::Result<()> {
        self.operation(HostOperation::EnableRaw)
    }

    fn disable_raw(&mut self) -> io::Result<()> {
        self.operation(HostOperation::DisableRaw)
    }

    fn enter_alternate(&mut self) -> io::Result<()> {
        self.operation(HostOperation::EnterAlternate)
    }

    fn leave_alternate(&mut self) -> io::Result<()> {
        self.operation(HostOperation::LeaveAlternate)
    }

    fn enable_mouse(&mut self) -> io::Result<()> {
        self.operation(HostOperation::EnableMouse)
    }

    fn disable_mouse(&mut self) -> io::Result<()> {
        self.operation(HostOperation::DisableMouse)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.operation(HostOperation::HideCursor)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.operation(HostOperation::ShowCursor)
    }

    fn poll_event(&mut self, _timeout: Duration) -> io::Result<bool> {
        match self.polls.front() {
            Some(ScriptPoll::Event(_)) => Ok(true),
            Some(ScriptPoll::Idle) => {
                self.polls.pop_front();
                Ok(false)
            }
            None => Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "scripted input exhausted before quit",
            )),
        }
    }

    fn read_event(&mut self, _generation: Option<u64>) -> io::Result<HostEvent> {
        match self.polls.pop_front() {
            Some(ScriptPoll::Event(event)) => Ok(event),
            Some(ScriptPoll::Idle) | None => Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "scripted event was unavailable",
            )),
        }
    }

    fn present(
        &mut self,
        frame: &VolumeFrame,
        _previous: Option<&VolumeFrame>,
        _force_clear: bool,
    ) -> io::Result<()> {
        self.operation(HostOperation::Present)?;
        self.observer
            .0
            .lock()
            .unwrap()
            .presentations
            .push(frame.semantic_snapshot());
        Ok(())
    }
}

#[cfg(test)]
#[derive(Default)]
pub(super) struct InlineWorker {
    completion: Option<FocusedEnrichmentCompletion>,
}

#[cfg(test)]
impl EnrichmentWorker for InlineWorker {
    fn start(&mut self, request: FocusedEnrichmentRequest) -> Result<(), FocusedTerminalError> {
        if self.completion.is_some() {
            return Err(FocusedTerminalError::WorkerBusy);
        }
        self.completion = Some(request.execute());
        Ok(())
    }

    fn try_completion(
        &mut self,
    ) -> Result<Option<FocusedEnrichmentCompletion>, FocusedTerminalError> {
        Ok(self.completion.take())
    }

    fn shutdown(&mut self) -> Result<(), FocusedTerminalError> {
        self.completion = None;
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn run_scripted(
    view: GraphView,
    policy: ResourcePolicy,
    host: ScriptHost,
) -> Result<FocusedExit, FocusedTerminalError> {
    run_with_host(view, policy, host, InlineWorker::default())
}

#[cfg(test)]
pub(super) fn effective_surface(width: u16, height: u16) -> Surface {
    bounded_surface(width, height)
}
