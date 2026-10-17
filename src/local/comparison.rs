//! Optional, bounded content observations. Equality never authorizes removal.
use super::*;
use crate::model::{Inventory, LocalSummary, Review};
use crate::output::{bounded_json_size, canonical_digest};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;

pub const SCHEMA: &str = "wt-sweep.local-comparison/1";
pub const CAPTURE_BYTES: usize = 8 * 1024 * 1024;
pub const DOCUMENT_BYTES: usize = 24 * 1024 * 1024;
#[derive(Debug, Default)]
pub struct CaptureState {
    pub captures: BTreeMap<BytePath, SourceCapture>,
    pub remaining_timeout: Duration,
}
pub type CaptureStore = Rc<RefCell<CaptureState>>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Staged,
    Unstaged,
    Conflicts,
    Untracked,
    Ignored,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Index,
    Disk,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultKind {
    MatchesTarget,
    MatchesReference,
    Different,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FsWitness {
    pub identity: Identity,
    pub mode: u32,
    pub size: i64,
    pub mtime: i64,
    pub mtime_nsec: i64,
    pub ctime: i64,
    pub ctime_nsec: i64,
}
impl FsWitness {
    pub(super) fn from_stat(st: &Stat) -> Self {
        Self {
            identity: identity(st),
            mode: u32::from(st.st_mode),
            size: st.st_size,
            mtime: st.st_mtime,
            mtime_nsec: st.st_mtime_nsec,
            ctime: st.st_ctime,
            ctime_nsec: st.st_ctime_nsec,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Value {
    Present {
        sha256: String,
        git_mode: u32,
        posix_mode: Option<u32>,
        witness: Option<FsWitness>,
    },
    Absent {
        parent: Option<FsWitness>,
    },
    Unknown {
        reason: String,
    },
}
impl Value {
    pub(super) fn unknown(reason: impl Into<String>) -> Self {
        Self::Unknown {
            reason: reason.into(),
        }
    }
    pub(super) fn git(digest: String, mode: u32) -> Self {
        Self::Present {
            sha256: digest,
            git_mode: mode,
            posix_mode: None,
            witness: None,
        }
    }
    pub(super) fn disk(digest: Option<String>, mode: u32, st: &Stat) -> Self {
        match digest {
            Some(sha256) if matches!(mode, 0o100644 | 0o100755 | 0o120000) => Self::Present {
                sha256,
                git_mode: mode,
                posix_mode: Some(u32::from(st.st_mode & 0o7777)),
                witness: Some(FsWitness::from_stat(st)),
            },
            _ => Self::unknown("content or type unavailable"),
        }
    }
    fn valid(&self) -> bool {
        match self {
            Self::Present {
                sha256,
                git_mode,
                posix_mode,
                witness,
            } => {
                sha256.len() == 64
                    && sha256.bytes().all(|b| b.is_ascii_hexdigit())
                    && matches!(*git_mode, 0o100644 | 0o100755 | 0o120000)
                    && posix_mode.is_none_or(|m| m <= 0o7777)
                    && witness
                        .as_ref()
                        .is_none_or(|s| s.mode & 0o7777 == posix_mode.unwrap_or(u32::MAX))
            }
            Self::Absent { .. } => true,
            Self::Unknown { reason } => !reason.is_empty() && reason.len() <= 1024,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRow {
    pub path: BytePath,
    pub category: Category,
    pub layer: Layer,
    pub stage: Option<u8>,
    pub source: Value,
    pub source_oid: Option<String>,
}
#[derive(Debug)]
pub struct SourceCapture {
    pub rows: Vec<SourceRow>,
    pub bytes: usize,
    pub summary_sha256: String,
    pub remaining: super::SummaryLimits,
    pub max_rows: usize,
    pub max_bytes: usize,
    pub omitted: usize,
    pub identity: Option<Identity>,
    pub sources: Vec<crate::model::LocalSource>,
}
impl SourceCapture {
    pub(super) fn new(limits: &super::SummaryLimits, max_rows: usize, max_bytes: usize) -> Self {
        Self {
            rows: Vec::new(),
            bytes: 0,
            summary_sha256: String::new(),
            remaining: limits.clone(),
            max_rows,
            max_bytes,
            omitted: 0,
            identity: None,
            sources: Vec::new(),
        }
    }
    pub(super) fn push(&mut self, row: SourceRow) {
        // Fixed overhead reserves vector/object allocation as well as wire bytes.
        let size = bounded_json_size(&row, self.max_bytes)
            .ok()
            .map(|n| n.saturating_add(512));
        if self.rows.len() >= self.max_rows
            || size.is_none_or(|n| n > self.max_bytes.saturating_sub(self.bytes))
        {
            self.omitted += 1;
        } else if let Some(size) = size {
            self.bytes += size;
            self.rows.push(row);
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckoutAdministration {
    pub root: FsWitness,
    pub marker: FsWitness,
    pub marker_sha256: Option<String>,
    pub git_dir: BytePath,
    pub git_dir_identity: Identity,
    pub common_dir: BytePath,
    pub common_dir_identity: Identity,
    pub commondir: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub origin: String,
    pub name: BytePath,
    pub commit_oid: Option<String>,
    pub tree_oid: Option<String>,
    pub identity: Option<Identity>,
    pub administration: Option<CheckoutAdministration>,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub checks: String,
    pub gap: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lookup {
    pub reference: usize,
    pub value: Value,
    pub object_oid: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    #[serde(flatten)]
    pub change: SourceRow,
    pub lookups: Vec<Lookup>,
    pub result: ResultKind,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Totals {
    pub matches_target: usize,
    pub matches_reference: usize,
    pub different: usize,
    pub unknown: usize,
}
impl Totals {
    fn add(&mut self, result: &ResultKind) {
        match result {
            ResultKind::MatchesTarget => self.matches_target += 1,
            ResultKind::MatchesReference => self.matches_reference += 1,
            ResultKind::Different => self.different += 1,
            ResultKind::Unknown => self.unknown += 1,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    pub observed: usize,
    pub emitted: usize,
    pub omitted: usize,
    pub source_complete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub path: BytePath,
    pub collection: String,
    pub summary_sha256: Option<String>,
    pub references: Vec<Reference>,
    pub rows: Vec<Row>,
    pub coverage: BTreeMap<Category, Coverage>,
    pub path_totals: Totals,
    pub layer_totals: Totals,
    pub complete: bool,
    pub gaps: Vec<String>,
    pub applied_limits: BTreeMap<String, u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub schema: String,
    pub inventory_sha256: String,
    pub review_sha256: String,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub max_bytes: usize,
    pub max_rows: usize,
    pub items: Vec<Item>,
    pub digest_sha256: String,
}
fn valid_oid(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn error(message: impl Into<String>) -> SweepError {
    SweepError::new("local_comparison", "compare_local", Path::new(""), message)
}
fn equal(source: &Value, target: &Value, checkout: bool) -> Option<bool> {
    match (source, target) {
        (Value::Unknown { .. }, _) | (_, Value::Unknown { .. }) => None,
        (Value::Absent { .. }, Value::Absent { .. }) => Some(true),
        (
            Value::Present {
                sha256: a,
                git_mode: am,
                posix_mode: ap,
                ..
            },
            Value::Present {
                sha256: b,
                git_mode: bm,
                posix_mode: bp,
                ..
            },
        ) => {
            if checkout {
                ap.zip(*bp)
                    .map(|(a_mode, b_mode)| a == b && am == bm && a_mode == b_mode)
            } else {
                Some(a == b && am == bm)
            }
        }
        _ => Some(false),
    }
}
pub(super) fn same_disk_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Absent { .. }, Value::Absent { .. }) => true,
        (Value::Present { witness: aw, .. }, Value::Present { witness: bw, .. }) => {
            aw == bw && equal(a, b, true) == Some(true)
        }
        _ => false,
    }
}
pub fn classify(row: &SourceRow, lookups: &[Lookup], references: &[Reference]) -> ResultKind {
    let mut unknown = lookups.is_empty();
    let mut local_match = false;
    let mut git_match = false;
    for lookup in lookups {
        let Some(reference) = references.get(lookup.reference) else {
            return ResultKind::Unknown;
        };
        match equal(&row.source, &lookup.value, reference.origin == "checkout") {
            Some(true) if reference.origin == "git" => git_match = true,
            Some(true) => local_match = true,
            Some(false) => (),
            None => unknown = true,
        }
    }
    if git_match {
        ResultKind::MatchesTarget
    } else if local_match {
        ResultKind::MatchesReference
    } else if unknown {
        ResultKind::Unknown
    } else {
        ResultKind::Different
    }
}
fn counts(summary: &LocalSummary) -> [(Category, &crate::model::LocalCount); 5] {
    [
        (Category::Staged, &summary.staged),
        (Category::Unstaged, &summary.unstaged),
        (Category::Conflicts, &summary.conflicts),
        (Category::Untracked, &summary.untracked),
        (Category::Ignored, &summary.ignored),
    ]
}
fn derive(
    item: &Item,
    summary: Option<&LocalSummary>,
) -> Result<(Totals, Totals, BTreeMap<Category, Coverage>, bool), SweepError> {
    let mut paths: BTreeMap<&BytePath, Vec<&ResultKind>> = BTreeMap::new();
    let mut layer_totals = Totals::default();
    let mut seen = BTreeSet::new();
    let mut emitted: BTreeMap<Category, BTreeSet<&BytePath>> = BTreeMap::new();
    let mut fully_compared = true;
    for row in &item.rows {
        if !valid_relative(&row.change.path.bytes()?)
            || !row.change.source.valid()
            || (row.change.layer == Layer::Disk
                && matches!(row.change.source, Value::Absent { parent: None }))
            || !seen.insert((
                row.change.path.clone(),
                row.change.category.clone(),
                row.change.stage,
            ))
            || row.lookups.len() != item.references.len()
        {
            return Err(error("invalid or duplicate comparison observation"));
        }
        for (i, lookup) in row.lookups.iter().enumerate() {
            if lookup.reference != i || !lookup.value.valid() {
                return Err(error("invalid reference lookup"));
            }
            let reference = &item.references[i];
            if reference.origin == "checkout"
                && matches!(lookup.value, Value::Absent { parent: None })
            {
                return Err(error("checkout absence lacks parent witness"));
            }
            if !matches!(lookup.value, Value::Unknown { .. }) && reference.checks != "stable" {
                return Err(error("positive lookup from an unvalidated reference"));
            }
            if reference.origin == "git"
                && matches!(lookup.value, Value::Present { .. })
                && lookup.object_oid.as_ref().is_none_or(|s| !valid_oid(s))
            {
                return Err(error("Git lookup lacks full object OID"));
            }
            fully_compared &= equal(
                &row.change.source,
                &lookup.value,
                reference.origin == "checkout",
            )
            .is_some();
        }
        if classify(&row.change, &row.lookups, &item.references) != row.result {
            return Err(error("comparison result differs from saved proofs"));
        }
        fully_compared &= !matches!(row.result, ResultKind::Unknown);
        emitted
            .entry(row.change.category.clone())
            .or_default()
            .insert(&row.change.path);
        paths.entry(&row.change.path).or_default().push(&row.result);
        layer_totals.add(&row.result);
    }
    let mut path_totals = Totals::default();
    for results in paths.values() {
        let result = if results.contains(&&ResultKind::Different) {
            ResultKind::Different
        } else if results.contains(&&ResultKind::Unknown) {
            ResultKind::Unknown
        } else if results.iter().all(|r| **r == ResultKind::MatchesTarget) {
            ResultKind::MatchesTarget
        } else {
            ResultKind::MatchesReference
        };
        path_totals.add(&result);
    }
    let mut coverage = BTreeMap::new();
    let mut complete = item.collection == "collected"
        && item.gaps.is_empty()
        && fully_compared
        && !item.references.is_empty()
        && item.references.iter().all(|r| r.checks == "stable");
    if let Some(summary) = summary {
        complete &= summary.head_complete
            && summary.index_complete
            && summary.disk_complete
            && summary.temporal_consistency != "change_observed"
            && summary.gaps.is_empty();
        for (category, count) in counts(summary) {
            let n = emitted.get(&category).map_or(0, BTreeSet::len);
            if n > count.observed {
                return Err(error("comparison emitted more paths than source observed"));
            }
            let omitted = count.observed - n;
            complete &= count.complete && omitted == 0;
            coverage.insert(
                category,
                Coverage {
                    observed: count.observed,
                    emitted: n,
                    omitted,
                    source_complete: count.complete,
                },
            );
        }
    } else {
        complete = false;
    }
    Ok((path_totals, layer_totals, coverage, complete))
}
pub fn verify(document: &Document, review: &Review) -> Result<(), SweepError> {
    if document.schema != SCHEMA
        || document.review_sha256 != review.digest_sha256
        || document.inventory_sha256 != review.inventory_sha256
        || canonical_digest(document)? != document.digest_sha256
        || canonical_digest(review)? != review.digest_sha256
        || document.started_unix_ms > document.finished_unix_ms
        || document.max_bytes > DOCUMENT_BYTES
        || document.max_rows > 32768
        || document.items.len() != review.items.len()
    {
        return Err(error(
            "invalid comparison schema, binding, interval or limits",
        ));
    }
    bounded_json_size(document, document.max_bytes)?;
    let mut paths = BTreeSet::new();
    for item in &document.items {
        let source = review
            .items
            .iter()
            .find(|i| i.path == item.path)
            .ok_or_else(|| error("comparison path absent from source review"))?;
        if !paths.insert(&item.path)
            || item.rows.len() > document.max_rows
            || !matches!(
                item.collection.as_str(),
                "collected" | "not_requested" | "unavailable"
            )
            || item.summary_sha256.as_ref()
                != source.local_summary.as_ref().map(|s| &s.digest_sha256)
            || (item.collection != "collected" && !item.rows.is_empty())
        {
            return Err(error("invalid source summary binding or collection scope"));
        }
        for reference in &item.references {
            reference.name.bytes()?;
            if reference.origin == "checkout" && reference.checks == "stable" {
                let a = reference
                    .administration
                    .as_ref()
                    .ok_or_else(|| error("checkout lacks administration proof"))?;
                if reference.identity.as_ref() != Some(&a.root.identity)
                    || source
                        .local_summary
                        .as_ref()
                        .and_then(|s| s.worktree_identity.as_ref())
                        == reference.identity.as_ref()
                    || !a.git_dir.to_path()?.is_absolute()
                    || !a.common_dir.to_path()?.is_absolute()
                    || a.root.mode & u32::from(libc::S_IFMT) != u32::from(libc::S_IFDIR)
                    || !a.commondir.valid()
                    || matches!(
                        a.commondir,
                        Value::Unknown { .. } | Value::Absent { parent: None }
                    )
                    || match a.marker.mode & u32::from(libc::S_IFMT) {
                        mode if mode == u32::from(libc::S_IFDIR) => a.marker_sha256.is_some(),
                        mode if mode == u32::from(libc::S_IFREG) => {
                            a.marker_sha256.as_ref().is_none_or(|d| {
                                d.len() != 64 || !d.bytes().all(|b| b.is_ascii_hexdigit())
                            })
                        }
                        _ => true,
                    }
                {
                    return Err(error(
                        "invalid checkout administration proof or self reference",
                    ));
                }
            } else if reference.origin == "git" && reference.administration.is_some() {
                return Err(error("Git target cannot contain checkout administration"));
            }
            if !matches!(reference.origin.as_str(), "git" | "checkout")
                || reference.started_unix_ms > reference.finished_unix_ms
                || !matches!(reference.checks.as_str(), "stable" | "unknown")
                || (reference.origin == "git"
                    && reference.checks == "stable"
                    && (reference.commit_oid.as_ref().is_none_or(|s| !valid_oid(s))
                        || reference.tree_oid.as_ref().is_none_or(|s| !valid_oid(s))))
            {
                return Err(error("invalid captured reference"));
            }
        }
        let (paths, layers, coverage, complete) = derive(item, source.local_summary.as_ref())?;
        if paths != item.path_totals
            || layers != item.layer_totals
            || coverage != item.coverage
            || complete != item.complete
        {
            return Err(error(
                "comparison counts or completeness differ from source evidence",
            ));
        }
    }
    Ok(())
}
pub fn collect(
    inventory: &Inventory,
    review: &Review,
    targets: &crate::review::targets::FamilyTargets,
    captures: &CaptureStore,
    checkout: Option<&BytePath>,
    selected: Option<&BytePath>,
    max_rows: usize,
) -> Result<Document, SweepError> {
    let phase_started = Instant::now();
    let remaining_timeout = captures.borrow().remaining_timeout;
    let checkout_binding = if let Some(reference) = checkout {
        if inventory.families.len() != 1 || selected == Some(reference) {
            return Err(error(
                "reference checkout requires one family and cannot be the selected worktree",
            ));
        }
        let binding = super::summary::validate_checkout(
            &reference.to_path()?,
            &inventory.families[0],
            remaining_timeout,
        )?;
        if selected.is_some_and(|path| {
            inventory.families[0]
                .worktrees
                .iter()
                .any(|w| &w.path == path && w.identity.as_ref() == Some(&binding.identity))
        }) {
            return Err(error(
                "reference checkout cannot be the selected physical worktree",
            ));
        }
        Some(binding)
    } else {
        None
    };
    if inventory
        .families
        .iter()
        .any(|f| targets.for_family(f).is_empty())
        && checkout.is_none()
    {
        return Err(error(
            "comparison requires explicit integration targets or a reference checkout",
        ));
    }
    let started = now_ms();
    let mut document = Document {
        schema: SCHEMA.into(),
        inventory_sha256: review.inventory_sha256.clone(),
        review_sha256: review.digest_sha256.clone(),
        started_unix_ms: started,
        finished_unix_ms: started,
        max_bytes: DOCUMENT_BYTES,
        max_rows,
        items: Vec::new(),
        digest_sha256: String::new(),
    };
    // Reserve bounded metadata first; details consume only the remaining run envelope.
    for source in &review.items {
        let collection = if selected.is_some_and(|p| p != &source.path)
            || checkout == Some(&source.path)
            || checkout_binding.as_ref().is_some_and(|binding| {
                inventory.families[0].worktrees.iter().any(|w| {
                    w.path == source.path && w.identity.as_ref() == Some(&binding.identity)
                })
            }) {
            "not_requested"
        } else {
            "unavailable"
        };
        let mut item = Item {
            path: source.path.clone(),
            collection: collection.into(),
            summary_sha256: source
                .local_summary
                .as_ref()
                .map(|s| s.digest_sha256.clone()),
            references: Vec::new(),
            rows: Vec::new(),
            coverage: BTreeMap::new(),
            path_totals: Totals::default(),
            layer_totals: Totals::default(),
            complete: false,
            gaps: Vec::new(),
            applied_limits: BTreeMap::new(),
        };
        let (p, l, c, done) = derive(&item, source.local_summary.as_ref())?;
        item.path_totals = p;
        item.layer_totals = l;
        item.coverage = c;
        item.complete = done;
        document.items.push(item);
    }
    let mut reserved = bounded_json_size(&document, DOCUMENT_BYTES)?;
    let mut state = captures.borrow_mut();
    for item in &mut document.items {
        if item.collection == "not_requested" {
            continue;
        }
        let Some(mut capture) = state.captures.remove(&item.path) else {
            item.gaps.push("source observations unavailable".into());
            continue;
        };
        let source = review
            .items
            .iter()
            .find(|i| i.path == item.path)
            .ok_or_else(|| error("source disappeared"))?;
        let family = inventory
            .families
            .iter()
            .find(|f| f.worktrees.iter().any(|w| w.path == item.path))
            .ok_or_else(|| error("family missing"))?;
        let capacity = DOCUMENT_BYTES.saturating_sub(reserved + 16384);
        capture.remaining.timeout = capture
            .remaining
            .timeout
            .min(remaining_timeout.saturating_sub(phase_started.elapsed()));
        item.applied_limits = [
            ("remaining_bytes", capture.remaining.max_total_bytes),
            ("max_file_bytes", capture.remaining.max_file_bytes),
            ("remaining_objects", capture.remaining.max_objects as u64),
            ("max_paths", capture.remaining.max_paths as u64),
            ("max_depth", capture.remaining.max_depth as u64),
            ("timeout_ms", capture.remaining.timeout.as_millis() as u64),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect();
        let source_valid = source.local_summary.as_ref().is_some_and(|s| {
            s.digest_sha256 == capture.summary_sha256 && s.temporal_consistency != "change_observed"
        });
        let (references, rows, gaps) = super::summary::compare_capture(
            &item.path,
            capture,
            targets.for_family(family),
            checkout_binding.as_ref(),
            source_valid,
            capacity,
        )?;
        item.collection = "collected".into();
        item.references = references;
        item.rows = rows;
        item.gaps = gaps;
        let (p, l, c, done) = derive(item, source.local_summary.as_ref())?;
        item.path_totals = p;
        item.layer_totals = l;
        item.coverage = c;
        item.complete = done;
        reserved = reserved.saturating_add(bounded_json_size(item, DOCUMENT_BYTES)?);
        if reserved > DOCUMENT_BYTES {
            return Err(error(
                "comparison metadata exceeds bounded publication allowance",
            ));
        }
    }
    document.finished_unix_ms = now_ms();
    document.digest_sha256 = canonical_digest(&document)?;
    verify(&document, review)?;
    Ok(document)
}
struct ReportText(crate::output::BoundedText);
impl ReportText {
    fn new(s: &str, limit: usize) -> Self {
        let mut out = Self(crate::output::BoundedText::new(limit));
        out.push_str(s);
        out
    }
    fn push_str(&mut self, s: &str) {
        let _ = std::fmt::Write::write_str(&mut self.0, s);
    }
    fn finish(self) -> Result<String, SweepError> {
        self.0.finish()
    }
}
impl Document {
    pub fn partial(&self) -> bool {
        self.items
            .iter()
            .any(|i| i.collection != "not_requested" && !i.complete)
    }
    pub fn text_bounded(&self, limit: usize) -> Result<String, SweepError> {
        let mut out = ReportText::new(
            "\nQue reste-t-il à récupérer ?\nComparaison du contenu local, sans autorisation de retrait.\n",
            limit,
        );
        for item in &self.items {
            out.push_str(&format!(
                "\n{}: {} ({})\n  {:?}\n",
                item.path.display(),
                if item.complete { "complete" } else { "partial" },
                item.collection,
                item.path_totals
            ));
            for row in &item.rows {
                out.push_str(&format!(
                    "  {} [{:?}/{:?}] {:?}\n",
                    row.change.path.display(),
                    row.change.category,
                    row.change.layer,
                    serde_json::to_string(&row.result).unwrap_or_default()
                ));
                out.push_str(&format!("    Source: {:?}\n", row.change.source));
                for lookup in &row.lookups {
                    let reference = item
                        .references
                        .get(lookup.reference)
                        .ok_or_else(|| error("invalid reference index in text rendering"))?;
                    out.push_str(&format!(
                        "    {} ({}): {:?}\n",
                        reference.name.display(),
                        reference.origin,
                        lookup.value
                    ));
                }
            }
            for gap in &item.gaps {
                out.push_str(&format!("  Non collecté : {gap}\n"));
            }
        }
        out.finish()
    }
    pub fn html_bounded(&self, limit: usize) -> Result<String, SweepError> {
        fn esc(s: &str) -> String {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&#39;")
        }
        let mut out = ReportText::new(
            "<details class=\"report-section\" id=\"local-comparison\" open><summary><h2>Que reste-t-il à récupérer ?</h2></summary><div class=\"section-body\"><p>Comparaison des changements locaux avec les références choisies. Un doublon local ne prouve ni une fusion ni une sauvegarde. Les verdicts Git restent inchangés. Ce bilan porte sur la collecte source, indépendamment des filtres de la liste.</p>",
            limit,
        );
        for item in &self.items {
            out.push_str(&format!("<article><h3><code>{}</code></h3><p>{}. {} dans une cible Git ; {} dans la copie locale ; {} différents ; {} non comparés.</p>",esc(&item.path.display()),if item.complete {"Comparaison complète"} else if item.collection == "not_requested" {"Comparaison non demandée"} else {"Comparaison incomplète"},item.path_totals.matches_target,item.path_totals.matches_reference,item.path_totals.different,item.path_totals.unknown));
            if !item.complete {
                out.push_str("<p>Les nombres sont des observations partielles. Un zéro ne signifie pas qu'il ne reste rien à examiner.</p>");
            }
            out.push_str("<ul>");
            for gap in &item.gaps {
                out.push_str(&format!("<li>{}</li>", esc(gap)));
            }
            for (category, c) in &item.coverage {
                if c.omitted > 0 {
                    out.push_str(&format!(
                        "<li>{:?} : {} chemins observés sans détail de comparaison.</li>",
                        category, c.omitted
                    ));
                }
            }
            out.push_str("</ul><table><thead><tr><th>Chemin</th><th>Changement</th><th>Résultat</th><th>Référence et preuve</th></tr></thead><tbody>");
            for row in &item.rows {
                let label = match row.result {
                    ResultKind::MatchesTarget => "Présent dans la cible Git",
                    ResultKind::MatchesReference => "Présent dans la copie locale",
                    ResultKind::Different => "Différent : à examiner",
                    ResultKind::Unknown => "Non comparé : à vérifier",
                };
                out.push_str(&format!("<tr><td><code>{}</code></td><td>{:?} / {:?}</td><td>{label}</td><td><details><summary>Voir les références et empreintes</summary>",esc(&row.change.path.display()),row.change.category,row.change.layer));
                out.push_str(&format!(
                    "<p>Observation source</p><pre>{}</pre>",
                    esc(&format!("{:?}", row.change.source))
                ));
                for lookup in &row.lookups {
                    let r = item
                        .references
                        .get(lookup.reference)
                        .ok_or_else(|| error("invalid reference index in HTML rendering"))?;
                    out.push_str(&format!(
                        "<p><code>{}</code> ({})</p><pre>{}</pre>",
                        esc(&r.name.display()),
                        esc(&r.origin),
                        esc(&format!("{:?}", lookup.value))
                    ));
                }
                out.push_str("</details></td></tr>");
            }
            out.push_str("</tbody></table></article>");
        }
        out.push_str("<p>Examinez les fichiers différents. Pour les observations manquantes, relancez une revue sur le worktree exact. Même une comparaison complète ne rend pas le worktree supprimable.</p></div></details>");
        out.finish()
    }
}
