//! Command-line adapter over the inspection seam.

use std::ffi::OsString;
use std::fmt;
use std::io::{self, IsTerminal, Write};
use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;

use crate::diagnostics::InspectionOutcome;
use crate::inspection::{
    CancelToken, GraphView, Inspection, OpenFailure, OpenRequest, ProgressObserver, QueryError,
    ResourcePolicy, RevisionSelector, ScanPhase, ScanProgress,
};
use crate::model::{FileId, Oid, PageId, SectorId, SlotId, Vfid, VolId, Vpid};
use crate::projection::{
    DataProjection, ResultDocument, deep_page_projection, file_header_projection,
    oos_chain_projection, overflow_chain_projection, page_projection, relocation_edge_projection,
    result_document, sector_projection, slot_projection, summary_projection, volume_projection,
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
    #[arg(long)]
    allow_remote_http: bool,
    #[arg(long)]
    external_origin: Option<String>,
    #[arg(long)]
    token_file: Option<PathBuf>,
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
            let (view, overview) =
                open_view(&command.input, &command.resources, OutputFormat::Human)?;
            crate::web::serve(
                view,
                crate::web::ServeOptions {
                    listen: command.listen,
                    allow_remote_http: command.allow_remote_http,
                    external_origin: command.external_origin,
                    token_file: command.token_file,
                    policy,
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
        view = view
            .enrich_file(vfid, enrichment_policy, &CancelToken::new())
            .map_err(|error| CliError::OpenAdapter(error.to_string()))?;
    }
    let overview = view.overview();
    let (volumes, sectors) = map_data(&view, selector)?;
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

fn run_inspect(command: InspectCommand) -> Result<i32, CliError> {
    command.output.validate()?;
    let selector = EntitySelector::parse(&command.selector)?;
    let enrichment_policy = command.resources.clone().policy()?;
    let (mut view, _) = open_view(&command.input, &command.resources, command.output.format)?;
    let data = match selector {
        EntitySelector::Volume(vol_id) => DataProjection::InspectVolume {
            volume: volume_projection(view.volume(vol_id).map_err(CliError::Query)?),
        },
        EntitySelector::Sector(vol_id, sector_id) => DataProjection::InspectSector {
            sector: sector_projection(view.sector(vol_id, sector_id).map_err(CliError::Query)?),
        },
        EntitySelector::Page(vpid) => {
            view = view
                .enrich_page(vpid, enrichment_policy, &CancelToken::new())
                .map_err(|error| CliError::OpenAdapter(error.to_string()))?;
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
                .map_err(|error| CliError::OpenAdapter(error.to_string()))?;
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
                .map_err(|error| CliError::OpenAdapter(error.to_string()))?;
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
            let overflow_chain = if selected_slot.record_type() == crate::format::RecordType::BigOne
            {
                view = view
                    .enrich_bigone(oid, enrichment_policy, &CancelToken::new())
                    .map_err(|error| CliError::OpenAdapter(error.to_string()))?;
                view.overflow_chain(oid).map(overflow_chain_projection)
            } else {
                None
            };
            let relocation_edge =
                if selected_slot.record_type() == crate::format::RecordType::Relocation {
                    view = view
                        .enrich_relocation(oid, enrichment_policy, &CancelToken::new())
                        .map_err(|error| CliError::OpenAdapter(error.to_string()))?;
                    view.relocation_edge(oid)
                        .map(relocation_edge_projection)
                        .map(Box::new)
                } else {
                    None
                };
            DataProjection::InspectSlot {
                page: page_projection(view.page(vpid).map_err(CliError::Query)?),
                deep: deep_page_projection(Some(deep)),
                selected_slot: slot_projection(selected_slot),
                overflow_chain,
                relocation_edge,
            }
        }
        EntitySelector::Oos(oid) => {
            view = view
                .enrich_oos(oid, enrichment_policy, &CancelToken::new())
                .map_err(|error| CliError::OpenAdapter(error.to_string()))?;
            DataProjection::InspectOos {
                chain: oos_chain_projection(
                    view.oos_chain(oid)
                        .ok_or(CliError::Query(QueryError::EntityNotFound))?,
                ),
            }
        }
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
    let inspection = if show_progress {
        Inspection::open(&request, policy, &cancel, Some(&mut observer))
    } else {
        Inspection::open(&request, policy, &cancel, None)
    }
    .map_err(CliError::Open)?;
    let view = inspection
        .view(RevisionSelector::Latest)
        .map_err(|error| CliError::Internal(error.to_string()))?;
    let view = match view.enrich_file_inventory(policy, &cancel) {
        Ok(enriched) => enriched,
        Err(crate::inspection::OperationError::Unsupported) => view,
        Err(error) => return Err(CliError::OpenAdapter(error.to_string())),
    };
    let overview = view.overview();
    Ok((view, overview))
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

fn write_jsonl(document: &ResultDocument) -> Result<(), CliError> {
    #[derive(Serialize)]
    struct Record<'a, T: Serialize> {
        schema: &'a str,
        schema_version: u32,
        #[serde(rename = "record_type")]
        kind: &'a str,
        sequence: String,
        snapshot_id: &'a str,
        revision: &'a str,
        data: T,
    }
    let records = [
        serde_json::to_string(&Record {
            schema: document.schema,
            schema_version: document.schema_version,
            kind: "header",
            sequence: "0".to_owned(),
            snapshot_id: &document.snapshot.id,
            revision: &document.snapshot.revision,
            data: (&document.tool, &document.command),
        }),
        serde_json::to_string(&Record {
            schema: document.schema,
            schema_version: document.schema_version,
            kind: "data",
            sequence: "1".to_owned(),
            snapshot_id: &document.snapshot.id,
            revision: &document.snapshot.revision,
            data: &document.data,
        }),
        serde_json::to_string(&Record {
            schema: document.schema,
            schema_version: document.schema_version,
            kind: "completion",
            sequence: "2".to_owned(),
            snapshot_id: &document.snapshot.id,
            revision: &document.snapshot.revision,
            data: (document.outcome, &document.coverage, &document.diagnostics),
        }),
    ];
    for record in records {
        let record = record.map_err(|error| CliError::Internal(error.to_string()))?;
        write_stdout(record.as_bytes())?;
        write_stdout(b"\n")?;
    }
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
