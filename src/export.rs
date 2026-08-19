//! Deterministic, self-contained HTML projection of one immutable graph revision.

use std::fmt::{self, Write as _};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::inspection::GraphView;
use crate::projection::{
    DataProjection, DeepPageResourceProjection, ResultDocument, SectorProjection,
    deep_page_projection, oos_chain_projection, overflow_chain_projection, page_projection,
    relocation_edge_projection, result_document, sector_projection, volume_projection,
};

pub const DEFAULT_MAX_HTML_BYTES: u64 = 64 * 1024 * 1024;
pub const HARD_MAX_HTML_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug)]
pub enum ExportError {
    DestinationExists,
    InvalidDestination,
    InvalidLimit,
    LimitExceeded { limit: u64 },
    Query,
    Serialization(serde_json::Error),
    Io(io::Error),
}

impl fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DestinationExists => formatter.write_str("export destination already exists"),
            Self::InvalidDestination => formatter.write_str("export destination is invalid"),
            Self::InvalidLimit => write!(
                formatter,
                "--max-html-bytes must be between 1 and {HARD_MAX_HTML_BYTES}"
            ),
            Self::LimitExceeded { limit } => write!(
                formatter,
                "HTML export exceeds the {limit}-byte limit; use `volmap serve` instead"
            ),
            Self::Query => formatter.write_str("could not project the complete graph revision"),
            Self::Serialization(error) => write!(formatter, "HTML data encoding failed: {error}"),
            Self::Io(error) => write!(formatter, "HTML export I/O failed: {error}"),
        }
    }
}

impl std::error::Error for ExportError {}

impl From<io::Error> for ExportError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// Export `view` without opening a source or interpreting any additional bytes.
pub fn export_html(view: &GraphView, output: &Path, limit: u64) -> Result<(), ExportError> {
    if limit == 0 || limit > HARD_MAX_HTML_BYTES {
        return Err(ExportError::InvalidLimit);
    }
    match fs::symlink_metadata(output) {
        Ok(_) => return Err(ExportError::DestinationExists),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(ExportError::Io(error)),
    }

    let document = complete_document(view)?;
    let json = serde_json::to_string(&document).map_err(ExportError::Serialization)?;
    let safe_json = json
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    let mut html = LimitedString::new(limit);
    html.push(HTML_PREFIX)?;
    html.push(&safe_json)?;
    let suffix = HTML_SUFFIX
        .strip_suffix("</body></html>\n")
        .ok_or(ExportError::InvalidDestination)?;
    html.push(suffix)?;
    html.push("<details id=\"licenses\"><summary>About and licenses</summary><pre>")?;
    html.push(&escape_html(crate::notices::THIRD_PARTY_NOTICES))?;
    html.push("</pre></details></body></html>\n")?;
    install_new_file(output, html.as_bytes())
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn complete_document(view: &GraphView) -> Result<ResultDocument, ExportError> {
    let overview = view.overview();
    let volumes = view
        .volumes()
        .into_iter()
        .map(volume_projection)
        .collect::<Vec<_>>();
    let mut sectors = Vec::<SectorProjection>::new();
    for volume in view.volumes() {
        for raw_sector in 0..volume.total_sectors {
            let sector_id = crate::model::SectorId::new(
                i32::try_from(raw_sector).map_err(|_| ExportError::Query)?,
            )
            .map_err(|_| ExportError::Query)?;
            sectors.push(sector_projection(
                view.sector(volume.vol_id, sector_id)
                    .map_err(|_| ExportError::Query)?,
            ));
        }
    }
    let deep_pages = view
        .deep_pages()
        .into_iter()
        .map(|deep| {
            Ok(DeepPageResourceProjection {
                page: page_projection(view.page(deep.vpid).map_err(|_| ExportError::Query)?),
                deep: deep_page_projection(Some(deep)),
            })
        })
        .collect::<Result<Vec<_>, ExportError>>()?;
    Ok(result_document(
        "export-html",
        None,
        &overview,
        DataProjection::Map {
            volumes,
            sectors,
            deep_pages,
            oos_chains: view
                .oos_chains()
                .into_iter()
                .map(oos_chain_projection)
                .collect(),
            overflow_chains: view
                .overflow_chains()
                .into_iter()
                .map(overflow_chain_projection)
                .collect(),
            relocation_edges: view
                .relocation_edges()
                .into_iter()
                .map(relocation_edge_projection)
                .collect(),
        },
    ))
}

struct LimitedString {
    value: String,
    limit: u64,
}

impl LimitedString {
    fn new(limit: u64) -> Self {
        Self {
            value: String::new(),
            limit,
        }
    }

    fn push(&mut self, value: &str) -> Result<(), ExportError> {
        let next = u64::try_from(self.value.len())
            .ok()
            .and_then(|current| current.checked_add(value.len() as u64))
            .ok_or(ExportError::LimitExceeded { limit: self.limit })?;
        if next > self.limit {
            return Err(ExportError::LimitExceeded { limit: self.limit });
        }
        self.value
            .write_str(value)
            .map_err(|_| ExportError::LimitExceeded { limit: self.limit })
    }

    fn as_bytes(&self) -> &[u8] {
        self.value.as_bytes()
    }
}

struct TempFile {
    path: PathBuf,
    installed: bool,
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.installed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn install_new_file(output: &Path, bytes: &[u8]) -> Result<(), ExportError> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(ExportError::InvalidDestination)?;
    let suffix = random_suffix()?;
    let temporary_path = parent.join(format!(".{name}.volmap-{suffix}.tmp"));
    let mut temporary = TempFile {
        path: temporary_path.clone(),
        installed: false,
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary_path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    match fs::hard_link(&temporary_path, output) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(ExportError::DestinationExists);
        }
        Err(error) => return Err(ExportError::Io(error)),
    }
    fs::remove_file(&temporary_path)?;
    temporary.installed = true;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn random_suffix() -> Result<String, ExportError> {
    let mut random = File::open("/dev/urandom")?;
    let mut bytes = [0_u8; 12];
    random.read_exact(&mut bytes)?;
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").map_err(|_| ExportError::InvalidDestination)?;
    }
    Ok(value)
}

const HTML_PREFIX: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="referrer" content="no-referrer"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'sha256-P5ODvS9bwNlrzaGrGOz82xPAfh5X1Vs3CCUDMXD5ubE='; script-src 'sha256-8PNlchPB3NDqWP1ef0sY2shuL98VodGYiXI6/ytCLU8='; img-src data:; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'"><title>Volmap Inspector Report</title><style>:root{color-scheme:dark;--bg:#071014;--panel:#0d1820;--line:#29404b;--text:#dce8ec;--muted:#8fa5ae;--cyan:#68d8d0;--blue:#244c66;--purple:#3b3761;--red:#6b2939}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px/1.45 system-ui,sans-serif}header{position:sticky;top:0;display:flex;gap:18px;align-items:center;padding:14px 20px;border-bottom:1px solid var(--line);background:#0a1319}header strong{letter-spacing:.08em}.spacer{flex:1}input{min-width:260px;padding:7px;background:var(--bg);color:var(--text);border:1px solid var(--line)}main{padding:20px;display:grid;gap:18px}.panel{padding:16px;border:1px solid var(--line);background:var(--panel)}h1,h2,p{margin-top:0}.muted{color:var(--muted)}.counts,.coverage{display:grid;grid-template-columns:repeat(auto-fit,minmax(190px,1fr));gap:8px}.card{padding:9px;border:1px solid var(--line)}.volume{margin:18px 0}.sector{display:grid;grid-template-columns:100px repeat(64,minmax(8px,1fr));gap:2px;align-items:center;margin:3px 0}.sector>a{height:17px;background:#263740;text-indent:-9999px;overflow:hidden}.sector>a.system-metadata{background:var(--purple)}.sector>a.reserved-unallocated{background:var(--blue)}.sector>a.finding{background:var(--red)}.diagnostic{border-left:3px solid var(--red);padding-left:10px;margin:8px 0}dialog{max-width:680px;background:var(--panel);color:var(--text);border:1px solid var(--cyan)}dialog::backdrop{background:#000a}dl{display:grid;grid-template-columns:150px 1fr;gap:7px}dd{margin:0}.withheld{font-family:ui-monospace,monospace;color:var(--muted)}@media(max-width:900px){.sector{grid-template-columns:78px repeat(64,10px);overflow-x:auto}header{position:static;flex-wrap:wrap}input{width:100%}}</style></head><body><header><strong>VOLMAP REPORT</strong><span id="identity"></span><span class="spacer"></span><input id="filter" aria-label="Filter volumes, sectors, pages, or diagnostics" placeholder="Filter visible facts"></header><main id="app"><noscript>This report requires JavaScript only to render its embedded, offline inspection facts.</noscript></main><dialog id="page"><button id="close">Close</button><div id="page-content"></div></dialog><script id="volmap-data" type="application/json">"#;

#[allow(clippy::needless_raw_string_hashes)]
const HTML_SUFFIX: &str = r#"</script><script>(()=>{'use strict';const d=JSON.parse(document.getElementById('volmap-data').textContent),app=document.getElementById('app'),el=(tag,text,cls)=>{const n=document.createElement(tag);if(text!==undefined)n.textContent=text;if(cls)n.className=cls;return n},add=(p,c)=>{p.append(c);return c},state=t=>t&&t.state==='known'?t.value:t?.state||'unknown';document.getElementById('identity').textContent=`snapshot ${d.snapshot.id.slice(0,12)} · revision ${d.snapshot.revision} · ${d.outcome}`;const intro=add(app,el('section',undefined,'panel'));add(intro,el('h1','Inspection overview'));add(intro,el('p',`${d.snapshot.validity} · ${d.snapshot.format_profile}`,'muted'));const summary=d.data.kind==='map'?d.data:null,counts=add(intro,el('div',undefined,'counts'));[['Volumes',summary.volumes.length],['Sectors',summary.sectors.length],['Diagnostics',d.diagnostics.length],['Outcome',d.outcome]].forEach(([k,v])=>add(counts,el('div',`${k}: ${v}`,'card')));const cov=add(app,el('section',undefined,'panel coverage'));d.coverage.forEach(c=>add(cov,el('div',`${c.facet}: ${c.coverage} (${c.conclusive}/${c.evaluated})`,'card')));const maps=add(app,el('section',undefined,'panel'));add(maps,el('h2','Sector maps'));summary.volumes.forEach(v=>{const box=add(maps,el('div',undefined,'volume searchable'));box.dataset.search=`volume ${v.vol_id} ${v.purpose}`;add(box,el('h2',`Volume ${v.vol_id} · ${v.purpose} · ${v.total_sectors} sectors`));summary.sectors.filter(s=>s.vol_id===v.vol_id).forEach(s=>{const row=add(box,el('div',undefined,'sector'));row.id=`sector-${s.vol_id}-${s.sector_id}`;add(row,el('a',`Sector ${s.sector_id}`)).href=`#${row.id}`;s.pages.forEach(p=>{const a=add(row,el('a',String(p.page_id),`${p.allocation}${state(p.diagnostic)!=='unknown'?' finding':''}`));a.href=`#page-${p.vol_id}-${p.page_id}`;a.title=`page ${p.page_id} · ${state(p.page_type)} · ${p.allocation}`;a.onclick=e=>{e.preventDefault();showPage(p)}})})});const diagnostics=add(app,el('section',undefined,'panel'));add(diagnostics,el('h2','Diagnostics'));if(!d.diagnostics.length)add(diagnostics,el('p','None'));d.diagnostics.forEach(x=>{const n=add(diagnostics,el('div',undefined,'diagnostic searchable'));n.dataset.search=`${x.code} ${x.subject} ${x.severity} ${x.message}`;add(n,el('strong',`${x.severity} · ${x.code}`));add(n,el('p',`${x.subject} — ${x.message}`))});const about=add(app,el('section',undefined,'panel'));add(about,el('h2','About and licenses'));add(about,el('p',`Volmap ${d.tool.version}. CUBRID format authority ${d.tool.format_profile}. Volmap and the referenced CUBRID format source are Apache-2.0 licensed. This deterministic report contains structural inspection facts only; source paths and source bytes are withheld.`));const dialog=document.getElementById('page'),content=document.getElementById('page-content');function showPage(p){content.replaceChildren();add(content,el('h2',`Page ${p.vol_id}:${p.page_id}`));const dl=add(content,el('dl'));[['Sector',p.sector_id],['Allocation',p.allocation],['Page type',state(p.page_type)],['Availability',p.availability],['Detail support',state(p.detail_support)],['TDE',p.tde_state],['LSA word',state(p.lsa_word)],['Diagnostic',state(p.diagnostic)]].forEach(([k,v])=>{add(dl,el('dt',String(k)));add(dl,el('dd',String(v)))});add(content,el('p','Structural evidence only · source bytes withheld','withheld'));dialog.showModal();history.replaceState(null,'',`#page-${p.vol_id}-${p.page_id}`)}document.getElementById('close').onclick=()=>dialog.close();document.getElementById('filter').oninput=e=>{const q=e.target.value.toLowerCase();document.querySelectorAll('.searchable').forEach(n=>n.hidden=q&&!n.dataset.search.toLowerCase().includes(q))}})();</script></body></html>
"#;
