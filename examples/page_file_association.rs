//! Print the validated File and class association for one physical CUBRID Page.
//!
//! Given `(volid, pageid)`, the offline allocation inventory identifies the
//! allocating (or reserving) file, its descriptor `class_oid`, and the stored
//! class name resolved once per class.
//!
//! Example:
//! `cargo run --example page_file_association -- --vinf /copy/demodb_vinf 1 1000`

use std::error::Error;
use std::path::PathBuf;

use clap::{ArgGroup, Parser};
use volmap::inspection::{CancelToken, Inspection, OpenRequest, ResourcePolicy, RevisionSelector};
use volmap::model::{PageId, VolId, Vpid};
use volmap::projection::{
    ClassNameProjection, FileAssociationBodyProjection, FileAssociationProjection,
    OptionalOidProjection, OptionalTextProjection, page_projection,
};
use volmap::source::InputSpec;

#[derive(Debug, Parser)]
#[command(
    name = "page-file-association",
    about = "Print one physical Page's validated File and class association",
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
    let page = page_projection(view.page(vpid)?);

    println!("requested: volume={}, page={}", page.vol_id, page.page_id);
    println!("allocation: {}", page.allocation);
    println!("page_type: {}", optional_text(&page.page_type));
    match page.file_association {
        FileAssociationProjection::None => println!("file: none"),
        FileAssociationProjection::MixedClaims => {
            println!("file: mixed (multiple file tables claim this sector)");
        }
        FileAssociationProjection::Allocated { file } => {
            println!("relationship: allocated");
            print_association(&file);
        }
        FileAssociationProjection::ReservedFor { file } => {
            println!("relationship: reserved-for");
            print_association(&file);
        }
    }
    Ok(())
}

fn print_association(association: &FileAssociationBodyProjection) {
    println!(
        "file: volume={}, file={}",
        association.vol_id, association.file_id
    );
    println!("file_role: {}", optional_text(&association.file_type));
    if let OptionalOidProjection::Present { oid } = association.class_oid {
        println!(
            "class_oid: volume={}, page={}, slot={}",
            oid.vol_id, oid.page_id, oid.slot_id
        );
    }
    match &association.class_name {
        ClassNameProjection::Resolved { value } => println!("table: {value}"),
        ClassNameProjection::Unresolved { reason, .. } => {
            println!("table: unresolved ({reason})");
        }
        ClassNameProjection::NotApplicable { reason, .. } => {
            println!("table: not-applicable ({reason})");
        }
    }
}

const fn optional_text(value: &OptionalTextProjection) -> &'static str {
    match value {
        OptionalTextProjection::Known(value) => value,
        OptionalTextProjection::Unknown => "unknown",
        OptionalTextProjection::Unsupported => "unsupported",
    }
}
