//! Focused-TUI session, renderer, and terminal host for Volume, Sector, and
//! Page inspection.

mod terminal;

pub(crate) use terminal::{FocusedExit, FocusedTerminalError, run};

use std::fmt;

#[cfg(test)]
use std::fmt::Write as _;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::inspection::{
    CancelToken, GraphView, OperationError, QueryError, RecordSelectionSupport, ResourcePolicy,
    VolumeView,
};
use crate::model::{InspectionRevision, Oid, SectorId, SlotId, SnapshotId, VolId, Vpid};
use crate::projection::{
    AttributeNameProjection, AttributeValueProjection, ByteRegionProjection, ClassNameProjection,
    FileAssociationBodyProjection, FileAssociationProjection, FreeRegionKindProjection,
    FreeRegionProjection, OidProjection, OptionalOidProjection, OptionalTextProjection,
    PageDistributionProjection, PageOccupancyProjection, PageProjection, RecordExtentProjection,
    RecordSelectionProjection, RecordTypeProjection, SectorAttributionProjection, SectorProjection,
    SlotEntryProjection, SlotEntryStateProjection, outcome_name, page_distribution_projection,
    page_projection, record_selection_projection, sector_projection, snapshot_id_hex,
    volume_projection,
};

const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 20;
const CARD_WIDTH: u16 = 19;
const CARD_STRIDE: u16 = 20;
const CARD_HEIGHT: u16 = 11;
const CARD_TOP: u16 = 4;
const RESERVED_ROWS: u16 = 7;
const PAGE_COUNT: usize = 64;
const SECTOR_GRID_TOP: u16 = 4;
const SECTOR_GRID_ROWS: u16 = 8;
const SECTOR_GRID_COLUMNS: u16 = 8;
const PAGE_ROWS_TOP: u16 = 7;
const PAGE_RESERVED_BOTTOM_ROWS: u16 = 3;
const INTERPRETATION_ROWS_TOP: u16 = 8;

fn page_visible_rows(surface: Surface) -> u16 {
    surface
        .height
        .saturating_sub(PAGE_ROWS_TOP + PAGE_RESERVED_BOTTOM_ROWS)
        .max(1)
}

fn interpretation_visible_rows(surface: Surface) -> u16 {
    surface
        .height
        .saturating_sub(INTERPRETATION_ROWS_TOP + PAGE_RESERVED_BOTTOM_ROWS)
        .max(1)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Surface {
    pub width: u16,
    pub height: u16,
}

impl Surface {
    pub(crate) const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VolumeLayout {
    pub columns: u16,
    pub visible_rows: u16,
}

impl VolumeLayout {
    pub(crate) fn for_surface(surface: Surface) -> Result<Self, FocusedError> {
        if surface.width < MIN_WIDTH || surface.height < MIN_HEIGHT {
            return Err(FocusedError::SurfaceTooSmall {
                width: surface.width,
                height: surface.height,
            });
        }
        let content_height = surface.height.saturating_sub(RESERVED_ROWS);
        Ok(Self {
            columns: (surface.width / CARD_STRIDE).max(1),
            visible_rows: (content_height / CARD_HEIGHT).max(1),
        })
    }

    fn visible_capacity(self) -> u32 {
        u32::from(self.columns) * u32::from(self.visible_rows)
    }

    fn projection_capacity(self) -> u32 {
        self.visible_capacity() + u32::from(self.columns)
    }
}

#[derive(Debug)]
pub(crate) enum FocusedError {
    Query(QueryError),
    EmptyInspection,
    Arithmetic,
    InvalidAllocation(&'static str),
    InvalidSectorPageCount {
        sector_id: i32,
        actual: usize,
    },
    InvalidPhysicalPageOrder {
        expected: i32,
        actual: i32,
    },
    InvalidEnrichmentSnapshot,
    InvalidEnrichmentRevision {
        expected: u64,
        actual: u64,
    },
    InvalidEnrichmentPage,
    WrongMode {
        expected: FocusedMode,
        actual: FocusedMode,
    },
    SurfaceTooSmall {
        width: u16,
        height: u16,
    },
}

impl fmt::Display for FocusedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Query(error) => write!(formatter, "focused TUI query failed: {error}"),
            Self::EmptyInspection => formatter.write_str("focused TUI requires one volume"),
            Self::Arithmetic => formatter.write_str("focused TUI arithmetic overflow"),
            Self::InvalidAllocation(value) => {
                write!(formatter, "focused TUI received invalid allocation {value}")
            }
            Self::InvalidSectorPageCount { sector_id, actual } => write!(
                formatter,
                "sector {sector_id} projected {actual} pages instead of 64"
            ),
            Self::InvalidPhysicalPageOrder { expected, actual } => write!(
                formatter,
                "projected page order is invalid: expected {expected}, found {actual}"
            ),
            Self::InvalidEnrichmentSnapshot => {
                formatter.write_str("Page enrichment returned a different snapshot")
            }
            Self::InvalidEnrichmentRevision { expected, actual } => write!(
                formatter,
                "Page enrichment returned revision {actual}, expected {expected}"
            ),
            Self::InvalidEnrichmentPage => {
                formatter.write_str("Page enrichment could not re-resolve the focused path")
            }
            Self::WrongMode { expected, actual } => {
                write!(
                    formatter,
                    "focused TUI expected {expected:?} mode, found {actual:?}"
                )
            }
            Self::SurfaceTooSmall { width, height } => write!(
                formatter,
                "focused TUI requires at least 60x20, found {width}x{height}"
            ),
        }
    }
}

impl std::error::Error for FocusedError {}

impl From<QueryError> for FocusedError {
    fn from(value: QueryError) -> Self {
        Self::Query(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VolumeState {
    pub snapshot_id: SnapshotId,
    pub revision: InspectionRevision,
    pub volume_id: VolId,
    pub volume_index: usize,
    pub volume_count: usize,
    pub focused_sector: u32,
    pub top_sector: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FocusedMode {
    Volume,
    Sector,
    Page,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PageLoadState {
    Idle,
    Loading,
    Ready,
    Unavailable(&'static str),
    Failed(PageEnrichmentFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InterpretationLoadState {
    Loading,
    Ready,
    Unavailable(&'static str),
    Failed(PageEnrichmentFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PageInterpretationState {
    Closed,
    Record {
        record: Oid,
        load: InterpretationLoadState,
        top_attribute: u32,
    },
}

impl PageLoadState {
    const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading => "loading structure",
            Self::Ready => "structure ready",
            Self::Unavailable(reason) => reason,
            Self::Failed(failure) => failure.label(),
        }
    }
}

impl InterpretationLoadState {
    const fn label(self) -> &'static str {
        match self {
            Self::Loading => "loading interpretation",
            Self::Ready => "interpretation ready",
            Self::Unavailable(reason) => reason,
            Self::Failed(failure) => failure.label(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PageEnrichmentFailure {
    RevisionNotFound,
    Source,
    Query,
    Interrupted,
    Unsupported,
    Structural,
    ResourceLimit,
    FactStore,
    Arithmetic,
}

impl PageEnrichmentFailure {
    const fn label(self) -> &'static str {
        match self {
            Self::RevisionNotFound => "base revision is unavailable",
            Self::Source => "source read failed",
            Self::Query => "Page is no longer addressable",
            Self::Interrupted => "Page enrichment cancelled",
            Self::Unsupported => "Page distribution is unsupported",
            Self::Structural => "Page structure is invalid",
            Self::ResourceLimit => "Page enrichment reached its resource limit",
            Self::FactStore => "Page facts are unavailable",
            Self::Arithmetic => "Page enrichment overflowed",
        }
    }
}

impl From<&OperationError> for PageEnrichmentFailure {
    fn from(value: &OperationError) -> Self {
        match value {
            OperationError::RevisionNotFound => Self::RevisionNotFound,
            OperationError::Source(_) => Self::Source,
            OperationError::Query(_) => Self::Query,
            OperationError::Interrupted => Self::Interrupted,
            OperationError::Unsupported => Self::Unsupported,
            OperationError::Structural(_) => Self::Structural,
            OperationError::ResourceLimit => Self::ResourceLimit,
            OperationError::FactStore => Self::FactStore,
            OperationError::Arithmetic => Self::Arithmetic,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum PageDistributionItemId {
    Header,
    Record(u16),
    FragmentedFree { offset: u32, length: u32 },
    ContiguousFree { offset: u32, length: u32 },
    SlotDirectory,
    SlotEntry(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FocusedState {
    pub volume: VolumeState,
    pub mode: FocusedMode,
    pub focused_page: u8,
    pub page_load: PageLoadState,
    pub selected_distribution_item: Option<PageDistributionItemId>,
    pub top_distribution_item: Option<PageDistributionItemId>,
    pub interpretation: PageInterpretationState,
    pub help_visible: bool,
    pub quit_requested: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VolumeAction {
    Left,
    Right,
    Up,
    Down,
    PreviousSector,
    NextSector,
    PreviousVolume,
    NextVolume,
    ScrollRows(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(test)]
pub(crate) struct Transition {
    pub changed: bool,
    pub state: VolumeState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FocusedAction {
    Left,
    Right,
    Up,
    Down,
    Activate,
    Ascend,
    PreviousSector,
    NextSector,
    PreviousVolume,
    NextVolume,
    ScrollRows(i32),
    FocusSector(u32),
    FocusPage(u8),
    FocusDistributionItem(PageDistributionItemId),
    ToggleHelp,
    Quit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FocusedTransition {
    pub changed: bool,
    pub state: FocusedState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PointerInput {
    ActivateSector(u32),
    ActivatePage(u8),
    FocusDistributionItem(PageDistributionItemId),
    WheelRows(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StructuralKey {
    Left,
    Right,
    Up,
    Down,
    Enter,
    Escape,
    Backspace,
    PreviousSector,
    NextSector,
    PreviousVolume,
    NextVolume,
    Help,
    Quit,
}

pub(crate) const fn key_action(key: StructuralKey) -> FocusedAction {
    match key {
        StructuralKey::Left => FocusedAction::Left,
        StructuralKey::Right => FocusedAction::Right,
        StructuralKey::Up => FocusedAction::Up,
        StructuralKey::Down => FocusedAction::Down,
        StructuralKey::Enter => FocusedAction::Activate,
        StructuralKey::Escape | StructuralKey::Backspace => FocusedAction::Ascend,
        StructuralKey::PreviousSector => FocusedAction::PreviousSector,
        StructuralKey::NextSector => FocusedAction::NextSector,
        StructuralKey::PreviousVolume => FocusedAction::PreviousVolume,
        StructuralKey::NextVolume => FocusedAction::NextVolume,
        StructuralKey::Help => FocusedAction::ToggleHelp,
        StructuralKey::Quit => FocusedAction::Quit,
    }
}

pub(crate) fn pointer_actions(mode: FocusedMode, input: PointerInput) -> Vec<FocusedAction> {
    match input {
        PointerInput::ActivateSector(sector) => {
            vec![FocusedAction::FocusSector(sector), FocusedAction::Activate]
        }
        PointerInput::ActivatePage(page) => {
            vec![FocusedAction::FocusPage(page), FocusedAction::Activate]
        }
        PointerInput::FocusDistributionItem(item) => {
            vec![FocusedAction::FocusDistributionItem(item)]
        }
        PointerInput::WheelRows(rows) if mode == FocusedMode::Volume => {
            vec![FocusedAction::ScrollRows(rows)]
        }
        PointerInput::WheelRows(rows) if mode == FocusedMode::Page => {
            vec![FocusedAction::ScrollRows(rows)]
        }
        PointerInput::WheelRows(rows) if rows.is_negative() => {
            vec![FocusedAction::PreviousSector]
        }
        PointerInput::WheelRows(rows) if rows > 0 => vec![FocusedAction::NextSector],
        PointerInput::WheelRows(_) => Vec::new(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EnrichmentRequestTarget {
    Page(Vpid),
    Record(Oid),
}

impl EnrichmentRequestTarget {
    const fn page(self) -> Vpid {
        match self {
            Self::Page(page) => page,
            Self::Record(record) => Vpid::new(record.vol_id, record.page_id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EnrichmentRequestKey {
    request_id: u64,
    snapshot_id: SnapshotId,
    base_revision: InspectionRevision,
    target: EnrichmentRequestTarget,
}

#[derive(Clone, Debug)]
struct ActiveEnrichmentRequest {
    key: EnrichmentRequestKey,
    cancel: CancelToken,
}

/// One bounded focused-TUI enrichment job for the eventual terminal host.
/// The semantic session retains only its identity and cancellation handle.
#[derive(Debug)]
pub(crate) struct FocusedEnrichmentRequest {
    key: EnrichmentRequestKey,
    base: GraphView,
    policy: ResourcePolicy,
    cancel: CancelToken,
}

impl FocusedEnrichmentRequest {
    #[cfg(test)]
    pub(crate) const fn snapshot_id(&self) -> SnapshotId {
        self.key.snapshot_id
    }

    #[cfg(test)]
    pub(crate) const fn base_revision(&self) -> InspectionRevision {
        self.key.base_revision
    }

    #[cfg(test)]
    pub(crate) const fn target(&self) -> EnrichmentRequestTarget {
        self.key.target
    }

    #[cfg(test)]
    pub(crate) const fn page(&self) -> Vpid {
        self.key.target.page()
    }

    pub(crate) fn execute(self) -> FocusedEnrichmentCompletion {
        let result = match self.key.target {
            EnrichmentRequestTarget::Page(page) => {
                self.base.enrich_page(page, self.policy, &self.cancel)
            }
            EnrichmentRequestTarget::Record(record) => {
                self.base
                    .enrich_record_selection(record, self.policy, &self.cancel)
            }
        };
        FocusedEnrichmentCompletion {
            key: self.key,
            result,
        }
    }
}

#[derive(Debug)]
pub(crate) struct FocusedEnrichmentCompletion {
    key: EnrichmentRequestKey,
    result: Result<GraphView, OperationError>,
}

#[derive(Debug)]
pub(crate) struct FocusedSession {
    view: GraphView,
    volumes: Vec<VolumeView>,
    policy: ResourcePolicy,
    volume_index: usize,
    focused_sector: u32,
    top_sector: u32,
    mode: FocusedMode,
    focused_page: u8,
    page_load: PageLoadState,
    page_distribution: PageDistributionProjection,
    selected_distribution_item: Option<PageDistributionItemId>,
    top_distribution_item: Option<PageDistributionItemId>,
    interpretation: PageInterpretationState,
    help_visible: bool,
    next_enrichment_request_id: u64,
    active_enrichment_request: Option<ActiveEnrichmentRequest>,
    pending_enrichment_request: Option<FocusedEnrichmentRequest>,
    quit_requested: bool,
}

impl FocusedSession {
    pub(crate) fn new(view: GraphView, policy: ResourcePolicy) -> Result<Self, FocusedError> {
        let volumes = view.volumes();
        if volumes.is_empty() {
            return Err(FocusedError::EmptyInspection);
        }
        Ok(Self {
            view,
            volumes,
            policy,
            volume_index: 0,
            focused_sector: 0,
            top_sector: 0,
            mode: FocusedMode::Volume,
            focused_page: 0,
            page_load: PageLoadState::Idle,
            page_distribution: PageDistributionProjection::NotAvailable,
            selected_distribution_item: None,
            top_distribution_item: None,
            interpretation: PageInterpretationState::Closed,
            help_visible: false,
            next_enrichment_request_id: 0,
            active_enrichment_request: None,
            pending_enrichment_request: None,
            quit_requested: false,
        })
    }

    pub(crate) fn state(&self) -> VolumeState {
        let overview = self.view.overview();
        VolumeState {
            snapshot_id: overview.snapshot_id,
            revision: overview.revision,
            volume_id: self.volumes[self.volume_index].vol_id,
            volume_index: self.volume_index,
            volume_count: self.volumes.len(),
            focused_sector: self.focused_sector,
            top_sector: self.top_sector,
        }
    }

    pub(crate) fn focused_state(&self) -> FocusedState {
        FocusedState {
            volume: self.state(),
            mode: self.mode,
            focused_page: self.focused_page,
            page_load: self.page_load,
            selected_distribution_item: self.selected_distribution_item,
            top_distribution_item: self.top_distribution_item,
            interpretation: self.interpretation,
            help_visible: self.help_visible,
            quit_requested: self.quit_requested,
        }
    }

    pub(crate) fn current_view(&self) -> GraphView {
        self.view.clone()
    }

    pub(crate) fn advance_focused(
        &mut self,
        action: FocusedAction,
        surface: Surface,
    ) -> Result<FocusedTransition, FocusedError> {
        let before = self.focused_state();
        let layout = VolumeLayout::for_surface(surface)?;
        self.apply_focused_action(action, surface, layout)?;
        let state = self.focused_state();
        Ok(FocusedTransition {
            changed: state != before,
            state,
        })
    }

    fn apply_focused_action(
        &mut self,
        action: FocusedAction,
        surface: Surface,
        layout: VolumeLayout,
    ) -> Result<(), FocusedError> {
        if self.help_visible {
            match action {
                FocusedAction::ToggleHelp | FocusedAction::Ascend => self.help_visible = false,
                FocusedAction::Quit => self.quit(),
                _ => {}
            }
            return Ok(());
        }
        if action == FocusedAction::ToggleHelp {
            self.help_visible = true;
            return Ok(());
        }
        match self.mode {
            FocusedMode::Volume => self.apply_volume_mode_action(action, layout),
            FocusedMode::Sector => self.apply_sector_mode_action(action, layout)?,
            FocusedMode::Page => self.apply_page_mode_action(action, surface, layout)?,
        }
        Ok(())
    }

    fn apply_volume_mode_action(&mut self, action: FocusedAction, layout: VolumeLayout) {
        let volume_action = match action {
            FocusedAction::Left => Some(VolumeAction::Left),
            FocusedAction::Right => Some(VolumeAction::Right),
            FocusedAction::Up => Some(VolumeAction::Up),
            FocusedAction::Down => Some(VolumeAction::Down),
            FocusedAction::PreviousSector => Some(VolumeAction::PreviousSector),
            FocusedAction::NextSector => Some(VolumeAction::NextSector),
            FocusedAction::PreviousVolume => Some(VolumeAction::PreviousVolume),
            FocusedAction::NextVolume => Some(VolumeAction::NextVolume),
            FocusedAction::ScrollRows(rows) => Some(VolumeAction::ScrollRows(rows)),
            _ => None,
        };
        if let Some(action) = volume_action {
            self.apply_volume_action(action, layout);
            return;
        }
        match action {
            FocusedAction::Activate => {
                self.mode = FocusedMode::Sector;
                self.focused_page = 0;
            }
            FocusedAction::FocusSector(sector) if sector < self.total_sectors() => {
                self.focused_sector = sector;
                self.reveal_focus(layout);
            }
            FocusedAction::Quit => self.quit(),
            _ => {}
        }
    }

    fn apply_sector_mode_action(
        &mut self,
        action: FocusedAction,
        layout: VolumeLayout,
    ) -> Result<(), FocusedError> {
        match action {
            FocusedAction::Left => self.move_page_horizontal(false),
            FocusedAction::Right => self.move_page_horizontal(true),
            FocusedAction::Up => self.move_page_vertical(false),
            FocusedAction::Down => self.move_page_vertical(true),
            FocusedAction::Activate => {
                self.mode = FocusedMode::Page;
                self.prepare_focused_page()?;
            }
            FocusedAction::Ascend => self.mode = FocusedMode::Volume,
            FocusedAction::PreviousSector => {
                self.apply_volume_action(VolumeAction::PreviousSector, layout);
            }
            FocusedAction::NextSector => {
                self.apply_volume_action(VolumeAction::NextSector, layout);
            }
            FocusedAction::FocusSector(sector) if sector < self.total_sectors() => {
                self.focused_sector = sector;
                self.reveal_focus(layout);
            }
            FocusedAction::FocusPage(page) if page < 64 => self.focused_page = page,
            FocusedAction::Quit => self.quit(),
            _ => {}
        }
        Ok(())
    }

    fn apply_page_mode_action(
        &mut self,
        action: FocusedAction,
        surface: Surface,
        layout: VolumeLayout,
    ) -> Result<(), FocusedError> {
        match action {
            FocusedAction::Up if self.interpretation == PageInterpretationState::Closed => {
                self.move_distribution_focus(false, surface)?;
            }
            FocusedAction::Down if self.interpretation == PageInterpretationState::Closed => {
                self.move_distribution_focus(true, surface)?;
            }
            FocusedAction::Up => self.scroll_interpretation(-1, surface),
            FocusedAction::Down => self.scroll_interpretation(1, surface),
            FocusedAction::Activate => self.activate_page_selection()?,
            FocusedAction::Ascend => {
                if self.interpretation == PageInterpretationState::Closed {
                    self.leave_page();
                    self.mode = FocusedMode::Sector;
                } else {
                    self.close_interpretation();
                }
            }
            FocusedAction::PreviousSector => self.move_page_to_sibling_sector(false, layout)?,
            FocusedAction::NextSector => self.move_page_to_sibling_sector(true, layout)?,
            FocusedAction::PreviousVolume => self.move_page_to_sibling_volume(false)?,
            FocusedAction::NextVolume => self.move_page_to_sibling_volume(true)?,
            FocusedAction::ScrollRows(rows)
                if self.interpretation == PageInterpretationState::Closed =>
            {
                self.scroll_distribution(rows, surface)?;
            }
            FocusedAction::ScrollRows(rows) => self.scroll_interpretation(rows, surface),
            FocusedAction::FocusDistributionItem(item)
                if self.interpretation == PageInterpretationState::Closed =>
            {
                self.focus_distribution_item(item, surface)?;
            }
            FocusedAction::FocusSector(sector)
                if sector < self.total_sectors() && sector != self.focused_sector =>
            {
                self.leave_page();
                self.focused_sector = sector;
                self.reveal_focus(layout);
                self.mode = FocusedMode::Page;
                self.prepare_focused_page()?;
            }
            FocusedAction::Quit => self.quit(),
            _ => {}
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn advance(&mut self, action: VolumeAction, layout: VolumeLayout) -> Transition {
        let before = self.state();
        self.apply_volume_action(action, layout);
        let state = self.state();
        Transition {
            changed: state != before,
            state,
        }
    }

    fn apply_volume_action(&mut self, action: VolumeAction, layout: VolumeLayout) {
        match action {
            VolumeAction::Left => {
                let column =
                    self.focused_sector.saturating_sub(self.top_sector) % u32::from(layout.columns);
                if column != 0 {
                    self.focused_sector = self.focused_sector.saturating_sub(1);
                    self.reveal_focus(layout);
                }
            }
            VolumeAction::Right => {
                let column =
                    self.focused_sector.saturating_sub(self.top_sector) % u32::from(layout.columns);
                if column + 1 < u32::from(layout.columns) {
                    self.move_focus_forward(1, layout);
                }
            }
            VolumeAction::Up => {
                let amount = u32::from(layout.columns);
                if self.focused_sector >= amount {
                    self.focused_sector -= amount;
                    self.reveal_focus(layout);
                }
            }
            VolumeAction::Down => self.move_focus_forward(u32::from(layout.columns), layout),
            VolumeAction::PreviousSector => {
                if self.focused_sector > 0 {
                    self.focused_sector -= 1;
                    self.reveal_focus(layout);
                }
            }
            VolumeAction::NextSector => self.move_focus_forward(1, layout),
            VolumeAction::PreviousVolume => self.move_volume(false),
            VolumeAction::NextVolume => self.move_volume(true),
            VolumeAction::ScrollRows(rows) => self.scroll_rows(rows, layout),
        }
    }

    pub(crate) fn scene(&self, layout: VolumeLayout) -> Result<VolumeScene, FocusedError> {
        let overview = self.view.overview();
        let volume = self.volumes[self.volume_index];
        let end = self
            .top_sector
            .saturating_add(layout.projection_capacity())
            .min(volume.total_sectors);
        let sectors = (self.top_sector..end)
            .map(|ordinal| {
                let raw = i32::try_from(ordinal).map_err(|_| FocusedError::Arithmetic)?;
                let sector_id = SectorId::new(raw).map_err(|_| FocusedError::Arithmetic)?;
                let projection = sector_projection(self.view.sector(volume.vol_id, sector_id)?);
                SectorCard::try_from_projection(projection)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(VolumeScene {
            snapshot_id: overview.snapshot_id,
            revision: overview.revision,
            outcome: outcome_name(overview.outcome),
            volume: volume_projection(volume),
            volume_index: self.volume_index,
            volume_count: self.volumes.len(),
            focused_sector: self.focused_sector,
            top_sector: self.top_sector,
            layout,
            sectors,
        })
    }

    pub(crate) fn sector_scene(&self) -> Result<SectorScene, FocusedError> {
        if self.mode != FocusedMode::Sector {
            return Err(FocusedError::WrongMode {
                expected: FocusedMode::Sector,
                actual: self.mode,
            });
        }
        let overview = self.view.overview();
        let volume = self.volumes[self.volume_index];
        let raw = i32::try_from(self.focused_sector).map_err(|_| FocusedError::Arithmetic)?;
        let sector_id = SectorId::new(raw).map_err(|_| FocusedError::Arithmetic)?;
        let sector = SectorCard::try_from_projection(sector_projection(
            self.view.sector(volume.vol_id, sector_id)?,
        ))?;
        Ok(SectorScene {
            snapshot_id: overview.snapshot_id,
            revision: overview.revision,
            outcome: outcome_name(overview.outcome),
            volume: volume_projection(volume),
            volume_index: self.volume_index,
            volume_count: self.volumes.len(),
            focused_page: self.focused_page,
            sector,
        })
    }

    pub(crate) fn page_scene(&self) -> Result<PageScene, FocusedError> {
        if self.mode != FocusedMode::Page {
            return Err(FocusedError::WrongMode {
                expected: FocusedMode::Page,
                actual: self.mode,
            });
        }
        let overview = self.view.overview();
        let volume = self.volumes[self.volume_index];
        let vpid = self.focused_vpid()?;
        let page = PageMark::try_from_projection(&page_projection(self.view.page(vpid)?))?;
        let items = PageDistributionItem::from_projection(vpid, &self.page_distribution)?;
        let record_selection = match self.interpretation {
            PageInterpretationState::Record {
                record,
                load:
                    InterpretationLoadState::Ready
                    | InterpretationLoadState::Unavailable(_)
                    | InterpretationLoadState::Failed(_),
                ..
            } => record_selection_projection(&self.view, record),
            PageInterpretationState::Closed
            | PageInterpretationState::Record {
                load: InterpretationLoadState::Loading,
                ..
            } => None,
        };
        Ok(PageScene {
            snapshot_id: overview.snapshot_id,
            revision: overview.revision,
            outcome: outcome_name(overview.outcome),
            volume: volume_projection(volume),
            volume_index: self.volume_index,
            volume_count: self.volumes.len(),
            sector_id: i32::try_from(self.focused_sector).map_err(|_| FocusedError::Arithmetic)?,
            page,
            load: self.page_load,
            distribution: self.page_distribution.clone(),
            items,
            selected_item: self.selected_distribution_item,
            top_item: self.top_distribution_item,
            interpretation_state: self.interpretation,
            record_selection,
        })
    }

    pub(crate) fn take_enrichment_request(&mut self) -> Option<FocusedEnrichmentRequest> {
        self.pending_enrichment_request.take()
    }

    pub(crate) fn complete_enrichment(
        &mut self,
        completion: FocusedEnrichmentCompletion,
    ) -> Result<FocusedTransition, FocusedError> {
        let before = self.focused_state();
        let Some(active) = self.active_enrichment_request.as_ref() else {
            return Ok(FocusedTransition {
                changed: false,
                state: before,
            });
        };
        let target_page = completion.key.target.page();
        let target_still_active = match completion.key.target {
            EnrichmentRequestTarget::Page(_) => {
                self.interpretation == PageInterpretationState::Closed
            }
            EnrichmentRequestTarget::Record(record) => {
                matches!(
                    self.interpretation,
                    PageInterpretationState::Record {
                        record: active_record,
                        ..
                    } if active_record == record
                ) && self
                    .selected_record()?
                    .is_some_and(|(selected, _)| selected == record)
            }
        };
        if completion.key != active.key
            || active.cancel.is_cancelled()
            || self.mode != FocusedMode::Page
            || self.focused_vpid()? != target_page
            || self.view.overview().snapshot_id != completion.key.snapshot_id
            || self.view.overview().revision != completion.key.base_revision
            || !target_still_active
        {
            return Ok(FocusedTransition {
                changed: false,
                state: before,
            });
        }

        self.active_enrichment_request = None;
        self.pending_enrichment_request = None;
        match completion.result {
            Ok(candidate) => match completion.key.target {
                EnrichmentRequestTarget::Page(page) => {
                    self.adopt_page_enrichment(candidate, completion.key, page)?;
                }
                EnrichmentRequestTarget::Record(record) => {
                    self.adopt_record_enrichment(candidate, completion.key, record)?;
                }
            },
            Err(error) => match completion.key.target {
                EnrichmentRequestTarget::Page(_) => {
                    self.page_load = PageLoadState::Failed(PageEnrichmentFailure::from(&error));
                }
                EnrichmentRequestTarget::Record(record) => {
                    self.interpretation = PageInterpretationState::Record {
                        record,
                        load: InterpretationLoadState::Failed(PageEnrichmentFailure::from(&error)),
                        top_attribute: 0,
                    };
                }
            },
        }
        let state = self.focused_state();
        Ok(FocusedTransition {
            changed: state != before,
            state,
        })
    }

    fn prepare_focused_page(&mut self) -> Result<(), FocusedError> {
        self.page_distribution = PageDistributionProjection::NotAvailable;
        self.selected_distribution_item = None;
        self.top_distribution_item = None;
        let vpid = self.focused_vpid()?;
        if let Some(deep) = self.view.deep_page(vpid) {
            if let Some(slotted) = deep.slotted.as_ref() {
                self.install_distribution(page_distribution_projection(slotted), vpid)?;
            } else {
                self.page_load = PageLoadState::Unavailable(
                    deep.diagnostic_rule
                        .unwrap_or("Page has no validated slotted distribution"),
                );
            }
            return Ok(());
        }

        let page = self.view.page(vpid)?;
        if !page.supports_slotted_distribution {
            self.page_load =
                PageLoadState::Unavailable("Page type has no slotted record distribution");
            return Ok(());
        }
        if self.active_enrichment_request.is_some() {
            return Ok(());
        }
        self.start_enrichment(EnrichmentRequestTarget::Page(vpid))?;
        self.page_load = PageLoadState::Loading;
        Ok(())
    }

    fn adopt_page_enrichment(
        &mut self,
        candidate: GraphView,
        key: EnrichmentRequestKey,
        page_key: Vpid,
    ) -> Result<(), FocusedError> {
        let overview = candidate.overview();
        if overview.snapshot_id != key.snapshot_id {
            return Err(FocusedError::InvalidEnrichmentSnapshot);
        }
        let expected = key
            .base_revision
            .next()
            .map_err(|_| FocusedError::Arithmetic)?;
        if overview.revision != expected {
            return Err(FocusedError::InvalidEnrichmentRevision {
                expected: expected.get(),
                actual: overview.revision.get(),
            });
        }
        let volumes = candidate.volumes();
        let Some(volume_index) = volumes
            .iter()
            .position(|volume| volume.vol_id == page_key.vol_id)
        else {
            return Err(FocusedError::InvalidEnrichmentPage);
        };
        let page = candidate
            .page(page_key)
            .map_err(|_| FocusedError::InvalidEnrichmentPage)?;
        if u32::try_from(page.sector_id.get()).ok() != Some(self.focused_sector) {
            return Err(FocusedError::InvalidEnrichmentPage);
        }
        let deep = candidate
            .deep_page(page_key)
            .ok_or(FocusedError::InvalidEnrichmentPage)?;
        let (distribution, load) = deep.slotted.as_ref().map_or_else(
            || {
                (
                    PageDistributionProjection::NotAvailable,
                    PageLoadState::Unavailable(
                        deep.diagnostic_rule
                            .unwrap_or("Page has no validated slotted distribution"),
                    ),
                )
            },
            |slotted| (page_distribution_projection(slotted), PageLoadState::Ready),
        );

        self.view = candidate;
        self.volumes = volumes;
        self.volume_index = volume_index;
        self.page_load = load;
        self.install_distribution(distribution, page_key)
    }

    fn adopt_record_enrichment(
        &mut self,
        candidate: GraphView,
        key: EnrichmentRequestKey,
        record: Oid,
    ) -> Result<(), FocusedError> {
        let overview = candidate.overview();
        if overview.snapshot_id != key.snapshot_id {
            return Err(FocusedError::InvalidEnrichmentSnapshot);
        }
        if overview.revision <= key.base_revision {
            let expected = key
                .base_revision
                .next()
                .map_err(|_| FocusedError::Arithmetic)?;
            return Err(FocusedError::InvalidEnrichmentRevision {
                expected: expected.get(),
                actual: overview.revision.get(),
            });
        }
        let page_key = Vpid::new(record.vol_id, record.page_id);
        let volumes = candidate.volumes();
        let Some(volume_index) = volumes
            .iter()
            .position(|volume| volume.vol_id == page_key.vol_id)
        else {
            return Err(FocusedError::InvalidEnrichmentPage);
        };
        let page = candidate
            .page(page_key)
            .map_err(|_| FocusedError::InvalidEnrichmentPage)?;
        if u32::try_from(page.sector_id.get()).ok() != Some(self.focused_sector) {
            return Err(FocusedError::InvalidEnrichmentPage);
        }
        let deep = candidate
            .deep_page(page_key)
            .ok_or(FocusedError::InvalidEnrichmentPage)?;
        let slotted = deep
            .slotted
            .as_ref()
            .ok_or(FocusedError::InvalidEnrichmentPage)?;
        let distribution = page_distribution_projection(slotted);
        let items = PageDistributionItem::from_projection(page_key, &distribution)?;
        if !items.iter().any(|item| item.record_oid() == Some(record)) {
            return Err(FocusedError::InvalidEnrichmentPage);
        }
        let selection = record_selection_projection(&candidate, record)
            .ok_or(FocusedError::InvalidEnrichmentPage)?;
        let load = if let Some(reason) = record_selection_limitation(&selection) {
            InterpretationLoadState::Unavailable(reason)
        } else if selection.interpretation.is_some() {
            InterpretationLoadState::Ready
        } else {
            return Err(FocusedError::InvalidEnrichmentPage);
        };

        self.view = candidate;
        self.volumes = volumes;
        self.volume_index = volume_index;
        self.page_load = PageLoadState::Ready;
        self.page_distribution = distribution;
        self.interpretation = PageInterpretationState::Record {
            record,
            load,
            top_attribute: 0,
        };
        Ok(())
    }

    fn install_distribution(
        &mut self,
        distribution: PageDistributionProjection,
        page: Vpid,
    ) -> Result<(), FocusedError> {
        let items = PageDistributionItem::from_projection(page, &distribution)?;
        self.selected_distribution_item = items.first().map(PageDistributionItem::id);
        self.top_distribution_item = self.selected_distribution_item;
        self.page_distribution = distribution;
        if !items.is_empty() {
            self.page_load = PageLoadState::Ready;
        }
        Ok(())
    }

    fn activate_page_selection(&mut self) -> Result<(), FocusedError> {
        if self.interpretation != PageInterpretationState::Closed
            || self.active_enrichment_request.is_some()
        {
            return Ok(());
        }
        let Some((record, _record_type)) = self.selected_record()? else {
            return Ok(());
        };
        if let RecordSelectionSupport::Unsupported(reason) =
            self.view.record_selection_support(record)?
        {
            self.interpretation = PageInterpretationState::Record {
                record,
                load: InterpretationLoadState::Unavailable(reason),
                top_attribute: 0,
            };
            return Ok(());
        }
        let page = Vpid::new(record.vol_id, record.page_id);
        if let Some(reason) = self.view.record_page_interpretation_failure(page) {
            self.interpretation = PageInterpretationState::Record {
                record,
                load: InterpretationLoadState::Unavailable(reason),
                top_attribute: 0,
            };
            return Ok(());
        }
        if let Some(selection) = record_selection_projection(&self.view, record) {
            if let Some(reason) = record_selection_limitation(&selection) {
                self.interpretation = PageInterpretationState::Record {
                    record,
                    load: InterpretationLoadState::Unavailable(reason),
                    top_attribute: 0,
                };
                return Ok(());
            }
            if selection.interpretation.is_some() {
                self.interpretation = PageInterpretationState::Record {
                    record,
                    load: InterpretationLoadState::Ready,
                    top_attribute: 0,
                };
                return Ok(());
            }
        }
        self.start_enrichment(EnrichmentRequestTarget::Record(record))?;
        self.interpretation = PageInterpretationState::Record {
            record,
            load: InterpretationLoadState::Loading,
            top_attribute: 0,
        };
        Ok(())
    }

    fn selected_record(&self) -> Result<Option<(Oid, RecordTypeProjection)>, FocusedError> {
        let Some(selected) = self.selected_distribution_item else {
            return Ok(None);
        };
        Ok(
            PageDistributionItem::from_projection(self.focused_vpid()?, &self.page_distribution)?
                .into_iter()
                .find(|item| item.id() == selected)
                .and_then(|item| item.record_identity()),
        )
    }

    fn start_enrichment(&mut self, target: EnrichmentRequestTarget) -> Result<(), FocusedError> {
        if self.active_enrichment_request.is_some() {
            return Ok(());
        }
        let overview = self.view.overview();
        let key = EnrichmentRequestKey {
            request_id: self.next_enrichment_request_id,
            snapshot_id: overview.snapshot_id,
            base_revision: overview.revision,
            target,
        };
        self.next_enrichment_request_id = self
            .next_enrichment_request_id
            .checked_add(1)
            .ok_or(FocusedError::Arithmetic)?;
        let cancel = CancelToken::new();
        self.active_enrichment_request = Some(ActiveEnrichmentRequest {
            key,
            cancel: cancel.clone(),
        });
        self.pending_enrichment_request = Some(FocusedEnrichmentRequest {
            key,
            base: self.view.clone(),
            policy: self.policy,
            cancel,
        });
        Ok(())
    }

    fn cancel_enrichment(&mut self) {
        if let Some(active) = self.active_enrichment_request.take() {
            active.cancel.cancel();
        }
        self.pending_enrichment_request = None;
    }

    fn close_interpretation(&mut self) {
        self.cancel_enrichment();
        self.interpretation = PageInterpretationState::Closed;
    }

    fn leave_page(&mut self) {
        self.cancel_enrichment();
        self.page_load = PageLoadState::Idle;
        self.page_distribution = PageDistributionProjection::NotAvailable;
        self.selected_distribution_item = None;
        self.top_distribution_item = None;
        self.interpretation = PageInterpretationState::Closed;
    }

    fn quit(&mut self) {
        if self.mode == FocusedMode::Page {
            self.leave_page();
        }
        self.quit_requested = true;
    }

    fn focused_vpid(&self) -> Result<Vpid, FocusedError> {
        let raw_sector =
            i32::try_from(self.focused_sector).map_err(|_| FocusedError::Arithmetic)?;
        let sector_id = SectorId::new(raw_sector).map_err(|_| FocusedError::Arithmetic)?;
        let sector = self
            .view
            .sector(self.volumes[self.volume_index].vol_id, sector_id)?;
        sector
            .pages
            .get(usize::from(self.focused_page))
            .map(|page| page.vpid)
            .ok_or(FocusedError::InvalidSectorPageCount {
                sector_id: raw_sector,
                actual: sector.pages.len(),
            })
    }

    fn move_page_to_sibling_sector(
        &mut self,
        forward: bool,
        layout: VolumeLayout,
    ) -> Result<(), FocusedError> {
        let target = if forward {
            self.focused_sector
                .checked_add(1)
                .filter(|candidate| *candidate < self.total_sectors())
        } else {
            self.focused_sector.checked_sub(1)
        };
        let Some(target) = target else {
            return Ok(());
        };
        self.leave_page();
        self.focused_sector = target;
        self.reveal_focus(layout);
        self.mode = FocusedMode::Page;
        self.prepare_focused_page()?;
        Ok(())
    }

    fn move_page_to_sibling_volume(&mut self, forward: bool) -> Result<(), FocusedError> {
        let target = if forward {
            self.volume_index
                .checked_add(1)
                .filter(|candidate| *candidate < self.volumes.len())
        } else {
            self.volume_index.checked_sub(1)
        };
        let Some(target) = target else {
            return Ok(());
        };
        self.leave_page();
        self.volume_index = target;
        self.focused_sector = 0;
        self.top_sector = 0;
        self.mode = FocusedMode::Page;
        self.prepare_focused_page()
    }

    fn move_distribution_focus(
        &mut self,
        forward: bool,
        surface: Surface,
    ) -> Result<(), FocusedError> {
        let items =
            PageDistributionItem::from_projection(self.focused_vpid()?, &self.page_distribution)?;
        if items.is_empty() {
            return Ok(());
        }
        let current = self
            .selected_distribution_item
            .and_then(|selected| items.iter().position(|item| item.id() == selected))
            .unwrap_or(0);
        let next = if forward {
            (current + 1).min(items.len() - 1)
        } else {
            current.saturating_sub(1)
        };
        self.selected_distribution_item = Some(items[next].id());
        self.reveal_distribution_focus(&items, surface);
        Ok(())
    }

    fn focus_distribution_item(
        &mut self,
        item: PageDistributionItemId,
        surface: Surface,
    ) -> Result<(), FocusedError> {
        let items =
            PageDistributionItem::from_projection(self.focused_vpid()?, &self.page_distribution)?;
        if items.iter().any(|candidate| candidate.id() == item) {
            self.selected_distribution_item = Some(item);
            self.reveal_distribution_focus(&items, surface);
        }
        Ok(())
    }

    fn reveal_distribution_focus(&mut self, items: &[PageDistributionItem], surface: Surface) {
        let visible = usize::from(page_visible_rows(surface));
        let selected = self
            .selected_distribution_item
            .and_then(|selected| items.iter().position(|item| item.id() == selected))
            .unwrap_or(0);
        let mut top = self
            .top_distribution_item
            .and_then(|top| items.iter().position(|item| item.id() == top))
            .unwrap_or(0);
        if selected < top {
            top = selected;
        } else if selected >= top.saturating_add(visible) {
            top = selected + 1 - visible;
        }
        self.top_distribution_item = items.get(top).map(PageDistributionItem::id);
    }

    fn scroll_distribution(&mut self, rows: i32, surface: Surface) -> Result<(), FocusedError> {
        let items =
            PageDistributionItem::from_projection(self.focused_vpid()?, &self.page_distribution)?;
        if items.is_empty() {
            return Ok(());
        }
        let visible = usize::from(page_visible_rows(surface));
        let current = self
            .top_distribution_item
            .and_then(|top| items.iter().position(|item| item.id() == top))
            .unwrap_or(0);
        let magnitude = usize::try_from(rows.unsigned_abs()).unwrap_or(usize::MAX);
        let maximum = items.len().saturating_sub(visible);
        let top = if rows.is_negative() {
            current.saturating_sub(magnitude)
        } else {
            current.saturating_add(magnitude).min(maximum)
        };
        self.top_distribution_item = items.get(top).map(PageDistributionItem::id);
        Ok(())
    }

    fn scroll_interpretation(&mut self, rows: i32, surface: Surface) {
        let PageInterpretationState::Record {
            record,
            top_attribute,
            ..
        } = self.interpretation
        else {
            return;
        };
        let count = record_selection_projection(&self.view, record)
            .and_then(|selection| selection.interpretation)
            .map_or(0, |interpretation| interpretation.attributes.len());
        let maximum = count.saturating_sub(usize::from(interpretation_visible_rows(surface)));
        let current = usize::try_from(top_attribute)
            .unwrap_or(usize::MAX)
            .min(maximum);
        let magnitude = usize::try_from(rows.unsigned_abs()).unwrap_or(usize::MAX);
        let next = if rows.is_negative() {
            current.saturating_sub(magnitude)
        } else {
            current.saturating_add(magnitude).min(maximum)
        };
        if let PageInterpretationState::Record { top_attribute, .. } = &mut self.interpretation {
            *top_attribute = u32::try_from(next).unwrap_or(u32::MAX);
        }
    }

    fn move_page_horizontal(&mut self, forward: bool) {
        let column = self.focused_page % 8;
        if forward {
            if column < 7 {
                self.focused_page += 1;
            }
        } else if column > 0 {
            self.focused_page -= 1;
        }
    }

    fn move_page_vertical(&mut self, forward: bool) {
        let row = self.focused_page / 8;
        if forward {
            if row < 7 {
                self.focused_page += 8;
            }
        } else if row > 0 {
            self.focused_page -= 8;
        }
    }

    fn total_sectors(&self) -> u32 {
        self.volumes[self.volume_index].total_sectors
    }

    fn move_focus_forward(&mut self, amount: u32, layout: VolumeLayout) {
        let Some(candidate) = self.focused_sector.checked_add(amount) else {
            return;
        };
        if candidate < self.total_sectors() {
            self.focused_sector = candidate;
            self.reveal_focus(layout);
        }
    }

    fn reveal_focus(&mut self, layout: VolumeLayout) {
        if self.focused_sector < self.top_sector {
            self.top_sector = self.focused_sector;
            return;
        }
        let capacity = layout.visible_capacity();
        if self.focused_sector >= self.top_sector.saturating_add(capacity) {
            let columns = u32::from(layout.columns);
            let focused_row = self.focused_sector - self.focused_sector % columns;
            self.top_sector = focused_row
                .saturating_sub(u32::from(layout.visible_rows.saturating_sub(1)) * columns);
        }
        self.clamp_top(layout);
    }

    fn scroll_rows(&mut self, rows: i32, layout: VolumeLayout) {
        let magnitude = rows
            .unsigned_abs()
            .saturating_mul(u32::from(layout.columns));
        self.top_sector = if rows.is_negative() {
            self.top_sector.saturating_sub(magnitude)
        } else {
            self.top_sector.saturating_add(magnitude)
        };
        self.clamp_top(layout);
    }

    fn clamp_top(&mut self, layout: VolumeLayout) {
        let maximum = self
            .total_sectors()
            .saturating_sub(layout.visible_capacity());
        self.top_sector = self.top_sector.min(maximum);
    }

    fn move_volume(&mut self, forward: bool) {
        let next = if forward {
            (self.volume_index + 1).min(self.volumes.len().saturating_sub(1))
        } else {
            self.volume_index.saturating_sub(1)
        };
        if next != self.volume_index {
            self.volume_index = next;
            self.focused_sector = 0;
            self.top_sector = 0;
        }
    }
}

impl Drop for FocusedSession {
    fn drop(&mut self) {
        self.cancel_enrichment();
    }
}

#[derive(Clone, Debug)]
pub(crate) struct VolumeScene {
    pub snapshot_id: SnapshotId,
    pub revision: InspectionRevision,
    pub outcome: &'static str,
    pub volume: crate::projection::VolumeProjection,
    pub volume_index: usize,
    pub volume_count: usize,
    pub focused_sector: u32,
    pub top_sector: u32,
    pub layout: VolumeLayout,
    pub sectors: Vec<SectorCard>,
}

#[derive(Clone, Debug)]
pub(crate) struct SectorScene {
    pub snapshot_id: SnapshotId,
    pub revision: InspectionRevision,
    pub outcome: &'static str,
    pub volume: crate::projection::VolumeProjection,
    pub volume_index: usize,
    pub volume_count: usize,
    pub focused_page: u8,
    pub sector: SectorCard,
}

#[derive(Clone, Debug)]
pub(crate) struct PageScene {
    pub snapshot_id: SnapshotId,
    pub revision: InspectionRevision,
    pub outcome: &'static str,
    pub volume: crate::projection::VolumeProjection,
    pub volume_index: usize,
    pub volume_count: usize,
    pub sector_id: i32,
    pub page: PageMark,
    pub load: PageLoadState,
    pub distribution: PageDistributionProjection,
    pub items: Vec<PageDistributionItem>,
    pub selected_item: Option<PageDistributionItemId>,
    pub top_item: Option<PageDistributionItemId>,
    pub interpretation_state: PageInterpretationState,
    pub record_selection: Option<RecordSelectionProjection>,
}

impl PageScene {
    #[cfg(test)]
    pub(crate) fn selected_record(&self) -> Option<Oid> {
        let selected = self.selected_item?;
        self.items
            .iter()
            .find(|item| item.id() == selected)
            .and_then(PageDistributionItem::record_oid)
    }
}

fn record_selection_limitation(selection: &RecordSelectionProjection) -> Option<&'static str> {
    selection.interpretation_unavailable.or_else(|| {
        selection
            .interpretation
            .as_ref()
            .and_then(|interpretation| match interpretation.diagnostic {
                OptionalTextProjection::Known(reason) => Some(reason),
                OptionalTextProjection::Unknown | OptionalTextProjection::Unsupported => None,
            })
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PageDistributionItem {
    Header {
        region: ByteRegionProjection,
    },
    Record {
        oid: Oid,
        slot_id: u16,
        region: ByteRegionProjection,
        record_type: RecordTypeProjection,
    },
    Free {
        region: ByteRegionProjection,
        kind: FreeRegionKindProjection,
    },
    SlotDirectory {
        region: ByteRegionProjection,
    },
    SlotEntry {
        slot_id: u16,
        region: ByteRegionProjection,
        state: SlotEntryStateProjection,
        record_type: &'static str,
    },
}

impl PageDistributionItem {
    fn from_projection(
        page: Vpid,
        distribution: &PageDistributionProjection,
    ) -> Result<Vec<Self>, FocusedError> {
        let PageDistributionProjection::Available {
            header,
            record_extents,
            free_regions,
            slot_directory,
            slot_entries,
            ..
        } = distribution
        else {
            return Ok(Vec::new());
        };

        let mut items = vec![Self::Header { region: *header }];
        let mut content = record_extents
            .iter()
            .map(|record| Self::record(page, record))
            .collect::<Result<Vec<_>, _>>()?;
        content.extend(free_regions.iter().map(Self::free));
        content.sort_unstable_by_key(|item| (item.offset(), item.id()));
        items.extend(content);
        items.push(Self::SlotDirectory {
            region: *slot_directory,
        });
        items.extend(slot_entries.iter().map(Self::slot_entry));
        Ok(items)
    }

    fn record(page: Vpid, record: &RecordExtentProjection) -> Result<Self, FocusedError> {
        let raw_slot = i16::try_from(record.slot_id).map_err(|_| FocusedError::Arithmetic)?;
        Ok(Self::Record {
            oid: Oid::new(
                page.vol_id,
                page.page_id,
                SlotId::new(raw_slot).map_err(|_| FocusedError::Arithmetic)?,
            ),
            slot_id: record.slot_id,
            region: ByteRegionProjection {
                offset: record.offset,
                length: record.length,
            },
            record_type: record.record_type,
        })
    }

    fn free(region: &FreeRegionProjection) -> Self {
        Self::Free {
            region: ByteRegionProjection {
                offset: region.offset,
                length: region.length,
            },
            kind: region.kind,
        }
    }

    fn slot_entry(entry: &SlotEntryProjection) -> Self {
        Self::SlotEntry {
            slot_id: entry.slot_id,
            region: ByteRegionProjection {
                offset: entry.offset,
                length: entry.length,
            },
            state: entry.state,
            record_type: entry.record_type,
        }
    }

    pub(crate) fn id(&self) -> PageDistributionItemId {
        match self {
            Self::Header { .. } => PageDistributionItemId::Header,
            Self::Record { slot_id, .. } => PageDistributionItemId::Record(*slot_id),
            Self::Free {
                region,
                kind: FreeRegionKindProjection::ContiguousFree,
            } => PageDistributionItemId::ContiguousFree {
                offset: region.offset,
                length: region.length,
            },
            Self::Free { region, .. } => PageDistributionItemId::FragmentedFree {
                offset: region.offset,
                length: region.length,
            },
            Self::SlotDirectory { .. } => PageDistributionItemId::SlotDirectory,
            Self::SlotEntry { slot_id, .. } => PageDistributionItemId::SlotEntry(*slot_id),
        }
    }

    pub(crate) const fn record_oid(&self) -> Option<Oid> {
        match self {
            Self::Record { oid, .. } => Some(*oid),
            _ => None,
        }
    }

    const fn record_identity(&self) -> Option<(Oid, RecordTypeProjection)> {
        match self {
            Self::Record {
                oid, record_type, ..
            } => Some((*oid, *record_type)),
            _ => None,
        }
    }

    const fn region(&self) -> ByteRegionProjection {
        match self {
            Self::Header { region }
            | Self::Record { region, .. }
            | Self::Free { region, .. }
            | Self::SlotDirectory { region }
            | Self::SlotEntry { region, .. } => *region,
        }
    }

    const fn offset(&self) -> u32 {
        self.region().offset
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AllocationMark {
    System,
    Allocated,
    Reserved,
    Unreserved,
}

impl AllocationMark {
    const fn from_name(value: &'static str) -> Result<Self, FocusedError> {
        match value.as_bytes() {
            b"system-metadata" => Ok(Self::System),
            b"allocated" => Ok(Self::Allocated),
            b"reserved-unallocated" => Ok(Self::Reserved),
            b"unreserved" => Ok(Self::Unreserved),
            _ => Err(FocusedError::InvalidAllocation(value)),
        }
    }

    const fn glyph(self) -> char {
        match self {
            Self::System => 'S',
            Self::Allocated => 'A',
            Self::Reserved => 'R',
            Self::Unreserved => 'U',
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::System => "system-metadata",
            Self::Allocated => "allocated",
            Self::Reserved => "reserved-unallocated",
            Self::Unreserved => "unreserved",
        }
    }

    const fn style(self) -> SemanticStyle {
        match self {
            Self::System => SemanticStyle::System,
            Self::Allocated => SemanticStyle::Allocated,
            Self::Reserved => SemanticStyle::Reserved,
            Self::Unreserved => SemanticStyle::Unreserved,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OccupancyMark {
    Zero,
    Level(u8),
    Unknown,
    NotApplicable,
}

impl OccupancyMark {
    #[cfg(test)]
    fn from_projection(allocation: AllocationMark, occupancy: &PageOccupancyProjection) -> Self {
        ExactOccupancy::from_projection(allocation, occupancy).volume_mark()
    }

    const fn glyph(self, glyphs: GlyphProfile) -> char {
        match (self, glyphs) {
            (Self::Zero, _) => '0',
            (Self::Unknown, _) => '?',
            (Self::NotApplicable, _) => '-',
            (Self::Level(level), GlyphProfile::Ascii) => match level {
                1 => '1',
                2 => '2',
                3 => '3',
                4 => '4',
                5 => '5',
                6 => '6',
                7 => '7',
                _ => '8',
            },
            (Self::Level(level), GlyphProfile::Unicode) => match level {
                1 => '⡀',
                2 => '⣀',
                3 => '⣄',
                4 => '⣤',
                5 => '⣦',
                6 => '⣶',
                7 => '⣷',
                _ => '⣿',
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExactOccupancy {
    Known {
        occupied_percent: u8,
        free_percent: u8,
    },
    Unknown,
    NotApplicable,
}

impl ExactOccupancy {
    fn from_projection(allocation: AllocationMark, occupancy: &PageOccupancyProjection) -> Self {
        if allocation != AllocationMark::Allocated {
            return Self::NotApplicable;
        }
        match occupancy {
            PageOccupancyProjection::Known {
                occupied_percent,
                free_percent,
            } => Self::Known {
                occupied_percent: *occupied_percent,
                free_percent: *free_percent,
            },
            PageOccupancyProjection::Unknown => Self::Unknown,
        }
    }

    fn volume_mark(self) -> OccupancyMark {
        match self {
            Self::Known {
                occupied_percent: 0,
                ..
            } => OccupancyMark::Zero,
            Self::Known {
                occupied_percent, ..
            } => {
                let scaled = u16::from(occupied_percent) * 8;
                let level = scaled.div_ceil(100);
                OccupancyMark::Level(u8::try_from(level).unwrap_or(8).min(8))
            }
            Self::Unknown => OccupancyMark::Unknown,
            Self::NotApplicable => OccupancyMark::NotApplicable,
        }
    }

    fn occupied_label(self) -> String {
        match self {
            Self::Known {
                occupied_percent, ..
            } => format!("{occupied_percent}%"),
            Self::Unknown => "?".to_owned(),
            Self::NotApplicable => "-".to_owned(),
        }
    }

    fn compact_value(self) -> String {
        match self {
            Self::Known {
                occupied_percent, ..
            } => occupied_percent.to_string(),
            Self::Unknown => "?".to_owned(),
            Self::NotApplicable => "-".to_owned(),
        }
    }

    fn descriptor(self) -> String {
        match self {
            Self::Known {
                occupied_percent,
                free_percent,
            } => format!("occupied {occupied_percent}% / free {free_percent}%"),
            Self::Unknown => "occupied ? / free ?".to_owned(),
            Self::NotApplicable => "occupied - / free -".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PageTypeMark {
    Known(&'static str),
    Unknown,
    Unsupported,
}

impl PageTypeMark {
    fn from_projection(value: &OptionalTextProjection) -> Self {
        match value {
            OptionalTextProjection::Known(value) => Self::Known(value),
            OptionalTextProjection::Unknown => Self::Unknown,
            OptionalTextProjection::Unsupported => Self::Unsupported,
        }
    }

    const fn label(&self) -> &'static str {
        match self {
            Self::Known(value) => value,
            Self::Unknown => "unknown",
            Self::Unsupported => "unsupported",
        }
    }

    fn compact(&self) -> &'static str {
        match self {
            Self::Known("file-table") => "FT",
            Self::Known("heap") => "HP",
            Self::Known("volume-header") => "VH",
            Self::Known("volume-bitmap") => "VB",
            Self::Known("query-result") => "QR",
            Self::Known("extensible-hash") => "EH",
            Self::Known("overflow") => "OV",
            Self::Known("oos") => "OS",
            Self::Known("area") => "AR",
            Self::Known("catalog") => "CA",
            Self::Known("btree") => "BT",
            Self::Known("log") => "LG",
            Self::Known("dropped-files") => "DF",
            Self::Known("vacuum-data") => "VD",
            Self::Known(_) | Self::Unknown => "??",
            Self::Unsupported => "--",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PageClaimKind {
    Allocated,
    ReservedFor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PageAttributionMark {
    None,
    MixedClaims,
    Single {
        kind: PageClaimKind,
        vol_id: i16,
        file_id: i32,
        role: Option<&'static str>,
        class_oid: Option<(i16, i32, i16)>,
        class: ClassAttributionMark,
    },
}

impl PageAttributionMark {
    fn from_projection(value: &FileAssociationProjection) -> Self {
        match value {
            FileAssociationProjection::None => Self::None,
            FileAssociationProjection::MixedClaims => Self::MixedClaims,
            FileAssociationProjection::Allocated { file } => {
                Self::from_body(PageClaimKind::Allocated, file)
            }
            FileAssociationProjection::ReservedFor { file } => {
                Self::from_body(PageClaimKind::ReservedFor, file)
            }
        }
    }

    fn from_body(kind: PageClaimKind, file: &FileAssociationBodyProjection) -> Self {
        Self::Single {
            kind,
            vol_id: file.vol_id,
            file_id: file.file_id,
            role: match file.file_type {
                OptionalTextProjection::Known(role) => Some(role),
                OptionalTextProjection::Unknown | OptionalTextProjection::Unsupported => None,
            },
            class_oid: match file.class_oid {
                OptionalOidProjection::Absent => None,
                OptionalOidProjection::Present { oid } => {
                    Some((oid.vol_id, oid.page_id, oid.slot_id))
                }
            },
            class: match &file.class_name {
                ClassNameProjection::Resolved { value } => {
                    ClassAttributionMark::Resolved(value.clone())
                }
                ClassNameProjection::Unresolved { reason } => {
                    ClassAttributionMark::Unresolved(reason)
                }
                ClassNameProjection::NotApplicable { reason } => {
                    ClassAttributionMark::NotApplicable(reason)
                }
            },
        }
    }

    fn labels(&self) -> (String, String) {
        match self {
            Self::None => ("file none / class -".to_owned(), "table -".to_owned()),
            Self::MixedClaims => ("file mixed / class ?".to_owned(), "table ?".to_owned()),
            Self::Single {
                kind,
                vol_id,
                file_id,
                role,
                class_oid,
                class,
            } => {
                let claim = match kind {
                    PageClaimKind::Allocated => "allocated-by",
                    PageClaimKind::ReservedFor => "reserved-for",
                };
                let file = role.map_or_else(
                    || format!("file {vol_id}:{file_id}"),
                    |role| format!("file {vol_id}:{file_id} ({role})"),
                );
                let class_oid = class_oid.map_or_else(
                    || "class -".to_owned(),
                    |(vol_id, page_id, slot_id)| format!("class {vol_id}:{page_id}:{slot_id}"),
                );
                let table = match class {
                    ClassAttributionMark::Resolved(value) => format!("table {value}"),
                    ClassAttributionMark::Unresolved(reason) => format!("table ? ({reason})"),
                    ClassAttributionMark::NotApplicable(reason) => format!("table - ({reason})"),
                };
                (format!("{claim} {file} / {class_oid}"), table)
            }
        }
    }

    fn label(&self) -> String {
        let (file_class, table) = self.labels();
        format!("{file_class} / {table}")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PageMark {
    pub page_id: i32,
    pub allocation: AllocationMark,
    pub occupancy: ExactOccupancy,
    pub page_type: PageTypeMark,
    pub finding: bool,
    pub diagnostic: Option<&'static str>,
    pub attribution: PageAttributionMark,
}

impl PageMark {
    fn try_from_projection(page: &PageProjection) -> Result<Self, FocusedError> {
        let allocation = AllocationMark::from_name(page.allocation)?;
        let diagnostic = match page.diagnostic {
            OptionalTextProjection::Known(value) => Some(value),
            OptionalTextProjection::Unknown | OptionalTextProjection::Unsupported => None,
        };
        Ok(Self {
            page_id: page.page_id,
            allocation,
            occupancy: ExactOccupancy::from_projection(allocation, &page.occupancy),
            page_type: PageTypeMark::from_projection(&page.page_type),
            finding: diagnostic.is_some(),
            diagnostic,
            attribution: PageAttributionMark::from_projection(&page.file_association),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ClassAttributionMark {
    Resolved(String),
    Unresolved(&'static str),
    NotApplicable(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SectorAttributionMark {
    Unclaimed,
    Mixed {
        claim_count: usize,
    },
    Single {
        vol_id: i16,
        file_id: i32,
        role: Option<&'static str>,
        class: ClassAttributionMark,
        full: bool,
        allocated_pages: u8,
        reserved_unallocated_pages: u8,
    },
}

impl SectorAttributionMark {
    fn from_projection(attribution: &SectorAttributionProjection) -> Self {
        match attribution {
            SectorAttributionProjection::Unclaimed => Self::Unclaimed,
            SectorAttributionProjection::Mixed { claims } => Self::Mixed {
                claim_count: claims.len(),
            },
            SectorAttributionProjection::Single {
                file,
                full,
                allocated_pages,
                reserved_unallocated_pages,
            } => Self::Single {
                vol_id: file.vol_id,
                file_id: file.file_id,
                role: match file.file_type {
                    OptionalTextProjection::Known(role) => Some(role),
                    OptionalTextProjection::Unknown | OptionalTextProjection::Unsupported => None,
                },
                class: match &file.class_name {
                    ClassNameProjection::Resolved { value } => {
                        ClassAttributionMark::Resolved(value.clone())
                    }
                    ClassNameProjection::Unresolved { reason } => {
                        ClassAttributionMark::Unresolved(reason)
                    }
                    ClassNameProjection::NotApplicable { reason } => {
                        ClassAttributionMark::NotApplicable(reason)
                    }
                },
                full: *full,
                allocated_pages: *allocated_pages,
                reserved_unallocated_pages: *reserved_unallocated_pages,
            },
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Unclaimed => "unclaimed".to_owned(),
            Self::Mixed { claim_count } => format!("mixed:{claim_count}"),
            Self::Single {
                vol_id,
                file_id,
                role,
                class,
                ..
            } => match class {
                ClassAttributionMark::Resolved(value) => format!("table:{value}"),
                ClassAttributionMark::Unresolved(reason) => format!("table:? ({reason})"),
                ClassAttributionMark::NotApplicable(_) => role.map_or_else(
                    || format!("file:{vol_id}:{file_id}"),
                    |role| format!("{role}:{file_id}"),
                ),
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SectorCard {
    pub sector_id: i32,
    pub reserved: bool,
    pub attribution: SectorAttributionMark,
    pub finding: bool,
    pub pages: [PageMark; PAGE_COUNT],
}

impl SectorCard {
    fn try_from_projection(sector: SectorProjection) -> Result<Self, FocusedError> {
        let actual = sector.pages.len();
        let pages: [PageProjection; PAGE_COUNT] =
            sector
                .pages
                .try_into()
                .map_err(|_| FocusedError::InvalidSectorPageCount {
                    sector_id: sector.sector_id,
                    actual,
                })?;
        let first_page = sector
            .sector_id
            .checked_mul(i32::try_from(PAGE_COUNT).map_err(|_| FocusedError::Arithmetic)?)
            .ok_or(FocusedError::Arithmetic)?;
        let pages: [PageMark; PAGE_COUNT] = pages
            .into_iter()
            .enumerate()
            .map(|(index, page)| {
                let expected = first_page
                    .checked_add(i32::try_from(index).map_err(|_| FocusedError::Arithmetic)?)
                    .ok_or(FocusedError::Arithmetic)?;
                if page.page_id != expected {
                    return Err(FocusedError::InvalidPhysicalPageOrder {
                        expected,
                        actual: page.page_id,
                    });
                }
                PageMark::try_from_projection(&page)
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| FocusedError::InvalidSectorPageCount {
                sector_id: sector.sector_id,
                actual,
            })?;
        let finding = pages.iter().any(|page| page.finding);
        Ok(Self {
            sector_id: sector.sector_id,
            reserved: sector.reserved,
            attribution: SectorAttributionMark::from_projection(&sector.attribution),
            finding,
            pages,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GlyphProfile {
    Unicode,
    Ascii,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ColorProfile {
    Ansi,
    Monochrome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PresentationProfile {
    pub glyphs: GlyphProfile,
    pub colors: ColorProfile,
}

impl PresentationProfile {
    pub(crate) const ANSI_UNICODE: Self = Self {
        glyphs: GlyphProfile::Unicode,
        colors: ColorProfile::Ansi,
    };
    pub(crate) const MONO_ASCII: Self = Self {
        glyphs: GlyphProfile::Ascii,
        colors: ColorProfile::Monochrome,
    };

    #[cfg(test)]
    const fn name(self) -> &'static str {
        match (self.colors, self.glyphs) {
            (ColorProfile::Ansi, GlyphProfile::Unicode) => "ansi-unicode",
            (ColorProfile::Ansi, GlyphProfile::Ascii) => "ansi-ascii",
            (ColorProfile::Monochrome, GlyphProfile::Unicode) => "mono-unicode",
            (ColorProfile::Monochrome, GlyphProfile::Ascii) => "mono-ascii",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SemanticStyle {
    Plain,
    Header,
    Focus,
    System,
    Allocated,
    Reserved,
    Unreserved,
    Occupancy,
    Unknown,
    Finding,
    Muted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Cell {
    glyph: String,
    style: SemanticStyle,
    continuation: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            glyph: " ".to_owned(),
            style: SemanticStyle::Plain,
            continuation: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HitRegion {
    pub sector_id: i32,
    pub left: u16,
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PageHitRegion {
    pub page_index: u8,
    pub page_id: i32,
    pub left: u16,
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DistributionHitRegion {
    pub item: PageDistributionItemId,
    pub left: u16,
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VolumeFrame {
    surface: Surface,
    profile: PresentationProfile,
    cells: Vec<Cell>,
    pub hits: Vec<HitRegion>,
    pub page_hits: Vec<PageHitRegion>,
    pub distribution_hits: Vec<DistributionHitRegion>,
    pub formatted_distribution_rows: usize,
}

impl VolumeFrame {
    fn new(surface: Surface, profile: PresentationProfile) -> Self {
        Self {
            surface,
            profile,
            cells: vec![Cell::default(); usize::from(surface.width) * usize::from(surface.height)],
            hits: Vec::new(),
            page_hits: Vec::new(),
            distribution_hits: Vec::new(),
            formatted_distribution_rows: 0,
        }
    }

    fn put(&mut self, column: u16, row: u16, glyph: char, style: SemanticStyle) {
        self.put_cluster(column, row, &glyph.to_string(), 1, style);
    }

    fn put_cluster(
        &mut self,
        column: u16,
        row: u16,
        glyph: &str,
        width: usize,
        style: SemanticStyle,
    ) {
        if row >= self.surface.height || column >= self.surface.width || width == 0 {
            return;
        }
        let remaining = usize::from(self.surface.width - column);
        if width > remaining {
            return;
        }
        let index = usize::from(row) * usize::from(self.surface.width) + usize::from(column);
        self.cells[index] = Cell {
            glyph: glyph.to_owned(),
            style,
            continuation: false,
        };
        for offset in 1..width {
            self.cells[index + offset] = Cell {
                glyph: String::new(),
                style,
                continuation: true,
            };
        }
    }

    fn put_text(&mut self, column: u16, row: u16, width: u16, value: &str, style: SemanticStyle) {
        let clusters = fitted_clusters(value, usize::from(width), self.profile.glyphs);
        let mut cursor = column;
        for cluster in clusters {
            self.put_cluster(cursor, row, &cluster.text, cluster.width, style);
            cursor = cursor.saturating_add(u16::try_from(cluster.width).unwrap_or(u16::MAX));
        }
    }

    #[cfg(test)]
    pub(crate) fn line(&self, row: u16) -> String {
        if row >= self.surface.height {
            return String::new();
        }
        let start = usize::from(row) * usize::from(self.surface.width);
        let end = start + usize::from(self.surface.width);
        self.cells[start..end]
            .iter()
            .filter(|cell| !cell.continuation)
            .map(|cell| cell.glyph.as_str())
            .collect::<String>()
            .trim_end()
            .to_owned()
    }

    #[cfg(test)]
    pub(crate) fn semantic_snapshot(&self) -> String {
        let mut output = format!(
            "surface {}x{} · profile {}\n",
            self.surface.width,
            self.surface.height,
            self.profile.name()
        );
        for row in 0..self.surface.height {
            writeln!(output, "{row:02}│{}", self.line(row)).expect("writing a String cannot fail");
        }
        output.push_str("hits:");
        for hit in &self.hits {
            write!(
                output,
                " S{}@{},{}-{},{}",
                hit.sector_id, hit.left, hit.top, hit.right, hit.bottom
            )
            .expect("writing a String cannot fail");
        }
        output.push('\n');
        if !self.page_hits.is_empty() {
            output.push_str("page-hits:\n");
            for (index, hit) in self.page_hits.iter().enumerate() {
                if index % usize::from(SECTOR_GRID_COLUMNS) == 0 {
                    output.push_str("  ");
                } else {
                    output.push(' ');
                }
                write!(
                    output,
                    "P{}={}@{},{}-{},{}",
                    hit.page_index, hit.page_id, hit.left, hit.top, hit.right, hit.bottom
                )
                .expect("writing a String cannot fail");
                if (index + 1) % usize::from(SECTOR_GRID_COLUMNS) == 0 {
                    output.push('\n');
                }
            }
        }
        if !self.distribution_hits.is_empty() {
            output.push_str("distribution-hits:\n  ");
            for (index, hit) in self.distribution_hits.iter().enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                write!(
                    output,
                    "{:?}@{},{}-{},{}",
                    hit.item, hit.left, hit.top, hit.right, hit.bottom
                )
                .expect("writing a String cannot fail");
            }
            writeln!(
                output,
                "\nformatted-distribution-rows:{}",
                self.formatted_distribution_rows
            )
            .expect("writing a String cannot fail");
        }
        output
    }

    #[cfg(test)]
    fn cell(&self, column: u16, row: u16) -> &Cell {
        &self.cells[usize::from(row) * usize::from(self.surface.width) + usize::from(column)]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DisplayCluster {
    text: String,
    width: usize,
}

fn fitted_clusters(value: &str, maximum: usize, glyphs: GlyphProfile) -> Vec<DisplayCluster> {
    if maximum == 0 {
        return Vec::new();
    }
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_control() {
                match glyphs {
                    GlyphProfile::Unicode => '�',
                    GlyphProfile::Ascii => '?',
                }
            } else if glyphs == GlyphProfile::Ascii && !character.is_ascii() {
                '?'
            } else {
                character
            }
        })
        .collect::<String>();
    let mut clusters = UnicodeSegmentation::graphemes(sanitized.as_str(), true)
        .map(|grapheme| {
            let width = UnicodeWidthStr::width(grapheme);
            if width == 0 {
                DisplayCluster {
                    text: format!("◌{grapheme}"),
                    width: 1,
                }
            } else {
                DisplayCluster {
                    text: grapheme.to_owned(),
                    width,
                }
            }
        })
        .collect::<Vec<_>>();
    let total = clusters.iter().map(|cluster| cluster.width).sum::<usize>();
    if total <= maximum {
        return clusters;
    }
    let ellipsis = match glyphs {
        GlyphProfile::Unicode => "…",
        GlyphProfile::Ascii => ".",
    };
    let content_limit = maximum.saturating_sub(1);
    let mut used = 0_usize;
    clusters.retain(|cluster| {
        if used.saturating_add(cluster.width) <= content_limit {
            used += cluster.width;
            true
        } else {
            false
        }
    });
    clusters.push(DisplayCluster {
        text: ellipsis.to_owned(),
        width: 1,
    });
    clusters
}

pub(crate) struct VolumeRenderer;

impl VolumeRenderer {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn render(
        scene: &VolumeScene,
        surface: Surface,
        profile: PresentationProfile,
    ) -> Result<VolumeFrame, FocusedError> {
        let layout = VolumeLayout::for_surface(surface)?;
        if layout != scene.layout {
            return Err(FocusedError::Arithmetic);
        }
        let mut frame = VolumeFrame::new(surface, profile);
        let separator = match profile.glyphs {
            GlyphProfile::Unicode => " · ",
            GlyphProfile::Ascii => " | ",
        };
        let fingerprint = snapshot_id_hex(scene.snapshot_id);
        frame.put_text(
            0,
            0,
            surface.width,
            &format!(
                " VOLMAP  snapshot {}  r{}  {} ",
                &fingerprint[..12],
                scene.revision.get(),
                scene.outcome
            ),
            SemanticStyle::Header,
        );
        frame.put_text(
            0,
            1,
            surface.width,
            &format!(
                "Volume {} ({}/{}){separator}{} sectors{separator}focus Sector {}",
                scene.volume.vol_id,
                scene.volume_index + 1,
                scene.volume_count,
                scene.volume.total_sectors,
                scene.focused_sector
            ),
            SemanticStyle::Plain,
        );
        frame.put_text(
            0,
            2,
            surface.width,
            &format!("S/A/R/U allocation{separator}fill 0,1-8,?,-{separator}! finding"),
            SemanticStyle::Muted,
        );

        let visible =
            usize::try_from(layout.visible_capacity()).map_err(|_| FocusedError::Arithmetic)?;
        for (index, sector) in scene.sectors.iter().take(visible).enumerate() {
            let column_index = u16::try_from(index % usize::from(layout.columns))
                .map_err(|_| FocusedError::Arithmetic)?;
            let row_index = u16::try_from(index / usize::from(layout.columns))
                .map_err(|_| FocusedError::Arithmetic)?;
            let left = column_index * CARD_STRIDE;
            let top = CARD_TOP + row_index * CARD_HEIGHT;
            draw_card(
                &mut frame,
                left,
                top,
                sector,
                u32::try_from(sector.sector_id).ok() == Some(scene.focused_sector),
                profile,
            );
            frame.hits.push(HitRegion {
                sector_id: sector.sector_id,
                left,
                top,
                right: left + CARD_WIDTH - 1,
                bottom: top + CARD_HEIGHT - 1,
            });
        }

        let focused = scene
            .sectors
            .iter()
            .find(|sector| u32::try_from(sector.sector_id).ok() == Some(scene.focused_sector));
        let descriptor = focused.map_or_else(
            || {
                format!(
                    "Sector {} · focus is outside the scrolled viewport",
                    scene.focused_sector
                )
            },
            |sector| {
                format!(
                    "Sector {}{separator}{}{separator}{}{separator}{}",
                    sector.sector_id,
                    if sector.reserved {
                        "reserved"
                    } else {
                        "unreserved"
                    },
                    sector.attribution.label(),
                    if sector.finding {
                        "finding"
                    } else {
                        "no findings"
                    }
                )
            },
        );
        frame.put_text(
            0,
            surface.height - 3,
            surface.width,
            &descriptor,
            SemanticStyle::Focus,
        );
        frame.put_text(
            0,
            surface.height - 2,
            surface.width,
            &format!(
                "S{}..S{} visible{separator}{} complete sectors incl. overscan",
                scene.top_sector,
                scene
                    .top_sector
                    .saturating_add(layout.visible_capacity())
                    .saturating_sub(1)
                    .min(scene.volume.total_sectors.saturating_sub(1)),
                scene.sectors.len()
            ),
            SemanticStyle::Muted,
        );
        frame.put_text(
            0,
            surface.height - 1,
            surface.width,
            &format!(
                "arrows move{separator}[ ] sector{separator}PgUp/PgDn volume{separator}wheel scroll{separator}Enter inspect{separator}q quit"
            ),
            SemanticStyle::Plain,
        );
        Ok(frame)
    }
}

pub(crate) struct SectorRenderer;

impl SectorRenderer {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn render(
        scene: &SectorScene,
        surface: Surface,
        profile: PresentationProfile,
    ) -> Result<VolumeFrame, FocusedError> {
        VolumeLayout::for_surface(surface)?;
        let cell_width = surface.width / SECTOR_GRID_COLUMNS;
        if cell_width < 7 {
            return Err(FocusedError::Arithmetic);
        }
        let mut frame = VolumeFrame::new(surface, profile);
        let separator = match profile.glyphs {
            GlyphProfile::Unicode => " · ",
            GlyphProfile::Ascii => " | ",
        };
        let fingerprint = snapshot_id_hex(scene.snapshot_id);
        frame.put_text(
            0,
            0,
            surface.width,
            &format!(
                " VOLMAP  snapshot {}  r{}  {} ",
                &fingerprint[..12],
                scene.revision.get(),
                scene.outcome
            ),
            SemanticStyle::Header,
        );
        frame.put_text(
            0,
            1,
            surface.width,
            &format!(
                "Volume {} ({}/{}) > Sector {}{separator}64 physical Pages",
                scene.volume.vol_id,
                scene.volume_index + 1,
                scene.volume_count,
                scene.sector.sector_id
            ),
            SemanticStyle::Plain,
        );
        frame.put_text(
            0,
            2,
            surface.width,
            if surface.width >= 80 {
                "cell: within-Sector Page · exact occupied % · physical type"
            } else {
                "cell: within-Sector Page + exact occupied %; focus details below"
            },
            SemanticStyle::Muted,
        );

        for row in 0..SECTOR_GRID_ROWS {
            for column in 0..SECTOR_GRID_COLUMNS {
                let page_index = u8::try_from(row * SECTOR_GRID_COLUMNS + column)
                    .map_err(|_| FocusedError::Arithmetic)?;
                let page = &scene.sector.pages[usize::from(page_index)];
                let left = column * surface.width / SECTOR_GRID_COLUMNS;
                let right = (column + 1) * surface.width / SECTOR_GRID_COLUMNS - 1;
                let occupied = page.occupancy.occupied_label();
                let label = if surface.width >= 80 {
                    format!("{page_index:02} {occupied:>4}{}", page.page_type.compact())
                } else {
                    format!("{page_index:02} {:>3}", page.occupancy.compact_value())
                };
                let style = if page_index == scene.focused_page {
                    SemanticStyle::Focus
                } else {
                    match page.occupancy {
                        ExactOccupancy::Unknown => SemanticStyle::Unknown,
                        ExactOccupancy::NotApplicable => SemanticStyle::Muted,
                        ExactOccupancy::Known { .. } => page.allocation.style(),
                    }
                };
                frame.put_text(left, SECTOR_GRID_TOP + row, right - left + 1, &label, style);
                frame.page_hits.push(PageHitRegion {
                    page_index,
                    page_id: page.page_id,
                    left,
                    top: SECTOR_GRID_TOP + row,
                    right,
                    bottom: SECTOR_GRID_TOP + row,
                });
            }
        }

        let focused = &scene.sector.pages[usize::from(scene.focused_page)];
        let detail_top = SECTOR_GRID_TOP + SECTOR_GRID_ROWS + 1;
        frame.put_text(
            0,
            detail_top,
            surface.width,
            &format!(
                "Page {} (cell {:02}){separator}type {}{separator}allocation {}",
                focused.page_id,
                scene.focused_page,
                focused.page_type.label(),
                focused.allocation.label()
            ),
            SemanticStyle::Focus,
        );
        frame.put_text(
            0,
            detail_top + 1,
            surface.width,
            &format!(
                "{}{separator}finding {}",
                focused.occupancy.descriptor(),
                focused.diagnostic.unwrap_or("none")
            ),
            if focused.finding {
                SemanticStyle::Finding
            } else {
                SemanticStyle::Plain
            },
        );
        let (attribution, context) = if surface.width < 80 {
            focused.attribution.labels()
        } else {
            (
                focused.attribution.label(),
                format!(
                    "Sector {}{separator}{}{separator}{}",
                    scene.sector.sector_id,
                    if scene.sector.reserved {
                        "reserved"
                    } else {
                        "unreserved"
                    },
                    scene.sector.attribution.label()
                ),
            )
        };
        frame.put_text(
            0,
            detail_top + 2,
            surface.width,
            &attribution,
            SemanticStyle::Plain,
        );
        frame.put_text(
            0,
            detail_top + 3,
            surface.width,
            &context,
            SemanticStyle::Muted,
        );
        let (status, help) = if surface.width < 80 {
            (
                format!(
                    "focus Page {}/64 | all 64 shown once | revision r{}",
                    u16::from(scene.focused_page) + 1,
                    scene.revision.get()
                ),
                "arrows move | [ ]/wheel Sector | Enter inspect Page".to_owned(),
            )
        } else {
            (
                format!(
                    "focus Page {} of 64{separator}all Pages shown once{separator}exact revision r{}",
                    u16::from(scene.focused_page) + 1,
                    scene.revision.get()
                ),
                format!(
                    "arrows move{separator}[ ] sibling Sector{separator}wheel sibling{separator}Enter inspect Page"
                ),
            )
        };
        frame.put_text(
            0,
            surface.height - 3,
            surface.width,
            &status,
            SemanticStyle::Muted,
        );
        frame.put_text(
            0,
            surface.height - 2,
            surface.width,
            &help,
            SemanticStyle::Plain,
        );
        frame.put_text(
            0,
            surface.height - 1,
            surface.width,
            &format!("Esc/Backspace Volume{separator}? help{separator}q quit"),
            SemanticStyle::Plain,
        );
        Ok(frame)
    }
}

pub(crate) struct PageRenderer;

impl PageRenderer {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn render(
        scene: &PageScene,
        surface: Surface,
        profile: PresentationProfile,
    ) -> Result<VolumeFrame, FocusedError> {
        VolumeLayout::for_surface(surface)?;
        let mut frame = VolumeFrame::new(surface, profile);
        let separator = match profile.glyphs {
            GlyphProfile::Unicode => " · ",
            GlyphProfile::Ascii => " | ",
        };
        let fingerprint = snapshot_id_hex(scene.snapshot_id);
        frame.put_text(
            0,
            0,
            surface.width,
            &format!(
                " VOLMAP  snapshot {}  r{}  {} ",
                &fingerprint[..12],
                scene.revision.get(),
                scene.outcome
            ),
            SemanticStyle::Header,
        );
        frame.put_text(
            0,
            1,
            surface.width,
            &format!(
                "Volume {} > Sector {} > Page {}{separator}volume {}/{}",
                scene.volume.vol_id,
                scene.sector_id,
                scene.page.page_id,
                scene.volume_index + 1,
                scene.volume_count,
            ),
            SemanticStyle::Plain,
        );
        let (file_class, table) = scene.page.attribution.labels();
        let page_facts = if surface.width < 80 {
            format!(
                "{}{separator}{}{separator}{}{separator}{table}{separator}{file_class}",
                scene.page.page_type.label(),
                scene.page.allocation.label(),
                scene.page.occupancy.descriptor(),
            )
        } else {
            format!(
                "type {}{separator}allocation {}{separator}{}{separator}{table}{separator}{file_class}",
                scene.page.page_type.label(),
                scene.page.allocation.label(),
                scene.page.occupancy.descriptor(),
            )
        };
        frame.put_text(0, 2, surface.width, &page_facts, SemanticStyle::Focus);

        if scene.interpretation_state != PageInterpretationState::Closed {
            draw_record_interpretation(&mut frame, scene, surface, separator);
            return Ok(frame);
        }

        match &scene.distribution {
            PageDistributionProjection::NotAvailable => {
                frame.put_text(
                    0,
                    3,
                    surface.width,
                    &format!("distribution: {}", scene.load.label()),
                    match scene.load {
                        PageLoadState::Failed(_) => SemanticStyle::Finding,
                        PageLoadState::Loading => SemanticStyle::Unknown,
                        _ => SemanticStyle::Muted,
                    },
                );
                frame.put_text(
                    0,
                    PAGE_ROWS_TOP,
                    surface.width,
                    scene.load.label(),
                    SemanticStyle::Muted,
                );
            }
            PageDistributionProjection::Available {
                content_size,
                record_extents,
                free_regions,
                slot_entries,
                allocated_record_bytes,
                unoccupied_bytes,
                ..
            } => {
                frame.put_text(
                    0,
                    3,
                    surface.width,
                    &format!(
                        "{} B{separator}{} records / {} B{separator}{} free regions / {} B{separator}{} slots",
                        grouped_number(*content_size),
                        record_extents.len(),
                        grouped_number(*allocated_record_bytes),
                        free_regions.len(),
                        grouped_number(*unoccupied_bytes),
                        slot_entries.len(),
                    ),
                    SemanticStyle::Plain,
                );
                frame.put_text(
                    0,
                    4,
                    surface.width,
                    "H header  R live record  f fragmented free  . contiguous free  D slot directory",
                    SemanticStyle::Muted,
                );
                draw_page_byte_map(&mut frame, scene, 5)?;
                frame.put_text(
                    0,
                    6,
                    surface.width,
                    "  region / exact byte range / length",
                    SemanticStyle::Header,
                );
                draw_distribution_rows(&mut frame, scene, surface)?;
            }
        }

        let selected = scene
            .selected_item
            .and_then(|selected| scene.items.iter().find(|item| item.id() == selected));
        let selected_label = selected.map_or_else(
            || format!("distribution status: {}", scene.load.label()),
            |item| {
                let action = if item.record_oid().is_some() {
                    "Enter can interpret this live record"
                } else {
                    "structural row; interpretation unavailable"
                };
                format!("{}{separator}{action}", format_distribution_item(item))
            },
        );
        frame.put_text(
            0,
            surface.height - 3,
            surface.width,
            &selected_label,
            SemanticStyle::Focus,
        );
        let top = scene
            .top_item
            .and_then(|top| scene.items.iter().position(|item| item.id() == top))
            .unwrap_or(0);
        frame.put_text(
            0,
            surface.height - 2,
            surface.width,
            &format!(
                "rows {}..{} of {}{separator}{}",
                if scene.items.is_empty() { 0 } else { top + 1 },
                (top + usize::from(page_visible_rows(surface))).min(scene.items.len()),
                scene.items.len(),
                scene.load.label(),
            ),
            SemanticStyle::Muted,
        );
        frame.put_text(
            0,
            surface.height - 1,
            surface.width,
            &format!(
                "Up/Down select{separator}wheel scroll{separator}[ ] sibling Sector{separator}Esc/Backspace Sector{separator}q quit"
            ),
            SemanticStyle::Plain,
        );
        Ok(frame)
    }
}

fn draw_record_interpretation(
    frame: &mut VolumeFrame,
    scene: &PageScene,
    surface: Surface,
    separator: &str,
) {
    let PageInterpretationState::Record {
        record,
        load,
        top_attribute,
    } = scene.interpretation_state
    else {
        return;
    };
    draw_interpretation_identity(frame, scene, surface, record, load, separator);

    let Some(selection) = scene.record_selection.as_ref() else {
        frame.put_text(
            0,
            5,
            surface.width,
            load.label(),
            match load {
                InterpretationLoadState::Failed(_) => SemanticStyle::Finding,
                _ => SemanticStyle::Muted,
            },
        );
        draw_interpretation_footer(frame, surface, 0, 0, load.label(), separator);
        return;
    };
    if let Some(reason) = record_selection_limitation(selection) {
        draw_unavailable_interpretation(frame, surface, reason, separator);
        return;
    }
    let Some(interpretation) = selection.interpretation.as_ref() else {
        draw_unavailable_interpretation(frame, surface, load.label(), separator);
        return;
    };

    draw_ready_interpretation(
        frame,
        selection,
        interpretation,
        top_attribute,
        load,
        surface,
        separator,
    );
}

fn draw_interpretation_identity(
    frame: &mut VolumeFrame,
    scene: &PageScene,
    surface: Surface,
    record: Oid,
    load: InterpretationLoadState,
    separator: &str,
) {
    let selected_type = scene.record_selection.as_ref().map_or_else(
        || {
            scene
                .items
                .iter()
                .find_map(|item| match item {
                    PageDistributionItem::Record {
                        oid, record_type, ..
                    } if *oid == record => Some(record_type.as_str()),
                    _ => None,
                })
                .unwrap_or("unknown")
        },
        |selection| selection.selected_slot.record_type,
    );
    let style = match load {
        InterpretationLoadState::Failed(_) => SemanticStyle::Finding,
        InterpretationLoadState::Loading => SemanticStyle::Unknown,
        InterpretationLoadState::Ready => SemanticStyle::Focus,
        InterpretationLoadState::Unavailable(_) => SemanticStyle::Muted,
    };
    frame.put_text(
        0,
        3,
        surface.width,
        &format!(
            "Record {}|{}|{}{separator}type {selected_type}{separator}{}",
            record.vol_id.get(),
            record.page_id.get(),
            record.slot_id.get(),
            load.label(),
        ),
        style,
    );
}

fn draw_unavailable_interpretation(
    frame: &mut VolumeFrame,
    surface: Surface,
    reason: &str,
    separator: &str,
) {
    frame.put_text(
        0,
        4,
        surface.width,
        &format!("record interpretation unavailable{separator}{reason}"),
        SemanticStyle::Finding,
    );
    frame.put_text(
        0,
        5,
        surface.width,
        "raw and undecodable record bytes remain withheld",
        SemanticStyle::Muted,
    );
    draw_interpretation_footer(frame, surface, 0, 0, reason, separator);
}

fn draw_ready_interpretation(
    frame: &mut VolumeFrame,
    selection: &RecordSelectionProjection,
    interpretation: &crate::projection::RecordInterpretationProjection,
    top_attribute: u32,
    load: InterpretationLoadState,
    surface: Surface,
    separator: &str,
) {
    let class = selection
        .class_representation
        .as_ref()
        .map_or("unresolved class", |representation| {
            class_name_label(&representation.class_name)
        });
    let relocation = optional_oid_label(&interpretation.relocated_from)
        .map_or_else(String::new, |origin| {
            format!("{separator}relocated from {origin}")
        });
    frame.put_text(
        0,
        4,
        surface.width,
        &format!(
            "class/table {class}{separator}representation {}{separator}record {}{relocation}",
            interpretation.representation_id,
            oid_label(interpretation.record),
        ),
        SemanticStyle::Plain,
    );
    draw_interpretation_layout(frame, interpretation, surface, separator);
    draw_interpretation_attributes(frame, interpretation, top_attribute, surface);
    let top = usize::try_from(top_attribute)
        .unwrap_or(usize::MAX)
        .min(interpretation.attributes.len());
    draw_interpretation_footer(
        frame,
        surface,
        top,
        interpretation.attributes.len(),
        load.label(),
        separator,
    );
}

fn draw_interpretation_layout(
    frame: &mut VolumeFrame,
    interpretation: &crate::projection::RecordInterpretationProjection,
    surface: Surface,
    separator: &str,
) {
    if let Some(layout) = interpretation.layout.as_ref() {
        let regions = layout
            .regions
            .iter()
            .map(|region| format!("{} @{}+{}", region.region, region.offset, region.length))
            .collect::<Vec<_>>();
        frame.put_text(
            0,
            5,
            surface.width,
            &format!(
                "layout {} B{separator}{}",
                layout.record_length,
                regions
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(separator)
            ),
            SemanticStyle::Plain,
        );
        frame.put_text(
            0,
            6,
            surface.width,
            &format!(
                "layout continued{separator}{}",
                regions
                    .iter()
                    .skip(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(separator)
            ),
            SemanticStyle::Muted,
        );
    } else {
        frame.put_text(
            0,
            5,
            surface.width,
            "record layout unavailable; record bytes remain withheld",
            SemanticStyle::Finding,
        );
    }
}

fn draw_interpretation_attributes(
    frame: &mut VolumeFrame,
    interpretation: &crate::projection::RecordInterpretationProjection,
    top_attribute: u32,
    surface: Surface,
) {
    frame.put_text(
        0,
        7,
        surface.width,
        "  # attribute / domain / storage / exact extent / typed state and value",
        SemanticStyle::Header,
    );

    let top = usize::try_from(top_attribute)
        .unwrap_or(usize::MAX)
        .min(interpretation.attributes.len());
    let visible = usize::from(interpretation_visible_rows(surface));
    for (visible_index, attribute) in interpretation
        .attributes
        .iter()
        .skip(top)
        .take(visible)
        .enumerate()
    {
        let row = INTERPRETATION_ROWS_TOP
            .saturating_add(u16::try_from(visible_index).unwrap_or(u16::MAX));
        frame.put_text(
            0,
            row,
            surface.width,
            &format_interpreted_attribute(attribute),
            if visible_index == 0 {
                SemanticStyle::Focus
            } else {
                SemanticStyle::Plain
            },
        );
    }
}

fn draw_interpretation_footer(
    frame: &mut VolumeFrame,
    surface: Surface,
    top: usize,
    total: usize,
    status: &str,
    separator: &str,
) {
    let visible = usize::from(interpretation_visible_rows(surface));
    frame.put_text(
        0,
        surface.height - 3,
        surface.width,
        &format!("{status}{separator}decoded values shown only for this explicit record action"),
        SemanticStyle::Focus,
    );
    frame.put_text(
        0,
        surface.height - 2,
        surface.width,
        &format!(
            "attributes {}..{} of {total}{separator}undecodable bytes withheld",
            if total == 0 { 0 } else { top + 1 },
            (top + visible).min(total),
        ),
        SemanticStyle::Muted,
    );
    frame.put_text(
        0,
        surface.height - 1,
        surface.width,
        &format!(
            "Esc/Backspace close interpretation{separator}Up/Down or wheel scroll{separator}q quit"
        ),
        SemanticStyle::Plain,
    );
}

fn oid_label(oid: OidProjection) -> String {
    format!("{}|{}|{}", oid.vol_id, oid.page_id, oid.slot_id)
}

fn optional_oid_label(oid: &OptionalOidProjection) -> Option<String> {
    match oid {
        OptionalOidProjection::Absent => None,
        OptionalOidProjection::Present { oid } => Some(oid_label(*oid)),
    }
}

fn class_name_label(class_name: &ClassNameProjection) -> &str {
    match class_name {
        ClassNameProjection::Resolved { value } => value,
        ClassNameProjection::Unresolved { reason }
        | ClassNameProjection::NotApplicable { reason } => reason,
    }
}

fn attribute_name_label(name: &AttributeNameProjection) -> &str {
    match name {
        AttributeNameProjection::Resolved { value } => value,
        AttributeNameProjection::Unresolved { reason } => reason,
    }
}

fn format_interpreted_attribute(
    attribute: &crate::projection::InterpretedAttributeProjection,
) -> String {
    let value = match &attribute.value {
        AttributeValueProjection::Decoded { value } => format!("decoded {value}"),
        AttributeValueProjection::Null => "null".to_owned(),
        AttributeValueProjection::OutOfRow { head, total_length } => {
            format!("out-of-row {} ({total_length} B)", oid_label(*head))
        }
        AttributeValueProjection::Withheld {
            reason,
            offset,
            length,
        } => format!("withheld {reason} @{offset}+{length}"),
    };
    format!(
        "{:03} {} {}({},{}) {} @{}+{} {value}",
        attribute.position,
        attribute_name_label(&attribute.name),
        attribute.type_name,
        attribute.precision,
        attribute.scale,
        attribute.storage,
        attribute.offset,
        attribute.length,
    )
}

fn grouped_number(value: u32) -> String {
    if value < 1_000 {
        value.to_string()
    } else {
        let high = value / 1_000;
        let low = value % 1_000;
        format!("{high},{low:03}")
    }
}

fn draw_page_byte_map(
    frame: &mut VolumeFrame,
    scene: &PageScene,
    row: u16,
) -> Result<(), FocusedError> {
    let PageDistributionProjection::Available {
        content_size,
        header,
        record_extents,
        free_regions,
        slot_directory,
        ..
    } = &scene.distribution
    else {
        return Ok(());
    };
    if *content_size == 0 {
        return Err(FocusedError::Arithmetic);
    }
    for free in free_regions {
        let glyph = match free.kind {
            FreeRegionKindProjection::ContiguousFree => '.',
            FreeRegionKindProjection::FragmentedFree => 'f',
        };
        paint_byte_region(
            frame,
            row,
            *content_size,
            free.offset,
            free.length,
            glyph,
            SemanticStyle::Muted,
        )?;
    }
    paint_byte_region(
        frame,
        row,
        *content_size,
        header.offset,
        header.length,
        'H',
        SemanticStyle::Header,
    )?;
    for record in record_extents {
        paint_byte_region(
            frame,
            row,
            *content_size,
            record.offset,
            record.length,
            'R',
            SemanticStyle::Allocated,
        )?;
    }
    paint_byte_region(
        frame,
        row,
        *content_size,
        slot_directory.offset,
        slot_directory.length,
        'D',
        SemanticStyle::Reserved,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn paint_byte_region(
    frame: &mut VolumeFrame,
    row: u16,
    content_size: u32,
    offset: u32,
    length: u32,
    glyph: char,
    style: SemanticStyle,
) -> Result<(), FocusedError> {
    if length == 0 {
        return Ok(());
    }
    let width = u64::from(frame.surface.width);
    let content = u64::from(content_size);
    let start = u64::from(offset)
        .checked_mul(width)
        .ok_or(FocusedError::Arithmetic)?
        / content;
    let end_bytes = u64::from(offset)
        .checked_add(u64::from(length))
        .ok_or(FocusedError::Arithmetic)?;
    let end = end_bytes
        .checked_mul(width)
        .and_then(|scaled| scaled.checked_add(content - 1))
        .ok_or(FocusedError::Arithmetic)?
        / content;
    let start =
        u16::try_from(start.min(width.saturating_sub(1))).map_err(|_| FocusedError::Arithmetic)?;
    let end = u16::try_from(end.max(u64::from(start) + 1).min(width))
        .map_err(|_| FocusedError::Arithmetic)?;
    for column in start..end {
        frame.put(column, row, glyph, style);
    }
    Ok(())
}

fn draw_distribution_rows(
    frame: &mut VolumeFrame,
    scene: &PageScene,
    surface: Surface,
) -> Result<(), FocusedError> {
    let visible = usize::from(page_visible_rows(surface));
    let top = scene
        .top_item
        .and_then(|top| scene.items.iter().position(|item| item.id() == top))
        .unwrap_or(0)
        .min(scene.items.len());
    let formatted_start = top.saturating_sub(visible);
    let formatted_end = top
        .saturating_add(visible.saturating_mul(2))
        .min(scene.items.len());
    let formatted = scene.items[formatted_start..formatted_end]
        .iter()
        .enumerate()
        .map(|(relative, item)| {
            (
                formatted_start + relative,
                format_distribution_item(item),
                distribution_item_style(item),
            )
        })
        .collect::<Vec<_>>();
    frame.formatted_distribution_rows = formatted.len();

    for (screen_index, item_index) in (top..scene.items.len()).take(visible).enumerate() {
        let item = &scene.items[item_index];
        let (_, text, style) = formatted
            .iter()
            .find(|(index, _, _)| *index == item_index)
            .ok_or(FocusedError::Arithmetic)?;
        let row = PAGE_ROWS_TOP
            .checked_add(u16::try_from(screen_index).map_err(|_| FocusedError::Arithmetic)?)
            .ok_or(FocusedError::Arithmetic)?;
        let selected = scene.selected_item == Some(item.id());
        frame.put_text(
            0,
            row,
            surface.width,
            &format!("{} {text}", if selected { '>' } else { ' ' }),
            if selected {
                SemanticStyle::Focus
            } else {
                *style
            },
        );
        frame.distribution_hits.push(DistributionHitRegion {
            item: item.id(),
            left: 0,
            top: row,
            right: surface.width - 1,
            bottom: row,
        });
    }
    Ok(())
}

fn format_distribution_item(item: &PageDistributionItem) -> String {
    let region = item.region();
    let end = region.offset.saturating_add(region.length);
    match item {
        PageDistributionItem::Header { .. } => {
            format!("header [{},{end}) {} B", region.offset, region.length)
        }
        PageDistributionItem::Record {
            slot_id,
            record_type,
            ..
        } => format!(
            "record slot {slot_id} {} [{},{end}) {} B",
            record_type.as_str(),
            region.offset,
            region.length
        ),
        PageDistributionItem::Free { kind, .. } => {
            format!(
                "{} [{},{end}) {} B",
                kind.as_str(),
                region.offset,
                region.length
            )
        }
        PageDistributionItem::SlotDirectory { .. } => format!(
            "slot directory [{},{end}) {} B",
            region.offset, region.length
        ),
        PageDistributionItem::SlotEntry {
            slot_id,
            state,
            record_type,
            ..
        } => format!(
            "slot {slot_id} {} {record_type} [{},{end}) {} B",
            state.as_str(),
            region.offset,
            region.length
        ),
    }
}

fn distribution_item_style(item: &PageDistributionItem) -> SemanticStyle {
    match item {
        PageDistributionItem::Header { .. } => SemanticStyle::Header,
        PageDistributionItem::Record { .. } => SemanticStyle::Allocated,
        PageDistributionItem::Free { .. } => SemanticStyle::Muted,
        PageDistributionItem::SlotDirectory { .. } => SemanticStyle::Reserved,
        PageDistributionItem::SlotEntry { state, .. } => match state {
            SlotEntryStateProjection::Allocated => SemanticStyle::Allocated,
            SlotEntryStateProjection::Deleted => SemanticStyle::Finding,
            SlotEntryStateProjection::Unallocated => SemanticStyle::Muted,
        },
    }
}

fn draw_card(
    frame: &mut VolumeFrame,
    left: u16,
    top: u16,
    sector: &SectorCard,
    focused: bool,
    profile: PresentationProfile,
) {
    let border = match (profile.glyphs, focused) {
        (GlyphProfile::Unicode, false) => ('┌', '─', '┐', '│', '└', '┘'),
        (GlyphProfile::Unicode, true) => ('╔', '═', '╗', '║', '╚', '╝'),
        (GlyphProfile::Ascii, _) => ('+', '-', '+', '|', '+', '+'),
    };
    let border_style = if focused {
        SemanticStyle::Focus
    } else {
        SemanticStyle::Muted
    };
    frame.put(left, top, border.0, border_style);
    for offset in 1..CARD_WIDTH - 1 {
        frame.put(left + offset, top, border.1, border_style);
    }
    frame.put(left + CARD_WIDTH - 1, top, border.2, border_style);

    frame.put(left, top + 1, border.3, border_style);
    frame.put(left + CARD_WIDTH - 1, top + 1, border.3, border_style);
    let heading = format!(
        "S{}{} {}",
        sector.sector_id,
        if sector.finding { "!" } else { "" },
        sector.attribution.label()
    );
    frame.put_text(
        left + 1,
        top + 1,
        CARD_WIDTH - 2,
        &heading,
        if sector.finding {
            SemanticStyle::Finding
        } else {
            SemanticStyle::Header
        },
    );

    for row in 0_u16..8 {
        let screen_row = top + 2 + row;
        frame.put(left, screen_row, border.3, border_style);
        frame.put(left + CARD_WIDTH - 1, screen_row, border.3, border_style);
        for column in 0_u16..8 {
            let index = usize::from(row * 8 + column);
            let page = &sector.pages[index];
            let screen_column = left + 1 + column * 2;
            frame.put(
                screen_column,
                screen_row,
                page.allocation.glyph(),
                page.allocation.style(),
            );
            let occupancy = page.occupancy.volume_mark();
            let occupancy_style = match occupancy {
                OccupancyMark::Unknown => SemanticStyle::Unknown,
                OccupancyMark::NotApplicable => SemanticStyle::Muted,
                OccupancyMark::Zero | OccupancyMark::Level(_) => SemanticStyle::Occupancy,
            };
            frame.put(
                screen_column + 1,
                screen_row,
                occupancy.glyph(profile.glyphs),
                occupancy_style,
            );
        }
    }

    let bottom = top + CARD_HEIGHT - 1;
    frame.put(left, bottom, border.4, border_style);
    for offset in 1..CARD_WIDTH - 1 {
        frame.put(left + offset, bottom, border.1, border_style);
    }
    frame.put(left + CARD_WIDTH - 1, bottom, border.5, border_style);
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs::{File, OpenOptions};
    use std::io::Write;
    use std::os::unix::fs::FileExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::format::{IO_PAGE_SIZE, PageType};
    use crate::inspection::{
        CancelToken, Inspection, OpenRequest, ResourcePolicy, RevisionSelector,
    };
    use crate::model::PageId;
    use crate::projection::{
        BytesWithheldProjection, FileAssociationProjection, OptionalCountProjection,
        VolumeProjection,
    };
    use crate::source::InputSpec;

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "volmap-focused-tui-test-{}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn envelope_page(vol_id: i16, page_id: i32, page_type: PageType) -> [u8; IO_PAGE_SIZE] {
        let mut page = [0_u8; IO_PAGE_SIZE];
        let lsa = u64::try_from(page_id).unwrap().to_le_bytes();
        page[0..8].copy_from_slice(&lsa);
        page[8..12].copy_from_slice(&page_id.to_le_bytes());
        page[12..14].copy_from_slice(&vol_id.to_le_bytes());
        page[14] = page_type.ordinal();
        page[IO_PAGE_SIZE - 8..].copy_from_slice(&lsa);
        page
    }

    fn volume_header_page(vol_id: i16) -> [u8; IO_PAGE_SIZE] {
        let mut page = envelope_page(vol_id, 0, PageType::VolumeHeader);
        let user = &mut page[32..IO_PAGE_SIZE - 8];
        user[..25].copy_from_slice(b"CUBRID/Volume\0\0\0\0\0\0\0\0\0\0\0\0");
        user[26..28].copy_from_slice(&16_384_i16.to_le_bytes());
        user[28..30].copy_from_slice(&vol_id.to_le_bytes());
        user[32..36].copy_from_slice(&0_i32.to_le_bytes());
        user[36..40].copy_from_slice(&0_i32.to_le_bytes());
        user[40..44].copy_from_slice(&64_i32.to_le_bytes());
        user[44..48].copy_from_slice(&64_i32.to_le_bytes());
        user[48..52].copy_from_slice(&64_i32.to_le_bytes());
        user[52..56].copy_from_slice(&(-1_i32).to_le_bytes());
        user[56..60].copy_from_slice(&1_i32.to_le_bytes());
        user[60..64].copy_from_slice(&1_i32.to_le_bytes());
        user[64..68].copy_from_slice(&1_i32.to_le_bytes());
        user[96..100].copy_from_slice(&(-1_i32).to_le_bytes());
        user[100..102].copy_from_slice(&(-1_i16).to_le_bytes());
        user[104..108].copy_from_slice(&(-1_i32).to_le_bytes());
        user[124..126].copy_from_slice(&(-1_i16).to_le_bytes());
        user[126..128].copy_from_slice(&0_i16.to_le_bytes());
        user[128..130].copy_from_slice(&1_i16.to_le_bytes());
        user[130..132].copy_from_slice(&2_i16.to_le_bytes());
        page
    }

    fn slotted_heap_page(vol_id: i16, page_id: i32) -> [u8; IO_PAGE_SIZE] {
        slotted_heap_page_with_selected_type(vol_id, page_id, 3)
    }

    fn slotted_heap_page_with_selected_type(
        vol_id: i16,
        page_id: i32,
        selected_type: u8,
    ) -> [u8; IO_PAGE_SIZE] {
        let mut page = envelope_page(vol_id, page_id, PageType::Heap);
        let user = &mut page[32..IO_PAGE_SIZE - 8];
        user[0..2].copy_from_slice(&4_i16.to_le_bytes());
        user[2..4].copy_from_slice(&3_i16.to_le_bytes());
        user[4..6].copy_from_slice(&1_i16.to_le_bytes());
        user[6..8].copy_from_slice(&8_u16.to_le_bytes());
        user[8..12].copy_from_slice(&16_256_i32.to_le_bytes());
        user[12..16].copy_from_slice(&16_200_i32.to_le_bytes());
        user[16..20].copy_from_slice(&128_i32.to_le_bytes());
        for (slot, offset, length, kind) in [
            (0_usize, 32_u16, 24_u16, 2_u8),
            (1, 0, 0, 9),
            // Retained tombstone geometry is validated but is not a live record.
            (2, 104, 16, 6),
            (3, 80, 16, selected_type),
        ] {
            let word = u32::from(offset) | (u32::from(length) << 14) | (u32::from(kind) << 28);
            let start = crate::format::DB_PAGE_SIZE - 4 * (slot + 1);
            user[start..start + 4].copy_from_slice(&word.to_le_bytes());
        }
        page
    }

    fn fixture_policy() -> ResourcePolicy {
        ResourcePolicy::new(4 * 1024 * 1024, 1024 * 1024, 1, 32, 1024 * 1024).unwrap()
    }

    fn interpretation_fixture_policy() -> ResourcePolicy {
        ResourcePolicy::new(8 * 1024 * 1024, 1024 * 1024, 1, 64, 8 * 1024 * 1024).unwrap()
    }

    fn fixture_view() -> (TestDirectory, GraphView) {
        let directory = TestDirectory::new();
        let vinf = directory.path().join("fixture_vinf");
        let mut manifest = File::create(&vinf).unwrap();
        for vol_id in 0_i16..=1 {
            let volume = directory.path().join(format!("fixture-{vol_id}"));
            let file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&volume)
                .unwrap();
            file.set_len(64 * 64 * u64::try_from(IO_PAGE_SIZE).unwrap())
                .unwrap();
            file.write_all_at(&volume_header_page(vol_id), 0).unwrap();
            let mut bitmap = envelope_page(vol_id, 1, PageType::VolumeBitmap);
            bitmap[32..40].copy_from_slice(&3_u64.to_le_bytes());
            file.write_all_at(&bitmap, u64::try_from(IO_PAGE_SIZE).unwrap())
                .unwrap();
            file.write_all_at(
                &slotted_heap_page(vol_id, 2),
                2 * u64::try_from(IO_PAGE_SIZE).unwrap(),
            )
            .unwrap();
            let mut invalid = slotted_heap_page(vol_id, 3);
            invalid[36..38].copy_from_slice(&0_i16.to_le_bytes());
            file.write_all_at(&invalid, 3 * u64::try_from(IO_PAGE_SIZE).unwrap())
                .unwrap();
            file.write_all_at(
                &slotted_heap_page(vol_id, 66),
                66 * u64::try_from(IO_PAGE_SIZE).unwrap(),
            )
            .unwrap();
            file.write_all_at(
                &slotted_heap_page_with_selected_type(vol_id, 67, 5),
                67 * u64::try_from(IO_PAGE_SIZE).unwrap(),
            )
            .unwrap();
            drop(file);
            writeln!(manifest, "{vol_id} {}", volume.display()).unwrap();
        }
        drop(manifest);
        let inspection = Inspection::open(
            &OpenRequest {
                input: InputSpec::Vinf {
                    path: vinf,
                    volume_root: None,
                },
                tde_keys_file: None,
                spill_directory: None,
            },
            fixture_policy(),
            &CancelToken::new(),
            None,
        )
        .unwrap();
        let view = inspection.view(RevisionSelector::Latest).unwrap();
        (directory, view)
    }

    fn corpus_page(name: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/e1e651de-records/pages")
                .join(name),
        )
        .unwrap()
    }

    fn write_interpretation_volume(path: &Path, vol_id: i16, pages: &[(i32, Vec<u8>)]) {
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .unwrap();
        file.set_len(64 * 64 * u64::try_from(IO_PAGE_SIZE).unwrap())
            .unwrap();
        file.write_all_at(&volume_header_page(vol_id), 0).unwrap();

        let mut reserved = 1_u64;
        for (page_id, _) in pages {
            reserved |= 1_u64 << (u32::try_from(*page_id).unwrap() / 64);
        }
        let mut bitmap = envelope_page(vol_id, 1, PageType::VolumeBitmap);
        bitmap[32..40].copy_from_slice(&reserved.to_le_bytes());
        file.write_all_at(&bitmap, u64::try_from(IO_PAGE_SIZE).unwrap())
            .unwrap();

        for sector in 0_u32..64 {
            if reserved & (1_u64 << sector) == 0 {
                continue;
            }
            for page_id in sector * 64..(sector + 1) * 64 {
                if page_id < 2 {
                    continue;
                }
                let page_id = i32::try_from(page_id).unwrap();
                let bytes = pages
                    .iter()
                    .find(|(candidate, _)| *candidate == page_id)
                    .map_or_else(
                        || envelope_page(vol_id, page_id, PageType::Unknown).to_vec(),
                        |(_, bytes)| bytes.clone(),
                    );
                file.write_all_at(
                    &bytes,
                    u64::try_from(page_id).unwrap() * u64::try_from(IO_PAGE_SIZE).unwrap(),
                )
                .unwrap();
            }
        }
    }

    fn interpretation_fixture_view() -> (TestDirectory, GraphView) {
        let directory = TestDirectory::new();
        let volume0 = directory.path().join("interpretation");
        let volume1 = directory.path().join("interpretation_x001");
        let vinf = directory.path().join("interpretation_vinf");
        write_interpretation_volume(&volume0, 0, &[(195, corpus_page("vol0-page195.bin"))]);
        write_interpretation_volume(&volume1, 1, &[(641, corpus_page("vol1-page641.bin"))]);
        let mut manifest = File::create(&vinf).unwrap();
        writeln!(manifest, "0 {}", volume0.display()).unwrap();
        writeln!(manifest, "1 {}", volume1.display()).unwrap();
        drop(manifest);
        let policy = interpretation_fixture_policy();
        let view = Inspection::open(
            &OpenRequest {
                input: InputSpec::Vinf {
                    path: vinf,
                    volume_root: None,
                },
                tde_keys_file: None,
                spill_directory: None,
            },
            policy,
            &CancelToken::new(),
            None,
        )
        .unwrap()
        .view(RevisionSelector::Latest)
        .unwrap();
        (directory, view)
    }

    fn page(
        page_id: i32,
        allocation: &'static str,
        occupancy: PageOccupancyProjection,
        finding: bool,
    ) -> PageProjection {
        PageProjection {
            vol_id: 0,
            page_id,
            sector_id: page_id / 64,
            allocation,
            page_type: OptionalTextProjection::Known("heap"),
            availability: "available",
            tde_state: "not-encrypted",
            detail_support: OptionalTextProjection::Known("semantic"),
            occupancy,
            lsa_word: OptionalCountProjection::Unknown,
            diagnostic: if finding {
                OptionalTextProjection::Known("page.test")
            } else {
                OptionalTextProjection::Unknown
            },
            bytes: BytesWithheldProjection {
                state: "bytes-withheld",
            },
            file_association: FileAssociationProjection::None,
        }
    }

    fn synthetic_card(sector_id: i32) -> SectorCard {
        let percentages = [1, 13, 26, 38, 51, 63, 76, 100];
        let pages = (0_i32..64)
            .map(|within| {
                let page_id = sector_id * 64 + within;
                let (allocation, occupancy) = match within {
                    0..=7 => (
                        "allocated",
                        PageOccupancyProjection::Known {
                            occupied_percent: percentages[usize::try_from(within).unwrap()],
                            free_percent: 100 - percentages[usize::try_from(within).unwrap()],
                        },
                    ),
                    8 => (
                        "allocated",
                        PageOccupancyProjection::Known {
                            occupied_percent: 0,
                            free_percent: 100,
                        },
                    ),
                    9 => ("allocated", PageOccupancyProjection::Unknown),
                    10..=25 => ("system-metadata", PageOccupancyProjection::Unknown),
                    26..=43 => ("reserved-unallocated", PageOccupancyProjection::Unknown),
                    _ => ("unreserved", PageOccupancyProjection::Unknown),
                };
                let mut page = page(page_id, allocation, occupancy, within == 7 || within == 17);
                if within == 7 {
                    page.file_association = FileAssociationProjection::Allocated {
                        file: crate::projection::FileAssociationBodyProjection {
                            vol_id: 0,
                            file_id: 91,
                            file_type: OptionalTextProjection::Known("heap"),
                            class_oid: crate::projection::OptionalOidProjection::Present {
                                oid: crate::projection::OidProjection {
                                    vol_id: 0,
                                    page_id: 777,
                                    slot_id: 3,
                                },
                            },
                            class_name: ClassNameProjection::Resolved {
                                value: "orders\u{1b}[31m".to_owned(),
                            },
                        },
                    };
                }
                page
            })
            .collect::<Vec<_>>();
        SectorCard::try_from_projection(SectorProjection {
            vol_id: 0,
            sector_id,
            reserved: sector_id % 2 == 0,
            attribution: if sector_id == 0 {
                SectorAttributionProjection::Single {
                    file: crate::projection::FileAssociationBodyProjection {
                        vol_id: 0,
                        file_id: 64,
                        file_type: OptionalTextProjection::Known("heap"),
                        class_oid: crate::projection::OptionalOidProjection::Absent,
                        class_name: ClassNameProjection::Resolved {
                            value: "한글e\u{301}\u{1b}[31m-table".to_owned(),
                        },
                    },
                    full: true,
                    allocated_pages: 64,
                    reserved_unallocated_pages: 0,
                }
            } else {
                SectorAttributionProjection::Unclaimed
            },
            pages,
        })
        .unwrap()
    }

    fn synthetic_scene(surface: Surface) -> VolumeScene {
        let layout = VolumeLayout::for_surface(surface).unwrap();
        let count = layout.projection_capacity();
        VolumeScene {
            snapshot_id: SnapshotId::from_bytes([0xAB; 16]),
            revision: InspectionRevision::new(7),
            outcome: "success-limited",
            volume: VolumeProjection {
                vol_id: 0,
                purpose: "permanent-data",
                volume_type: "permanent",
                total_sectors: 64,
                maximum_sectors: 64,
                system_last_page: 1,
                reserved_sectors: 32,
            },
            volume_index: 0,
            volume_count: 1,
            focused_sector: 0,
            top_sector: 0,
            layout,
            sectors: (0..count)
                .map(|sector| synthetic_card(i32::try_from(sector).unwrap()))
                .collect(),
        }
    }

    fn synthetic_sector_scene(focused_page: u8) -> SectorScene {
        SectorScene {
            snapshot_id: SnapshotId::from_bytes([0xAB; 16]),
            revision: InspectionRevision::new(7),
            outcome: "success-limited",
            volume: VolumeProjection {
                vol_id: 0,
                purpose: "permanent-data",
                volume_type: "permanent",
                total_sectors: 64,
                maximum_sectors: 64,
                system_last_page: 1,
                reserved_sectors: 32,
            },
            volume_index: 0,
            volume_count: 1,
            focused_page,
            sector: synthetic_card(3),
        }
    }

    fn ready_page_scene() -> PageScene {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(80, 24);
        let mut session = FocusedSession::new(view, fixture_policy()).unwrap();
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let completion = session.take_enrichment_request().unwrap().execute();
        session.complete_enrichment(completion).unwrap();
        let mut scene = session.page_scene().unwrap();
        scene.snapshot_id = SnapshotId::from_bytes([0xAB; 16]);
        scene.revision = InspectionRevision::new(7);
        scene.outcome = "success-limited";
        scene.page.allocation = AllocationMark::Allocated;
        scene.page.occupancy = ExactOccupancy::Known {
            occupied_percent: 63,
            free_percent: 37,
        };
        scene.page.attribution = PageAttributionMark::Single {
            kind: PageClaimKind::Allocated,
            vol_id: 0,
            file_id: 91,
            role: Some("heap"),
            class_oid: Some((0, 777, 3)),
            class: ClassAttributionMark::Resolved("orders".to_owned()),
        };
        scene
    }

    fn ready_interpretation_scene() -> PageScene {
        let (_directory, view) = interpretation_fixture_view();
        let surface = Surface::new(80, 24);
        let mut session = FocusedSession::new(view, interpretation_fixture_policy()).unwrap();
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::NextVolume,
                FocusedAction::FocusSector(10),
                FocusedAction::Activate,
                FocusedAction::FocusPage(1),
                FocusedAction::Activate,
            ],
        );
        let page = session.take_enrichment_request().unwrap().execute();
        session.complete_enrichment(page).unwrap();
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::FocusDistributionItem(PageDistributionItemId::Record(1)),
                FocusedAction::Activate,
            ],
        );
        let record = session.take_enrichment_request().unwrap().execute();
        session.complete_enrichment(record).unwrap();
        let mut scene = session.page_scene().unwrap();
        scene.snapshot_id = SnapshotId::from_bytes([0xAB; 16]);
        scene.revision = InspectionRevision::new(7);
        scene.outcome = "success-limited";
        scene
    }

    fn apply_actions(
        session: &mut FocusedSession,
        surface: Surface,
        actions: impl IntoIterator<Item = FocusedAction>,
    ) {
        for action in actions {
            session.advance_focused(action, surface).unwrap();
        }
    }

    fn pending_synthetic_record_request(
        view: GraphView,
        surface: Surface,
    ) -> (FocusedSession, FocusedEnrichmentRequest, InspectionRevision) {
        let mut session = FocusedSession::new(view, fixture_policy()).unwrap();
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let page = session.take_enrichment_request().unwrap().execute();
        session.complete_enrichment(page).unwrap();
        let revision = session.focused_state().volume.revision;
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::FocusDistributionItem(PageDistributionItemId::Record(3)),
                FocusedAction::Activate,
            ],
        );
        let request = session.take_enrichment_request().unwrap();
        (session, request, revision)
    }

    #[test]
    fn occupancy_vocabulary_is_exhaustive_and_stable() {
        let percentages = [1, 13, 26, 38, 51, 63, 76, 100];
        for (index, percent) in percentages.into_iter().enumerate() {
            let mark = OccupancyMark::from_projection(
                AllocationMark::Allocated,
                &PageOccupancyProjection::Known {
                    occupied_percent: percent,
                    free_percent: 100 - percent,
                },
            );
            assert_eq!(mark, OccupancyMark::Level(u8::try_from(index + 1).unwrap()));
        }
        assert_eq!(
            OccupancyMark::from_projection(
                AllocationMark::Allocated,
                &PageOccupancyProjection::Known {
                    occupied_percent: 7,
                    free_percent: 93,
                }
            ),
            OccupancyMark::Level(1)
        );
        assert_eq!(
            OccupancyMark::from_projection(
                AllocationMark::Allocated,
                &PageOccupancyProjection::Known {
                    occupied_percent: 0,
                    free_percent: 100,
                }
            ),
            OccupancyMark::Zero
        );
        assert_eq!(
            OccupancyMark::from_projection(
                AllocationMark::Allocated,
                &PageOccupancyProjection::Unknown
            ),
            OccupancyMark::Unknown
        );
        for allocation in [
            AllocationMark::System,
            AllocationMark::Reserved,
            AllocationMark::Unreserved,
        ] {
            assert_eq!(
                OccupancyMark::from_projection(
                    allocation,
                    &PageOccupancyProjection::Known {
                        occupied_percent: 100,
                        free_percent: 0,
                    }
                ),
                OccupancyMark::NotApplicable
            );
        }
        assert_eq!(
            (1_u8..=8)
                .map(|level| OccupancyMark::Level(level).glyph(GlyphProfile::Unicode))
                .collect::<String>(),
            "⡀⣀⣄⣤⣦⣶⣷⣿"
        );
        assert_eq!(
            (1_u8..=8)
                .map(|level| OccupancyMark::Level(level).glyph(GlyphProfile::Ascii))
                .collect::<String>(),
            "12345678"
        );
    }

    #[test]
    fn session_navigation_is_grid_clamped_and_projection_is_viewport_bounded() {
        let (_directory, view) = fixture_view();
        let mut traversal = FocusedSession::new(view.clone(), fixture_policy()).unwrap();
        let layout = VolumeLayout::for_surface(Surface::new(60, 20)).unwrap();
        assert_eq!(
            layout,
            VolumeLayout {
                columns: 3,
                visible_rows: 1
            }
        );
        let mut seen = BTreeSet::new();
        loop {
            let scene = traversal.scene(layout).unwrap();
            seen.extend(
                scene
                    .sectors
                    .iter()
                    .take(usize::try_from(layout.visible_capacity()).unwrap())
                    .map(|sector| sector.sector_id),
            );
            if !traversal
                .advance(VolumeAction::ScrollRows(1), layout)
                .changed
            {
                break;
            }
        }
        assert_eq!(seen, (0_i32..64).collect());

        let mut session = FocusedSession::new(view, fixture_policy()).unwrap();
        assert!(!session.advance(VolumeAction::Left, layout).changed);
        assert!(session.advance(VolumeAction::Right, layout).changed);
        assert!(session.advance(VolumeAction::Right, layout).changed);
        assert!(!session.advance(VolumeAction::Right, layout).changed);
        let moved = session.advance(VolumeAction::Down, layout);
        assert_eq!(moved.state.focused_sector, 5);
        assert_eq!(moved.state.top_sector, 3);
        let sibling = session.advance(VolumeAction::NextSector, layout);
        assert_eq!(sibling.state.focused_sector, 6);
        assert_eq!(sibling.state.top_sector, 6);
        let previous = session.advance(VolumeAction::PreviousSector, layout);
        assert_eq!(previous.state.focused_sector, 5);
        assert_eq!(previous.state.top_sector, 5);
        let restored = session.advance(VolumeAction::NextSector, layout);
        assert_eq!(restored.state.focused_sector, 6);
        assert_eq!(restored.state.top_sector, 5);
        let up = session.advance(VolumeAction::Up, layout);
        assert_eq!(up.state.focused_sector, 3);
        assert_eq!(up.state.top_sector, 3);
        let down = session.advance(VolumeAction::Down, layout);
        assert_eq!(down.state.focused_sector, 6);
        assert_eq!(down.state.top_sector, 6);
        let scrolled = session.advance(VolumeAction::ScrollRows(1), layout);
        assert_eq!(scrolled.state.focused_sector, 6);
        assert_eq!(scrolled.state.top_sector, 9);
        let scene = session.scene(layout).unwrap();
        assert_eq!(scene.sectors.len(), 6);
        assert_eq!(scene.sectors[0].sector_id, 9);
        assert!(scene.sectors.iter().all(|sector| sector.pages.len() == 64));
        let wide_layout = VolumeLayout::for_surface(Surface::new(120, 36)).unwrap();
        assert_eq!(session.scene(wide_layout).unwrap().sectors.len(), 18);
        assert!(
            !session
                .advance(VolumeAction::PreviousVolume, layout)
                .changed
        );
        let next_volume = session.advance(VolumeAction::NextVolume, layout);
        assert_eq!(next_volume.state.volume_id.get(), 1);
        assert_eq!(next_volume.state.focused_sector, 0);
        assert_eq!(next_volume.state.top_sector, 0);
        let previous_volume = session.advance(VolumeAction::PreviousVolume, layout);
        assert_eq!(previous_volume.state.volume_id.get(), 0);
    }

    #[test]
    fn every_sector_card_retains_physical_page_order_and_distinct_facts() {
        let card = synthetic_card(3);
        assert_eq!(card.pages[0].page_id, 192);
        assert_eq!(card.pages[63].page_id, 255);
        assert_eq!(card.pages[0].allocation, AllocationMark::Allocated);
        assert_eq!(
            card.pages[0].occupancy.volume_mark(),
            OccupancyMark::Level(1)
        );
        assert_eq!(card.pages[8].occupancy.volume_mark(), OccupancyMark::Zero);
        assert_eq!(
            card.pages[9].occupancy.volume_mark(),
            OccupancyMark::Unknown
        );
        assert_eq!(card.pages[10].allocation, AllocationMark::System);
        assert_eq!(
            card.pages[10].occupancy.volume_mark(),
            OccupancyMark::NotApplicable
        );
        assert_eq!(card.pages[26].allocation, AllocationMark::Reserved);
        assert_eq!(card.pages[44].allocation, AllocationMark::Unreserved);
        assert!(card.pages[17].finding);
        assert!(matches!(
            card.pages[7].attribution,
            PageAttributionMark::Single {
                kind: PageClaimKind::Allocated,
                file_id: 91,
                ..
            }
        ));
        assert!(card.finding);
        assert_eq!(card.attribution, SectorAttributionMark::Unclaimed);

        let attributed = synthetic_card(0);
        assert!(matches!(
            attributed.attribution,
            SectorAttributionMark::Single {
                vol_id: 0,
                file_id: 64,
                role: Some("heap"),
                class: ClassAttributionMark::Resolved(_),
                full: true,
                allocated_pages: 64,
                reserved_unallocated_pages: 0,
            }
        ));
    }

    #[test]
    fn sector_descent_rover_resize_siblings_and_ascent_preserve_structural_state() {
        let (_directory, view) = fixture_view();
        let compact = Surface::new(60, 20);
        let mut session = FocusedSession::new(view, fixture_policy()).unwrap();
        session
            .advance_focused(FocusedAction::FocusSector(11), compact)
            .unwrap();
        let volume_anchor = session.state();
        assert_eq!(volume_anchor.focused_sector, 11);
        assert_eq!(volume_anchor.top_sector, 9);

        session
            .advance_focused(key_action(StructuralKey::Enter), compact)
            .unwrap();
        assert_eq!(session.focused_state().mode, FocusedMode::Sector);
        assert_eq!(session.sector_scene().unwrap().sector.sector_id, 11);
        assert!(
            session
                .scene(VolumeLayout::for_surface(compact).unwrap())
                .is_ok()
        );

        assert!(
            !session
                .advance_focused(FocusedAction::Left, compact)
                .unwrap()
                .changed
        );
        apply_actions(&mut session, compact, [FocusedAction::Right; 7]);
        assert_eq!(session.focused_state().focused_page, 7);
        assert!(
            !session
                .advance_focused(FocusedAction::Right, compact)
                .unwrap()
                .changed
        );
        apply_actions(&mut session, compact, [FocusedAction::Down; 7]);
        assert_eq!(session.focused_state().focused_page, 63);
        assert!(
            !session
                .advance_focused(FocusedAction::Down, compact)
                .unwrap()
                .changed
        );
        session
            .advance_focused(FocusedAction::Left, compact)
            .unwrap();
        assert_eq!(session.focused_state().focused_page, 62);
        session.advance_focused(FocusedAction::Up, compact).unwrap();
        assert_eq!(session.focused_state().focused_page, 54);

        let before_resize = session.focused_state();
        for (surface, profile) in [
            (Surface::new(120, 36), PresentationProfile::ANSI_UNICODE),
            (Surface::new(80, 24), PresentationProfile::ANSI_UNICODE),
            (Surface::new(60, 20), PresentationProfile::MONO_ASCII),
        ] {
            SectorRenderer::render(&session.sector_scene().unwrap(), surface, profile).unwrap();
            assert_eq!(session.focused_state(), before_resize);
        }

        session
            .advance_focused(key_action(StructuralKey::Backspace), compact)
            .unwrap();
        assert_eq!(session.focused_state().mode, FocusedMode::Volume);
        assert_eq!(session.state(), volume_anchor);
        assert_eq!(
            key_action(StructuralKey::Escape),
            key_action(StructuralKey::Backspace)
        );

        session
            .advance_focused(FocusedAction::Activate, compact)
            .unwrap();
        session
            .advance_focused(FocusedAction::FocusPage(17), compact)
            .unwrap();
        session
            .advance_focused(FocusedAction::NextSector, compact)
            .unwrap();
        assert_eq!(session.focused_state().volume.focused_sector, 12);
        assert_eq!(session.focused_state().focused_page, 17);
        session
            .advance_focused(FocusedAction::PreviousSector, compact)
            .unwrap();
        assert_eq!(session.focused_state().volume.focused_sector, 11);
    }

    #[test]
    fn page_descent_requests_one_bounded_revision_and_adopts_its_distribution() {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(80, 24);
        let base = view.overview();
        let mut session = FocusedSession::new(view, fixture_policy()).unwrap();

        session
            .advance_focused(FocusedAction::Activate, surface)
            .unwrap();
        session
            .advance_focused(FocusedAction::FocusPage(2), surface)
            .unwrap();
        session
            .advance_focused(FocusedAction::Activate, surface)
            .unwrap();

        assert_eq!(session.focused_state().mode, FocusedMode::Page);
        assert_eq!(session.focused_state().page_load, PageLoadState::Loading);
        let loading = session.page_scene().unwrap();
        assert_eq!(loading.revision, base.revision);
        assert_eq!(
            loading.distribution,
            PageDistributionProjection::NotAvailable
        );

        let request = session.take_enrichment_request().unwrap();
        assert_eq!(request.snapshot_id(), base.snapshot_id);
        assert_eq!(request.base_revision(), base.revision);
        assert_eq!(request.page().vol_id.get(), 0);
        assert_eq!(request.page().page_id.get(), 2);
        assert_eq!(request.policy, fixture_policy());
        assert!(session.take_enrichment_request().is_none());
        assert!(
            !session
                .advance_focused(FocusedAction::Activate, surface)
                .unwrap()
                .changed
        );

        let completion = request.execute();
        let adopted = session.complete_enrichment(completion).unwrap();
        assert!(adopted.changed);
        assert_eq!(adopted.state.mode, FocusedMode::Page);
        assert_eq!(adopted.state.page_load, PageLoadState::Ready);
        assert_eq!(adopted.state.volume.revision.get(), base.revision.get() + 1);

        let page = session.page_scene().unwrap();
        assert_eq!(page.revision, adopted.state.volume.revision);
        assert_eq!(page.selected_item, Some(PageDistributionItemId::Header));
        assert_eq!(page.items.len(), 11);
        assert_eq!(
            page.items
                .iter()
                .filter_map(PageDistributionItem::record_oid)
                .collect::<Vec<_>>(),
            vec![
                Oid::new(
                    VolId::new(0).unwrap(),
                    PageId::new(2).unwrap(),
                    SlotId::new(0).unwrap(),
                ),
                Oid::new(
                    VolId::new(0).unwrap(),
                    PageId::new(2).unwrap(),
                    SlotId::new(3).unwrap(),
                ),
            ]
        );

        session
            .advance_focused(FocusedAction::Ascend, surface)
            .unwrap();
        assert_eq!(session.focused_state().mode, FocusedMode::Sector);
        assert_eq!(session.focused_state().focused_page, 2);
    }

    #[test]
    fn page_enrichment_cancellation_and_late_completion_never_adopt() {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(80, 24);
        let base_revision = view.overview().revision;

        let mut cancelled = FocusedSession::new(view.clone(), fixture_policy()).unwrap();
        apply_actions(
            &mut cancelled,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let request = cancelled.take_enrichment_request().unwrap();
        cancelled
            .advance_focused(FocusedAction::Ascend, surface)
            .unwrap();
        let completion = request.execute();
        assert!(matches!(
            completion.result,
            Err(OperationError::Interrupted)
        ));
        assert!(!cancelled.complete_enrichment(completion).unwrap().changed);
        assert_eq!(cancelled.focused_state().mode, FocusedMode::Sector);
        assert_eq!(cancelled.focused_state().volume.revision, base_revision);

        let mut late = FocusedSession::new(view.clone(), fixture_policy()).unwrap();
        apply_actions(
            &mut late,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let completion = late.take_enrichment_request().unwrap().execute();
        late.advance_focused(FocusedAction::Ascend, surface)
            .unwrap();
        assert!(!late.complete_enrichment(completion).unwrap().changed);
        assert_eq!(late.focused_state().volume.revision, base_revision);

        let mut switched = FocusedSession::new(view.clone(), fixture_policy()).unwrap();
        apply_actions(
            &mut switched,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let old_volume = switched.take_enrichment_request().unwrap();
        switched
            .advance_focused(FocusedAction::NextVolume, surface)
            .unwrap();
        assert_eq!(switched.focused_state().mode, FocusedMode::Page);
        assert_eq!(switched.focused_state().volume.volume_id.get(), 1);
        assert!(matches!(
            old_volume.execute().result,
            Err(OperationError::Interrupted)
        ));
        assert!(switched.take_enrichment_request().is_some());

        let mut quit = FocusedSession::new(view, fixture_policy()).unwrap();
        apply_actions(
            &mut quit,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let cancelled_by_quit = quit.take_enrichment_request().unwrap();
        quit.advance_focused(FocusedAction::Quit, surface).unwrap();
        assert!(quit.focused_state().quit_requested);
        assert!(matches!(
            cancelled_by_quit.execute().result,
            Err(OperationError::Interrupted)
        ));
    }

    #[test]
    fn a_sibling_page_replaces_the_request_and_rejects_the_old_result() {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(80, 24);
        let mut session = FocusedSession::new(view, fixture_policy()).unwrap();
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let old = session.take_enrichment_request().unwrap();
        let old_key = old.key;
        let old_completion = old.execute();

        session
            .advance_focused(FocusedAction::NextSector, surface)
            .unwrap();
        let current = session.take_enrichment_request().unwrap();
        assert_ne!(current.key.request_id, old_key.request_id);
        assert_eq!(current.page().page_id.get(), 66);
        assert!(!session.complete_enrichment(old_completion).unwrap().changed);
        assert_eq!(session.focused_state().page_load, PageLoadState::Loading);

        assert!(
            session
                .complete_enrichment(current.execute())
                .unwrap()
                .changed
        );
        assert_eq!(session.focused_state().page_load, PageLoadState::Ready);
        assert_eq!(session.page_scene().unwrap().page.page_id, 66);
    }

    #[test]
    fn page_resource_failure_keeps_the_old_revision_and_invalid_decode_adopts_a_diagnostic() {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(80, 24);
        let base_revision = view.overview().revision;
        let tiny = ResourcePolicy::new(1, 1, 1, 1, 1).unwrap();
        let mut limited = FocusedSession::new(view.clone(), tiny).unwrap();
        apply_actions(
            &mut limited,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let completion = limited.take_enrichment_request().unwrap().execute();
        limited.complete_enrichment(completion).unwrap();
        assert_eq!(limited.focused_state().volume.revision, base_revision);
        assert_eq!(
            limited.focused_state().page_load,
            PageLoadState::Failed(PageEnrichmentFailure::ResourceLimit)
        );

        let mut invalid = FocusedSession::new(view, fixture_policy()).unwrap();
        apply_actions(
            &mut invalid,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(3),
                FocusedAction::Activate,
            ],
        );
        let completion = invalid.take_enrichment_request().unwrap().execute();
        invalid.complete_enrichment(completion).unwrap();
        assert_eq!(
            invalid.focused_state().volume.revision.get(),
            base_revision.get() + 1
        );
        assert_eq!(
            invalid.focused_state().page_load,
            PageLoadState::Unavailable("slotted.header.anchor")
        );
        assert_eq!(
            invalid.page_scene().unwrap().distribution,
            PageDistributionProjection::NotAvailable
        );
    }

    #[test]
    fn every_distribution_row_is_reachable_and_only_live_records_are_interpretation_eligible() {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(60, 20);
        let mut session = FocusedSession::new(view, fixture_policy()).unwrap();
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let completion = session.take_enrichment_request().unwrap().execute();
        session.complete_enrichment(completion).unwrap();

        let expected = session
            .page_scene()
            .unwrap()
            .items
            .iter()
            .map(PageDistributionItem::id)
            .collect::<BTreeSet<_>>();
        let mut reached = BTreeSet::new();
        loop {
            let scene = session.page_scene().unwrap();
            reached.insert(scene.selected_item.unwrap());
            if !session
                .advance_focused(FocusedAction::Down, surface)
                .unwrap()
                .changed
            {
                break;
            }
        }
        assert_eq!(reached, expected);
        assert_eq!(
            session.focused_state().selected_distribution_item,
            Some(PageDistributionItemId::SlotEntry(3))
        );
        assert_ne!(
            session.focused_state().top_distribution_item,
            Some(PageDistributionItemId::Header)
        );

        session
            .advance_focused(
                FocusedAction::FocusDistributionItem(PageDistributionItemId::Record(0)),
                surface,
            )
            .unwrap();
        assert_eq!(
            session
                .page_scene()
                .unwrap()
                .selected_record()
                .unwrap()
                .slot_id
                .get(),
            0
        );
        session
            .advance_focused(
                FocusedAction::FocusDistributionItem(PageDistributionItemId::SlotEntry(2)),
                surface,
            )
            .unwrap();
        assert!(session.page_scene().unwrap().selected_record().is_none());

        let slots = session
            .page_scene()
            .unwrap()
            .items
            .into_iter()
            .filter_map(|item| match item {
                PageDistributionItem::SlotEntry {
                    slot_id,
                    region,
                    state,
                    record_type,
                } => Some((
                    slot_id,
                    region.offset,
                    region.length,
                    state.as_str(),
                    record_type,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            slots,
            vec![
                (0, 16_340, 4, "allocated", "home"),
                (1, 16_336, 4, "unallocated", "reserved"),
                (2, 16_332, 4, "deleted", "marked-deleted"),
                (3, 16_328, 4, "allocated", "new-home"),
            ]
        );
    }

    #[test]
    fn enter_on_a_live_record_loads_page_local_interpretation_and_escape_closes_it() {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(80, 24);
        let mut session = FocusedSession::new(view, fixture_policy()).unwrap();
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let page = session.take_enrichment_request().unwrap().execute();
        session.complete_enrichment(page).unwrap();
        let structural_revision = session.focused_state().volume.revision;
        session
            .advance_focused(
                FocusedAction::FocusDistributionItem(PageDistributionItemId::Record(3)),
                surface,
            )
            .unwrap();

        session
            .advance_focused(FocusedAction::Activate, surface)
            .unwrap();
        let selected = Oid::new(
            VolId::new(0).unwrap(),
            PageId::new(2).unwrap(),
            SlotId::new(3).unwrap(),
        );
        assert_eq!(
            session.focused_state().interpretation,
            PageInterpretationState::Record {
                record: selected,
                load: InterpretationLoadState::Loading,
                top_attribute: 0,
            }
        );
        assert_eq!(session.page_scene().unwrap().revision, structural_revision);
        let request = session.take_enrichment_request().unwrap();
        assert_eq!(request.target(), EnrichmentRequestTarget::Record(selected));
        assert!(session.take_enrichment_request().is_none());

        let completion = request.execute();
        session.complete_enrichment(completion).unwrap();
        assert_eq!(
            session.focused_state().interpretation,
            PageInterpretationState::Record {
                record: selected,
                load: InterpretationLoadState::Unavailable(
                    "heap page slot 0 is not a recognized heap record"
                ),
                top_attribute: 0,
            }
        );
        assert_eq!(
            session.focused_state().volume.revision.get(),
            structural_revision.get() + 1
        );

        let selected_item = session.focused_state().selected_distribution_item;
        let top_item = session.focused_state().top_distribution_item;
        session
            .advance_focused(FocusedAction::Ascend, surface)
            .unwrap();
        assert_eq!(
            session.focused_state().interpretation,
            PageInterpretationState::Closed
        );
        assert_eq!(
            session.focused_state().selected_distribution_item,
            selected_item
        );
        assert_eq!(session.focused_state().top_distribution_item, top_item);
        assert_eq!(session.focused_state().mode, FocusedMode::Page);

        let durable_revision = session.focused_state().volume.revision;
        session
            .advance_focused(FocusedAction::Activate, surface)
            .unwrap();
        assert!(matches!(
            session.focused_state().interpretation,
            PageInterpretationState::Record {
                load: InterpretationLoadState::Unavailable(
                    "heap page slot 0 is not a recognized heap record"
                ),
                ..
            }
        ));
        assert!(session.take_enrichment_request().is_none());
        assert_eq!(session.focused_state().volume.revision, durable_revision);
    }

    fn assert_ready_scalar_interpretation(scene: &PageScene) {
        assert!(matches!(
            scene.interpretation_state,
            PageInterpretationState::Record {
                load: InterpretationLoadState::Ready,
                ..
            }
        ));
        let selection = scene.record_selection.as_ref().unwrap();
        assert_eq!(selection.selected_slot.record_type, "home");
        let interpretation = selection.interpretation.as_ref().unwrap();
        assert_eq!(
            (
                interpretation.record.vol_id,
                interpretation.record.page_id,
                interpretation.record.slot_id,
                interpretation.representation_id,
            ),
            (1, 641, 1, 1)
        );
        assert!(interpretation.layout.is_some());
        assert!(
            interpretation
                .attributes
                .windows(2)
                .all(|attributes| attributes[0].position < attributes[1].position)
        );
        let id = interpretation
            .attributes
            .iter()
            .find(|attribute| {
                matches!(
                    &attribute.name,
                    AttributeNameProjection::Resolved { value } if value == "id"
                )
            })
            .unwrap();
        assert!(matches!(
            &id.value,
            AttributeValueProjection::Decoded { value } if value == "1"
        ));
        assert!(matches!(
            selection.class_representation.as_ref().unwrap().class_name,
            ClassNameProjection::Resolved { ref value } if value == "dba.interp_scalars"
        ));
        let projected = serde_json::to_string(selection).unwrap();
        for forbidden in ["\"hex\"", "\"raw\"", "\"bytes\":[", "0x"] {
            assert!(
                !projected.contains(forbidden),
                "selection leaked {forbidden}"
            );
        }
    }

    #[test]
    fn explicit_record_action_projects_typed_values_and_schema_into_the_page_scene() {
        let (_directory, view) = interpretation_fixture_view();
        let surface = Surface::new(80, 24);
        let mut session = FocusedSession::new(view, interpretation_fixture_policy()).unwrap();
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::NextVolume,
                FocusedAction::FocusSector(10),
                FocusedAction::Activate,
                FocusedAction::FocusPage(1),
                FocusedAction::Activate,
            ],
        );
        let page = session.take_enrichment_request().unwrap().execute();
        session.complete_enrichment(page).unwrap();
        session
            .advance_focused(
                FocusedAction::FocusDistributionItem(PageDistributionItemId::Record(1)),
                surface,
            )
            .unwrap();

        assert!(session.page_scene().unwrap().record_selection.is_none());
        session
            .advance_focused(FocusedAction::Activate, surface)
            .unwrap();
        assert!(session.page_scene().unwrap().record_selection.is_none());
        let request = session.take_enrichment_request().unwrap();
        assert!(
            matches!(request.target(), EnrichmentRequestTarget::Record(record)
            if record.vol_id.get() == 1 && record.page_id.get() == 641 && record.slot_id.get() == 1)
        );
        session.complete_enrichment(request.execute()).unwrap();

        assert_ready_scalar_interpretation(&session.page_scene().unwrap());

        session
            .advance_focused(FocusedAction::Ascend, surface)
            .unwrap();
        session
            .advance_focused(
                FocusedAction::FocusDistributionItem(PageDistributionItemId::Record(2)),
                surface,
            )
            .unwrap();
        assert!(session.page_scene().unwrap().record_selection.is_none());
        session
            .advance_focused(FocusedAction::Activate, surface)
            .unwrap();
        assert!(session.take_enrichment_request().is_none());
        let unset = session.page_scene().unwrap();
        let unset = unset.record_selection.as_ref().unwrap();
        assert!(
            unset
                .interpretation
                .as_ref()
                .unwrap()
                .attributes
                .iter()
                .any(|attribute| matches!(
                    attribute.value,
                    crate::projection::AttributeValueProjection::Null
                ))
        );
    }

    #[test]
    fn interpretation_navigation_scrolls_attributes_without_losing_the_record_anchor() {
        let (_directory, view) = interpretation_fixture_view();
        let surface = Surface::new(60, 20);
        let mut session = FocusedSession::new(view, interpretation_fixture_policy()).unwrap();
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::NextVolume,
                FocusedAction::FocusSector(10),
                FocusedAction::Activate,
                FocusedAction::FocusPage(1),
                FocusedAction::Activate,
            ],
        );
        let page = session.take_enrichment_request().unwrap().execute();
        session.complete_enrichment(page).unwrap();
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::FocusDistributionItem(PageDistributionItemId::Record(1)),
                FocusedAction::Activate,
            ],
        );
        let record = session.take_enrichment_request().unwrap().execute();
        session.complete_enrichment(record).unwrap();
        let selected = session.focused_state().selected_distribution_item;
        let top = session.focused_state().top_distribution_item;

        session
            .advance_focused(FocusedAction::Down, surface)
            .unwrap();
        assert!(matches!(
            session.focused_state().interpretation,
            PageInterpretationState::Record {
                top_attribute: 1,
                ..
            }
        ));
        session
            .advance_focused(FocusedAction::ScrollRows(10_000), surface)
            .unwrap();
        let PageInterpretationState::Record { top_attribute, .. } =
            session.focused_state().interpretation
        else {
            panic!("interpretation unexpectedly closed");
        };
        assert!(top_attribute > 1);
        session.advance_focused(FocusedAction::Up, surface).unwrap();
        assert!(matches!(
            session.focused_state().interpretation,
            PageInterpretationState::Record {
                top_attribute: current,
                ..
            } if current + 1 == top_attribute
        ));
        assert_eq!(session.focused_state().selected_distribution_item, selected);
        assert_eq!(session.focused_state().top_distribution_item, top);
    }

    #[test]
    fn interpretation_renderer_shows_typed_facts_layout_and_bounded_attributes() {
        let scene = ready_interpretation_scene();
        for (surface, profile) in [
            (Surface::new(120, 36), PresentationProfile::ANSI_UNICODE),
            (Surface::new(80, 24), PresentationProfile::ANSI_UNICODE),
            (Surface::new(60, 20), PresentationProfile::MONO_ASCII),
        ] {
            let frame = PageRenderer::render(&scene, surface, profile).unwrap();
            let snapshot = frame.semantic_snapshot();
            let screen = (0..surface.height)
                .map(|row| frame.line(row))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(snapshot.contains("Record 1|641|1"));
            assert!(snapshot.contains("type home"));
            assert!(snapshot.contains("class/table dba.interp_scalars"));
            assert!(snapshot.contains("representation 1"));
            assert!(snapshot.contains("object-header"));
            assert!(snapshot.contains("id"));
            assert!(snapshot.contains("decoded 1"));
            assert!(snapshot.contains("Esc/Backspace close interpretation"));
            for forbidden in ["raw bytes", "hex", "0x"] {
                assert!(
                    !screen.contains(forbidden),
                    "interpretation frame leaked {forbidden}:\n{screen}"
                );
            }
            if profile.glyphs == GlyphProfile::Ascii {
                assert!((0..surface.height).all(|row| frame.line(row).is_ascii()));
            }
        }
    }

    #[test]
    fn interpretation_renderer_names_one_hop_relocation_origin_and_target() {
        let mut scene = ready_interpretation_scene();
        let source = scene.selected_record().unwrap();
        scene.interpretation_state = PageInterpretationState::Record {
            record: source,
            load: InterpretationLoadState::Ready,
            top_attribute: 0,
        };
        let selection = scene.record_selection.as_mut().unwrap();
        selection.selected_slot.record_type = "relocation";
        let interpretation = selection.interpretation.as_mut().unwrap();
        interpretation.record = OidProjection {
            vol_id: 1,
            page_id: 642,
            slot_id: 3,
        };
        interpretation.relocated_from = OptionalOidProjection::Present {
            oid: OidProjection {
                vol_id: source.vol_id.get(),
                page_id: source.page_id.get(),
                slot_id: source.slot_id.get(),
            },
        };
        let frame = PageRenderer::render(
            &scene,
            Surface::new(120, 36),
            PresentationProfile::ANSI_UNICODE,
        )
        .unwrap();
        assert!(frame.line(3).contains("type relocation"));
        assert!(frame.line(4).contains("record 1|642|3"));
        assert!(frame.line(4).contains("relocated from 1|641|1"));
    }

    #[test]
    fn interpretation_renderer_preserves_typed_edge_record_and_attribute_limitations() {
        let surface = Surface::new(120, 36);
        let mut malformed = ready_interpretation_scene();
        let selected = malformed.selected_record().unwrap();
        malformed.interpretation_state = PageInterpretationState::Record {
            record: selected,
            load: InterpretationLoadState::Unavailable("heap.relocation.target_slot_role"),
            top_attribute: 0,
        };
        let selection = malformed.record_selection.as_mut().unwrap();
        selection.selected_slot.record_type = "relocation";
        selection.interpretation = None;
        selection.interpretation_unavailable = Some("heap.relocation.target_slot_role");
        let frame =
            PageRenderer::render(&malformed, surface, PresentationProfile::ANSI_UNICODE).unwrap();
        assert!(frame.line(4).contains("heap.relocation.target_slot_role"));
        assert!(!frame.semantic_snapshot().contains("decoded 1"));

        let mut diagnostic = ready_interpretation_scene();
        let selected = diagnostic.selected_record().unwrap();
        diagnostic.interpretation_state = PageInterpretationState::Record {
            record: selected,
            load: InterpretationLoadState::Unavailable("record.layout.invalid"),
            top_attribute: 0,
        };
        diagnostic
            .record_selection
            .as_mut()
            .unwrap()
            .interpretation
            .as_mut()
            .unwrap()
            .diagnostic = OptionalTextProjection::Known("record.layout.invalid");
        let frame =
            PageRenderer::render(&diagnostic, surface, PresentationProfile::ANSI_UNICODE).unwrap();
        assert!(frame.line(4).contains("record.layout.invalid"));
        assert!(!frame.semantic_snapshot().contains("decoded 1"));

        let mut partial = ready_interpretation_scene();
        partial
            .record_selection
            .as_mut()
            .unwrap()
            .interpretation
            .as_mut()
            .unwrap()
            .attributes[0]
            .value = AttributeValueProjection::Withheld {
            reason: "attribute.decode.unsupported",
            offset: 20,
            length: 4,
        };
        let frame =
            PageRenderer::render(&partial, surface, PresentationProfile::ANSI_UNICODE).unwrap();
        assert!(
            frame
                .line(8)
                .contains("withheld attribute.decode.unsupported @20+4")
        );
    }

    #[test]
    fn unsupported_and_non_record_rows_open_no_enrichment_work() {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(80, 24);
        let mut session = FocusedSession::new(view, fixture_policy()).unwrap();
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let page = session.take_enrichment_request().unwrap().execute();
        session.complete_enrichment(page).unwrap();
        let structural_revision = session.focused_state().volume.revision;

        session
            .advance_focused(
                FocusedAction::FocusDistributionItem(PageDistributionItemId::Header),
                surface,
            )
            .unwrap();
        assert!(
            !session
                .advance_focused(FocusedAction::Activate, surface)
                .unwrap()
                .changed
        );
        assert!(session.take_enrichment_request().is_none());

        session
            .advance_focused(
                FocusedAction::FocusDistributionItem(PageDistributionItemId::Record(0)),
                surface,
            )
            .unwrap();
        session
            .advance_focused(FocusedAction::Activate, surface)
            .unwrap();
        assert!(matches!(
            session.focused_state().interpretation,
            PageInterpretationState::Record {
                load: InterpretationLoadState::Unavailable(
                    "slot 0 holds heap Page metadata, not a class instance"
                ),
                ..
            }
        ));
        assert!(session.take_enrichment_request().is_none());
        assert_eq!(session.focused_state().volume.revision, structural_revision);
    }

    #[test]
    fn bigone_record_opens_a_typed_limitation_without_worker_or_revision() {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(80, 24);
        let mut session = FocusedSession::new(view, fixture_policy()).unwrap();
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::FocusSector(1),
                FocusedAction::Activate,
                FocusedAction::FocusPage(3),
                FocusedAction::Activate,
            ],
        );
        let page = session.take_enrichment_request().unwrap().execute();
        session.complete_enrichment(page).unwrap();
        let structural_revision = session.focused_state().volume.revision;
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::FocusDistributionItem(PageDistributionItemId::Record(3)),
                FocusedAction::Activate,
            ],
        );

        assert!(matches!(
            session.focused_state().interpretation,
            PageInterpretationState::Record {
                load: InterpretationLoadState::Unavailable(
                    "REC_BIGONE carries an overflow reference, not an inline class instance"
                ),
                ..
            }
        ));
        assert!(session.take_enrichment_request().is_none());
        assert_eq!(session.focused_state().volume.revision, structural_revision);
        let frame = PageRenderer::render(
            &session.page_scene().unwrap(),
            surface,
            PresentationProfile::ANSI_UNICODE,
        )
        .unwrap();
        assert!(frame.semantic_snapshot().contains("REC_BIGONE"));
    }

    #[test]
    fn record_interpretation_cancel_and_stale_completion_never_adopt() {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(80, 24);
        let mut session = FocusedSession::new(view.clone(), fixture_policy()).unwrap();
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let page = session.take_enrichment_request().unwrap().execute();
        session.complete_enrichment(page).unwrap();
        let structural_revision = session.focused_state().volume.revision;
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::FocusDistributionItem(PageDistributionItemId::Record(3)),
                FocusedAction::Activate,
            ],
        );
        let cancelled = session.take_enrichment_request().unwrap();
        session
            .advance_focused(FocusedAction::Ascend, surface)
            .unwrap();
        assert_eq!(session.focused_state().mode, FocusedMode::Page);
        assert_eq!(
            session.focused_state().interpretation,
            PageInterpretationState::Closed
        );
        let completion = cancelled.execute();
        assert!(matches!(
            completion.result,
            Err(OperationError::Interrupted)
        ));
        assert!(!session.complete_enrichment(completion).unwrap().changed);
        assert_eq!(session.focused_state().volume.revision, structural_revision);

        let mut late = FocusedSession::new(view.clone(), fixture_policy()).unwrap();
        apply_actions(
            &mut late,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let page = late.take_enrichment_request().unwrap().execute();
        late.complete_enrichment(page).unwrap();
        apply_actions(
            &mut late,
            surface,
            [
                FocusedAction::FocusDistributionItem(PageDistributionItemId::Record(3)),
                FocusedAction::Activate,
            ],
        );
        let completion = late.take_enrichment_request().unwrap().execute();
        late.advance_focused(FocusedAction::Ascend, surface)
            .unwrap();
        assert!(!late.complete_enrichment(completion).unwrap().changed);
        assert_eq!(late.focused_state().volume.revision, structural_revision);

        let mut sibling = FocusedSession::new(view, fixture_policy()).unwrap();
        apply_actions(
            &mut sibling,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let page = sibling.take_enrichment_request().unwrap().execute();
        sibling.complete_enrichment(page).unwrap();
        apply_actions(
            &mut sibling,
            surface,
            [
                FocusedAction::FocusDistributionItem(PageDistributionItemId::Record(3)),
                FocusedAction::Activate,
            ],
        );
        let cancelled = sibling.take_enrichment_request().unwrap();
        sibling
            .advance_focused(FocusedAction::NextSector, surface)
            .unwrap();
        assert!(matches!(
            cancelled.execute().result,
            Err(OperationError::Interrupted)
        ));
        assert_eq!(
            sibling.focused_state().interpretation,
            PageInterpretationState::Closed
        );
    }

    #[test]
    fn quit_cancels_active_record_interpretation_before_exiting() {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(80, 24);
        let (mut session, request, revision) = pending_synthetic_record_request(view, surface);

        session
            .advance_focused(FocusedAction::Quit, surface)
            .unwrap();

        assert!(session.focused_state().quit_requested);
        assert_eq!(
            session.focused_state().interpretation,
            PageInterpretationState::Closed
        );
        assert_eq!(session.focused_state().volume.revision, revision);
        assert!(matches!(
            request.execute().result,
            Err(OperationError::Interrupted)
        ));
    }

    #[test]
    fn page_renderer_draws_proportional_geometry_and_formats_only_a_bounded_row_window() {
        let scene = ready_page_scene();

        for (surface, profile) in [
            (Surface::new(120, 36), PresentationProfile::ANSI_UNICODE),
            (Surface::new(80, 24), PresentationProfile::ANSI_UNICODE),
            (Surface::new(60, 20), PresentationProfile::MONO_ASCII),
        ] {
            let frame = PageRenderer::render(&scene, surface, profile).unwrap();
            let visible = usize::from(page_visible_rows(surface));
            assert_eq!(
                frame.distribution_hits.len(),
                scene.items.len().min(visible)
            );
            assert!(frame.formatted_distribution_rows <= scene.items.len().min(visible * 3));
            assert!(frame.line(1).contains("Volume 0 > Sector 0 > Page 2"));
            assert!(frame.line(3).contains("16,344 B"));
            let byte_map = frame.line(5);
            assert!(byte_map.starts_with('R'));
            assert_eq!(byte_map.chars().nth(1), Some('.'));
            assert!(byte_map.ends_with('D'));
            let rows = (PAGE_ROWS_TOP..surface.height - PAGE_RESERVED_BOTTOM_ROWS)
                .map(|row| frame.line(row))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(rows.contains("header [0,32) 32 B"));
            assert!(rows.contains("record slot 0 home [32,56) 24 B"));
            assert!(!frame.semantic_snapshot().contains('\u{1b}'));
            if profile.glyphs == GlyphProfile::Ascii {
                assert!((0..surface.height).all(|row| frame.line(row).is_ascii()));
            }
        }
    }

    #[test]
    fn page_renderer_keeps_the_old_revision_visible_for_loading_failure_and_unsupported_pages() {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(80, 24);
        let base_revision = view.overview().revision;
        let mut session =
            FocusedSession::new(view.clone(), ResourcePolicy::new(1, 1, 1, 1, 1).unwrap()).unwrap();
        apply_actions(
            &mut session,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(2),
                FocusedAction::Activate,
            ],
        );
        let loading = PageRenderer::render(
            &session.page_scene().unwrap(),
            surface,
            PresentationProfile::ANSI_UNICODE,
        )
        .unwrap();
        assert!(
            loading
                .line(0)
                .contains(&format!("r{}", base_revision.get()))
        );
        assert!(loading.line(3).contains("loading structure"));
        let completion = session.take_enrichment_request().unwrap().execute();
        session.complete_enrichment(completion).unwrap();
        let failed = PageRenderer::render(
            &session.page_scene().unwrap(),
            surface,
            PresentationProfile::ANSI_UNICODE,
        )
        .unwrap();
        assert!(
            failed
                .line(0)
                .contains(&format!("r{}", base_revision.get()))
        );
        assert!(failed.line(3).contains("resource limit"));

        let mut unsupported = FocusedSession::new(view, fixture_policy()).unwrap();
        apply_actions(
            &mut unsupported,
            surface,
            [
                FocusedAction::Activate,
                FocusedAction::FocusPage(4),
                FocusedAction::Activate,
            ],
        );
        assert!(unsupported.take_enrichment_request().is_none());
        assert_eq!(
            unsupported.focused_state().page_load,
            PageLoadState::Unavailable("Page type has no slotted record distribution")
        );
        let frame = PageRenderer::render(
            &unsupported.page_scene().unwrap(),
            surface,
            PresentationProfile::ANSI_UNICODE,
        )
        .unwrap();
        assert!(frame.line(3).contains("no slotted record distribution"));
    }

    #[test]
    fn page_pointer_selection_and_scroll_use_the_same_semantic_actions_as_keys() {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(60, 20);
        let mut keyboard = FocusedSession::new(view.clone(), fixture_policy()).unwrap();
        let mut pointer = FocusedSession::new(view, fixture_policy()).unwrap();
        for session in [&mut keyboard, &mut pointer] {
            apply_actions(
                session,
                surface,
                [
                    FocusedAction::Activate,
                    FocusedAction::FocusPage(2),
                    FocusedAction::Activate,
                ],
            );
            let completion = session.take_enrichment_request().unwrap().execute();
            session.complete_enrichment(completion).unwrap();
        }

        keyboard
            .advance_focused(key_action(StructuralKey::Down), surface)
            .unwrap();
        apply_actions(
            &mut pointer,
            surface,
            pointer_actions(
                FocusedMode::Page,
                PointerInput::FocusDistributionItem(PageDistributionItemId::Record(0)),
            ),
        );
        assert_eq!(pointer.focused_state(), keyboard.focused_state());

        keyboard
            .advance_focused(FocusedAction::ScrollRows(1), surface)
            .unwrap();
        apply_actions(
            &mut pointer,
            surface,
            pointer_actions(FocusedMode::Page, PointerInput::WheelRows(1)),
        );
        assert_eq!(pointer.focused_state(), keyboard.focused_state());
    }

    #[test]
    fn maximum_slot_directory_retains_every_row_but_formats_only_the_compact_window() {
        let slot_count = (crate::format::DB_PAGE_SIZE - crate::format::SLOTTED_HEADER_SIZE)
            / crate::format::SLOTTED_SLOT_SIZE;
        assert_eq!(slot_count, 4_078);
        let slot_directory_length =
            u32::try_from(slot_count * crate::format::SLOTTED_SLOT_SIZE).unwrap();
        let slot_directory_offset =
            u32::try_from(crate::format::DB_PAGE_SIZE).unwrap() - slot_directory_length;
        let distribution = PageDistributionProjection::Available {
            content_size: u32::try_from(crate::format::DB_PAGE_SIZE).unwrap(),
            header: ByteRegionProjection {
                offset: 0,
                length: u32::try_from(crate::format::SLOTTED_HEADER_SIZE).unwrap(),
            },
            record_extents: Vec::new(),
            free_regions: Vec::new(),
            slot_directory: ByteRegionProjection {
                offset: slot_directory_offset,
                length: slot_directory_length,
            },
            slot_entries: (0..slot_count)
                .map(|slot| {
                    let slot_id = u16::try_from(slot).unwrap();
                    SlotEntryProjection {
                        slot_id,
                        offset: u32::try_from(crate::format::DB_PAGE_SIZE).unwrap()
                            - (u32::from(slot_id) + 1)
                                * u32::try_from(crate::format::SLOTTED_SLOT_SIZE).unwrap(),
                        length: u32::try_from(crate::format::SLOTTED_SLOT_SIZE).unwrap(),
                        state: SlotEntryStateProjection::Unallocated,
                        record_type: "reserved",
                    }
                })
                .collect(),
            allocated_record_bytes: 0,
            unoccupied_bytes: 0,
        };
        let vpid = Vpid::new(VolId::new(0).unwrap(), PageId::new(2).unwrap());
        let items = PageDistributionItem::from_projection(vpid, &distribution).unwrap();
        assert_eq!(items.len(), slot_count + 2);
        assert_eq!(
            items
                .iter()
                .map(PageDistributionItem::id)
                .collect::<BTreeSet<_>>()
                .len(),
            items.len()
        );
        assert_eq!(
            items.last().map(PageDistributionItem::id),
            Some(PageDistributionItemId::SlotEntry(4_077))
        );

        let surface = Surface::new(60, 20);
        let visible = usize::from(page_visible_rows(surface));
        let top = items[items.len() - visible].id();
        let scene = PageScene {
            snapshot_id: SnapshotId::from_bytes([0xAB; 16]),
            revision: InspectionRevision::new(7),
            outcome: "success-limited",
            volume: synthetic_sector_scene(0).volume,
            volume_index: 0,
            volume_count: 1,
            sector_id: 0,
            page: PageMark::try_from_projection(&page(
                2,
                "allocated",
                PageOccupancyProjection::Unknown,
                false,
            ))
            .unwrap(),
            load: PageLoadState::Ready,
            distribution,
            selected_item: Some(PageDistributionItemId::SlotEntry(4_077)),
            top_item: Some(top),
            items,
            interpretation_state: PageInterpretationState::Closed,
            record_selection: None,
        };
        let frame = PageRenderer::render(&scene, surface, PresentationProfile::MONO_ASCII).unwrap();
        assert_eq!(frame.distribution_hits.len(), visible);
        assert_eq!(
            frame.distribution_hits.last().map(|hit| hit.item),
            Some(PageDistributionItemId::SlotEntry(4_077))
        );
        assert!(frame.formatted_distribution_rows <= visible * 3);
    }

    #[test]
    #[ignore = "manual Page-renderer preview"]
    fn print_page_mode_preview() {
        let scene = ready_page_scene();
        for (surface, profile) in [
            (Surface::new(120, 36), PresentationProfile::ANSI_UNICODE),
            (Surface::new(80, 24), PresentationProfile::ANSI_UNICODE),
            (Surface::new(60, 20), PresentationProfile::MONO_ASCII),
        ] {
            println!(
                "\n{}",
                PageRenderer::render(&scene, surface, profile)
                    .unwrap()
                    .semantic_snapshot()
            );
        }
    }

    #[test]
    #[ignore = "manual interpretation-renderer preview"]
    fn print_interpretation_mode_preview() {
        let scene = ready_interpretation_scene();
        for (surface, profile) in [
            (Surface::new(120, 36), PresentationProfile::ANSI_UNICODE),
            (Surface::new(80, 24), PresentationProfile::ANSI_UNICODE),
            (Surface::new(60, 20), PresentationProfile::MONO_ASCII),
        ] {
            println!(
                "\n{}",
                PageRenderer::render(&scene, surface, profile)
                    .unwrap()
                    .semantic_snapshot()
            );
        }
    }

    #[test]
    fn mouse_and_wheel_translate_to_the_same_semantic_actions_as_keyboard() {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(60, 20);
        let mut keyboard = FocusedSession::new(view.clone(), fixture_policy()).unwrap();
        apply_actions(
            &mut keyboard,
            surface,
            std::iter::repeat_n(key_action(StructuralKey::NextSector), 11)
                .chain([key_action(StructuralKey::Enter)]),
        );

        let mut pointer = FocusedSession::new(view, fixture_policy()).unwrap();
        apply_actions(
            &mut pointer,
            surface,
            pointer_actions(FocusedMode::Volume, PointerInput::ActivateSector(11)),
        );
        assert_eq!(pointer.focused_state(), keyboard.focused_state());

        apply_actions(
            &mut keyboard,
            surface,
            [
                key_action(StructuralKey::Right),
                key_action(StructuralKey::Down),
                key_action(StructuralKey::Down),
                key_action(StructuralKey::Enter),
            ],
        );
        apply_actions(
            &mut pointer,
            surface,
            pointer_actions(FocusedMode::Sector, PointerInput::ActivatePage(17)),
        );
        assert_eq!(pointer.focused_state(), keyboard.focused_state());

        apply_actions(
            &mut pointer,
            surface,
            pointer_actions(FocusedMode::Sector, PointerInput::WheelRows(1)),
        );
        keyboard
            .advance_focused(key_action(StructuralKey::NextSector), surface)
            .unwrap();
        assert_eq!(pointer.focused_state(), keyboard.focused_state());
        assert_eq!(
            pointer_actions(FocusedMode::Sector, PointerInput::WheelRows(-1)),
            vec![key_action(StructuralKey::PreviousSector)]
        );
    }

    #[test]
    fn sector_renderer_keeps_all_pages_exact_and_exposes_compact_context() {
        let scene = synthetic_sector_scene(7);
        for (surface, profile) in [
            (Surface::new(120, 36), PresentationProfile::ANSI_UNICODE),
            (Surface::new(80, 24), PresentationProfile::ANSI_UNICODE),
            (Surface::new(60, 20), PresentationProfile::MONO_ASCII),
        ] {
            let frame = SectorRenderer::render(&scene, surface, profile).unwrap();
            assert_eq!(frame.page_hits.len(), 64);
            assert_eq!(
                frame
                    .page_hits
                    .iter()
                    .map(|hit| hit.page_id)
                    .collect::<BTreeSet<_>>(),
                (192_i32..256).collect()
            );
            for (index, left) in frame.page_hits.iter().enumerate() {
                assert!(left.right < surface.width);
                assert!(left.bottom < surface.height);
                assert!(frame.page_hits[index + 1..].iter().all(|right| {
                    left.right < right.left
                        || right.right < left.left
                        || left.bottom < right.top
                        || right.bottom < left.top
                }));
            }
            let grid = (SECTOR_GRID_TOP..SECTOR_GRID_TOP + SECTOR_GRID_ROWS)
                .map(|row| frame.line(row))
                .collect::<Vec<_>>()
                .join("\n");
            if surface.width >= 80 {
                assert!(grid.contains("00   1%HP"));
                assert!(grid.contains("07 100%HP"));
                assert!(grid.contains("08   0%HP"));
                assert!(grid.contains("09    ?HP"));
                assert!(grid.contains("10    -HP"));
                assert_eq!(grid.matches("HP").count(), 64);
            } else {
                assert!(grid.contains("00   1"));
                assert!(grid.contains("07 100"));
                assert!(grid.contains("08   0"));
                assert!(grid.contains("09   ?"));
                assert!(grid.contains("10   -"));
                assert!(!grid.contains("HP"));
            }
            let detail = (13..=16)
                .map(|row| frame.line(row))
                .collect::<Vec<_>>()
                .join(" ");
            assert!(detail.contains("Page 199 (cell 07)"));
            assert!(detail.contains("type heap"));
            assert!(detail.contains("allocation allocated"));
            assert!(detail.contains("occupied 100%"));
            assert!(detail.contains("free 0%"));
            assert!(detail.contains("finding page.test"));
            assert!(detail.contains("file 0:91 (heap)"));
            assert!(detail.contains("class 0:777:3"));
            assert!(detail.contains("table orders"));
            assert!(!frame.semantic_snapshot().contains('\u{1b}'));
            if profile.glyphs == GlyphProfile::Ascii {
                assert!((0..surface.height).all(|row| frame.line(row).is_ascii()));
            }
        }
    }

    #[test]
    fn volume_buckets_and_sector_percentages_share_one_exact_occupancy_fact() {
        let card = synthetic_card(3);
        let expected = [
            (0, "1%", OccupancyMark::Level(1)),
            (1, "13%", OccupancyMark::Level(2)),
            (2, "26%", OccupancyMark::Level(3)),
            (3, "38%", OccupancyMark::Level(4)),
            (4, "51%", OccupancyMark::Level(5)),
            (5, "63%", OccupancyMark::Level(6)),
            (6, "76%", OccupancyMark::Level(7)),
            (7, "100%", OccupancyMark::Level(8)),
            (8, "0%", OccupancyMark::Zero),
            (9, "?", OccupancyMark::Unknown),
            (10, "-", OccupancyMark::NotApplicable),
        ];
        for (page, label, mark) in expected {
            assert_eq!(card.pages[page].occupancy.occupied_label(), label);
            assert_eq!(card.pages[page].occupancy.volume_mark(), mark);
        }
        let seven = ExactOccupancy::from_projection(
            AllocationMark::Allocated,
            &PageOccupancyProjection::Known {
                occupied_percent: 7,
                free_percent: 93,
            },
        );
        assert_eq!(seven.occupied_label(), "7%");
        assert_eq!(seven.descriptor(), "occupied 7% / free 93%");
        assert_eq!(seven.volume_mark(), OccupancyMark::Level(1));
    }

    #[test]
    fn terminal_text_is_control_safe_grapheme_safe_and_display_width_bounded() {
        let clusters = fitted_clusters("한글e\u{301}\u{1b}[31m", 8, GlyphProfile::Unicode);
        let text = clusters
            .iter()
            .map(|cluster| cluster.text.as_str())
            .collect::<String>();
        assert!(!text.contains('\u{1b}'));
        assert!(text.contains("e\u{301}"));
        assert!(clusters.iter().map(|cluster| cluster.width).sum::<usize>() <= 8);

        let mut frame = VolumeFrame::new(Surface::new(60, 20), PresentationProfile::ANSI_UNICODE);
        frame.put_text(0, 0, 5, "界界界", SemanticStyle::Plain);
        assert_eq!(UnicodeWidthStr::width(frame.line(0).as_str()), 5);
        assert!(frame.line(0).ends_with('…'));
    }

    #[test]
    fn renderer_exposes_complete_visible_cards_and_semantic_hit_regions() {
        let surface = Surface::new(60, 20);
        let scene = synthetic_scene(surface);
        let frame =
            VolumeRenderer::render(&scene, surface, PresentationProfile::ANSI_UNICODE).unwrap();
        assert_eq!(frame.hits.len(), 3);
        assert_eq!(frame.hits[0].sector_id, 0);
        assert_eq!(frame.hits[2].sector_id, 2);
        for (index, left) in frame.hits.iter().enumerate() {
            assert!(left.right < surface.width);
            assert!(left.bottom < surface.height);
            assert!(frame.hits[index + 1..].iter().all(|right| {
                left.right < right.left
                    || right.right < left.left
                    || left.bottom < right.top
                    || right.bottom < left.top
            }));
        }
        assert_eq!(frame.cell(1, CARD_TOP + 2).glyph, "A");
        assert_eq!(frame.cell(2, CARD_TOP + 2).glyph, "⡀");
        assert_eq!(frame.cell(1, CARD_TOP + 2).style, SemanticStyle::Allocated);
        assert_eq!(frame.cell(2, CARD_TOP + 2).style, SemanticStyle::Occupancy);
        assert!(frame.line(CARD_TOP + 1).contains("table:"));
        assert!(!frame.semantic_snapshot().contains('\u{1b}'));

        let ascii =
            VolumeRenderer::render(&scene, surface, PresentationProfile::MONO_ASCII).unwrap();
        assert!((0..surface.height).all(|row| ascii.line(row).is_ascii()));
    }

    #[test]
    fn volume_goldens_are_stable_at_all_required_sizes_and_profiles() {
        for (surface, profile, expected) in [
            (
                Surface::new(120, 36),
                PresentationProfile::ANSI_UNICODE,
                include_str!("../../tests/goldens/tui-volume-120x36-ansi-unicode.txt"),
            ),
            (
                Surface::new(80, 24),
                PresentationProfile::ANSI_UNICODE,
                include_str!("../../tests/goldens/tui-volume-80x24-ansi-unicode.txt"),
            ),
            (
                Surface::new(60, 20),
                PresentationProfile::MONO_ASCII,
                include_str!("../../tests/goldens/tui-volume-60x20-mono-ascii.txt"),
            ),
        ] {
            let scene = synthetic_scene(surface);
            let frame = VolumeRenderer::render(&scene, surface, profile).unwrap();
            assert_eq!(frame.semantic_snapshot(), expected);
        }
    }

    #[test]
    fn sector_goldens_are_stable_at_all_required_sizes_and_profiles() {
        for (surface, profile, expected) in [
            (
                Surface::new(120, 36),
                PresentationProfile::ANSI_UNICODE,
                include_str!("../../tests/goldens/tui-sector-120x36-ansi-unicode.txt"),
            ),
            (
                Surface::new(80, 24),
                PresentationProfile::ANSI_UNICODE,
                include_str!("../../tests/goldens/tui-sector-80x24-ansi-unicode.txt"),
            ),
            (
                Surface::new(60, 20),
                PresentationProfile::MONO_ASCII,
                include_str!("../../tests/goldens/tui-sector-60x20-mono-ascii.txt"),
            ),
        ] {
            let scene = synthetic_sector_scene(7);
            let frame = SectorRenderer::render(&scene, surface, profile).unwrap();
            assert_eq!(frame.semantic_snapshot(), expected);
        }
    }

    #[test]
    fn page_goldens_are_stable_at_all_required_sizes_and_profiles() {
        let scene = ready_page_scene();
        for (surface, profile, expected) in [
            (
                Surface::new(120, 36),
                PresentationProfile::ANSI_UNICODE,
                include_str!("../../tests/goldens/tui-page-120x36-ansi-unicode.txt"),
            ),
            (
                Surface::new(80, 24),
                PresentationProfile::ANSI_UNICODE,
                include_str!("../../tests/goldens/tui-page-80x24-ansi-unicode.txt"),
            ),
            (
                Surface::new(60, 20),
                PresentationProfile::MONO_ASCII,
                include_str!("../../tests/goldens/tui-page-60x20-mono-ascii.txt"),
            ),
        ] {
            let frame = PageRenderer::render(&scene, surface, profile).unwrap();
            assert_eq!(frame.semantic_snapshot(), expected);
        }
    }

    #[test]
    fn interpretation_goldens_are_stable_at_all_required_sizes_and_profiles() {
        let scene = ready_interpretation_scene();
        for (surface, profile, expected) in [
            (
                Surface::new(120, 36),
                PresentationProfile::ANSI_UNICODE,
                include_str!("../../tests/goldens/tui-interpretation-120x36-ansi-unicode.txt"),
            ),
            (
                Surface::new(80, 24),
                PresentationProfile::ANSI_UNICODE,
                include_str!("../../tests/goldens/tui-interpretation-80x24-ansi-unicode.txt"),
            ),
            (
                Surface::new(60, 20),
                PresentationProfile::MONO_ASCII,
                include_str!("../../tests/goldens/tui-interpretation-60x20-mono-ascii.txt"),
            ),
        ] {
            assert_eq!(
                PageRenderer::render(&scene, surface, profile)
                    .unwrap()
                    .semantic_snapshot(),
                expected
            );
        }
    }

    fn primary_capabilities() -> terminal::TerminalCapabilities {
        terminal::TerminalCapabilities {
            ansi_color: true,
            unicode: true,
            mouse: true,
        }
    }

    #[test]
    fn scripted_host_enters_once_draws_only_on_change_and_cleans_up_in_reverse() {
        use terminal::{HostEvent, HostOperation, ScriptHost, ScriptPoll};

        let (_directory, view) = fixture_view();
        let (host, observer) = ScriptHost::new(
            (80, 24),
            primary_capabilities(),
            [
                ScriptPoll::Idle,
                ScriptPoll::Event(HostEvent::Key(StructuralKey::Quit)),
            ],
        );
        let exit = terminal::run_scripted(view, fixture_policy(), host).unwrap();

        assert!(exit.state().quit_requested);
        assert_eq!(
            observer.presentations().len(),
            1,
            "idle and quit do not redraw"
        );
        assert_eq!(
            observer.operations(),
            [
                HostOperation::EnableRaw,
                HostOperation::EnterAlternate,
                HostOperation::EnableMouse,
                HostOperation::HideCursor,
                HostOperation::Present,
                HostOperation::ShowCursor,
                HostOperation::DisableMouse,
                HostOperation::LeaveAlternate,
                HostOperation::DisableRaw,
            ]
        );
    }

    #[test]
    fn scripted_host_refuses_non_ttys_and_unwinds_every_partial_entry() {
        use terminal::{HostEvent, HostOperation, ScriptHost, ScriptPoll};

        let (_directory, view) = fixture_view();
        let script = [ScriptPoll::Event(HostEvent::Key(StructuralKey::Quit))];
        let (host, observer) = ScriptHost::new((80, 24), primary_capabilities(), script);
        let result = terminal::run_scripted(view.clone(), fixture_policy(), host.not_terminal());
        assert!(matches!(result, Err(FocusedTerminalError::NotTerminal)));
        assert!(observer.operations().is_empty());

        for (failure, expected) in [
            (HostOperation::EnableRaw, vec![HostOperation::EnableRaw]),
            (
                HostOperation::EnterAlternate,
                vec![
                    HostOperation::EnableRaw,
                    HostOperation::EnterAlternate,
                    HostOperation::DisableRaw,
                ],
            ),
            (
                HostOperation::EnableMouse,
                vec![
                    HostOperation::EnableRaw,
                    HostOperation::EnterAlternate,
                    HostOperation::EnableMouse,
                    HostOperation::LeaveAlternate,
                    HostOperation::DisableRaw,
                ],
            ),
            (
                HostOperation::HideCursor,
                vec![
                    HostOperation::EnableRaw,
                    HostOperation::EnterAlternate,
                    HostOperation::EnableMouse,
                    HostOperation::HideCursor,
                    HostOperation::DisableMouse,
                    HostOperation::LeaveAlternate,
                    HostOperation::DisableRaw,
                ],
            ),
        ] {
            let (host, observer) = ScriptHost::new((80, 24), primary_capabilities(), script);
            let result =
                terminal::run_scripted(view.clone(), fixture_policy(), host.fail_on(failure));
            assert!(matches!(result, Err(FocusedTerminalError::Io(_))));
            assert_eq!(observer.operations(), expected, "failure at {failure:?}");
        }
    }

    #[test]
    fn presentation_and_cleanup_failures_still_attempt_the_complete_unwind() {
        use terminal::{HostEvent, HostOperation, ScriptHost, ScriptPoll};

        let (_directory, view) = fixture_view();
        let script = [ScriptPoll::Event(HostEvent::Key(StructuralKey::Quit))];
        for failure in [
            HostOperation::Present,
            HostOperation::ShowCursor,
            HostOperation::DisableMouse,
            HostOperation::LeaveAlternate,
            HostOperation::DisableRaw,
        ] {
            let (host, observer) = ScriptHost::new((80, 24), primary_capabilities(), script);
            let result =
                terminal::run_scripted(view.clone(), fixture_policy(), host.fail_on(failure));
            assert!(matches!(result, Err(FocusedTerminalError::Io(_))));
            let operations = observer.operations();
            assert!(operations.contains(&HostOperation::DisableMouse));
            assert!(operations.contains(&HostOperation::LeaveAlternate));
            assert_eq!(operations.last(), Some(&HostOperation::DisableRaw));
        }
    }

    #[test]
    fn fallback_profile_and_maximum_logical_surface_are_bounded_and_semantic() {
        use terminal::{HostEvent, HostOperation, ScriptHost, ScriptPoll};

        assert_eq!(
            terminal::effective_surface(u16::MAX, u16::MAX),
            Surface::new(240, 80)
        );
        let (_directory, view) = fixture_view();
        let (host, observer) = ScriptHost::new(
            (u16::MAX, u16::MAX),
            terminal::TerminalCapabilities {
                ansi_color: false,
                unicode: false,
                mouse: false,
            },
            [ScriptPoll::Event(HostEvent::Key(StructuralKey::Quit))],
        );
        terminal::run_scripted(view, fixture_policy(), host).unwrap();
        let frames = observer.presentations();
        assert_eq!(frames.len(), 1);
        assert!(frames[0].starts_with("surface 240x80 · profile mono-ascii"));
        assert!(frames[0].lines().skip(1).take(80).all(|line| {
            line.split_once('│')
                .is_some_and(|(_, cells)| cells.is_ascii())
        }));
        assert!(!observer.operations().contains(&HostOperation::EnableMouse));
        assert!(!observer.operations().contains(&HostOperation::DisableMouse));
    }

    #[test]
    fn runtime_retains_one_capped_frame_independent_of_navigation_count() {
        use terminal::{HostEvent, ScriptHost};

        let (_directory, view) = fixture_view();
        let session = FocusedSession::new(view, fixture_policy()).unwrap();
        let (mut host, _observer) =
            ScriptHost::new((u16::MAX, u16::MAX), primary_capabilities(), []);
        let mut runtime = terminal::FocusedRuntime::new(
            session,
            u16::MAX,
            u16::MAX,
            PresentationProfile::ANSI_UNICODE,
        );
        runtime.render(&mut host, true).unwrap();
        for _ in 0..3 {
            assert!(
                runtime
                    .handle_event(HostEvent::Key(StructuralKey::NextSector))
                    .unwrap()
            );
            runtime.render(&mut host, false).unwrap();
        }
        assert_eq!(runtime.retained_frame_count(), 1);
        assert_eq!(runtime.retained_frame_cells(), 240 * 80);
    }

    #[test]
    fn generation_stamped_pointer_rejects_a_stale_layout() {
        use terminal::{HostEvent, PointerKind, ScriptHost, ScriptPoll};

        let (_directory, view) = fixture_view();
        let (host, observer) = ScriptHost::new(
            (60, 20),
            primary_capabilities(),
            [
                ScriptPoll::Event(HostEvent::Pointer {
                    generation: Some(0),
                    column: 1,
                    row: CARD_TOP,
                    kind: PointerKind::Activate,
                }),
                ScriptPoll::Event(HostEvent::Pointer {
                    generation: Some(0),
                    column: 0,
                    row: SECTOR_GRID_TOP,
                    kind: PointerKind::Activate,
                }),
                ScriptPoll::Event(HostEvent::Key(StructuralKey::Quit)),
            ],
        );
        let exit = terminal::run_scripted(view, fixture_policy(), host).unwrap();
        assert_eq!(exit.state().mode, FocusedMode::Sector);
        assert_eq!(exit.state().focused_page, 0);
        assert_eq!(observer.presentations().len(), 2);
    }

    #[test]
    fn contextual_help_has_escape_precedence_and_disables_background_hits() {
        use terminal::{HostEvent, PointerKind, ScriptHost, ScriptPoll};

        let (_directory, view) = fixture_view();
        let (host, observer) = ScriptHost::new(
            (60, 20),
            primary_capabilities(),
            [
                ScriptPoll::Event(HostEvent::Key(StructuralKey::Help)),
                ScriptPoll::Event(HostEvent::Pointer {
                    generation: Some(1),
                    column: 1,
                    row: CARD_TOP,
                    kind: PointerKind::Activate,
                }),
                ScriptPoll::Event(HostEvent::Key(StructuralKey::Escape)),
                ScriptPoll::Event(HostEvent::Key(StructuralKey::Quit)),
            ],
        );
        let exit = terminal::run_scripted(view, fixture_policy(), host).unwrap();
        assert_eq!(exit.state().mode, FocusedMode::Volume);
        assert!(!exit.state().help_visible);
        let frames = observer.presentations();
        assert_eq!(frames.len(), 3);
        assert!(frames[1].contains("Focused Volume keys"));
        assert!(!frames[2].contains("Focused Volume keys"));
    }

    #[test]
    fn resize_bursts_coalesce_without_dropping_interleaved_keyboard_input() {
        use terminal::{HostEvent, ScriptHost, ScriptPoll};

        let (_directory, view) = fixture_view();
        let (host, observer) = ScriptHost::new(
            (60, 20),
            primary_capabilities(),
            [
                ScriptPoll::Event(HostEvent::Resize {
                    width: 80,
                    height: 24,
                }),
                ScriptPoll::Event(HostEvent::Resize {
                    width: 120,
                    height: 36,
                }),
                ScriptPoll::Event(HostEvent::Key(StructuralKey::Enter)),
                ScriptPoll::Idle,
                ScriptPoll::Event(HostEvent::Key(StructuralKey::Quit)),
            ],
        );
        let exit = terminal::run_scripted(view, fixture_policy(), host).unwrap();
        assert_eq!(exit.state().mode, FocusedMode::Sector);
        let frames = observer.presentations();
        assert_eq!(frames.len(), 3, "two Resize events produce one new frame");
        assert!(frames[1].starts_with("surface 120x36"));
        assert!(frames[2].contains("64 physical Pages"));
    }

    #[test]
    fn input_precedes_a_ready_completion_and_the_cancelled_result_never_adopts() {
        use terminal::{HostEvent, PointerKind, ScriptHost, ScriptPoll};

        let (_directory, view) = fixture_view();
        let original_revision = view.overview().revision;
        let (host, observer) = ScriptHost::new(
            (80, 24),
            primary_capabilities(),
            [
                ScriptPoll::Event(HostEvent::Key(StructuralKey::Enter)),
                ScriptPoll::Event(HostEvent::Pointer {
                    generation: Some(1),
                    column: 20,
                    row: SECTOR_GRID_TOP,
                    kind: PointerKind::Activate,
                }),
                ScriptPoll::Event(HostEvent::Key(StructuralKey::Escape)),
                ScriptPoll::Idle,
                ScriptPoll::Event(HostEvent::Key(StructuralKey::Quit)),
            ],
        );
        let exit = terminal::run_scripted(view, fixture_policy(), host).unwrap();
        assert_eq!(exit.state().mode, FocusedMode::Sector);
        assert_eq!(exit.state().focused_page, 2);
        assert_eq!(exit.state().volume.revision, original_revision);
        assert_eq!(observer.presentations().len(), 4);
    }

    #[test]
    fn too_small_resize_round_trip_restores_selected_record_and_interpretation() {
        use terminal::{HostEvent, ScriptHost, ScriptPoll};

        let (_directory, view) = interpretation_fixture_view();
        let initial_revision = view.overview().revision;
        let mut polls = vec![ScriptPoll::Event(HostEvent::Key(StructuralKey::NextVolume))];
        polls.extend((0..10).map(|_| ScriptPoll::Event(HostEvent::Key(StructuralKey::NextSector))));
        polls.extend([
            ScriptPoll::Event(HostEvent::Key(StructuralKey::Enter)),
            ScriptPoll::Event(HostEvent::Key(StructuralKey::Right)),
            ScriptPoll::Event(HostEvent::Key(StructuralKey::Enter)),
            ScriptPoll::Idle,
            ScriptPoll::Event(HostEvent::Key(StructuralKey::Down)),
            ScriptPoll::Event(HostEvent::Key(StructuralKey::Down)),
            ScriptPoll::Event(HostEvent::Key(StructuralKey::Enter)),
            ScriptPoll::Idle,
            ScriptPoll::Event(HostEvent::Resize {
                width: 60,
                height: 20,
            }),
            ScriptPoll::Idle,
            ScriptPoll::Event(HostEvent::Resize {
                width: 59,
                height: 19,
            }),
            ScriptPoll::Idle,
            ScriptPoll::Event(HostEvent::Resize {
                width: 60,
                height: 20,
            }),
            ScriptPoll::Idle,
            ScriptPoll::Event(HostEvent::Key(StructuralKey::Quit)),
        ]);
        let (host, observer) = ScriptHost::new((80, 24), primary_capabilities(), polls);
        let exit = terminal::run_scripted(view, interpretation_fixture_policy(), host).unwrap();

        assert_eq!(exit.state().mode, FocusedMode::Page);
        let final_revision = exit.state().volume.revision;
        assert!(final_revision > initial_revision);
        let frames = observer.presentations();
        let suspended = frames
            .iter()
            .position(|frame| frame.contains("focused inspector paused"))
            .unwrap();
        assert_eq!(
            frames[suspended - 1],
            frames[suspended + 1],
            "60x20 recovery must restore the exact selected-record scene"
        );
        assert!(
            frames
                .last()
                .is_some_and(|frame| frame.contains("Record 1|641|1")),
            "{}",
            frames.last().unwrap()
        );
        assert_eq!(exit.into_view().overview().revision, final_revision);
    }

    #[test]
    fn failed_present_does_not_commit_the_prepared_generation() {
        use terminal::{HostEvent, HostOperation, ScriptHost};

        let (_directory, view) = fixture_view();
        let session = FocusedSession::new(view, fixture_policy()).unwrap();
        let (host, observer) = ScriptHost::new((80, 24), primary_capabilities(), []);
        let mut host = host.fail_on_occurrence(HostOperation::Present, 2);
        let mut runtime =
            terminal::FocusedRuntime::new(session, 80, 24, PresentationProfile::ANSI_UNICODE);
        runtime.render(&mut host, true).unwrap();
        assert_eq!(runtime.generation(), Some(0));
        assert!(
            runtime
                .handle_event(HostEvent::Key(StructuralKey::Enter))
                .unwrap()
        );
        assert!(matches!(
            runtime.render(&mut host, false),
            Err(FocusedTerminalError::Io(_))
        ));
        assert_eq!(runtime.generation(), Some(0));
        assert_eq!(observer.presentations().len(), 1);
    }

    #[test]
    fn focused_terminal_real_pty_restores_raw_mode() {
        const HELPER: &str = "VOLMAP_FOCUSED_PTY_HELPER";
        if std::env::var_os(HELPER).is_some() {
            let (directory, _view) = fixture_view();
            let vinf = directory.path().join("fixture_vinf");
            assert!(!crossterm::terminal::is_raw_mode_enabled().unwrap());
            let exit = crate::cli::run_from([
                "volmap",
                "tui",
                "--vinf",
                vinf.to_str().unwrap(),
                "--progress",
                "never",
            ]);
            assert_eq!(exit, 1);
            assert!(!crossterm::terminal::is_raw_mode_enabled().unwrap());
            println!("VOLMAP_FOCUSED_PTY_CLEAN");
            return;
        }

        let executable = std::env::current_exe().unwrap();
        let test_name = "tui::focused::tests::focused_terminal_real_pty_restores_raw_mode";
        let command = format!(
            "{} --exact {test_name} --nocapture --test-threads=1",
            executable.display()
        );
        let mut child = std::process::Command::new("script")
            .args(["-q", "-e", "-c", &command, "/dev/null"])
            .env(HELPER, "1")
            .env("TERM", "xterm-256color")
            .env("LANG", "C.UTF-8")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"q").unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "PTY child failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("VOLMAP_FOCUSED_PTY_CLEAN"));
    }
}
