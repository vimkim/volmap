//! Resolve one physical CUBRID page to its associated table name.
//!
//! Given `(volid, pageid)`, the offline allocation inventory identifies the
//! allocating (or reserving) file, its descriptor `class_oid`, and the stored
//! class name resolved once per class.
//!
//! Example:
//! `cargo run --example page_table_poc -- --vinf /copy/demodb_vinf 1 1000`

use std::error::Error;
use std::path::PathBuf;

use clap::{ArgGroup, Parser};
use volmap::inspection::{
    CancelToken, ClassAssociation, ClassNameResolution, FileAssociation, Inspection, OpenRequest,
    PageFileAssociation, ResourcePolicy, RevisionSelector,
};
use volmap::model::{PageAllocationClass, PageId, VolId, Vpid};
use volmap::source::InputSpec;

#[derive(Debug, Parser)]
#[command(
    name = "page-table-poc",
    about = "Resolve one physical page to a CUBRID table name",
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
    let page = view.page(vpid)?;

    println!(
        "requested: volume={}, page={}",
        page.vpid.vol_id.get(),
        page.vpid.page_id.get()
    );
    println!("allocation: {}", allocation_name(page.allocation));
    println!(
        "page_type: {}",
        page.page_type.map_or("unknown", |kind| kind.as_str())
    );
    match page.file_association {
        PageFileAssociation::None => println!("file: none"),
        PageFileAssociation::MixedClaims => {
            println!("file: mixed (multiple file tables claim this sector)");
        }
        PageFileAssociation::Allocated(association) => {
            println!("relationship: allocated");
            print_association(&association);
        }
        PageFileAssociation::ReservedFor(association) => {
            println!("relationship: reserved-for");
            print_association(&association);
        }
    }
    Ok(())
}

fn print_association(association: &FileAssociation) {
    println!(
        "file: volume={}, file={}",
        association.vfid.vol_id.get(),
        association.vfid.file_id.get()
    );
    println!(
        "file_role: {}",
        association
            .file_type
            .map_or("unavailable", |kind| kind.as_str())
    );
    match &association.class {
        ClassAssociation::None(reason) => println!("table: not-applicable ({reason})"),
        ClassAssociation::Class { oid, name } => {
            println!(
                "class_oid: volume={}, page={}, slot={}",
                oid.vol_id.get(),
                oid.page_id.get(),
                oid.slot_id.get()
            );
            match name {
                ClassNameResolution::Resolved(value) => println!("table: {value}"),
                ClassNameResolution::Unresolved(reason) => {
                    println!("table: unresolved ({reason})");
                }
            }
        }
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
