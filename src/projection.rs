//! Stable, disclosure-safe projection types shared by all adapters.

use serde::Serialize;

use crate::diagnostics::InspectionOutcome;
use crate::format::{PageType, VolumePurpose, VolumeType};
use crate::inspection::{
    CoverageRecord, DeepPageView, DiagnosticRecord, OosChainView, OverflowChainView, OverviewView,
    PageView, RawPageView, SectorView, VolumeView,
};
use crate::model::{
    Availability, Coverage, PageAllocationClass, SnapshotId, SnapshotValidity, TdeInspectionState,
};

pub const SCHEMA_NAME: &str = "volmap.inspection";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize)]
pub struct ToolProjection {
    pub name: &'static str,
    pub version: &'static str,
    pub format_profile: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct CommandProjection {
    pub name: String,
    pub input_kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SnapshotProjection {
    pub id: String,
    pub revision: String,
    pub validity: &'static str,
    pub format_profile: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct CoverageProjection {
    pub facet: &'static str,
    pub coverage: &'static str,
    pub evaluated: String,
    pub conclusive: String,
    pub total: CountProjection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", content = "value", rename_all = "kebab-case")]
pub enum CountProjection {
    Known(String),
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiagnosticProjection {
    pub code: &'static str,
    pub severity: &'static str,
    pub message: &'static str,
    pub subject: String,
    pub rule: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResultDocument {
    pub schema: &'static str,
    pub schema_version: u32,
    pub document_type: &'static str,
    pub tool: ToolProjection,
    pub command: CommandProjection,
    pub snapshot: SnapshotProjection,
    pub outcome: &'static str,
    pub coverage: Vec<CoverageProjection>,
    pub data: DataProjection,
    pub diagnostics: Vec<DiagnosticProjection>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DataProjection {
    Summary {
        overview: OverviewProjection,
    },
    Map {
        volumes: Vec<VolumeProjection>,
        sectors: Vec<SectorProjection>,
        deep_pages: Vec<DeepPageResourceProjection>,
        oos_chains: Vec<OosChainProjection>,
        overflow_chains: Vec<OverflowChainProjection>,
    },
    InspectVolume {
        volume: VolumeProjection,
    },
    InspectSector {
        sector: SectorProjection,
    },
    InspectFile {
        file: FileHeaderProjection,
    },
    InspectPage {
        page: PageProjection,
        deep: DeepPageProjection,
    },
    InspectSlot {
        page: PageProjection,
        deep: DeepPageProjection,
        selected_slot: SlotProjection,
        overflow_chain: Option<OverflowChainProjection>,
    },
    InspectOos {
        chain: OosChainProjection,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct OverviewProjection {
    pub volume_count: String,
    pub sector_count: String,
    pub reserved_sector_count: String,
    pub physical_page_count: String,
    pub inspected_page_envelopes: String,
    pub page_type_counts: Vec<PageTypeCountProjection>,
    pub tde_opaque_pages: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PageTypeCountProjection {
    pub page_type: &'static str,
    pub count: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct VolumeProjection {
    pub vol_id: i16,
    pub purpose: &'static str,
    pub volume_type: &'static str,
    pub total_sectors: u32,
    pub maximum_sectors: u32,
    pub system_last_page: i32,
    pub reserved_sectors: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct SectorProjection {
    pub vol_id: i16,
    pub sector_id: i32,
    pub reserved: bool,
    pub pages: Vec<PageProjection>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PageProjection {
    pub vol_id: i16,
    pub page_id: i32,
    pub sector_id: i32,
    pub allocation: &'static str,
    pub page_type: OptionalTextProjection,
    pub availability: &'static str,
    pub tde_state: &'static str,
    pub detail_support: OptionalTextProjection,
    pub lsa_word: OptionalCountProjection,
    pub diagnostic: OptionalTextProjection,
    pub bytes: BytesWithheldProjection,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum DeepPageProjection {
    NotEnriched,
    EnvelopeOnly,
    FileHeader {
        structure: FileHeaderProjection,
    },
    Slotted {
        structure: SlottedPageProjection,
    },
    HeapHeader {
        structure: HeapHeaderProjection,
    },
    HeapChain {
        structure: HeapChainProjection,
    },
    BtreeRoot {
        structure: BtreeRootProjection,
    },
    BtreeNode {
        structure: BtreeNodeProjection,
    },
    BtreeOidOverflow {
        structure: BtreeOidOverflowProjection,
    },
    Catalog {
        structure: CatalogPageProjection,
    },
    Vacuum {
        structure: VacuumPageProjection,
    },
    DroppedFiles {
        structure: DroppedFilesPageProjection,
    },
    Invalid {
        rule: &'static str,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct HeapHeaderProjection {
    pub class_oid: OptionalOidProjection,
    pub overflow_file: OptionalVfidProjection,
    pub next: OptionalVpidProjection,
    pub last: OptionalVpidProjection,
    pub oos_file: OptionalVfidProjection,
    pub unfill_space: String,
    pub estimated_pages: String,
    pub estimated_records: String,
    pub estimated_record_bytes: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct HeapChainProjection {
    pub class_oid: OptionalOidProjection,
    pub previous: OptionalVpidProjection,
    pub next: OptionalVpidProjection,
    pub max_mvccid: String,
    pub flags: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct BtreeRootProjection {
    pub node: BtreeNodeProjection,
    pub oid_count: String,
    pub null_count: String,
    pub key_count: String,
    pub top_class: OidProjection,
    pub constraint_flags: String,
    pub revision_level: i16,
    pub deduplicate_key_encoded: i16,
    pub overflow_key_file: OptionalVfidProjection,
    pub creator_mvccid: String,
    pub domain_offset: u16,
    pub domain_length: u16,
    pub domain_bytes: BytesWithheldProjection,
}

#[derive(Clone, Debug, Serialize)]
pub struct BtreeNodeProjection {
    pub role: &'static str,
    pub previous: OptionalVpidProjection,
    pub next: OptionalVpidProjection,
    pub level: u16,
    pub max_key_length: u16,
    pub common_prefix: OptionalCountProjection,
    pub split_pivot_bits: String,
    pub split_index: String,
    pub record_count: u16,
    pub record_bytes: String,
    pub child_count: u16,
    pub overflow_key_count: u16,
}

#[derive(Clone, Debug, Serialize)]
pub struct BtreeOidOverflowProjection {
    pub next: OptionalVpidProjection,
    pub record_count: u16,
    pub record_bytes: String,
    pub bytes: BytesWithheldProjection,
}

#[derive(Clone, Debug, Serialize)]
pub struct CatalogPageProjection {
    pub next_overflow: OptionalVpidProjection,
    pub directory_count: String,
    pub role: &'static str,
    pub record_count: u16,
    pub record_bytes: String,
    pub bytes: BytesWithheldProjection,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum OptionalVfidProjection {
    Absent,
    Present { vol_id: i16, file_id: i32 },
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum OptionalOidProjection {
    Absent,
    Present { oid: OidProjection },
}

#[derive(Clone, Debug, Serialize)]
pub struct VacuumPageProjection {
    pub next: OptionalVpidProjection,
    pub index_unvacuumed: OptionalCountProjection,
    pub index_free: String,
    pub entries: Vec<VacuumEntryProjection>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VacuumEntryProjection {
    pub block_id: String,
    pub flags: String,
    pub start_lsa_word: String,
    pub oldest_visible_mvccid: String,
    pub newest_mvccid: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DroppedFilesPageProjection {
    pub next: OptionalVpidProjection,
    pub entries: Vec<DroppedFileProjection>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DroppedFileProjection {
    pub vol_id: i16,
    pub file_id: i32,
    pub mvccid: String,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum OptionalVpidProjection {
    Terminal,
    Link { vol_id: i16, page_id: i32 },
}

#[derive(Clone, Debug, Serialize)]
pub struct SlottedPageProjection {
    pub anchor: &'static str,
    pub alignment: u16,
    pub total_free: String,
    pub contiguous_free: String,
    pub free_area_offset: String,
    pub flags: String,
    pub is_saving: bool,
    pub slots: Vec<SlotProjection>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FileHeaderProjection {
    pub vol_id: i16,
    pub file_id: i32,
    pub file_type: &'static str,
    pub class_oid: OptionalOidProjection,
    pub flags: String,
    pub page_total: String,
    pub page_user: String,
    pub page_ftab: String,
    pub page_free: String,
    pub page_marked_delete: String,
    pub sector_total: String,
    pub sector_partial: String,
    pub sector_full: String,
    pub sector_empty: String,
    pub bytes: BytesWithheldProjection,
}

#[derive(Clone, Debug, Serialize)]
pub struct SlotProjection {
    pub slot_id: u16,
    pub offset: u16,
    pub length: u16,
    pub record_type: &'static str,
    pub record_type_ordinal: u8,
    pub bytes: BytesWithheldProjection,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeepPageResourceProjection {
    pub page: PageProjection,
    pub deep: DeepPageProjection,
}

#[derive(Clone, Debug, Serialize)]
pub struct OosChainProjection {
    pub head: OidProjection,
    pub total_data_length: OptionalCountProjection,
    pub validated_payload_bytes: String,
    pub complete: bool,
    pub chunks: Vec<OosChunkProjection>,
    pub diagnostic: OptionalTextProjection,
    pub bytes: BytesWithheldProjection,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct OidProjection {
    pub vol_id: i16,
    pub page_id: i32,
    pub slot_id: i16,
}

#[derive(Clone, Debug, Serialize)]
pub struct OosChunkProjection {
    pub oid: OidProjection,
    pub total_data_length: String,
    pub chunk_index: String,
    pub next: OosNextProjection,
    pub payload_offset: u16,
    pub payload_length: u16,
    pub bytes: BytesWithheldProjection,
}

#[derive(Clone, Debug, Serialize)]
pub struct OverflowChainProjection {
    pub source: OidProjection,
    pub head: OptionalVpidProjection,
    pub total_data_length: OptionalCountProjection,
    pub validated_payload_bytes: String,
    pub complete: bool,
    pub pages: Vec<OverflowPageProjection>,
    pub diagnostic: OptionalTextProjection,
    pub bytes: BytesWithheldProjection,
}

#[derive(Clone, Debug, Serialize)]
pub struct OverflowPageProjection {
    pub vol_id: i16,
    pub page_id: i32,
    pub role: &'static str,
    pub next: OptionalVpidProjection,
    pub payload_offset: u16,
    pub payload_length: u16,
    pub bytes: BytesWithheldProjection,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum OosNextProjection {
    Terminal,
    Link { oid: OidProjection },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", content = "value", rename_all = "kebab-case")]
pub enum OptionalTextProjection {
    Known(&'static str),
    Unknown,
    Unsupported,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", content = "value", rename_all = "kebab-case")]
pub enum OptionalCountProjection {
    Known(String),
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct BytesWithheldProjection {
    pub state: &'static str,
}

pub fn result_document(
    command_name: &str,
    selector: Option<String>,
    overview: &OverviewView,
    data: DataProjection,
) -> ResultDocument {
    ResultDocument {
        schema: SCHEMA_NAME,
        schema_version: SCHEMA_VERSION,
        document_type: "result",
        tool: ToolProjection {
            name: "volmap",
            version: env!("CARGO_PKG_VERSION"),
            format_profile: overview.format_profile,
        },
        command: CommandProjection {
            name: command_name.to_owned(),
            input_kind: overview.input_kind,
            selector,
        },
        snapshot: SnapshotProjection {
            id: snapshot_id_hex(overview.snapshot_id),
            revision: overview.revision.get().to_string(),
            validity: validity_name(overview.validity),
            format_profile: overview.format_profile,
        },
        outcome: outcome_name(overview.outcome),
        coverage: overview
            .coverage
            .iter()
            .copied()
            .map(coverage_projection)
            .collect(),
        data,
        diagnostics: overview
            .diagnostics
            .iter()
            .cloned()
            .map(diagnostic_projection)
            .collect(),
    }
}

#[must_use]
pub fn summary_projection(overview: &OverviewView) -> OverviewProjection {
    OverviewProjection {
        volume_count: overview.volume_count.to_string(),
        sector_count: overview.sector_count.to_string(),
        reserved_sector_count: overview.reserved_sector_count.to_string(),
        physical_page_count: overview.physical_page_count.to_string(),
        inspected_page_envelopes: overview.inspected_page_envelopes.to_string(),
        page_type_counts: overview
            .page_type_counts
            .iter()
            .map(|(page_type, count)| PageTypeCountProjection {
                page_type: page_type.as_str(),
                count: count.to_string(),
            })
            .collect(),
        tde_opaque_pages: overview.tde_opaque_pages.to_string(),
    }
}

#[must_use]
pub fn volume_projection(volume: VolumeView) -> VolumeProjection {
    VolumeProjection {
        vol_id: volume.vol_id.get(),
        purpose: purpose_name(volume.purpose),
        volume_type: volume_type_name(volume.volume_type),
        total_sectors: volume.total_sectors,
        maximum_sectors: volume.maximum_sectors,
        system_last_page: volume.system_last_page.get(),
        reserved_sectors: volume.reserved_sectors,
    }
}

#[must_use]
pub fn sector_projection(sector: SectorView) -> SectorProjection {
    SectorProjection {
        vol_id: sector.vol_id.get(),
        sector_id: sector.sector_id.get(),
        reserved: sector.reserved,
        pages: sector.pages.into_iter().map(page_projection).collect(),
    }
}

#[must_use]
pub fn page_projection(page: PageView) -> PageProjection {
    PageProjection {
        vol_id: page.vpid.vol_id.get(),
        page_id: page.vpid.page_id.get(),
        sector_id: page.sector_id.get(),
        allocation: allocation_name(page.allocation),
        page_type: page
            .page_type
            .map_or(OptionalTextProjection::Unknown, |page_type| {
                OptionalTextProjection::Known(page_type.as_str())
            }),
        availability: availability_name(page.availability),
        tde_state: tde_state_name(page.tde_state),
        detail_support: page
            .detail_support
            .map_or(OptionalTextProjection::Unsupported, |support| {
                OptionalTextProjection::Known(support.as_str())
            }),
        lsa_word: page
            .lsa_word
            .map_or(OptionalCountProjection::Unknown, |value| {
                OptionalCountProjection::Known(value.to_string())
            }),
        diagnostic: page.diagnostic_code.map_or(
            OptionalTextProjection::Unknown,
            OptionalTextProjection::Known,
        ),
        bytes: BytesWithheldProjection {
            state: "bytes-withheld",
        },
    }
}

#[must_use]
#[allow(clippy::too_many_lines)]
pub fn deep_page_projection(deep: Option<DeepPageView>) -> DeepPageProjection {
    let Some(deep) = deep else {
        return DeepPageProjection::NotEnriched;
    };
    if let Some(rule) = deep.diagnostic_rule {
        return DeepPageProjection::Invalid { rule };
    }
    if let Some(file_header) = deep.file_header {
        return DeepPageProjection::FileHeader {
            structure: file_header_projection(file_header),
        };
    }
    if let Some(raw) = deep.raw {
        return match raw {
            RawPageView::Btree(crate::format::BtreePageFact::Root(root)) => {
                DeepPageProjection::BtreeRoot {
                    structure: BtreeRootProjection {
                        node: btree_node_projection(root.node),
                        oid_count: root.oid_count.to_string(),
                        null_count: root.null_count.to_string(),
                        key_count: root.key_count.to_string(),
                        top_class: oid_projection(root.top_class),
                        constraint_flags: root.constraint_flags.to_string(),
                        revision_level: root.revision_level,
                        deduplicate_key_encoded: root.deduplicate_key_encoded,
                        overflow_key_file: optional_vfid_projection(root.overflow_key_file),
                        creator_mvccid: root.creator_mvccid.to_string(),
                        domain_offset: root.domain_offset,
                        domain_length: root.domain_length,
                        domain_bytes: BytesWithheldProjection {
                            state: "bytes-withheld",
                        },
                    },
                }
            }
            RawPageView::Btree(
                crate::format::BtreePageFact::Leaf(node)
                | crate::format::BtreePageFact::NonLeaf(node),
            ) => DeepPageProjection::BtreeNode {
                structure: btree_node_projection(node),
            },
            RawPageView::Btree(crate::format::BtreePageFact::OidOverflow(overflow)) => {
                DeepPageProjection::BtreeOidOverflow {
                    structure: BtreeOidOverflowProjection {
                        next: optional_vpid_projection(overflow.next),
                        record_count: overflow.record_count,
                        record_bytes: overflow.record_bytes.to_string(),
                        bytes: BytesWithheldProjection {
                            state: "bytes-withheld",
                        },
                    },
                }
            }
            RawPageView::Catalog(page) => DeepPageProjection::Catalog {
                structure: CatalogPageProjection {
                    next_overflow: optional_vpid_projection(page.next_overflow),
                    directory_count: page.directory_count.to_string(),
                    role: if page.is_overflow {
                        "overflow"
                    } else {
                        "primary"
                    },
                    record_count: page.record_count,
                    record_bytes: page.record_bytes.to_string(),
                    bytes: BytesWithheldProjection {
                        state: "bytes-withheld",
                    },
                },
            },
            RawPageView::Heap(crate::format::HeapPageFact::Header(header)) => {
                DeepPageProjection::HeapHeader {
                    structure: HeapHeaderProjection {
                        class_oid: optional_oid_projection(header.class_oid),
                        overflow_file: optional_vfid_projection(header.overflow_vfid),
                        next: optional_vpid_projection(header.next),
                        last: optional_vpid_projection(Some(header.last)),
                        oos_file: optional_vfid_projection(header.oos_vfid),
                        unfill_space: header.unfill_space.to_string(),
                        estimated_pages: header.estimated_pages.to_string(),
                        estimated_records: header.estimated_records.to_string(),
                        estimated_record_bytes: header.estimated_record_bytes.to_string(),
                    },
                }
            }
            RawPageView::Heap(crate::format::HeapPageFact::Chain(chain)) => {
                DeepPageProjection::HeapChain {
                    structure: HeapChainProjection {
                        class_oid: optional_oid_projection(chain.class_oid),
                        previous: optional_vpid_projection(chain.previous),
                        next: optional_vpid_projection(chain.next),
                        max_mvccid: chain.max_mvccid.to_string(),
                        flags: chain.flags.to_string(),
                    },
                }
            }
            RawPageView::Vacuum(page) => DeepPageProjection::Vacuum {
                structure: VacuumPageProjection {
                    next: optional_vpid_projection(page.next),
                    index_unvacuumed: page
                        .index_unvacuumed
                        .map_or(OptionalCountProjection::Unknown, |value| {
                            OptionalCountProjection::Known(value.to_string())
                        }),
                    index_free: page.index_free.to_string(),
                    entries: page
                        .entries
                        .into_iter()
                        .map(|entry| VacuumEntryProjection {
                            block_id: entry.block_id.to_string(),
                            flags: entry.flags.to_string(),
                            start_lsa_word: entry.start_lsa_word.to_string(),
                            oldest_visible_mvccid: entry.oldest_visible_mvccid.to_string(),
                            newest_mvccid: entry.newest_mvccid.to_string(),
                        })
                        .collect(),
                },
            },
            RawPageView::DroppedFiles(page) => DeepPageProjection::DroppedFiles {
                structure: DroppedFilesPageProjection {
                    next: optional_vpid_projection(page.next),
                    entries: page
                        .entries
                        .into_iter()
                        .map(|entry| DroppedFileProjection {
                            vol_id: entry.vfid.vol_id.get(),
                            file_id: entry.vfid.file_id.get(),
                            mvccid: entry.mvccid.to_string(),
                        })
                        .collect(),
                },
            },
        };
    }
    let Some(slotted) = deep.slotted else {
        return DeepPageProjection::EnvelopeOnly;
    };
    DeepPageProjection::Slotted {
        structure: SlottedPageProjection {
            anchor: slotted.anchor().as_str(),
            alignment: slotted.alignment(),
            total_free: slotted.total_free().to_string(),
            contiguous_free: slotted.contiguous_free().to_string(),
            free_area_offset: slotted.free_area_offset().to_string(),
            flags: slotted.flags().to_string(),
            is_saving: slotted.is_saving(),
            slots: slotted
                .slots()
                .iter()
                .copied()
                .map(slot_projection)
                .collect(),
        },
    }
}

fn btree_node_projection(node: crate::format::BtreeNodeFact) -> BtreeNodeProjection {
    BtreeNodeProjection {
        role: if node.level == 1 { "leaf" } else { "nonleaf" },
        previous: optional_vpid_projection(node.previous),
        next: optional_vpid_projection(node.next),
        level: node.level,
        max_key_length: node.max_key_length,
        common_prefix: node
            .common_prefix
            .map_or(OptionalCountProjection::Unknown, |value| {
                OptionalCountProjection::Known(value.to_string())
            }),
        split_pivot_bits: node.split_pivot_bits.to_string(),
        split_index: node.split_index.to_string(),
        record_count: node.record_count,
        record_bytes: node.record_bytes.to_string(),
        child_count: node.child_count,
        overflow_key_count: node.overflow_key_count,
    }
}

const fn optional_vfid_projection(vfid: Option<crate::model::Vfid>) -> OptionalVfidProjection {
    match vfid {
        Some(vfid) => OptionalVfidProjection::Present {
            vol_id: vfid.vol_id.get(),
            file_id: vfid.file_id.get(),
        },
        None => OptionalVfidProjection::Absent,
    }
}

const fn optional_oid_projection(oid: Option<crate::model::Oid>) -> OptionalOidProjection {
    match oid {
        Some(oid) => OptionalOidProjection::Present {
            oid: oid_projection(oid),
        },
        None => OptionalOidProjection::Absent,
    }
}

const fn optional_vpid_projection(vpid: Option<crate::model::Vpid>) -> OptionalVpidProjection {
    match vpid {
        Some(vpid) => OptionalVpidProjection::Link {
            vol_id: vpid.vol_id.get(),
            page_id: vpid.page_id.get(),
        },
        None => OptionalVpidProjection::Terminal,
    }
}

#[must_use]
pub fn file_header_projection(header: crate::format::FileHeader) -> FileHeaderProjection {
    FileHeaderProjection {
        vol_id: header.vfid().vol_id.get(),
        file_id: header.vfid().file_id.get(),
        file_type: header.file_type().as_str(),
        class_oid: optional_oid_projection(header.class_oid()),
        flags: header.flags().to_string(),
        page_total: header.page_total().to_string(),
        page_user: header.page_user().to_string(),
        page_ftab: header.page_ftab().to_string(),
        page_free: header.page_free().to_string(),
        page_marked_delete: header.page_marked_delete().to_string(),
        sector_total: header.sector_total().to_string(),
        sector_partial: header.sector_partial().to_string(),
        sector_full: header.sector_full().to_string(),
        sector_empty: header.sector_empty().to_string(),
        bytes: BytesWithheldProjection {
            state: "bytes-withheld",
        },
    }
}

#[must_use]
pub fn slot_projection(slot: crate::format::SlotFact) -> SlotProjection {
    SlotProjection {
        slot_id: slot.slot_id(),
        offset: slot.offset(),
        length: slot.length(),
        record_type: slot.record_type().as_str(),
        record_type_ordinal: slot.record_type().ordinal(),
        bytes: BytesWithheldProjection {
            state: "bytes-withheld",
        },
    }
}

#[must_use]
pub fn oos_chain_projection(chain: OosChainView) -> OosChainProjection {
    OosChainProjection {
        head: oid_projection(chain.head),
        total_data_length: chain
            .total_data_length
            .map_or(OptionalCountProjection::Unknown, |value| {
                OptionalCountProjection::Known(value.to_string())
            }),
        validated_payload_bytes: chain.validated_payload_bytes.to_string(),
        complete: chain.complete,
        chunks: chain
            .chunks
            .into_iter()
            .map(|chunk| OosChunkProjection {
                oid: oid_projection(chunk.oid),
                total_data_length: chunk.total_data_length.to_string(),
                chunk_index: chunk.chunk_index.to_string(),
                next: chunk.next.map_or(OosNextProjection::Terminal, |oid| {
                    OosNextProjection::Link {
                        oid: oid_projection(oid),
                    }
                }),
                payload_offset: chunk.payload_offset,
                payload_length: chunk.payload_length,
                bytes: BytesWithheldProjection {
                    state: "bytes-withheld",
                },
            })
            .collect(),
        diagnostic: chain.diagnostic_rule.map_or(
            OptionalTextProjection::Unknown,
            OptionalTextProjection::Known,
        ),
        bytes: BytesWithheldProjection {
            state: "bytes-withheld",
        },
    }
}

#[must_use]
pub fn overflow_chain_projection(chain: OverflowChainView) -> OverflowChainProjection {
    OverflowChainProjection {
        source: oid_projection(chain.source),
        head: optional_vpid_projection(chain.head),
        total_data_length: chain
            .total_data_length
            .map_or(OptionalCountProjection::Unknown, |value| {
                OptionalCountProjection::Known(value.to_string())
            }),
        validated_payload_bytes: chain.validated_payload_bytes.to_string(),
        complete: chain.complete,
        pages: chain
            .pages
            .into_iter()
            .map(|page| OverflowPageProjection {
                vol_id: page.vpid.vol_id.get(),
                page_id: page.vpid.page_id.get(),
                role: if page.head { "head" } else { "continuation" },
                next: optional_vpid_projection(page.next),
                payload_offset: page.payload_offset,
                payload_length: page.payload_length,
                bytes: BytesWithheldProjection {
                    state: "bytes-withheld",
                },
            })
            .collect(),
        diagnostic: chain.diagnostic_rule.map_or(
            OptionalTextProjection::Unknown,
            OptionalTextProjection::Known,
        ),
        bytes: BytesWithheldProjection {
            state: "bytes-withheld",
        },
    }
}

const fn oid_projection(oid: crate::model::Oid) -> OidProjection {
    OidProjection {
        vol_id: oid.vol_id.get(),
        page_id: oid.page_id.get(),
        slot_id: oid.slot_id.get(),
    }
}

#[must_use]
pub const fn outcome_name(outcome: InspectionOutcome) -> &'static str {
    match outcome {
        InspectionOutcome::Success => "success",
        InspectionOutcome::SuccessLimited => "success-limited",
        InspectionOutcome::Findings => "findings",
        InspectionOutcome::Incomplete => "incomplete",
        InspectionOutcome::Fatal => "fatal",
    }
}

#[must_use]
pub fn snapshot_id_hex(snapshot_id: SnapshotId) -> String {
    let mut output = String::with_capacity(32);
    for byte in snapshot_id.as_bytes() {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn coverage_projection(coverage: CoverageRecord) -> CoverageProjection {
    CoverageProjection {
        facet: coverage.facet,
        coverage: coverage_name(coverage.coverage),
        evaluated: coverage.evaluated.to_string(),
        conclusive: coverage.conclusive.to_string(),
        total: coverage
            .trusted_total
            .map_or(CountProjection::Unknown, |total| {
                CountProjection::Known(total.to_string())
            }),
        stop_reason: coverage.stop_reason,
    }
}

fn diagnostic_projection(diagnostic: DiagnosticRecord) -> DiagnosticProjection {
    DiagnosticProjection {
        code: diagnostic.code,
        severity: diagnostic.severity,
        message: diagnostic.message,
        subject: diagnostic.subject,
        rule: diagnostic.rule,
    }
}

const fn validity_name(validity: SnapshotValidity) -> &'static str {
    match validity {
        SnapshotValidity::Valid => "valid",
        SnapshotValidity::Invalidated => "invalidated",
    }
}

const fn coverage_name(coverage: Coverage) -> &'static str {
    match coverage {
        Coverage::NotRequested => "not-requested",
        Coverage::Partial => "partial",
        Coverage::Complete => "complete",
    }
}

const fn purpose_name(purpose: VolumePurpose) -> &'static str {
    match purpose {
        VolumePurpose::PermanentData => "permanent-data",
        VolumePurpose::TemporaryData => "temporary-data",
    }
}

const fn volume_type_name(volume_type: VolumeType) -> &'static str {
    match volume_type {
        VolumeType::Permanent => "permanent",
        VolumeType::Temporary => "temporary",
    }
}

const fn allocation_name(allocation: PageAllocationClass) -> &'static str {
    match allocation {
        PageAllocationClass::SystemMetadata => "system-metadata",
        PageAllocationClass::Unreserved => "unreserved",
        PageAllocationClass::ReservedUnallocated => "reserved-unallocated",
        PageAllocationClass::Allocated => "allocated",
    }
}

const fn availability_name(availability: Availability) -> &'static str {
    match availability {
        Availability::Available => "available",
        Availability::Unreadable => "unreadable",
        Availability::Unsupported => "unsupported",
        Availability::EncryptedOpaque => "encrypted-opaque",
    }
}

const fn tde_state_name(state: TdeInspectionState) -> &'static str {
    match state {
        TdeInspectionState::NotEncrypted => "not-encrypted",
        TdeInspectionState::Decrypted => "decrypted",
        TdeInspectionState::EncryptedOpaque => "encrypted-opaque",
        TdeInspectionState::KeyError => "key-error",
        TdeInspectionState::DecryptedInvalid => "decrypted-invalid",
        TdeInspectionState::InvalidFlags => "invalid-flags",
    }
}

#[allow(dead_code)]
const fn _page_type_is_stable(_page_type: PageType) {}
