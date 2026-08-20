//! PROTOTYPE: resolve one physical CUBRID page to its associated table name.
//!
//! The question is intentionally narrow: given `(volid, pageid)`, can the
//! existing offline allocation inventory identify the file, follow its
//! descriptor `class_oid`, and read the stored class name?
//!
//! Example:
//! `cargo run --example page_table_poc -- --vinf /copy/demodb_vinf 1 1000`

use std::error::Error;
use std::path::PathBuf;

use clap::{ArgGroup, Parser};
use volmap::inspection::{
    CancelToken, Inspection, OpenRequest, PrototypeTableName, ResourcePolicy, RevisionSelector,
};
use volmap::model::{PageAllocationClass, PageId, VolId, Vpid};
use volmap::source::InputSpec;

#[derive(Debug, Parser)]
#[command(
    name = "page-table-poc",
    about = "PROTOTYPE: resolve one allocated physical page to a CUBRID table name",
    group(
        ArgGroup::new("snapshot-input")
            .required(true)
            .multiple(false)
            .args(["database", "vinf"])
    )
)]
struct Args {
    #[arg(long)]
    database: Option<String>,

    #[arg(long, requires = "database")]
    databases_file: Option<PathBuf>,

    #[arg(long)]
    vinf: Option<PathBuf>,

    #[arg(long, requires = "vinf")]
    volume_root: Option<PathBuf>,

    #[arg(long)]
    tde_keys_file: Option<PathBuf>,

    /// Physical CUBRID volume identifier.
    volume: i16,

    /// Physical page identifier inside the volume.
    page: i32,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let input = match (args.database, args.vinf) {
        (Some(name), None) => InputSpec::Database {
            name,
            databases_file: args.databases_file,
        },
        (None, Some(path)) => InputSpec::Vinf {
            path,
            volume_root: args.volume_root,
        },
        _ => unreachable!("clap enforces exactly one snapshot input"),
    };
    let policy = ResourcePolicy::new(
        256 * 1024 * 1024,
        2 * 1024 * 1024 * 1024,
        4,
        16_384,
        256 * 1024 * 1024,
    )?;
    let inspection = Inspection::open(
        &OpenRequest {
            input,
            tde_keys_file: args.tde_keys_file,
            spill_directory: None,
        },
        policy,
        &CancelToken::new(),
        None,
    )?;
    let view = inspection.view(RevisionSelector::Latest)?;
    let vpid = Vpid::new(VolId::new(args.volume)?, PageId::new(args.page)?);
    let result = view.prototype_page_table_lookup(vpid)?;

    println!(
        "requested: volume={}, page={}",
        result.page.vpid.vol_id.get(),
        result.page.vpid.page_id.get()
    );
    println!("allocation: {}", allocation_name(result.page.allocation));
    println!(
        "page_type: {}",
        result
            .page
            .page_type
            .map_or("unknown", |kind| kind.as_str())
    );
    match result.owner_file {
        Some(owner) => println!(
            "allocated_by_file: volume={}, file={}",
            owner.vol_id.get(),
            owner.file_id.get()
        ),
        None => println!("allocated_by_file: none"),
    }
    println!(
        "file_role: {}",
        result.file_type.map_or("none", |kind| kind.as_str())
    );
    match result.class_oid {
        Some(oid) => println!(
            "class_oid: volume={}, page={}, slot={}",
            oid.vol_id.get(),
            oid.page_id.get(),
            oid.slot_id.get()
        ),
        None => println!("class_oid: none"),
    }
    match result.table_name {
        PrototypeTableName::Resolved(name) => println!("table: {name}"),
        PrototypeTableName::NotApplicable(reason) => {
            println!("table: not-applicable ({reason})");
        }
        PrototypeTableName::Unresolved(reason) => println!("table: unresolved ({reason})"),
    }
    Ok(())
}

const fn allocation_name(allocation: PageAllocationClass) -> &'static str {
    match allocation {
        PageAllocationClass::SystemMetadata => "system-metadata",
        PageAllocationClass::Unreserved => "unreserved",
        PageAllocationClass::ReservedUnallocated => "reserved-unallocated",
        PageAllocationClass::Allocated => "allocated",
    }
}
