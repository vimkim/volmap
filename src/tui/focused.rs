//! Staged focused-TUI session with Volume and Sector modes.
//!
//! This module deliberately has no terminal event or I/O dependency. Tickets
//! 01 and 02 keep it beside the legacy production path so semantic state,
//! bounded projection, and rendering can be proved before cutover.

use std::fmt::{self, Write as _};

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::inspection::{GraphView, QueryError, VolumeView};
use crate::model::{InspectionRevision, SectorId, SnapshotId, VolId};
use crate::projection::{
    ClassNameProjection, FileAssociationBodyProjection, FileAssociationProjection,
    OptionalOidProjection, OptionalTextProjection, PageOccupancyProjection, PageProjection,
    SectorAttributionProjection, SectorProjection, outcome_name, sector_projection,
    snapshot_id_hex, volume_projection,
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FocusedState {
    pub volume: VolumeState,
    pub mode: FocusedMode,
    pub focused_page: u8,
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
        PointerInput::WheelRows(rows) if mode == FocusedMode::Volume => {
            vec![FocusedAction::ScrollRows(rows)]
        }
        PointerInput::WheelRows(rows) if rows.is_negative() => {
            vec![FocusedAction::PreviousSector]
        }
        PointerInput::WheelRows(rows) if rows > 0 => vec![FocusedAction::NextSector],
        PointerInput::WheelRows(_) => Vec::new(),
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FocusedSession {
    view: GraphView,
    volumes: Vec<VolumeView>,
    volume_index: usize,
    focused_sector: u32,
    top_sector: u32,
    mode: FocusedMode,
    focused_page: u8,
}

impl FocusedSession {
    pub(crate) fn new(view: GraphView) -> Result<Self, FocusedError> {
        let volumes = view.volumes();
        if volumes.is_empty() {
            return Err(FocusedError::EmptyInspection);
        }
        Ok(Self {
            view,
            volumes,
            volume_index: 0,
            focused_sector: 0,
            top_sector: 0,
            mode: FocusedMode::Volume,
            focused_page: 0,
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
        }
    }

    pub(crate) fn advance_focused(
        &mut self,
        action: FocusedAction,
        surface: Surface,
    ) -> Result<FocusedTransition, FocusedError> {
        let before = self.focused_state();
        let layout = VolumeLayout::for_surface(surface)?;
        match (self.mode, action) {
            (FocusedMode::Volume, FocusedAction::Left) => {
                self.apply_volume_action(VolumeAction::Left, layout);
            }
            (FocusedMode::Volume, FocusedAction::Right) => {
                self.apply_volume_action(VolumeAction::Right, layout);
            }
            (FocusedMode::Volume, FocusedAction::Up) => {
                self.apply_volume_action(VolumeAction::Up, layout);
            }
            (FocusedMode::Volume, FocusedAction::Down) => {
                self.apply_volume_action(VolumeAction::Down, layout);
            }
            (FocusedMode::Sector, FocusedAction::Left) => self.move_page_horizontal(false),
            (FocusedMode::Sector, FocusedAction::Right) => self.move_page_horizontal(true),
            (FocusedMode::Sector, FocusedAction::Up) => self.move_page_vertical(false),
            (FocusedMode::Sector, FocusedAction::Down) => self.move_page_vertical(true),
            (FocusedMode::Volume, FocusedAction::Activate) => {
                self.mode = FocusedMode::Sector;
                self.focused_page = 0;
            }
            (FocusedMode::Sector, FocusedAction::Ascend) => {
                self.mode = FocusedMode::Volume;
            }
            (_, FocusedAction::PreviousSector) => {
                self.apply_volume_action(VolumeAction::PreviousSector, layout);
            }
            (_, FocusedAction::NextSector) => {
                self.apply_volume_action(VolumeAction::NextSector, layout);
            }
            (FocusedMode::Volume, FocusedAction::PreviousVolume) => {
                self.apply_volume_action(VolumeAction::PreviousVolume, layout);
            }
            (FocusedMode::Volume, FocusedAction::NextVolume) => {
                self.apply_volume_action(VolumeAction::NextVolume, layout);
            }
            (FocusedMode::Volume, FocusedAction::ScrollRows(rows)) => {
                self.apply_volume_action(VolumeAction::ScrollRows(rows), layout);
            }
            (_, FocusedAction::FocusSector(sector)) if sector < self.total_sectors() => {
                self.focused_sector = sector;
                self.reveal_focus(layout);
            }
            (FocusedMode::Sector, FocusedAction::FocusPage(page)) if page < 64 => {
                self.focused_page = page;
            }
            _ => {}
        }
        let state = self.focused_state();
        Ok(FocusedTransition {
            changed: state != before,
            state,
        })
    }

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VolumeFrame {
    surface: Surface,
    profile: PresentationProfile,
    cells: Vec<Cell>,
    pub hits: Vec<HitRegion>,
    pub page_hits: Vec<PageHitRegion>,
}

impl VolumeFrame {
    fn new(surface: Surface, profile: PresentationProfile) -> Self {
        Self {
            surface,
            profile,
            cells: vec![Cell::default(); usize::from(surface.width) * usize::from(surface.height)],
            hits: Vec::new(),
            page_hits: Vec::new(),
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
            bitmap[32..40].copy_from_slice(&1_u64.to_le_bytes());
            file.write_all_at(&bitmap, u64::try_from(IO_PAGE_SIZE).unwrap())
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
            ResourcePolicy::new(4 * 1024 * 1024, 1024 * 1024, 1, 32, 1024 * 1024).unwrap(),
            &CancelToken::new(),
            None,
        )
        .unwrap();
        let view = inspection.view(RevisionSelector::Latest).unwrap();
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

    fn apply_actions(
        session: &mut FocusedSession,
        surface: Surface,
        actions: impl IntoIterator<Item = FocusedAction>,
    ) {
        for action in actions {
            session.advance_focused(action, surface).unwrap();
        }
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
        let mut traversal = FocusedSession::new(view.clone()).unwrap();
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

        let mut session = FocusedSession::new(view).unwrap();
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
        let mut session = FocusedSession::new(view).unwrap();
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
    fn mouse_and_wheel_translate_to_the_same_semantic_actions_as_keyboard() {
        let (_directory, view) = fixture_view();
        let surface = Surface::new(60, 20);
        let mut keyboard = FocusedSession::new(view.clone()).unwrap();
        apply_actions(
            &mut keyboard,
            surface,
            std::iter::repeat_n(key_action(StructuralKey::NextSector), 11)
                .chain([key_action(StructuralKey::Enter)]),
        );

        let mut pointer = FocusedSession::new(view).unwrap();
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

        let mut wheel = pointer.clone();
        apply_actions(
            &mut wheel,
            surface,
            pointer_actions(FocusedMode::Sector, PointerInput::WheelRows(1)),
        );
        pointer
            .advance_focused(key_action(StructuralKey::NextSector), surface)
            .unwrap();
        assert_eq!(wheel.focused_state(), pointer.focused_state());
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
}
