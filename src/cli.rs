//! Command-line adapter over the inspection seam.

use std::ffi::OsString;
use std::fmt;
use std::io::{self, IsTerminal, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;

use crate::diagnostics::InspectionOutcome;
use crate::follow::FollowConfig;
use crate::inspection::{
    CancelToken, GraphView, Inspection, OpenFailure, OpenRequest, OperationError, ProgressObserver,
    QueryError, ResourcePolicy, RevisionSelector, ScanPhase, ScanProgress, SourceMode,
};
use crate::model::{FileId, Oid, PageId, SectorId, SlotId, Vfid, VolId, Vpid};
use crate::projection::{
    AttributeNameProjection, AttributeValueProjection, ClassRepresentationProjection,
    DataProjection, RecordInterpretationProjection, ResultDocument,
    class_representation_projection, deep_page_projection, file_header_projection,
    oos_chain_projection, overflow_chain_projection, page_projection,
    record_interpretation_projection, relocation_edge_projection, result_document,
    sector_projection, slot_projection, summary_projection, volume_projection,
};
use crate::source::InputSpec;

const DEFAULT_MEMORY_LIMIT: &str = "256MiB";
const DEFAULT_SPILL_LIMIT: &str = "2GiB";
const DEFAULT_MAX_DECODED_BYTES: &str = "256MiB";
const DEFAULT_WORKERS: u32 = 4;

// One physical page per step reaches the decoded-byte limit at this boundary.
const DEFAULT_MAX_CHAIN_STEPS: u64 = 16_384;

#[derive(Debug, Parser)]
#[command(name = "volmap", version, about = "Read-only CUBRID volume inspector")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Summary(FiniteCommand),
    Map(MapCommand),
    Inspect(InspectCommand),
    Tui(InteractiveCommand),
    Export(ExportCommand),
    Serve(ServeCommand),
    Licenses(LicensesCommand),
}

#[derive(Debug, Args)]
struct FiniteCommand {
    #[command(flatten)]
    input: InputArgs,
    #[command(flatten)]
    resources: ResourceArgs,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct MapCommand {
    #[command(flatten)]
    input: InputArgs,
    selector: Option<String>,
    #[command(flatten)]
    resources: ResourceArgs,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct InspectCommand {
    #[command(flatten)]
    input: InputArgs,
    selector: String,
    #[command(flatten)]
    resources: ResourceArgs,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct InteractiveCommand {
    #[command(flatten)]
    input: InputArgs,
    #[command(flatten)]
    resources: ResourceArgs,
}

#[derive(Debug, Args)]
struct ExportCommand {
    #[command(subcommand)]
    command: ExportSubcommand,
}

#[derive(Debug, Subcommand)]
enum ExportSubcommand {
    Html(HtmlExportCommand),
}

#[derive(Debug, Args)]
struct HtmlExportCommand {
    #[command(flatten)]
    input: InputArgs,
    #[arg(long)]
    output: PathBuf,
    #[arg(
        long,
        default_value_t = crate::export::DEFAULT_MAX_HTML_BYTES,
        value_parser = parse_byte_quantity
    )]
    max_html_bytes: u64,
    #[arg(long)]
    enrich: Vec<String>,
    #[command(flatten)]
    resources: ResourceArgs,
}

#[derive(Debug, Args)]
struct ServeCommand {
    #[command(flatten)]
    input: InputArgs,
    #[command(flatten)]
    resources: ResourceArgs,
    #[arg(long, default_value = "127.0.0.1:0")]
    listen: SocketAddr,
    /// Watch the input and publish a new generation when it changes. On by
    /// default, and accepted explicitly so a script can state the intent.
    #[arg(long, overrides_with = "no_follow")]
    follow: bool,
    /// Hold one immutable reading for the life of the process, so a changed
    /// input invalidates the session instead of advancing it.
    #[arg(long, overrides_with = "follow")]
    no_follow: bool,
    /// How often the input fingerprint manifest is read, in milliseconds.
    #[arg(long, default_value_t = 500, value_parser = clap::value_parser!(u64).range(50..=60_000))]
    follow_interval_ms: u64,
    /// How many recent generations stay addressable, so a collection load
    /// finishes on the generation it started on.
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u16).range(1..=64))]
    follow_retain: u16,
}

#[derive(Clone, Copy, Debug, Args)]
struct LicensesCommand {
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("snapshot-input")
        .required(true)
        .multiple(false)
        .args(["database", "vinf"])
))]
struct InputArgs {
    #[arg(long)]
    database: Option<String>,
    #[arg(long, requires = "database", conflicts_with = "vinf")]
    databases_file: Option<PathBuf>,
    #[arg(long)]
    vinf: Option<PathBuf>,
    #[arg(long, requires = "vinf", conflicts_with = "database")]
    volume_root: Option<PathBuf>,
    #[arg(long)]
    tde_keys_file: Option<PathBuf>,
}

impl InputArgs {
    fn input_spec(&self) -> Result<InputSpec, CliError> {
        match (&self.database, &self.vinf) {
            (Some(name), None) => Ok(InputSpec::Database {
                name: name.clone(),
                databases_file: self.databases_file.clone(),
            }),
            (None, Some(path)) => Ok(InputSpec::Vinf {
                path: path.clone(),
                volume_root: self.volume_root.clone(),
            }),
            _ => Err(CliError::Usage(
                "exactly one snapshot input is required".to_owned(),
            )),
        }
    }
}

#[derive(Clone, Debug, Args)]
struct ResourceArgs {
    #[arg(long, default_value = DEFAULT_MEMORY_LIMIT, value_parser = parse_byte_quantity)]
    memory_limit: u64,
    #[arg(long, default_value = DEFAULT_SPILL_LIMIT, value_parser = parse_byte_quantity)]
    spill_limit: u64,
    #[arg(long, default_value_t = DEFAULT_WORKERS, value_parser = parse_positive_u32)]
    workers: u32,
    #[arg(long, default_value_t = DEFAULT_MAX_CHAIN_STEPS, value_parser = parse_positive_u64)]
    max_chain_steps: u64,
    #[arg(long, default_value = DEFAULT_MAX_DECODED_BYTES, value_parser = parse_byte_quantity)]
    max_decoded_bytes: u64,
    #[arg(long)]
    spill_directory: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = ProgressChoice::Auto)]
    progress: ProgressChoice,
}

impl ResourceArgs {
    fn policy(&self) -> Result<ResourcePolicy, CliError> {
        ResourcePolicy::new(
            self.memory_limit,
            self.spill_limit,
            self.workers,
            self.max_chain_steps,
            self.max_decoded_bytes,
        )
        .map_err(|error| CliError::Usage(error.to_string()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ProgressChoice {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
    Jsonl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ColorChoice {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum DiagnosticsChoice {
    Summary,
    Full,
}

#[derive(Clone, Copy, Debug, Args)]
struct OutputArgs {
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    color: ColorChoice,
    #[arg(long, value_enum, default_value_t = DiagnosticsChoice::Summary)]
    diagnostics: DiagnosticsChoice,
    #[arg(long, default_value_t = 1)]
    schema_version: u32,
}

impl OutputArgs {
    fn validate(self) -> Result<(), CliError> {
        if self.schema_version != 1 {
            return Err(CliError::Usage("unsupported schema version".to_owned()));
        }
        if self.format != OutputFormat::Human
            && (self.color != ColorChoice::Auto || self.diagnostics != DiagnosticsChoice::Summary)
        {
            return Err(CliError::Usage(
                "--color and --diagnostics are human-output options".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntitySelector {
    Volume(VolId),
    Sector(VolId, SectorId),
    File(Vfid),
    Page(Vpid),
    Slot(Oid),
    Oos(Oid),
}

impl EntitySelector {
    fn parse(value: &str) -> Result<Self, CliError> {
        let fields: Vec<&str> = value.split(':').collect();
        match fields.as_slice() {
            ["volume", vol] => Ok(Self::Volume(parse_vol_id(vol)?)),
            ["sector", vol, sector] => {
                Ok(Self::Sector(parse_vol_id(vol)?, parse_sector_id(sector)?))
            }
            ["file", vol, file] => Ok(Self::File(Vfid::new(
                parse_vol_id(vol)?,
                FileId::new(parse_i32(file, "file identifier")?)
                    .map_err(|error| CliError::Usage(error.to_string()))?,
            ))),
            ["page", vol, page] => Ok(Self::Page(Vpid::new(
                parse_vol_id(vol)?,
                parse_page_id(page)?,
            ))),
            ["slot", vol, page, slot] => Ok(Self::Slot(parse_oid(vol, page, slot)?)),
            ["oos", vol, page, slot] => Ok(Self::Oos(parse_oid(vol, page, slot)?)),
            _ => Err(CliError::Usage("invalid entity selector".to_owned())),
        }
    }
}

pub fn run_from<I, T>(arguments: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(arguments) {
        Ok(cli) => cli,
        Err(error) => {
            let code = if error.use_stderr() { 2 } else { 0 };
            let _ = error.print();
            return code;
        }
    };
    match run(cli) {
        Ok(code) => code,
        Err(CliError::BrokenPipe) => 141,
        Err(error) => {
            let _ = writeln!(
                io::stderr().lock(),
                "{}",
                escape_control(&error.to_string())
            );
            error.exit_code()
        }
    }
}

#[allow(clippy::too_many_lines)]
fn run(cli: Cli) -> Result<i32, CliError> {
    match cli.command {
        Command::Summary(command) => run_summary(&command),
        Command::Map(command) => run_map(command),
        Command::Inspect(command) => run_inspect(command),
        Command::Licenses(command) => run_licenses(command),
        Command::Tui(command) => {
            let (view, overview) =
                open_view(&command.input, &command.resources, OutputFormat::Human)?;
            crate::tui::run(&view).map_err(|error| CliError::OpenAdapter(error.to_string()))?;
            Ok(outcome_exit(overview.outcome))
        }
        Command::Export(command) => match command.command {
            ExportSubcommand::Html(command) => {
                if command.max_html_bytes > crate::export::HARD_MAX_HTML_BYTES {
                    return Err(CliError::Usage(format!(
                        "--max-html-bytes cannot exceed {}",
                        crate::export::HARD_MAX_HTML_BYTES
                    )));
                }
                let mut enrichments = command
                    .enrich
                    .iter()
                    .map(|value| EntitySelector::parse(value))
                    .collect::<Result<Vec<_>, _>>()?;
                if enrichments.iter().any(|selector| {
                    !matches!(
                        selector,
                        EntitySelector::Page(_) | EntitySelector::Slot(_) | EntitySelector::Oos(_)
                    )
                }) {
                    return Err(CliError::Usage(
                        "--enrich accepts only page, slot, or OOS selectors".to_owned(),
                    ));
                }
                enrichments.sort_by_key(entity_order_key);
                enrichments.dedup();
                let enrichment_policy = command.resources.clone().policy()?;
                let (mut view, _) =
                    open_view(&command.input, &command.resources, OutputFormat::Human)?;
                for selector in enrichments {
                    match selector {
                        EntitySelector::Page(vpid) => {
                            view = view
                                .enrich_page(vpid, enrichment_policy, &CancelToken::new())
                                .map_err(|error| CliError::OpenAdapter(error.to_string()))?;
                        }
                        EntitySelector::Slot(oid) => {
                            let vpid = Vpid::new(oid.vol_id, oid.page_id);
                            view = view
                                .enrich_page(vpid, enrichment_policy, &CancelToken::new())
                                .map_err(|error| CliError::OpenAdapter(error.to_string()))?;
                            let selected_slot = view
                                .deep_page(vpid)
                                .and_then(|deep| deep.slotted)
                                .and_then(|slotted| {
                                    slotted
                                        .slots()
                                        .get(usize::try_from(oid.slot_id.get()).ok()?)
                                        .copied()
                                })
                                .ok_or(CliError::Query(QueryError::EntityNotFound))?;
                            if selected_slot.record_type() == crate::format::RecordType::BigOne {
                                view = view
                                    .enrich_bigone(oid, enrichment_policy, &CancelToken::new())
                                    .map_err(|error| CliError::OpenAdapter(error.to_string()))?;
                            } else if selected_slot.record_type()
                                == crate::format::RecordType::Relocation
                            {
                                view = view
                                    .enrich_relocation(oid, enrichment_policy, &CancelToken::new())
                                    .map_err(|error| CliError::OpenAdapter(error.to_string()))?;
                            }
                        }
                        EntitySelector::Oos(oid) => {
                            view = view
                                .enrich_oos(oid, enrichment_policy, &CancelToken::new())
                                .map_err(|error| CliError::OpenAdapter(error.to_string()))?;
                        }
                        EntitySelector::Volume(_)
                        | EntitySelector::Sector(_, _)
                        | EntitySelector::File(_) => unreachable!("validated above"),
                    }
                }
                let overview = view.overview();
                crate::export::export_html(&view, &command.output, command.max_html_bytes)
                    .map_err(|error| CliError::OpenAdapter(error.to_string()))?;
                Ok(outcome_exit(overview.outcome))
            }
        },
        Command::Serve(command) => {
            let policy = command.resources.clone().policy()?;
            // Follow is the default, and the only command that follows. The
            // offline commands keep the immutable contract untouched.
            let following = !command.no_follow;
            let mode = if following {
                SourceMode::Live
            } else {
                SourceMode::Immutable
            };
            let (view, overview, request) = open_reading(
                &command.input,
                &command.resources,
                OutputFormat::Human,
                mode,
            )?;
            let follow = following.then(|| FollowConfig {
                poll_interval: Duration::from_millis(command.follow_interval_ms),
                retain: usize::from(command.follow_retain),
                ..FollowConfig::default()
            });
            crate::web::serve(
                view,
                crate::web::ServeOptions {
                    listen: command.listen,
                    policy,
                    request,
                    follow,
                },
            )
            .map_err(|error| CliError::OpenAdapter(error.to_string()))?;
            Ok(outcome_exit(overview.outcome))
        }
    }
}

fn entity_order_key(selector: &EntitySelector) -> (u8, i16, i32, i32) {
    match *selector {
        EntitySelector::Page(vpid) => (0, vpid.vol_id.get(), vpid.page_id.get(), 0),
        EntitySelector::Slot(oid) => (
            1,
            oid.vol_id.get(),
            oid.page_id.get(),
            i32::from(oid.slot_id.get()),
        ),
        EntitySelector::Oos(oid) => (
            2,
            oid.vol_id.get(),
            oid.page_id.get(),
            i32::from(oid.slot_id.get()),
        ),
        EntitySelector::Volume(vol_id) => (3, vol_id.get(), 0, 0),
        EntitySelector::Sector(vol_id, sector_id) => (4, vol_id.get(), sector_id.get(), 0),
        EntitySelector::File(vfid) => (5, vfid.vol_id.get(), vfid.file_id.get(), 0),
    }
}

fn run_summary(command: &FiniteCommand) -> Result<i32, CliError> {
    command.output.validate()?;
    let (view, overview) = open_view(&command.input, &command.resources, command.output.format)?;
    let _ = view;
    let document = result_document(
        "summary",
        None,
        &overview,
        DataProjection::Summary {
            overview: summary_projection(&overview),
        },
    );
    write_document(&document, command.output.format)?;
    Ok(outcome_exit(overview.outcome))
}

fn run_map(command: MapCommand) -> Result<i32, CliError> {
    command.output.validate()?;
    let enrichment_policy = command.resources.clone().policy()?;
    let selector = command
        .selector
        .as_deref()
        .map(EntitySelector::parse)
        .transpose()?;
    if matches!(
        selector,
        Some(EntitySelector::Page(_) | EntitySelector::Slot(_) | EntitySelector::Oos(_))
    ) {
        return Err(CliError::Usage(
            "map accepts only volume, sector, or file selectors".to_owned(),
        ));
    }
    let (mut view, _) = open_view(&command.input, &command.resources, command.output.format)?;
    if let Some(EntitySelector::File(vfid)) = selector {
        view = match view.enrich_file(vfid, enrichment_policy, &CancelToken::new()) {
            Ok(enriched) => enriched,
            Err(OperationError::Query(QueryError::EntityNotFound)) => {
                let overview = view.overview();
                return write_command_error(
                    command.output.format,
                    "map",
                    command.selector.as_deref().unwrap_or_default(),
                    &overview,
                    "entity-not-found",
                );
            }
            Err(error) => return Err(operation_error(error)),
        };
    }
    let overview = view.overview();
    let (volumes, sectors) = match map_data(&view, selector) {
        Ok(data) => data,
        Err(CliError::Query(QueryError::EntityNotFound)) => {
            return write_command_error(
                command.output.format,
                "map",
                command.selector.as_deref().unwrap_or_default(),
                &overview,
                "entity-not-found",
            );
        }
        Err(error) => return Err(error),
    };
    let document = result_document(
        "map",
        command.selector,
        &overview,
        DataProjection::Map {
            volumes,
            sectors,
            deep_pages: Vec::new(),
            oos_chains: Vec::new(),
            overflow_chains: Vec::new(),
            relocation_edges: Vec::new(),
        },
    );
    write_document(&document, command.output.format)?;
    Ok(outcome_exit(overview.outcome))
}

#[allow(clippy::too_many_lines)]
fn run_inspect(command: InspectCommand) -> Result<i32, CliError> {
    command.output.validate()?;
    let selector = EntitySelector::parse(&command.selector)?;
    let enrichment_policy = command.resources.clone().policy()?;
    let (mut view, _) = open_view(&command.input, &command.resources, command.output.format)?;
    let data = (|| -> Result<DataProjection, CliError> {
        Ok(match selector {
            EntitySelector::Volume(vol_id) => DataProjection::InspectVolume {
                volume: volume_projection(view.volume(vol_id).map_err(CliError::Query)?),
            },
            EntitySelector::Sector(vol_id, sector_id) => DataProjection::InspectSector {
                sector: sector_projection(view.sector(vol_id, sector_id).map_err(CliError::Query)?),
            },
            EntitySelector::Page(vpid) => {
                view = view
                    .enrich_page(vpid, enrichment_policy, &CancelToken::new())
                    .map_err(operation_error)?;
                DataProjection::InspectPage {
                    page: page_projection(view.page(vpid).map_err(CliError::Query)?),
                    deep: deep_page_projection(view.deep_page(vpid)),
                }
            }
            EntitySelector::File(vfid) => {
                let header_page = Vpid::new(
                    vfid.vol_id,
                    PageId::new(vfid.file_id.get())
                        .map_err(|error| CliError::Internal(error.to_string()))?,
                );
                view = view
                    .enrich_file(vfid, enrichment_policy, &CancelToken::new())
                    .map_err(operation_error)?;
                let header = view
                    .deep_page(header_page)
                    .and_then(|deep| deep.file_header)
                    .ok_or(CliError::Query(QueryError::EntityNotFound))?;
                DataProjection::InspectFile {
                    file: file_header_projection(header),
                }
            }
            EntitySelector::Slot(oid) => {
                let vpid = Vpid::new(oid.vol_id, oid.page_id);
                view = view
                    .enrich_page(vpid, enrichment_policy, &CancelToken::new())
                    .map_err(operation_error)?;
                let deep = view
                    .deep_page(vpid)
                    .ok_or(CliError::Query(QueryError::EntityNotFound))?;
                let selected_slot = deep
                    .slotted
                    .as_ref()
                    .and_then(|slotted| {
                        slotted
                            .slots()
                            .get(usize::try_from(oid.slot_id.get()).ok()?)
                    })
                    .copied()
                    .ok_or(CliError::Query(QueryError::EntityNotFound))?;
                let overflow_chain =
                    if selected_slot.record_type() == crate::format::RecordType::BigOne {
                        view = view
                            .enrich_bigone(oid, enrichment_policy, &CancelToken::new())
                            .map_err(operation_error)?;
                        view.overflow_chain(oid).map(overflow_chain_projection)
                    } else {
                        None
                    };
                let relocation_edge =
                    if selected_slot.record_type() == crate::format::RecordType::Relocation {
                        view = view
                            .enrich_relocation(oid, enrichment_policy, &CancelToken::new())
                            .map_err(operation_error)?;
                        view.relocation_edge(oid)
                            .map(relocation_edge_projection)
                            .map(Box::new)
                    } else {
                        None
                    };
                // Interpretation is page-granular: one selected slot enriches
                // every home record of its page, which is what makes the
                // class-record read pay for itself.
                view = view
                    .enrich_record_page(vpid, enrichment_policy, &CancelToken::new())
                    .map_err(operation_error)?;
                // A relocation's values live in its target, so follow the edge
                // the graph just published rather than reporting nothing.
                let interpreted_oid = view
                    .relocation_edge(oid)
                    .and_then(|edge| edge.target)
                    .unwrap_or(oid);
                if interpreted_oid != oid {
                    let target_page = Vpid::new(interpreted_oid.vol_id, interpreted_oid.page_id);
                    view = view
                        .enrich_record_page(target_page, enrichment_policy, &CancelToken::new())
                        .map_err(operation_error)?;
                }
                // Read the schema the record actually resolved through, rather
                // than re-deriving its identity from the projection.
                let interpreted = view.record_interpretation(interpreted_oid);
                let class_representation = interpreted
                    .as_ref()
                    .and_then(|interpretation| {
                        view.class_representation(
                            interpretation.class_oid,
                            interpretation.representation_id,
                        )
                    })
                    .map(class_representation_projection)
                    .map(Box::new);
                let interpretation = interpreted
                    .map(record_interpretation_projection)
                    .map(Box::new);
                DataProjection::InspectSlot {
                    page: page_projection(view.page(vpid).map_err(CliError::Query)?),
                    deep: deep_page_projection(Some(deep)),
                    selected_slot: slot_projection(selected_slot),
                    overflow_chain,
                    relocation_edge,
                    interpretation,
                    class_representation,
                }
            }
            EntitySelector::Oos(oid) => {
                view = view
                    .enrich_oos(oid, enrichment_policy, &CancelToken::new())
                    .map_err(operation_error)?;
                DataProjection::InspectOos {
                    chain: oos_chain_projection(
                        view.oos_chain(oid)
                            .ok_or(CliError::Query(QueryError::EntityNotFound))?,
                    ),
                }
            }
        })
    })();
    let data = match data {
        Ok(data) => data,
        Err(CliError::Query(QueryError::EntityNotFound)) => {
            let overview = view.overview();
            return write_command_error(
                command.output.format,
                "inspect",
                &command.selector,
                &overview,
                "entity-not-found",
            );
        }
        Err(error) => return Err(error),
    };
    let overview = view.overview();
    let document = result_document("inspect", Some(command.selector), &overview, data);
    write_document(&document, command.output.format)?;
    Ok(outcome_exit(overview.outcome))
}

fn run_licenses(command: LicensesCommand) -> Result<i32, CliError> {
    match command.format {
        OutputFormat::Human => write_stdout(crate::notices::THIRD_PARTY_NOTICES.as_bytes())?,
        OutputFormat::Json => {
            #[derive(Serialize)]
            struct NoticeDocument<'a> {
                schema: &'a str,
                schema_version: u32,
                #[serde(rename = "notice")]
                text: &'a str,
            }
            let bytes = serde_json::to_vec_pretty(&NoticeDocument {
                schema: "volmap.licenses",
                schema_version: 1,
                text: crate::notices::THIRD_PARTY_NOTICES,
            })
            .map_err(|error| CliError::Internal(error.to_string()))?;
            write_stdout(&bytes)?;
            write_stdout(b"\n")?;
        }
        OutputFormat::Jsonl => {
            return Err(CliError::Usage(
                "licenses supports human or json output".to_owned(),
            ));
        }
    }
    Ok(0)
}

fn open_view(
    input: &InputArgs,
    resources: &ResourceArgs,
    format: OutputFormat,
) -> Result<(GraphView, crate::inspection::OverviewView), CliError> {
    open_reading(input, resources, format, SourceMode::Immutable)
        .map(|(view, overview, _)| (view, overview))
}

/// Reads the input once and hands back the request that produced the reading,
/// so a caller that intends to read the same input again can do so.
fn open_reading(
    input: &InputArgs,
    resources: &ResourceArgs,
    format: OutputFormat,
    mode: SourceMode,
) -> Result<(GraphView, crate::inspection::OverviewView, OpenRequest), CliError> {
    let input_spec = input.input_spec()?;
    let progress_choice = resources.progress;
    let policy = resources.policy()?;
    let cancel = CancelToken::new();
    let request = OpenRequest {
        input: input_spec,
        tde_keys_file: input.tde_keys_file.clone(),
        spill_directory: resources.spill_directory.clone(),
    };
    let show_progress = match progress_choice {
        ProgressChoice::Never => false,
        ProgressChoice::Always => true,
        ProgressChoice::Auto => format == OutputFormat::Human && io::stderr().is_terminal(),
    };
    let mut observer = StderrProgress::default();
    let open = match mode {
        SourceMode::Immutable => Inspection::open,
        SourceMode::Live => Inspection::open_live,
    };
    let inspection = if show_progress {
        open(&request, policy, &cancel, Some(&mut observer))
    } else {
        open(&request, policy, &cancel, None)
    }
    .map_err(CliError::Open)?;
    let view = inspection
        .view(RevisionSelector::Latest)
        .map_err(|error| CliError::Internal(error.to_string()))?;
    let overview = view.overview();
    Ok((view, overview, request))
}

fn map_data(
    view: &GraphView,
    selector: Option<EntitySelector>,
) -> Result<
    (
        Vec<crate::projection::VolumeProjection>,
        Vec<crate::projection::SectorProjection>,
    ),
    CliError,
> {
    let selected_volumes = match selector {
        None => view.volumes(),
        Some(EntitySelector::Volume(vol_id) | EntitySelector::Sector(vol_id, _)) => {
            vec![view.volume(vol_id).map_err(CliError::Query)?]
        }
        Some(EntitySelector::File(vfid)) => {
            vec![view.volume(vfid.vol_id).map_err(CliError::Query)?]
        }
        Some(EntitySelector::Page(_) | EntitySelector::Slot(_) | EntitySelector::Oos(_)) => {
            return Err(CliError::Usage("invalid map selector".to_owned()));
        }
    };
    let mut sectors = Vec::new();
    for volume in &selected_volumes {
        match selector {
            Some(EntitySelector::Sector(_, sector_id)) => sectors.push(sector_projection(
                view.sector(volume.vol_id, sector_id)
                    .map_err(CliError::Query)?,
            )),
            Some(EntitySelector::File(vfid)) => {
                let sector_ids = view
                    .file_pages(vfid)
                    .map_err(CliError::Query)?
                    .into_iter()
                    .map(|page| page.sector_id)
                    .collect::<std::collections::BTreeSet<_>>();
                for sector_id in sector_ids {
                    sectors.push(sector_projection(
                        view.sector(volume.vol_id, sector_id)
                            .map_err(CliError::Query)?,
                    ));
                }
            }
            _ => {
                for raw_sector in 0..volume.total_sectors {
                    let sector_id = SectorId::new(
                        i32::try_from(raw_sector)
                            .map_err(|_| CliError::Internal("sector range overflow".to_owned()))?,
                    )
                    .map_err(|error| CliError::Internal(error.to_string()))?;
                    sectors.push(sector_projection(
                        view.sector(volume.vol_id, sector_id)
                            .map_err(CliError::Query)?,
                    ));
                }
            }
        }
    }
    Ok((
        selected_volumes
            .into_iter()
            .map(volume_projection)
            .collect(),
        sectors,
    ))
}

fn write_document(document: &ResultDocument, format: OutputFormat) -> Result<(), CliError> {
    match format {
        OutputFormat::Human => write_stdout(render_human(document).as_bytes()),
        OutputFormat::Json => {
            let mut bytes = serde_json::to_vec_pretty(document)
                .map_err(|error| CliError::Internal(error.to_string()))?;
            bytes.push(b'\n');
            write_stdout(&bytes)
        }
        OutputFormat::Jsonl => write_jsonl(document),
    }
}

fn operation_error(error: OperationError) -> CliError {
    match error {
        OperationError::Query(error) => CliError::Query(error),
        error => CliError::OpenAdapter(error.to_string()),
    }
}

#[derive(Serialize)]
struct CommandErrorDetail<'a> {
    code: &'static str,
    selector: &'a str,
}

#[derive(Serialize)]
struct CommandErrorDocument<'a> {
    schema: &'static str,
    schema_version: u32,
    document_type: &'static str,
    tool: &'a crate::projection::ToolProjection,
    command: &'a crate::projection::CommandProjection,
    snapshot: &'a crate::projection::SnapshotProjection,
    error: CommandErrorDetail<'a>,
}

fn write_command_error(
    format: OutputFormat,
    command: &str,
    selector: &str,
    overview: &crate::inspection::OverviewView,
    code: &'static str,
) -> Result<i32, CliError> {
    if format == OutputFormat::Human {
        return Err(CliError::Query(QueryError::EntityNotFound));
    }
    let shell = result_document(
        command,
        Some(selector.to_owned()),
        overview,
        DataProjection::Summary {
            overview: summary_projection(overview),
        },
    );
    let detail = CommandErrorDetail { code, selector };
    match format {
        OutputFormat::Human => unreachable!("handled above"),
        OutputFormat::Json => {
            let mut bytes = serde_json::to_vec_pretty(&CommandErrorDocument {
                schema: shell.schema,
                schema_version: shell.schema_version,
                document_type: "command-error",
                tool: &shell.tool,
                command: &shell.command,
                snapshot: &shell.snapshot,
                error: detail,
            })
            .map_err(|error| CliError::Internal(error.to_string()))?;
            bytes.push(b'\n');
            write_stdout(&bytes)?;
        }
        OutputFormat::Jsonl => {
            let mut sequence = 0;
            write_jsonl_record(
                &shell,
                &mut sequence,
                "header",
                &(&shell.tool, &shell.command, &shell.snapshot),
            )?;
            write_jsonl_record(&shell, &mut sequence, "command-error", &detail)?;
            write_jsonl_record(&shell, &mut sequence, "completion", &("request-error", "2"))?;
        }
    }
    Ok(2)
}

fn write_jsonl(document: &ResultDocument) -> Result<(), CliError> {
    let mut sequence = 0_u64;
    write_jsonl_record(
        document,
        &mut sequence,
        "header",
        &(&document.tool, &document.command, &document.snapshot),
    )?;
    match &document.data {
        DataProjection::Summary { overview } => {
            write_jsonl_record(document, &mut sequence, "overview", overview)?;
        }
        DataProjection::Map {
            volumes,
            sectors,
            deep_pages,
            oos_chains,
            overflow_chains,
            relocation_edges,
        } => {
            for volume in volumes {
                write_jsonl_record(document, &mut sequence, "volume", volume)?;
            }
            for sector in sectors {
                write_jsonl_record(document, &mut sequence, "sector", sector)?;
            }
            for page in deep_pages {
                write_jsonl_record(document, &mut sequence, "deep-page", page)?;
            }
            for chain in oos_chains {
                write_jsonl_record(document, &mut sequence, "oos-chain", chain)?;
            }
            for chain in overflow_chains {
                write_jsonl_record(document, &mut sequence, "overflow-chain", chain)?;
            }
            for edge in relocation_edges {
                write_jsonl_record(document, &mut sequence, "relocation-edge", edge)?;
            }
        }
        DataProjection::InspectVolume { volume } => {
            write_jsonl_record(document, &mut sequence, "volume", volume)?;
        }
        DataProjection::InspectSector { sector } => {
            write_jsonl_record(document, &mut sequence, "sector", sector)?;
        }
        DataProjection::InspectFile { file } => {
            write_jsonl_record(document, &mut sequence, "file", file)?;
        }
        DataProjection::InspectPage { page, deep } => {
            write_jsonl_record(document, &mut sequence, "page", page)?;
            write_jsonl_record(document, &mut sequence, "deep-page", deep)?;
        }
        DataProjection::InspectSlot {
            page,
            deep,
            selected_slot,
            overflow_chain,
            relocation_edge,
            interpretation,
            class_representation,
        } => {
            write_jsonl_record(document, &mut sequence, "page", page)?;
            write_jsonl_record(document, &mut sequence, "deep-page", deep)?;
            write_jsonl_record(document, &mut sequence, "slot", selected_slot)?;
            write_jsonl_slot_details(
                document,
                &mut sequence,
                overflow_chain.as_ref(),
                relocation_edge.as_deref(),
                class_representation.as_deref(),
                interpretation.as_deref(),
            )?;
        }
        DataProjection::InspectOos { chain } => {
            write_jsonl_record(document, &mut sequence, "oos-chain", chain)?;
        }
    }
    for coverage in &document.coverage {
        write_jsonl_record(document, &mut sequence, "coverage", coverage)?;
    }
    for diagnostic in &document.diagnostics {
        write_jsonl_record(document, &mut sequence, "diagnostic", diagnostic)?;
    }
    let emitted_records = sequence;
    write_jsonl_record(
        document,
        &mut sequence,
        "completion",
        &(document.outcome, emitted_records),
    )
}

/// Emits the optional records that accompany one selected slot.
fn write_jsonl_slot_details(
    document: &ResultDocument,
    sequence: &mut u64,
    overflow_chain: Option<&crate::projection::OverflowChainProjection>,
    relocation_edge: Option<&crate::projection::RelocationEdgeProjection>,
    class_representation: Option<&ClassRepresentationProjection>,
    interpretation: Option<&RecordInterpretationProjection>,
) -> Result<(), CliError> {
    if let Some(chain) = overflow_chain {
        write_jsonl_record(document, sequence, "overflow-chain", chain)?;
    }
    if let Some(edge) = relocation_edge {
        write_jsonl_record(document, sequence, "relocation-edge", edge)?;
    }
    if let Some(representation) = class_representation {
        write_jsonl_record(document, sequence, "class-representation", representation)?;
    }
    if let Some(interpretation) = interpretation {
        write_jsonl_record(document, sequence, "record-interpretation", interpretation)?;
    }
    Ok(())
}

fn write_jsonl_record<T: Serialize>(
    document: &ResultDocument,
    sequence: &mut u64,
    kind: &str,
    data: &T,
) -> Result<(), CliError> {
    #[derive(Serialize)]
    struct Record<'a, T: Serialize> {
        schema: &'a str,
        schema_version: u32,
        #[serde(rename = "record_type")]
        kind: &'a str,
        sequence: String,
        snapshot_id: &'a str,
        revision: &'a str,
        data: &'a T,
    }
    let record = serde_json::to_string(&Record {
        schema: document.schema,
        schema_version: document.schema_version,
        kind,
        sequence: sequence.to_string(),
        snapshot_id: &document.snapshot.id,
        revision: &document.snapshot.revision,
        data,
    })
    .map_err(|error| CliError::Internal(error.to_string()))?;
    write_stdout(record.as_bytes())?;
    write_stdout(b"\n")?;
    *sequence = sequence
        .checked_add(1)
        .ok_or_else(|| CliError::Internal("JSONL sequence overflow".to_owned()))?;
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn render_human(document: &ResultDocument) -> String {
    use std::fmt::Write as _;
    let mut output = String::new();
    let _ = writeln!(output, "outcome: {}", document.outcome);
    let _ = writeln!(
        output,
        "snapshot: {} revision {} ({})",
        document.snapshot.id, document.snapshot.revision, document.snapshot.validity
    );
    let _ = writeln!(output, "coverage:");
    for ledger in &document.coverage {
        let total = match &ledger.total {
            crate::projection::CountProjection::Known(value) => value.as_str(),
            crate::projection::CountProjection::Unknown => "unknown",
        };
        let _ = writeln!(
            output,
            "  {}: {} {}/{} total={}",
            ledger.facet, ledger.coverage, ledger.conclusive, ledger.evaluated, total
        );
    }
    match &document.data {
        DataProjection::Summary { overview } => {
            let _ = writeln!(output, "volumes: {}", overview.volume_count);
            let _ = writeln!(output, "sectors: {}", overview.sector_count);
            let _ = writeln!(
                output,
                "reserved sectors: {}",
                overview.reserved_sector_count
            );
            let _ = writeln!(output, "physical pages: {}", overview.physical_page_count);
            let _ = writeln!(
                output,
                "inspected page envelopes: {}",
                overview.inspected_page_envelopes
            );
            for count in &overview.page_type_counts {
                let _ = writeln!(output, "  page type {}: {}", count.page_type, count.count);
            }
        }
        DataProjection::Map {
            volumes, sectors, ..
        } => {
            for volume in volumes {
                let _ = writeln!(
                    output,
                    "volume:{} sectors={} reserved={}",
                    volume.vol_id, volume.total_sectors, volume.reserved_sectors
                );
            }
            for sector in sectors {
                let markers: String = sector
                    .pages
                    .iter()
                    .map(|page| match page.allocation {
                        "system-metadata" => 'S',
                        "reserved-unallocated" => 'r',
                        "allocated" => 'A',
                        _ => '.',
                    })
                    .collect();
                let _ = writeln!(
                    output,
                    "sector:{}:{} reserved={} {}",
                    sector.vol_id, sector.sector_id, sector.reserved, markers
                );
            }
        }
        DataProjection::InspectVolume { volume } => {
            let _ = writeln!(output, "volume:{}", volume.vol_id);
            let _ = writeln!(output, "purpose: {}", volume.purpose);
            let _ = writeln!(output, "sectors: {}", volume.total_sectors);
            let _ = writeln!(output, "reserved sectors: {}", volume.reserved_sectors);
        }
        DataProjection::InspectSector { sector } => {
            let _ = writeln!(output, "sector:{}:{}", sector.vol_id, sector.sector_id);
            for page in &sector.pages {
                let page_type = match page.page_type {
                    crate::projection::OptionalTextProjection::Known(value) => value,
                    crate::projection::OptionalTextProjection::Unknown => "unknown",
                    crate::projection::OptionalTextProjection::Unsupported => "unsupported",
                };
                let _ = writeln!(
                    output,
                    "  page:{}:{} {:<20} {}",
                    page.vol_id, page.page_id, page.allocation, page_type
                );
            }
        }
        DataProjection::InspectFile { file } => {
            let _ = writeln!(output, "file:{}:{}", file.vol_id, file.file_id);
            let _ = writeln!(output, "type: {} flags={}", file.file_type, file.flags);
            let _ = writeln!(
                output,
                "pages: total={} user={} ftab={} free={} marked-delete={}",
                file.page_total,
                file.page_user,
                file.page_ftab,
                file.page_free,
                file.page_marked_delete
            );
            let _ = writeln!(
                output,
                "sectors: total={} partial={} full={} empty={}",
                file.sector_total, file.sector_partial, file.sector_full, file.sector_empty
            );
            let _ = writeln!(output, "bytes: withheld");
        }
        DataProjection::InspectPage { page, deep } => {
            let page_type = match page.page_type {
                crate::projection::OptionalTextProjection::Known(value) => value,
                crate::projection::OptionalTextProjection::Unknown => "unknown",
                crate::projection::OptionalTextProjection::Unsupported => "unsupported",
            };
            let _ = writeln!(output, "page:{}:{}", page.vol_id, page.page_id);
            let _ = writeln!(output, "sector: {}", page.sector_id);
            let _ = writeln!(output, "allocation: {}", page.allocation);
            let _ = writeln!(output, "physical type: {page_type}");
            let _ = writeln!(output, "availability: {}", page.availability);
            let _ = writeln!(output, "TDE: {}", page.tde_state);
            match deep {
                crate::projection::DeepPageProjection::Slotted { structure } => {
                    let _ = writeln!(
                        output,
                        "slotted: anchor={} alignment={} slots={}",
                        structure.anchor,
                        structure.alignment,
                        structure.slots.len()
                    );
                    for slot in &structure.slots {
                        let _ = writeln!(
                            output,
                            "  slot:{} type={}({}) offset={} length={} bytes=withheld",
                            slot.slot_id,
                            slot.record_type,
                            slot.record_type_ordinal,
                            slot.offset,
                            slot.length
                        );
                    }
                }
                crate::projection::DeepPageProjection::EnvelopeOnly => {
                    let _ = writeln!(output, "deep detail: envelope-only");
                }
                crate::projection::DeepPageProjection::FileHeader { structure } => {
                    let _ = writeln!(
                        output,
                        "file header: {}:{} type={} pages={}",
                        structure.vol_id,
                        structure.file_id,
                        structure.file_type,
                        structure.page_total
                    );
                }
                crate::projection::DeepPageProjection::HeapHeader { structure } => {
                    let _ = writeln!(
                        output,
                        "heap header: pages={} records={} record-bytes={} unfill={}",
                        structure.estimated_pages,
                        structure.estimated_records,
                        structure.estimated_record_bytes,
                        structure.unfill_space
                    );
                }
                crate::projection::DeepPageProjection::HeapChain { structure } => {
                    let _ = writeln!(
                        output,
                        "heap chain: max-mvccid={} flags={}",
                        structure.max_mvccid, structure.flags
                    );
                }
                crate::projection::DeepPageProjection::BtreeRoot { structure } => {
                    let _ = writeln!(
                        output,
                        "btree root: level={} role={} records={} keys={} oids={} nulls={} domain-offset={} domain-length={} bytes=withheld",
                        structure.node.level,
                        structure.node.role,
                        structure.node.record_count,
                        structure.key_count,
                        structure.oid_count,
                        structure.null_count,
                        structure.domain_offset,
                        structure.domain_length
                    );
                }
                crate::projection::DeepPageProjection::BtreeNode { structure } => {
                    let _ = writeln!(
                        output,
                        "btree node: level={} role={} records={} record-bytes={} children={} overflow-keys={}",
                        structure.level,
                        structure.role,
                        structure.record_count,
                        structure.record_bytes,
                        structure.child_count,
                        structure.overflow_key_count
                    );
                }
                crate::projection::DeepPageProjection::BtreeOidOverflow { structure } => {
                    let _ = writeln!(
                        output,
                        "btree OID overflow: records={} record-bytes={} bytes=withheld",
                        structure.record_count, structure.record_bytes
                    );
                }
                crate::projection::DeepPageProjection::Catalog { structure } => {
                    let _ = writeln!(
                        output,
                        "catalog page: role={} directories={} records={} record-bytes={} bytes=withheld",
                        structure.role,
                        structure.directory_count,
                        structure.record_count,
                        structure.record_bytes
                    );
                    for directory in &structure.directories {
                        let _ = writeln!(
                            output,
                            "  directory:{} pages={} objects={} representations={}",
                            directory.slot_id,
                            directory.total_pages,
                            directory.total_objects,
                            directory.representations.len()
                        );
                    }
                }
                crate::projection::DeepPageProjection::Vacuum { structure } => {
                    let _ = writeln!(
                        output,
                        "vacuum queue: free-index={} entries={}",
                        structure.index_free,
                        structure.entries.len()
                    );
                    for entry in &structure.entries {
                        let _ = writeln!(
                            output,
                            "  block:{} flags={} start-lsa={} oldest={} newest={}",
                            entry.block_id,
                            entry.flags,
                            entry.start_lsa_word,
                            entry.oldest_visible_mvccid,
                            entry.newest_mvccid
                        );
                    }
                }
                crate::projection::DeepPageProjection::DroppedFiles { structure } => {
                    let _ = writeln!(output, "dropped files: entries={}", structure.entries.len());
                    for entry in &structure.entries {
                        let _ = writeln!(
                            output,
                            "  file:{}:{} mvccid={}",
                            entry.vol_id, entry.file_id, entry.mvccid
                        );
                    }
                }
                crate::projection::DeepPageProjection::Invalid { rule } => {
                    let _ = writeln!(output, "deep detail: invalid ({rule})");
                }
                crate::projection::DeepPageProjection::NotEnriched => {
                    let _ = writeln!(output, "deep detail: not-enriched");
                }
            }
            let _ = writeln!(output, "bytes: withheld");
        }
        DataProjection::InspectSlot {
            page,
            deep: _,
            selected_slot,
            overflow_chain,
            relocation_edge,
            interpretation,
            class_representation,
        } => {
            let _ = writeln!(
                output,
                "slot:{}:{}:{}",
                page.vol_id, page.page_id, selected_slot.slot_id
            );
            let _ = writeln!(
                output,
                "record type: {} ({})",
                selected_slot.record_type, selected_slot.record_type_ordinal
            );
            let _ = writeln!(
                output,
                "extent: offset={} length={} bytes=withheld",
                selected_slot.offset, selected_slot.length
            );
            if let Some(chain) = overflow_chain {
                let _ = writeln!(
                    output,
                    "overflow chain: complete={} validated-bytes={} pages={}",
                    chain.complete,
                    chain.validated_payload_bytes,
                    chain.pages.len()
                );
                for overflow_page in &chain.pages {
                    let _ = writeln!(
                        output,
                        "  {}:{}:{} payload-offset={} payload-length={} bytes=withheld",
                        overflow_page.role,
                        overflow_page.vol_id,
                        overflow_page.page_id,
                        overflow_page.payload_offset,
                        overflow_page.payload_length
                    );
                }
            }
            if let Some(edge) = relocation_edge {
                match edge.target {
                    crate::projection::OptionalOidProjection::Present { oid } => {
                        let _ = writeln!(
                            output,
                            "relocation: valid={} target={}:{}:{} bytes=withheld",
                            edge.valid, oid.vol_id, oid.page_id, oid.slot_id
                        );
                    }
                    crate::projection::OptionalOidProjection::Absent => {
                        let _ = writeln!(
                            output,
                            "relocation: valid={} target=unknown bytes=withheld",
                            edge.valid
                        );
                    }
                }
            }
            if let Some(representation) = class_representation {
                write_class_representation(&mut output, representation);
            }
            if let Some(interpretation) = interpretation {
                write_record_interpretation(&mut output, interpretation);
            }
        }
        DataProjection::InspectOos { chain } => {
            let _ = writeln!(
                output,
                "oos:{}:{}:{} complete={} validated-bytes={}",
                chain.head.vol_id,
                chain.head.page_id,
                chain.head.slot_id,
                chain.complete,
                chain.validated_payload_bytes
            );
            for chunk in &chain.chunks {
                let _ = writeln!(
                    output,
                    "  chunk:{} oid={}:{}:{} payload-offset={} payload-length={} bytes=withheld",
                    chunk.chunk_index,
                    chunk.oid.vol_id,
                    chunk.oid.page_id,
                    chunk.oid.slot_id,
                    chunk.payload_offset,
                    chunk.payload_length
                );
            }
        }
    }
    if document.diagnostics.is_empty() {
        let _ = writeln!(output, "diagnostics: none");
    } else {
        let _ = writeln!(output, "diagnostics:");
        for diagnostic in &document.diagnostics {
            let _ = writeln!(
                output,
                "  {} {} {}: {}",
                diagnostic.severity, diagnostic.code, diagnostic.subject, diagnostic.message
            );
        }
    }
    output
}

fn write_stdout(bytes: &[u8]) -> Result<(), CliError> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(bytes).map_err(|error| {
        if error.kind() == io::ErrorKind::BrokenPipe {
            CliError::BrokenPipe
        } else {
            CliError::Output(error)
        }
    })
}

const fn outcome_exit(outcome: InspectionOutcome) -> i32 {
    match outcome {
        InspectionOutcome::Success | InspectionOutcome::SuccessLimited => 0,
        InspectionOutcome::Findings => 1,
        InspectionOutcome::Incomplete => 3,
        InspectionOutcome::Fatal => 4,
    }
}

#[derive(Default)]
struct StderrProgress {
    last_phase: Option<ScanPhase>,
}

impl ProgressObserver for StderrProgress {
    fn update(&mut self, progress: ScanProgress) {
        if self.last_phase != Some(progress.phase) {
            let total = progress
                .trusted_total
                .map_or_else(|| "unknown".to_owned(), |total| total.to_string());
            let _ = writeln!(
                io::stderr().lock(),
                "progress {:?}: {} / {}",
                progress.phase,
                progress.completed,
                total
            );
            self.last_phase = Some(progress.phase);
        }
    }
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Open(OpenFailure),
    Query(QueryError),
    Output(io::Error),
    BrokenPipe,
    OpenAdapter(String),
    Internal(String),
}

impl CliError {
    const fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) | Self::Query(QueryError::EntityNotFound) => 2,
            Self::Query(QueryError::Arithmetic) | Self::Internal(_) => 70,
            Self::Query(QueryError::FactStore)
            | Self::Open(_)
            | Self::Output(_)
            | Self::OpenAdapter(_) => 4,
            Self::BrokenPipe => 141,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) | Self::OpenAdapter(message) | Self::Internal(message) => {
                formatter.write_str(message)
            }
            Self::Open(error) => write!(formatter, "{error}"),
            Self::Query(QueryError::EntityNotFound) => formatter.write_str("entity not found"),
            Self::Query(QueryError::FactStore) => {
                formatter.write_str("packed page fact storage is unavailable")
            }
            Self::Query(QueryError::Arithmetic) => formatter.write_str("query arithmetic overflow"),
            Self::Output(error) => write!(formatter, "output failed: {error}"),
            Self::BrokenPipe => formatter.write_str("broken output pipe"),
        }
    }
}

fn parse_byte_quantity(value: &str) -> Result<u64, String> {
    let (digits, multiplier) = [
        ("KiB", 1024_u64),
        ("MiB", 1024_u64.pow(2)),
        ("GiB", 1024_u64.pow(3)),
        ("TiB", 1024_u64.pow(4)),
    ]
    .into_iter()
    .find_map(|(suffix, multiplier)| {
        value
            .strip_suffix(suffix)
            .map(|digits| (digits, multiplier))
    })
    .unwrap_or((value, 1));
    let count = parse_canonical_u64(digits, "byte quantity")?;
    count
        .checked_mul(multiplier)
        .filter(|result| *result != 0)
        .ok_or_else(|| "byte quantity overflow or zero".to_owned())
}

fn parse_positive_u32(value: &str) -> Result<u32, String> {
    let parsed = parse_canonical_u64(value, "count")?;
    u32::try_from(parsed)
        .ok()
        .filter(|result| *result != 0)
        .ok_or_else(|| "count outside positive u32 range".to_owned())
}

fn parse_positive_u64(value: &str) -> Result<u64, String> {
    parse_canonical_u64(value, "count").and_then(|parsed| {
        (parsed != 0)
            .then_some(parsed)
            .ok_or_else(|| "count must be nonzero".to_owned())
    })
}

fn parse_canonical_u64(value: &str, kind: &str) -> Result<u64, String> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("invalid canonical {kind}"));
    }
    value
        .parse::<u64>()
        .map_err(|_| format!("{kind} outside u64 range"))
}

fn parse_i32(value: &str, kind: &str) -> Result<i32, CliError> {
    let parsed = parse_canonical_u64(value, kind).map_err(CliError::Usage)?;
    i32::try_from(parsed).map_err(|_| CliError::Usage(format!("{kind} outside pinned range")))
}

fn parse_vol_id(value: &str) -> Result<VolId, CliError> {
    let parsed = parse_canonical_u64(value, "volume identifier").map_err(CliError::Usage)?;
    let narrowed = i16::try_from(parsed)
        .map_err(|_| CliError::Usage("volume identifier outside pinned range".to_owned()))?;
    VolId::new(narrowed).map_err(|error| CliError::Usage(error.to_string()))
}

fn parse_page_id(value: &str) -> Result<PageId, CliError> {
    PageId::new(parse_i32(value, "page identifier")?)
        .map_err(|error| CliError::Usage(error.to_string()))
}

fn parse_sector_id(value: &str) -> Result<SectorId, CliError> {
    SectorId::new(parse_i32(value, "sector identifier")?)
        .map_err(|error| CliError::Usage(error.to_string()))
}

fn parse_oid(vol: &str, page: &str, slot: &str) -> Result<Oid, CliError> {
    let slot = parse_canonical_u64(slot, "slot identifier").map_err(CliError::Usage)?;
    let slot = i16::try_from(slot)
        .map_err(|_| CliError::Usage("slot identifier outside pinned range".to_owned()))?;
    Ok(Oid::new(
        parse_vol_id(vol)?,
        parse_page_id(page)?,
        SlotId::new(slot).map_err(|error| CliError::Usage(error.to_string()))?,
    ))
}

fn escape_control(value: &str) -> String {
    value.chars().flat_map(char::escape_default).collect()
}

/// Renders the schema a record was interpreted against.
fn write_class_representation(output: &mut String, representation: &ClassRepresentationProjection) {
    use std::fmt::Write as _;

    let name = match &representation.class_name {
        crate::projection::ClassNameProjection::Resolved { value } => value.clone(),
        crate::projection::ClassNameProjection::Unresolved { reason }
        | crate::projection::ClassNameProjection::NotApplicable { reason } => {
            format!("unresolved ({reason})")
        }
    };
    let _ = writeln!(
        output,
        "class: {name} oid={}:{}:{} representation={} fixed={} variable={}",
        representation.class_oid.vol_id,
        representation.class_oid.page_id,
        representation.class_oid.slot_id,
        representation.representation_id,
        representation.fixed_count,
        representation.variable_count
    );
    if let crate::projection::OptionalTextProjection::Known(state) = representation.is_current {
        let _ = writeln!(output, "representation state: {state}");
    }
}

/// Renders one record's attribute values, one per line.
fn write_record_interpretation(
    output: &mut String,
    interpretation: &RecordInterpretationProjection,
) {
    use std::fmt::Write as _;

    if let crate::projection::OptionalOidProjection::Present { oid } = interpretation.relocated_from
    {
        let _ = writeln!(
            output,
            "interpreted via relocation from {}:{}:{}",
            oid.vol_id, oid.page_id, oid.slot_id
        );
    }
    if let crate::projection::OptionalTextProjection::Known(reason) = interpretation.diagnostic {
        let _ = writeln!(output, "interpretation: unavailable ({reason})");
        return;
    }
    if let Some(layout) = &interpretation.layout {
        let _ = writeln!(output, "record bytes: {}", layout.record_length);
        for region in &layout.regions {
            let _ = writeln!(
                output,
                "  region {:<16} offset={:<6} length={}",
                region.region, region.offset, region.length
            );
        }
    }
    let _ = writeln!(output, "interpretation:");
    for attribute in &interpretation.attributes {
        let name = match &attribute.name {
            AttributeNameProjection::Resolved { value } => value.clone(),
            AttributeNameProjection::Unresolved { reason } => format!("unnamed ({reason})"),
        };
        let value = match &attribute.value {
            AttributeValueProjection::Decoded { value } => value.clone(),
            AttributeValueProjection::Null => "NULL".to_owned(),
            AttributeValueProjection::OutOfRow { head, total_length } => format!(
                "out-of-row oos:{}:{}:{} total-length={total_length} bytes=withheld",
                head.vol_id, head.page_id, head.slot_id
            ),
            AttributeValueProjection::Withheld {
                reason,
                offset,
                length,
            } => format!("withheld offset={offset} length={length} bytes=withheld ({reason})"),
        };
        let _ = writeln!(
            output,
            "  {name} {} [{} offset={} length={}] = {value}",
            attribute.type_name, attribute.storage, attribute.offset, attribute.length,
        );
    }
}
