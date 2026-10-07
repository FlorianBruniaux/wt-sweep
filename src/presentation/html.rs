//! Pure, bounded offline HTML projection of captured observations.
//! Historical family bindings label rows; they do not prove current identity.
use crate::{
    disk::{DiskKind, DiskMeasurement, DiskUsage},
    model::*,
    output::BoundedText,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Write},
    io,
    path::Path,
};

const CSS: &str = include_str!("html/report.css");
const JS: &str = include_str!("html/report.js");
type FamilyKey<'a> = (&'a BytePath, u64, u64);
// bool distinguishes shared Git from worktree measurements without changing the model.
type MeasurementIndex<'a> = BTreeMap<(&'a BytePath, bool), Vec<&'a DiskMeasurement>>;

struct Html {
    out: BoundedText,
    invalid: Option<SweepError>,
}
impl Html {
    fn new(limit: usize) -> Self {
        Self {
            out: BoundedText::new(limit),
            invalid: None,
        }
    }
    fn raw(&mut self, value: &str) -> fmt::Result {
        self.out.write_str(value)
    }
    fn text(&mut self, value: &str) -> fmt::Result {
        for c in value.chars() {
            match c {
                '&' => self.raw("&amp;")?,
                '<' => self.raw("&lt;")?,
                '>' => self.raw("&gt;")?,
                '"' => self.raw("&quot;")?,
                '\'' => self.raw("&#39;")?,
                '\n' => self.raw("\\n")?,
                '\r' => self.raw("\\r")?,
                '\t' => self.raw("\\t")?,
                c if c.is_control()
                    || ('\u{202a}'..='\u{202e}').contains(&c)
                    || ('\u{2066}'..='\u{2069}').contains(&c) =>
                {
                    write!(self.out, "\\u{{{:x}}}", c as u32)?
                }
                c => self.out.write_char(c)?,
            }
        }
        Ok(())
    }
    fn path(&mut self, path: &BytePath) -> fmt::Result {
        // Decode directly into bounded output; never allocate a decoded path/display.
        let hex = path.hex.as_bytes();
        if hex.len() % 2 != 0
            || !hex
                .iter()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
        {
            return self.bad_path();
        }
        for pair in hex.chunks_exact(2) {
            let digit = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
            let byte = digit(pair[0]) * 16 + digit(pair[1]);
            match byte {
                0 => return self.bad_path(),
                b'\n' => self.raw("\\n")?,
                b'\r' => self.raw("\\r")?,
                b'\t' => self.raw("\\t")?,
                b'\\' => self.raw("\\\\")?,
                b'&' => self.raw("&amp;")?,
                b'<' => self.raw("&lt;")?,
                b'>' => self.raw("&gt;")?,
                b'"' => self.raw("&quot;")?,
                b'\'' => self.raw("&#39;")?,
                32..=126 => self.out.write_char(char::from(byte))?,
                _ => write!(self.out, "\\x{byte:02x}")?,
            }
        }
        Ok(())
    }
    fn bad_path(&mut self) -> fmt::Result {
        self.invalid = Some(error(
            "invalid_path",
            "path must be canonical lowercase hex without NUL",
        ));
        Err(fmt::Error)
    }
    fn json_text(&mut self, value: &str) -> fmt::Result {
        for c in value.chars() {
            match c {
                '&' => self.raw("&amp;")?,
                '<' => self.raw("&lt;")?,
                '>' => self.raw("&gt;")?,
                '"' => self.raw("&quot;")?,
                '\'' => self.raw("&#39;")?,
                c if (c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                    || ('\u{202a}'..='\u{202e}').contains(&c)
                    || ('\u{2066}'..='\u{2069}').contains(&c) =>
                {
                    write!(self.out, "\\u{:04x}", c as u32)?
                }
                c => self.out.write_char(c)?,
            }
        }
        Ok(())
    }
    fn json<T: Serialize>(&mut self, value: &T) -> fmt::Result {
        struct EscapedJson<'a>(&'a mut Html);
        impl io::Write for EscapedJson<'_> {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                let s = std::str::from_utf8(bytes).map_err(io::Error::other)?;
                self.0
                    .json_text(s)
                    .map_err(|_| io::Error::other("HTML output limit"))?;
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        self.raw("<details><summary>Recorded evidence (JSON)</summary><pre>")?;
        // Serialization is streamed through the same bounded escaping sink.
        serde_json::to_writer_pretty(EscapedJson(self), value).map_err(|_| fmt::Error)?;
        self.raw("</pre></details>")
    }
    fn complete(self, rendered: fmt::Result) -> Result<String, SweepError> {
        let output = self.finish()?;
        rendered.map_err(|_| {
            error(
                "html_serialization",
                "HTML rendering failed; no partial document returned",
            )
        })?;
        Ok(output)
    }
    fn finish(self) -> Result<String, SweepError> {
        if let Some(error) = self.invalid {
            Err(error)
        } else {
            self.out.finish()
        }
    }
}
fn error(code: &str, message: &str) -> SweepError {
    SweepError::new(code, "render_html", Path::new(""), message)
}

/// Render captured inventory facts. Does no collection, Git execution or judgement.
pub fn inventory(source: &Inventory, limit: usize) -> Result<String, SweepError> {
    let mut html = Html::new(limit);
    let rendered = inventory_into(&mut html, source);
    html.complete(rendered)
}

/// Render selected judged rows while retaining full source totals and coverage.
pub fn review(view: &ReviewView, limit: usize) -> Result<String, SweepError> {
    let mut html = Html::new(limit);
    // Refuse a tiny limit before building indexes proportional to source rows.
    if start(
        &mut html,
        "Worktree review",
        view.source.started_unix_ms,
        view.source.finished_unix_ms,
    )
    .is_err()
    {
        return html.finish();
    }
    crate::view::validate_filters(&view.filters, &view.group_by)?;
    let mut bindings = BTreeMap::new();
    let mut family_identities = BTreeMap::new();
    for row in &view.row_bindings {
        if bindings.insert(&row.path, row).is_some() {
            return Err(error(
                "invalid_review_view",
                "duplicate historical row binding",
            ));
        }
        let id = (row.identity.device, row.identity.inode);
        if family_identities
            .insert(&row.common_dir, id)
            .is_some_and(|previous| previous != id)
        {
            return Err(error(
                "invalid_review_view",
                "contradictory historical family identity",
            ));
        }
    }
    let mut source_rows = BTreeMap::new();
    let mut families: BTreeMap<FamilyKey<'_>, Vec<&ReviewItem>> = BTreeMap::new();
    for row in &view.source.items {
        let Some(binding) = bindings.get(&row.path) else {
            return Err(error(
                "invalid_review_view",
                "source row has no historical binding",
            ));
        };
        if source_rows.insert(&row.path, row).is_some() {
            return Err(error("invalid_review_view", "duplicate source row"));
        }
        families
            .entry((
                &binding.common_dir,
                binding.identity.device,
                binding.identity.inode,
            ))
            .or_default();
    }
    if source_rows.len() != bindings.len() {
        return Err(error(
            "invalid_review_view",
            "historical binding has no source row",
        ));
    }
    let mut selected = BTreeSet::new();
    for path in &view.selected_paths {
        if !selected.insert(path) {
            return Err(error("invalid_review_view", "duplicate selected row"));
        }
        let Some(row) = source_rows.get(path) else {
            return Err(error(
                "invalid_review_view",
                "selected row is absent from source",
            ));
        };
        let binding = bindings[path];
        families
            .entry((
                &binding.common_dir,
                binding.identity.device,
                binding.identity.inode,
            ))
            .or_default()
            .push(row);
    }
    let rendered = review_into(&mut html, view, &families);
    html.complete(rendered)
}

fn start(h: &mut Html, title: &str, started: u64, finished: u64) -> fmt::Result {
    h.raw("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"color-scheme\" content=\"light dark\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; object-src 'none'\"><title>")?;
    h.text(title)?;
    h.raw(" | Worktree Sweep</title><style>")?;
    h.raw(CSS)?;
    h.raw("</style></head><body><a class=\"skip\" href=\"#content\">Skip to report</a><div class=\"layout\"><aside class=\"sidebar\" aria-label=\"Report navigation\"><div class=\"brand\"><span class=\"mark\" aria-hidden=\"true\"></span>wt-sweep</div><p class=\"eyebrow\">Read-only report</p><nav aria-label=\"Contents\"><a href=\"#overview\">Overview</a><a href=\"#worktree-families\">Git families</a><a href=\"#disk\">Disk observations</a><a href=\"#coverage\">Coverage &amp; evidence</a></nav><p class=\"boundary\">Captured facts, deterministic verdicts.<br>No removal authorized.<br>Offline; no external resources.</p></aside><main class=\"main\" id=\"content\"><header class=\"top\" id=\"overview\"><div><p class=\"eyebrow\">Worktree Sweep / captured observations</p><h1>")?;
    h.text(title)?;
    h.raw("</h1><p class=\"meta\">Observation interval: ")?;
    date(h, started)?;
    h.raw(" to ")?;
    date(h, finished)?;
    write!(
        h.out,
        "<br>Unix ms: {started}..{finished}</p></div><button class=\"theme js-only\" id=\"theme\" type=\"button\">Switch theme</button></header>"
    )?;
    h.raw("<div class=\"notice\"><strong>No removal authorized.</strong> A cleanup candidate still needs a usage check. Writer exclusion, durable backup and independent restore are not established by this report. Observations are not an atomic snapshot.</div>")
}
fn date(h: &mut Html, ms: u64) -> fmt::Result {
    // Gregorian civil date from Unix days; arithmetic remains valid for all u64 ms.
    let seconds = ms / 1000;
    let z = i128::from(seconds / 86400) + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + i128::from(month <= 2);
    write!(
        h.out,
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}.{:03} UTC",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60,
        ms % 1000
    )
}
fn end(h: &mut Html) -> fmt::Result {
    h.raw("<footer class=\"footnote\">Generated locally from captured observations. Git family grouping uses historical registrations; it does not establish current controls. Path labels use reversible byte escapes; full hex identities remain in recorded evidence.</footer></main></div><script>")?;
    h.raw(JS)?;
    h.raw("</script></body></html>\n")
}
fn controls(h: &mut Html, triaged: bool, selected: usize) -> fmt::Result {
    h.raw("<div class=\"toolbar js-only\"><label class=\"field search\">Search recorded rows<input id=\"search\" type=\"search\" placeholder=\"Path, branch, reason, full OID…\" aria-controls=\"worktree-families\"></label><label class=\"field\">Verdict<select id=\"verdict\" aria-controls=\"worktree-families\"><option value=\"\">All verdicts</option>")?;
    if triaged {
        h.raw("<option value=\"keep\">keep</option><option value=\"verify\">verify</option><option value=\"cleanup_candidate\">cleanup_candidate</option>")?;
    } else {
        h.raw("<option value=\"not_reviewed\">Not triaged</option>")?;
    }
    h.raw("</select></label><label class=\"field\">Sort within families<select id=\"sort\" aria-controls=\"worktree-families\"><option value=\"path\">Path bytes</option><option value=\"branch\">Branch bytes</option><option value=\"verdict\">Verdict</option><option value=\"allocated\">Allocated bytes, largest first</option></select></label></div>")?;
    write!(
        h.out,
        "<p class=\"view-status\" id=\"visible-status\" role=\"status\" aria-live=\"polite\">Visible {selected} selected worktrees. Source totals and disk measurements are unchanged.</p>"
    )?;
    h.raw("<noscript><p class=\"notice\">JavaScript is disabled. All selected rows and recorded evidence remain readable; family links and expandable details work. Search, sorting and the theme button require JavaScript.</p></noscript><p id=\"no-matches\" class=\"empty\" hidden>No rows match. Clear the search or verdict filter.</p>")
}
fn counts<'a>(h: &mut Html, rows: impl Iterator<Item = &'a ReviewItem>) -> fmt::Result {
    let (mut keep, mut verify, mut candidates, mut other) = (0, 0, 0, 0);
    for row in rows {
        match row.classification.as_str() {
            "keep" => keep += 1,
            "verify" => verify += 1,
            "cleanup_candidate" => candidates += 1,
            _ => other += 1,
        }
    }
    write!(
        h.out,
        "{keep} keep · {verify} verify · {candidates} cleanup_candidate"
    )?;
    if other > 0 {
        write!(h.out, " · {other} other/unknown")?;
    }
    Ok(())
}
fn inventory_into(h: &mut Html, source: &Inventory) -> fmt::Result {
    start(
        h,
        "Worktree inventory",
        source.started_unix_ms,
        source.finished_unix_ms,
    )?;
    let total: usize = source.families.iter().map(|f| f.worktrees.len()).sum();
    write!(
        h.out,
        "<div class=\"metrics\"><div class=\"metric\"><span class=\"eyebrow\">Source totals</span><strong>{total} worktrees</strong><span class=\"breakdown\">{} Git families in published inventory</span></div><div class=\"metric\"><span class=\"eyebrow\">Selected totals</span><strong>{total} worktrees</strong><span class=\"breakdown\">Full published inventory</span></div><div class=\"metric\"><span class=\"eyebrow\">Triage</span><strong>Not triaged</strong><span class=\"breakdown\">scan records facts; review or plan supplies verdicts.</span></div></div>",
        source.families.len()
    )?;
    h.raw("<details class=\"panel\"><summary>Requested scan scope</summary>")?;
    match &source.scope {
        ScanScope::Repo { path } => {
            h.raw("Repository: <code>")?;
            h.path(path)?;
            h.raw("</code>")?;
        }
        ScanScope::Roots { paths } => {
            h.raw("Explicit roots:<ul class=\"coverage-list\">")?;
            for path in paths {
                h.raw("<li><code>")?;
                h.path(path)?;
                h.raw("</code></li>")?;
            }
            h.raw("</ul>")?;
        }
        ScanScope::AllMounted => h.raw(
            "All visible mounted volumes; unavailable and non-visible data are not certified.",
        )?,
    }
    h.raw("</details>")?;
    controls(h, false, total)?;
    h.raw("<div id=\"worktree-families\"><h2 class=\"section-title\">Git families</h2>")?;
    let sizes = measurements(source.disk_usage.as_ref());
    let mut families: Vec<_> = source.families.iter().collect();
    families.sort_by_key(|f| (&f.common_dir, f.identity.device, f.identity.inode));
    family_index(
        h,
        families
            .iter()
            .enumerate()
            .map(|(index, f)| (index, &f.common_dir)),
    )?;
    for (index, family) in families.into_iter().enumerate() {
        family_start(
            h,
            index,
            &family.common_dir,
            &family.identity,
            family.worktrees.len(),
            family.worktrees.len(),
            &sizes,
        )?;
        let mut rows: Vec<_> = family.worktrees.iter().collect();
        rows.sort_by_key(|row| &row.path);
        for row in rows {
            row_start(h, &row.path, row.branch.as_ref(), "not_reviewed", &sizes)?;
            path_cell(h, &row.path, row.head.as_deref())?;
            branch_cell(h, row.branch.as_ref(), Some(row))?;
            h.raw("<td data-label=\"Verdict\"><span class=\"badge\">Not triaged</span><p class=\"muted evidence\">Discovery only. No deterministic review verdict.</p></td>")?;
            disk_cell(h, &row.path, &sizes)?;
            h.raw("<td data-label=\"Evidence\"><details><summary>Registration details</summary><dl class=\"evidence\"><dt>Directory state</dt><dd>")?;
            directory_state(h, &row.path_state)?;
            h.raw("</dd></dl>")?;
            h.json(row)?;
            h.raw("</details></td></tr>")?;
        }
        family_end(h)?;
    }
    if total == 0 {
        h.raw("<p class=\"empty\">No worktrees selected. Source coverage and omissions remain below.</p>")?;
    }
    h.raw("</div>")?;
    disk_overview(h, source.disk_usage.as_ref())?;
    coverage(h, &source.coverage, &source.not_collected)?;
    h.raw("<details class=\"panel\"><summary>Additional inventory evidence</summary>")?;
    h.json(&source.markers)?;
    h.raw("<p>Local manifests are historical observations, not a backup. Local bytes have not been revalidated during rendering.</p>")?;
    write!(
        h.out,
        "<p>{} recorded local manifests.</p>",
        source.local_inventories.len()
    )?;
    for manifest in &source.local_inventories {
        h.raw("<details><summary>Local manifest: ")?;
        h.path(&manifest.worktree)?;
        h.raw("</summary><p>Traversal: ")?;
        h.raw(if manifest.traversal_complete {
            "complete"
        } else {
            "incomplete"
        })?;
        h.raw(". Temporal consistency: ")?;
        h.text(&manifest.temporal_consistency)?;
        write!(
            h.out,
            ". Unix ms {}..{}.</p><p>{} file entries, {} index entries, {} administration entries recorded.</p>",
            manifest.started_unix_ms,
            manifest.finished_unix_ms,
            manifest.entries.len(),
            manifest.index_entries.len(),
            manifest.administration_entries.len()
        )?;
        h.raw("<p class=\"digest\">Manifest content SHA-256: <code>")?;
        h.text(&manifest.content_digest_sha256)?;
        h.raw("</code></p>")?;
        write!(
            h.out,
            "<h3>Captured gaps: {}</h3><ul class=\"coverage-list\">",
            manifest.gaps.len()
        )?;
        for gap in &manifest.gaps {
            h.raw("<li><code>")?;
            h.path(&gap.path)?;
            h.raw("</code>: ")?;
            h.text(&gap.reason)?;
            h.raw("</li>")?;
        }
        write!(
            h.out,
            "</ul><h3>Not collected: {}</h3>",
            manifest.not_collected.len()
        )?;
        list(h, &manifest.not_collected)?;
        h.raw("</details>")?;
    }
    h.raw("</details>")?;
    end(h)
}
fn review_into(
    h: &mut Html,
    view: &ReviewView,
    families: &BTreeMap<FamilyKey<'_>, Vec<&ReviewItem>>,
) -> fmt::Result {
    let source = &view.source;
    write!(
        h.out,
        "<div class=\"metrics\"><div class=\"metric\"><span class=\"eyebrow\">Source totals</span><strong>{} worktrees</strong><span class=\"breakdown\">",
        source.items.len()
    )?;
    counts(h, source.items.iter())?;
    write!(
        h.out,
        "</span></div><div class=\"metric\"><span class=\"eyebrow\">Selected totals</span><strong>{} worktrees</strong><span class=\"breakdown\">",
        view.selected_paths.len()
    )?;
    counts(h, families.values().flat_map(|rows| rows.iter().copied()))?;
    write!(
        h.out,
        "</span></div><div class=\"metric\"><span class=\"eyebrow\">Policy</span><strong>"
    )?;
    h.text(&source.policy_version)?;
    h.raw("</strong><span class=\"breakdown\">Deterministic verdicts, no model decision</span></div></div>")?;
    h.raw("<div class=\"legend\"><span><span class=\"badge keep\">keep</span> Positive retention reason observed.</span><span><span class=\"badge verify\">verify</span> Required evidence is missing or inconsistent.</span><span><span class=\"badge cleanup_candidate\">cleanup_candidate</span> Current C1-C5 checks satisfied; usage remains unverified.</span></div>")?;
    h.raw("<details class=\"panel\"><summary>View filters and source identity</summary><p>HTML groups rows by historical Git family. The saved view grouping is <code>")?;
    h.text(&view.group_by)?;
    h.raw(
        "</code>. Filters select rows only; coverage and disk totals retain the source scope.</p>",
    )?;
    h.json(&view.filters)?;
    for (label, digest) in [
        ("Inventory SHA-256", &source.inventory_sha256),
        ("Review SHA-256", &source.digest_sha256),
        ("View SHA-256", &view.digest_sha256),
    ] {
        h.raw("<p class=\"digest\">")?;
        h.text(label)?;
        h.raw(": <code>")?;
        h.text(digest)?;
        h.raw("</code></p>")?;
    }
    h.raw("</details>")?;
    controls(h, true, view.selected_paths.len())?;
    h.raw("<div id=\"worktree-families\"><h2 class=\"section-title\">Git families</h2>")?;
    family_index(
        h,
        families
            .iter()
            .enumerate()
            .filter(|(_, (_, rows))| !rows.is_empty())
            .map(|(index, ((common, _, _), _))| (index, *common)),
    )?;
    let sizes = measurements(source.disk_usage.as_ref());
    let mut source_counts: BTreeMap<&BytePath, usize> = BTreeMap::new();
    for binding in &view.row_bindings {
        *source_counts.entry(&binding.common_dir).or_default() += 1;
    }
    for (index, ((common, device, inode), rows)) in families.iter().enumerate() {
        if rows.is_empty() {
            continue;
        }
        let source_total = source_counts[common];
        family_start(
            h,
            index,
            common,
            &Identity {
                device: *device,
                inode: *inode,
            },
            source_total,
            rows.len(),
            &sizes,
        )?;
        let mut ordered = rows.clone();
        ordered.sort_by_key(|row| &row.path);
        for row in ordered {
            row_start(
                h,
                &row.path,
                row.branch.as_ref(),
                &row.classification,
                &sizes,
            )?;
            path_cell(h, &row.path, row.head.as_deref())?;
            branch_cell(
                h,
                row.branch.as_ref(),
                row.worktree_state
                    .as_ref()
                    .and_then(|s| s.observed.as_ref()),
            )?;
            h.raw("<td data-label=\"Verdict and reasons\"><span class=\"badge ")?;
            h.raw(match row.classification.as_str() {
                "keep" => "keep",
                "verify" => "verify",
                "cleanup_candidate" => "cleanup_candidate",
                _ => "",
            })?;
            h.raw("\">")?;
            h.text(&row.classification)?;
            h.raw("</span>")?;
            if let Some(decision) = &row.decision {
                h.raw("<ul class=\"reasons\">")?;
                for reason in &decision.keep_reasons {
                    h.raw("<li><code>")?;
                    h.text(reason)?;
                    h.raw("</code>: ")?;
                    h.text(super::keep_reason(reason))?;
                    h.raw("</li>")?;
                }
                for condition in &decision.missing_conditions {
                    h.raw("<li>Missing <code>")?;
                    h.text(condition)?;
                    h.raw("</code>: ")?;
                    h.text(super::next_check(condition))?;
                    h.raw("</li>")?;
                }
                h.raw("</ul>")?;
            } else {
                h.raw("<p class=\"muted evidence\">Decision conditions UNKNOWN</p>")?;
            }
            if row.classification == "cleanup_candidate" {
                h.raw("<p class=\"evidence\">Usage UNKNOWN. Confirm before removal.</p>")?;
            }
            h.raw("</td>")?;
            disk_cell(h, &row.path, &sizes)?;
            review_evidence(h, row)?;
            h.raw("</tr>")?;
        }
        family_end(h)?;
    }
    if view.selected_paths.is_empty() {
        h.raw("<p class=\"empty\">No worktrees selected. Source totals, coverage and disk observations are retained.</p>")?;
    }
    h.raw("</div>")?;
    disk_overview(h, source.disk_usage.as_ref())?;
    coverage(h, &source.coverage, &source.not_collected)?;
    h.raw("<details class=\"panel\"><summary>Shared and descriptive evidence</summary><p>Shared stashes and descriptive context are source observations; they do not change a verdict or prove integration.</p>")?;
    h.json(&source.shared_stashes)?;
    h.json(&source.context)?;
    h.json(&source.local_inventory_refs)?;
    h.raw("</details>")?;
    end(h)
}
fn family_index<'a>(
    h: &mut Html,
    families: impl Iterator<Item = (usize, &'a BytePath)>,
) -> fmt::Result {
    h.raw("<details class=\"panel\"><summary>Family index</summary><nav aria-label=\"Git family index\"><ul class=\"coverage-list\">")?;
    for (index, common) in families {
        write!(h.out, "<li><a class=\"mono\" href=\"#family-{index}\">")?;
        h.path(common)?;
        h.raw("</a></li>")?;
    }
    h.raw("</ul></nav></details>")
}
fn family_start(
    h: &mut Html,
    index: usize,
    common: &BytePath,
    identity: &Identity,
    source_count: usize,
    selected: usize,
    sizes: &MeasurementIndex<'_>,
) -> fmt::Result {
    write!(
        h.out,
        "<section class=\"family\" id=\"family-{index}\" aria-labelledby=\"family-title-{index}\"><header class=\"family-header\"><div><p class=\"eyebrow\">Git family {:02}</p><h2 id=\"family-title-{index}\" class=\"mono\">",
        index + 1
    )?;
    h.path(common)?;
    write!(
        h.out,
        "</h2><p class=\"identity muted\">Historical device {} / inode {}. Current identity is not established by grouping.</p></div><div class=\"family-summary\"><span class=\"visible-count\">{selected}</span> visible / {selected} selected / {source_count} source rows",
        identity.device, identity.inode
    )?;
    if let Some(rows) = sizes.get(&(common, true)) {
        for size in rows {
            h.raw("<p>Shared Git: ")?;
            disk_size(h, Some(size))?;
            h.raw("</p>")?;
        }
    }
    h.raw("</div></header><div class=\"table-wrap\"><table><caption>Recorded worktrees. Row sizes may overlap; only the source disk total deduplicates.</caption><thead><tr><th scope=\"col\">Worktree / HEAD</th><th scope=\"col\">Branch</th><th scope=\"col\">Verdict / reasons</th><th scope=\"col\">Disk observations</th><th scope=\"col\">Evidence / gaps</th></tr></thead><tbody>")
}
fn family_end(h: &mut Html) -> fmt::Result {
    h.raw("</tbody></table></div></section>")
}
fn measurements(usage: Option<&DiskUsage>) -> MeasurementIndex<'_> {
    let mut index = MeasurementIndex::new();
    if let Some(usage) = usage {
        for row in &usage.measurements {
            index
                .entry((&row.path, row.kind == DiskKind::GitCommon))
                .or_default()
                .push(row);
        }
    }
    index
}
fn measurement<'a>(sizes: &MeasurementIndex<'a>, path: &BytePath) -> Option<&'a DiskMeasurement> {
    sizes
        .get(&(path, false))
        .and_then(|rows| rows.first().copied())
}
fn row_start(
    h: &mut Html,
    path: &BytePath,
    branch: Option<&BytePath>,
    verdict: &str,
    sizes: &MeasurementIndex<'_>,
) -> fmt::Result {
    h.raw("<tr data-worktree=\"\" data-path-hex=\"")?;
    h.text(&path.hex)?;
    h.raw("\" data-verdict=\"")?;
    h.text(verdict)?;
    h.raw("\" data-branch=\"")?;
    if let Some(branch) = branch {
        h.text(&branch.hex)?;
    }
    h.raw("\" data-allocated=\"")?;
    if let Some(bytes) = measurement(sizes, path).and_then(|r| r.bytes.as_ref()) {
        write!(h.out, "{}", bytes.allocated)?;
    }
    h.raw("\">")
}
fn path_cell(h: &mut Html, path: &BytePath, head: Option<&str>) -> fmt::Result {
    h.raw("<td data-label=\"Worktree and full HEAD\"><span class=\"path mono\">")?;
    h.path(path)?;
    h.raw("</span><span class=\"head mono\">HEAD ")?;
    h.text(head.unwrap_or("UNKNOWN"))?;
    h.raw("</span></td>")
}
fn branch_cell(h: &mut Html, branch: Option<&BytePath>, tree: Option<&Worktree>) -> fmt::Result {
    h.raw("<td data-label=\"Branch\"><span class=\"mono\">")?;
    if let Some(branch) = branch {
        h.path(branch)?;
    } else if tree.is_some_and(|t| t.detached) {
        h.raw("HEAD detached")?;
    } else if tree.is_some_and(|t| t.bare) {
        h.raw("Bare repository; no worktree branch")?;
    } else {
        h.raw("Branch UNKNOWN; detached state unconfirmed")?;
    }
    h.raw("</span></td>")
}
fn disk_size(h: &mut Html, row: Option<&DiskMeasurement>) -> fmt::Result {
    let Some(row) = row else {
        return h.raw("Not measured; size UNKNOWN");
    };
    if let Some(bytes) = &row.bytes {
        write!(
            h.out,
            "<strong>{} B allocated</strong><small>{} B logical</small>",
            bytes.allocated, bytes.logical
        )?;
        h.raw(if row.complete {
            "<small>complete</small>"
        } else {
            "<small>partial observation, not a complete size</small>"
        })?;
    } else {
        h.raw("Not measured; size UNKNOWN")?;
    }
    Ok(())
}
fn disk_cell(h: &mut Html, path: &BytePath, sizes: &MeasurementIndex<'_>) -> fmt::Result {
    h.raw("<td class=\"size\" data-label=\"Allocated and logical bytes\">")?;
    disk_size(h, measurement(sizes, path))?;
    h.raw("</td>")
}
fn directory_state(h: &mut Html, state: &DirState) -> fmt::Result {
    match state {
        DirState::Present => h.raw("present"),
        DirState::Absent => h.raw("absent"),
        DirState::Inaccessible { reason }
        | DirState::ReadError { reason }
        | DirState::VolumeUnavailable { reason }
        | DirState::Unknown { reason } => {
            h.raw("Unavailable / UNKNOWN: ")?;
            h.text(reason)
        }
    }
}
fn list(h: &mut Html, values: &[String]) -> fmt::Result {
    h.raw("<ul>")?;
    for value in values {
        h.raw("<li>")?;
        h.text(value)?;
        h.raw("</li>")?;
    }
    h.raw("</ul>")
}
fn review_evidence(h: &mut Html, row: &ReviewItem) -> fmt::Result {
    h.raw("<td data-label=\"Evidence and gaps\"><details><summary>Inspect evidence")?;
    let gaps = row.unknowns.len() + row.worktree_state.as_ref().map_or(0, |s| s.gaps.len());
    if gaps > 0 {
        write!(h.out, " ({gaps} gaps)")?;
    }
    h.raw("</summary><dl class=\"evidence\"><dt>Commit subject</dt><dd class=\"mono\">")?;
    if let Some(subject) = &row.subject {
        h.path(subject)?;
    } else {
        h.raw("UNKNOWN")?;
    }
    if row.subject_truncated {
        h.raw(" [truncated]")?;
    }
    h.raw("</dd><dt>Current controls</dt><dd>")?;
    if let Some(state) = &row.worktree_state {
        h.text(&state.role)?;
        h.raw(" / ")?;
        h.text(&state.checks)?;
        write!(
            h.out,
            "; Unix ms {}..{}",
            state.started_unix_ms, state.finished_unix_ms
        )?;
        list(h, &state.gaps)?;
    } else {
        h.raw("UNKNOWN; not collected")?;
    }
    h.raw("</dd><dt>Current local summary</dt><dd>")?;
    if let Some(local) = &row.local_summary {
        for (label, count) in [
            ("staged", &local.staged),
            ("unstaged", &local.unstaged),
            ("conflicts", &local.conflicts),
            ("untracked", &local.untracked),
            ("ignored", &local.ignored),
        ] {
            h.text(label)?;
            h.raw(": ")?;
            if count.complete {
                write!(h.out, "{}", count.observed)?;
            } else if count.observed > 0 {
                write!(h.out, "&gt;={} (partial)", count.observed)?;
            } else {
                h.raw("UNKNOWN (partial zero)")?;
            }
            h.raw("<br>")?;
        }
    } else {
        h.raw("UNKNOWN; not collected")?;
    }
    h.raw("</dd><dt>Risks</dt><dd>")?;
    list(h, &row.risks)?;
    h.raw("</dd><dt>Unknowns</dt><dd>")?;
    list(h, &row.unknowns)?;
    h.raw("</dd></dl>")?;
    h.json(row)?;
    h.raw("</details></td>")
}
fn disk_overview(h: &mut Html, usage: Option<&DiskUsage>) -> fmt::Result {
    h.raw("<section class=\"panel\" id=\"disk\"><h2>Disk observations</h2><p class=\"muted\">All recorded source roots. Review and browser filters do not narrow this scope.</p>")?;
    if let Some(usage) = usage {
        write!(
            h.out,
            "<div class=\"disk-totals\"><div><span class=\"eyebrow\">Deduplicated allocated</span><strong>{} B</strong></div><div><span class=\"eyebrow\">Deduplicated logical</span><strong>{} B</strong></div></div><p>{} measurements, ",
            usage.unique_bytes.allocated,
            usage.unique_bytes.logical,
            usage.measurements.len()
        )?;
        h.raw(if usage.complete {
            "complete"
        } else {
            "partial; totals cover observed entries only"
        })?;
        write!(
            h.out,
            ". Unix ms {}..{}. Temporal consistency: ",
            usage.started_unix_ms, usage.finished_unix_ms
        )?;
        h.text(&usage.temporal_consistency)?;
        h.raw(".</p>")?;
        for row in &usage.measurements {
            h.raw("<details><summary>")?;
            h.raw(if row.kind == DiskKind::GitCommon {
                "Shared Git: "
            } else {
                "Worktree: "
            })?;
            h.path(&row.path)?;
            h.raw("</summary><div class=\"size\">")?;
            disk_size(h, Some(row))?;
            h.raw("</div><ul class=\"coverage-list\">")?;
            for gap in &row.gaps {
                h.raw("<li><code>")?;
                h.path(&gap.path)?;
                h.raw("</code>: ")?;
                h.text(&gap.reason)?;
                h.raw("</li>")?;
            }
            write!(
                h.out,
                "</ul><p>Additional disk gaps: {}</p>",
                row.gaps_omitted
            )?;
            h.json(row)?;
            h.raw("</details>")?;
        }
    } else {
        h.raw("<p>Not measured; sizes UNKNOWN. Use scan or review --disk-usage to collect measurements.</p>")?;
    }
    h.raw("<p class=\"footnote\">Hidden and ignored files are included in captured measurements; symlink targets are not followed. Row and family sizes may overlap. The deduplicated total uses device/inode across all roots, including shared Git. APFS shared extents and snapshots are not deduplicated. These figures are not reclaimable space and do not affect a verdict.</p></section>")
}
fn coverage(h: &mut Html, coverage: &Coverage, not_collected: &[String]) -> fmt::Result {
    h.raw("<section class=\"panel\" id=\"coverage\"><h2>Coverage &amp; evidence limits</h2><p>Traversal: ")?;
    h.raw(if coverage.traversal_complete {
        "complete"
    } else {
        "incomplete"
    })?;
    h.raw(". Temporal consistency: ")?;
    h.text(&coverage.temporal_consistency)?;
    write!(
        h.out,
        ". {} recorded coverage gaps.</p>",
        coverage.gaps.len()
    )?;
    if let Some(remaining) = &coverage.remaining {
        h.raw("<p>Unvisited scope: ")?;
        h.text(remaining.reason.as_str())?;
        write!(
            h.out,
            "; {} known pending paths. Covers the requested scope.</p>",
            remaining.known_pending_paths
        )?;
    }
    if let Some(omitted) = &coverage.publication {
        write!(
            h.out,
            "<p class=\"notice\">Publication omitted: {} families, {} worktrees, {} local manifests, {} markers. These omissions cover the requested scope and registered worktrees; displayed totals count the published source only.</p>",
            omitted.families, omitted.worktrees, omitted.local_inventories, omitted.markers
        )?;
    }
    h.raw("<ul class=\"coverage-list\">")?;
    for gap in &coverage.gaps {
        h.raw("<li><code>")?;
        h.path(&gap.path)?;
        h.raw("</code>: ")?;
        h.text(&gap.reason)?;
        h.raw("</li>")?;
    }
    h.raw("</ul><details><summary>Not collected</summary>")?;
    list(h, not_collected)?;
    h.raw("</details>")?;
    h.json(coverage)?;
    h.raw("</section>")
}
