//! Pure, bounded offline HTML projection of captured observations.
//! Historical family bindings label rows; they do not prove current identity.
mod dashboard;
mod help;
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
const THEME_INIT: &str = include_str!("html/theme.js");
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
        if !hex.len().is_multiple_of(2)
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
    fn copy_button(&mut self, path: &BytePath) -> fmt::Result {
        // Only call after path() has validated and displayed this filesystem path.
        self.raw("<button class=\"copy-path js-only\" type=\"button\" data-copy-path=\"")?;
        self.text(&path.hex)?;
        self.raw("\"><span class=\"copy-icon\" aria-hidden=\"true\"></span>Copy path</button>")
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
    h.raw("</style><script>")?;
    h.raw(THEME_INIT)?;
    h.raw("</script></head><body><a class=\"skip\" href=\"#content\">Skip to report</a><div class=\"layout\"><aside class=\"sidebar\" aria-label=\"Report navigation\"><div class=\"brand\"><span class=\"mark\" aria-hidden=\"true\"></span>wt-sweep</div><p class=\"eyebrow\">Read-only report</p>")?;
    navigation(h)?;
    h.raw("<p class=\"boundary\">Explore the saved report.<br>No files are removed.<br>Works offline.</p></aside><main class=\"main\" id=\"content\"><header class=\"top\"><div><p class=\"eyebrow\">Worktree Sweep / your Git folders</p><h1>")?;
    h.text(title)?;
    h.raw("</h1><p class=\"meta\">Captured ")?;
    date(h, finished)?;
    h.raw(" · Read-only</p><details class=\"capture-details\"><summary>Capture timestamps</summary><p>Observation interval: ")?;
    date(h, started)?;
    h.raw(" to ")?;
    date(h, finished)?;
    write!(
        h.out,
        "<br>Unix ms: {started}..{finished}</p></details></div><div class=\"header-actions\"><button class=\"command-open js-only\" id=\"command-open\" type=\"button\">Search report <kbd>⌘K</kbd></button><button class=\"theme js-only\" id=\"theme\" type=\"button\">Switch theme</button><details class=\"display-options js-only\"><summary>Display options</summary><div class=\"section-controls\" role=\"group\" aria-label=\"Report sections\"><button type=\"button\" id=\"collapse-sections\">Collapse all sections</button><button type=\"button\" id=\"expand-sections\">Expand all sections</button></div></details></div></header>"
    )
}
fn section_start(h: &mut Html, id: &str, title: &str, class: &str) -> fmt::Result {
    section_start_state(h, id, title, class, true)
}
fn section_start_state(
    h: &mut Html,
    id: &str,
    title: &str,
    class: &str,
    open: bool,
) -> fmt::Result {
    h.raw("<details class=\"report-section ")?;
    h.text(class)?;
    h.raw("\" id=\"")?;
    h.text(id)?;
    h.raw(if open {
        "\" open><summary><h2>"
    } else {
        "\"><summary><h2>"
    })?;
    help::icon(h, id)?;
    h.text(title)?;
    h.raw("</h2></summary><div class=\"section-body\">")
}
fn section_end(h: &mut Html) -> fmt::Result {
    h.raw("</div></details>")
}
fn boundary_notice(h: &mut Html) -> fmt::Result {
    h.raw("<div class=\"notice\"><strong>No removal authorized.</strong> A cleanup candidate still needs a usage check. Writer exclusion, durable backup and independent restore are not established by this report. Observations are not an atomic snapshot.</div>")
}
fn navigation(h: &mut Html) -> fmt::Result {
    h.raw("<nav aria-label=\"Contents\">")?;
    for (id, label, help) in [
        (
            "overview",
            "Overview",
            "Recorded totals, review verdicts and the observation period. A discovery-only scan has no cleanup verdict.",
        ),
        (
            "worktree-families",
            "Worktree list",
            "Recorded paths, branches, sizes and directory states. Expand a row for its evidence. Reviews group rows by repository and include verdicts.",
        ),
        (
            "disk",
            "Disk observations",
            "Measured allocated and logical sizes per worktree and shared Git directory. Partial totals are marked. These figures are not recoverable disk space.",
        ),
        (
            "coverage",
            "Scan limits",
            "Filesystem paths not explored, repeated symbolic links and access refusals. These describe discovery and collection limits. Original diagnostics are available in technical details.",
        ),
    ] {
        write!(
            h.out,
            "<div class=\"nav-item\"><a href=\"#{id}\" aria-describedby=\"help-{id}\" title=\""
        )?;
        h.text(help)?;
        h.raw("\">")?;
        help::icon(h, id)?;
        h.text(label)?;
        write!(
            h.out,
            "</a><div class=\"nav-tooltip\" id=\"help-{id}\" role=\"tooltip\" popover>"
        )?;
        h.text(help)?;
        h.raw("</div></div>")?;
    }
    h.raw("</nav>")
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
    h.raw("<div id=\"copy-status\" class=\"copy-status js-only\" role=\"status\" aria-live=\"polite\" aria-atomic=\"true\"></div><dialog id=\"copy-dialog\" aria-labelledby=\"copy-title\" aria-describedby=\"copy-help\"><h2 id=\"copy-title\">Copy path</h2><p id=\"copy-help\">Automatic copy is unavailable. The exact path is selected below. Press ⌘C on Mac or Ctrl+C, or use your device’s Copy action.</p><label for=\"copy-value\">Path (plain text)</label><textarea id=\"copy-value\" class=\"mono\" readonly spellcheck=\"false\" rows=\"4\"></textarea><button id=\"copy-close\" type=\"button\">Close</button></dialog>")?;
    h.raw("<dialog id=\"command-dialog\" aria-labelledby=\"command-title\" aria-describedby=\"command-help\"><div class=\"command-heading\"><h2 id=\"command-title\">Search this report</h2><button type=\"button\" id=\"command-close\">Close</button></div><p id=\"command-help\">Jump to a section or worktree. Selecting a worktree clears list filters. Use ↑ ↓ and Enter; Escape closes. ⌘K on Mac, Ctrl+K elsewhere.</p><label for=\"command-query\">Section, repository, branch or path</label><input id=\"command-query\" type=\"search\" role=\"combobox\" aria-autocomplete=\"list\" aria-controls=\"command-results\" aria-expanded=\"false\" autocomplete=\"off\" spellcheck=\"false\"><p id=\"command-status\" role=\"status\" aria-live=\"polite\"></p><div id=\"command-results\" role=\"listbox\" aria-label=\"Search results\"></div></dialog>")?;
    h.raw("<footer class=\"footnote\">Generated locally from captured observations. Git family grouping uses historical registrations; it does not establish current controls. Path labels use reversible byte escapes; full hex identities remain in recorded evidence.</footer></main></div><script>")?;
    h.raw(JS)?;
    h.raw("</script></body></html>\n")
}
fn controls(h: &mut Html, triaged: bool, selected: usize) -> fmt::Result {
    h.raw("<div class=\"toolbar js-only\"><label class=\"field search\">Search the list<input id=\"search\" type=\"search\" placeholder=\"Repository, branch, path or reason\" aria-controls=\"worktree-families\"></label><label class=\"field\">Status<select id=\"verdict\" aria-controls=\"worktree-families\"><option value=\"\">All statuses</option>")?;
    if triaged {
        h.raw("<option value=\"keep\">Keep</option><option value=\"verify\">Needs checking</option><option value=\"cleanup_candidate\">Possible cleanup</option>")?;
    } else {
        h.raw("<option value=\"not_reviewed\">Not triaged</option>")?;
    }
    h.raw("</select></label><label class=\"field\">Sort each repository<select id=\"sort\" aria-controls=\"worktree-families\"><option value=\"path\">Path</option><option value=\"branch\">Branch</option><option value=\"verdict\">Status</option><option value=\"allocated\">Size, largest first</option></select></label></div>")?;
    write!(
        h.out,
        "<p class=\"view-status\" id=\"visible-status\" role=\"status\" aria-live=\"polite\">Visible {selected} selected worktrees. Source totals and disk measurements are unchanged.</p>"
    )?;
    h.raw("<noscript><p class=\"muted\">Search, sorting and copying need JavaScript. All entries and expandable details remain readable.</p></noscript><p id=\"no-matches\" class=\"empty\" hidden>No rows match. Clear the search or verdict filter.</p>")
}
fn verdict_label(verdict: &str) -> &str {
    match verdict {
        "keep" => "Keep",
        "verify" => "Needs checking",
        "cleanup_candidate" => "Possible cleanup",
        _ => "Unknown status",
    }
}
fn overview_actions(h: &mut Html, reviewed: bool) -> fmt::Result {
    h.raw("<div class=\"overview-actions\"><a class=\"primary-action\" href=\"#worktree-families\">Browse worktrees</a><button class=\"js-only\" type=\"button\" data-list-sort=\"allocated\">")?;
    h.raw(if reviewed {
        "Sort each repository by size"
    } else {
        "Largest measured first"
    })?;
    h.raw("</button>")?;
    if !reviewed {
        help::review_command(h)?;
    }
    h.raw("</div>")
}
fn list_shortcuts(h: &mut Html, reviewed: bool) -> fmt::Result {
    h.raw("<details class=\"list-shortcuts-disclosure js-only\"><summary>Quick filters</summary><div class=\"list-shortcuts\" role=\"group\" aria-label=\"List shortcuts\"><span>Show:</span>")?;
    let filters: &[(&str, &str)] = if reviewed {
        &[
            ("keep", "Keep"),
            ("verify", "Needs checking"),
            ("cleanup_candidate", "Possible cleanup"),
        ]
    } else {
        &[
            ("present", "Working folders"),
            ("bare", "Bare repositories"),
            ("absent", "Absent paths"),
            ("unknown", "Not verified"),
        ]
    };
    for (value, label) in filters {
        write!(
            h.out,
            "<button type=\"button\" data-list-filter=\"{value}\">{label}</button>"
        )?;
    }
    h.raw("<button type=\"button\" data-list-reset>Show all</button></div></details>")
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
fn inventory_controls(h: &mut Html, total: usize) -> fmt::Result {
    h.raw("<div class=\"toolbar js-only\"><label class=\"field search\">Search the list<input id=\"search\" type=\"search\" placeholder=\"Repository, branch or full path\" aria-controls=\"worktree-families\"></label><label class=\"field\">Directory state<select id=\"verdict\" aria-controls=\"worktree-families\"><option value=\"\">All entries</option><option value=\"present\">Present working folders</option><option value=\"bare\">Bare repositories</option><option value=\"absent\">Absent folders</option><option value=\"unknown\">Not verified</option></select></label><label class=\"field\">Sort the list<select id=\"sort\" aria-controls=\"worktree-families\"><option value=\"state\">Present first</option><option value=\"path\">Path</option><option value=\"branch\">Branch</option><option value=\"allocated\">Size, largest first</option></select></label></div>")?;
    write!(
        h.out,
        "<p class=\"view-status\" id=\"visible-status\" role=\"status\" aria-live=\"polite\">Showing {total} of {total} recorded entries. The counts above cover the saved inventory.</p>"
    )?;
    h.raw("<noscript><p class=\"muted\">Search, sorting and copying need JavaScript. All entries and expandable details remain readable.</p></noscript><p id=\"no-matches\" class=\"empty\" hidden>No entries match. Clear the search or directory state filter.</p>")
}
fn inventory_state(row: &Worktree) -> (u8, &'static str, &'static str) {
    if row.bare {
        (1, "bare", "Bare repository")
    } else {
        match row.path_state {
            DirState::Present => (0, "present", "Present folder"),
            DirState::Absent => (2, "absent", "Absent folder"),
            _ => (3, "unknown", "Not verified"),
        }
    }
}
fn repository_label(h: &mut Html, common: &BytePath) -> fmt::Result {
    // Display a short label only; full bytes and physical binding stay in the evidence.
    let hex = common.hex.strip_suffix("2f2e676974").unwrap_or(&common.hex);
    let start = hex
        .as_bytes()
        .chunks_exact(2)
        .rposition(|pair| pair == b"2f")
        .map_or(0, |slash| (slash + 1) * 2);
    if start == hex.len() {
        h.path(common)
    } else {
        h.path(&BytePath {
            hex: hex[start..].to_owned(),
        })
    }
}
fn discovery_summary(h: &mut Html, coverage: &Coverage) -> fmt::Result {
    h.raw("<p class=\"view-status\" data-discovery-status>")?;
    if coverage.remaining.is_some() {
        h.raw(
            "<strong>Discovery interrupted: additional worktrees may remain undiscovered.</strong>",
        )?;
        h.raw(" <a href=\"#coverage\">See scan limits</a>.")?;
    } else if !coverage.traversal_complete {
        h.raw("<strong>Scan or evidence incomplete.</strong> <a href=\"#coverage\">See scan limits</a>.")?;
    } else {
        h.raw("Recorded discovery traversal completed within the requested scope. This does not certify the entire machine.")?;
    }
    if let Some(omitted) = &coverage.publication
        && (omitted.worktrees > 0 || omitted.families > 0)
    {
        write!(
            h.out,
            " <strong>Published list excludes {} worktree observations and {} repository families.</strong>",
            omitted.worktrees, omitted.families
        )?;
    }
    h.raw("</p>")
}

fn inventory_into(h: &mut Html, source: &Inventory) -> fmt::Result {
    start(
        h,
        "Worktree inventory",
        source.started_unix_ms,
        source.finished_unix_ms,
    )?;
    section_start(h, "overview", "What was found", "overview-section")?;
    let total: usize = source.families.iter().map(|f| f.worktrees.len()).sum();
    let (mut present, mut bare, mut absent, mut unknown) = (0, 0, 0, 0);
    for tree in source.families.iter().flat_map(|family| &family.worktrees) {
        if tree.bare {
            bare += 1;
        } else {
            match tree.path_state {
                DirState::Present => present += 1,
                DirState::Absent => absent += 1,
                DirState::Unknown { .. }
                | DirState::Inaccessible { .. }
                | DirState::ReadError { .. }
                | DirState::VolumeUnavailable { .. } => unknown += 1,
            }
        }
    }
    let unavailable = absent + unknown;
    write!(
        h.out,
        "<div class=\"metrics metrics4\"><div class=\"metric\"><span class=\"eyebrow\">Recorded Git entries</span><strong>{total}</strong><span class=\"breakdown\">{} repositories. All recorded entries.</span></div><div class=\"metric\"><span class=\"eyebrow\">Working folders found</span><strong data-inventory-count=\"present\">{present}</strong><span class=\"breakdown\">Main and linked checkouts.</span></div><div class=\"metric\"><span class=\"eyebrow\">Bare repositories</span><strong data-inventory-count=\"bare\">{bare}</strong><span class=\"breakdown\">No working folder.</span></div><div class=\"metric\"><span class=\"eyebrow\">Absent or unverified paths</span><strong data-inventory-count=\"unavailable\">{unavailable}</strong><span class=\"breakdown\">{absent} absent; {unknown} not verified.</span></div></div>",
        source.families.len()
    )?;
    discovery_summary(h, &source.coverage)?;
    help::recommendations(h, None, &source.coverage)?;
    overview_actions(h, false)?;
    section_end(h)?;
    section_start(
        h,
        "worktree-families",
        "Worktree list",
        "families-section inventory-list",
    )?;
    inventory_controls(h, total)?;
    list_shortcuts(h, false)?;
    let sizes = measurements(source.disk_usage.as_ref());
    let mut families: Vec<_> = source.families.iter().collect();
    families.sort_by_key(|f| (&f.common_dir, f.identity.device, f.identity.inode));
    h.raw("<div class=\"table-wrap\"><table class=\"inventory-table\" data-flat-list=\"\"><caption>All recorded entries, with present folders first. Sizes are observations, not recoverable space.</caption><thead><tr><th scope=\"col\">Repository</th><th scope=\"col\">Branch</th><th scope=\"col\">Path</th><th scope=\"col\">Measured size</th><th scope=\"col\">Directory state</th></tr></thead><tbody data-row-group=\"\">")?;
    let mut rows: Vec<_> = families
        .iter()
        .flat_map(|f| f.worktrees.iter().map(move |row| (*f, row)))
        .collect();
    rows.sort_by_key(|(_, row)| (inventory_state(row).0, &row.path));
    for (family, row) in rows {
        let (_, state, label) = inventory_state(row);
        row_start_state(
            h,
            &row.path,
            row.branch.as_ref(),
            "not_reviewed",
            &sizes,
            Some(state),
        )?;
        h.raw("<td data-label=\"Repository\"><span class=\"mono\">")?;
        repository_label(h, &family.common_dir)?;
        h.raw("</span></td>")?;
        branch_cell(h, row.branch.as_ref(), Some(row))?;
        h.raw("<td data-label=\"Path\"><span class=\"path mono\">")?;
        h.path(&row.path)?;
        h.raw("</span>")?;
        h.copy_button(&row.path)?;
        h.raw("</td><td class=\"size\" data-label=\"Measured size\">")?;
        if let Some(size) = measurement(&sizes, &row.path).filter(|size| size.requested) {
            if let Some(bytes) = &size.bytes {
                byte_size(h, bytes.allocated)?;
                h.raw(if size.complete {
                    "<small>complete measurement</small>"
                } else {
                    "<small>partial measurement</small>"
                })?;
            } else {
                h.raw("Not measured")?;
            }
        } else {
            h.raw("Not measured")?;
        }
        h.raw("</td><td data-label=\"Directory state\"><span class=\"badge\">")?;
        h.text(label)?;
        h.raw("</span><details><summary>Recorded evidence</summary><dl class=\"evidence\"><dt>Git repository (shared directory)</dt><dd>")?;
        h.path(&family.common_dir)?;
        write!(
            h.out,
            "</dd><dt>Historical repository identity</dt><dd>device {} / inode {}</dd><dt>Full HEAD</dt><dd>",
            family.identity.device, family.identity.inode
        )?;
        h.text(row.head.as_deref().unwrap_or("UNKNOWN"))?;
        h.raw("</dd><dt>Directory state</dt><dd>")?;
        directory_state(h, &row.path_state)?;
        h.raw("</dd></dl>")?;
        h.json(row)?;
        h.raw("</details></td></tr>")?;
    }
    h.raw("</tbody></table></div>")?;
    if total == 0 {
        h.raw("<p class=\"empty\">No worktrees selected. Source coverage and omissions remain below.</p>")?;
    }
    h.raw("<details class=\"panel\"><summary>Git repository details</summary>")?;
    family_index(
        h,
        families
            .iter()
            .enumerate()
            .map(|(index, f)| (index, &f.common_dir)),
    )?;
    for (index, family) in families.iter().enumerate() {
        write!(h.out, "<details id=\"family-{index}\"><summary>")?;
        h.path(&family.common_dir)?;
        h.raw("</summary>")?;
        h.copy_button(&family.common_dir)?;
        h.json(&family.identity)?;
        if let Some(shared) = sizes.get(&(&family.common_dir, true)) {
            for size in shared {
                h.raw("<p>Shared Git: ")?;
                disk_size(h, Some(size))?;
                h.raw("</p>")?;
            }
        }
        h.raw("</details>")?;
    }
    h.raw("</details>")?;
    section_end(h)?;
    help::guide(h, false)?;
    disk_overview(h, source.disk_usage.as_ref(), None)?;
    section_start_state(h, "coverage", "Scan limits", "panel", false)?;
    h.raw("<details class=\"panel\"><summary>Requested scan scope</summary><div id=\"requested-scan-scope\"><p>Git registrations include main checkouts, linked worktrees and bare repositories. This is not a count of extra linked worktrees.</p>")?;
    match &source.scope {
        ScanScope::Repo { path } => {
            h.raw("Repository: <code>")?;
            h.path(path)?;
            h.raw("</code>")?;
            h.copy_button(path)?;
        }
        ScanScope::Roots { paths } => {
            h.raw("Explicit roots:<ul class=\"coverage-list\">")?;
            for path in paths {
                h.raw("<li><code>")?;
                h.path(path)?;
                h.raw("</code>")?;
                h.copy_button(path)?;
                h.raw("</li>")?;
            }
            h.raw("</ul>")?;
        }
        ScanScope::AllMounted => h.raw(
            "All visible mounted volumes; unavailable and non-visible data are not certified.",
        )?,
    }
    h.raw("</div></details>")?;
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
        h.raw("</summary>")?;
        h.copy_button(&manifest.worktree)?;
        h.raw("<p>Traversal: ")?;
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
            gap_item(h, gap)?;
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
    section_end(h)?;
    end(h)
}
fn review_into(
    h: &mut Html,
    view: &ReviewView,
    families: &BTreeMap<FamilyKey<'_>, Vec<&ReviewItem>>,
) -> fmt::Result {
    let source = &view.source;
    section_start(h, "overview", "What to inspect", "overview-section")?;
    let keep = source
        .items
        .iter()
        .filter(|i| i.classification == "keep")
        .count();
    let verify = source
        .items
        .iter()
        .filter(|i| i.classification == "verify")
        .count();
    let candidates = source
        .items
        .iter()
        .filter(|i| i.classification == "cleanup_candidate")
        .count();
    write!(
        h.out,
        "<div class=\"metrics metrics4\"><div class=\"metric\"><span class=\"eyebrow\">Entries reviewed</span><strong>{}</strong><span class=\"breakdown\">In the saved Git review</span></div>",
        source.items.len()
    )?;
    for (class, label, count, explanation) in [
        (
            "keep",
            "Keep",
            keep,
            "A reason to preserve this work was found.",
        ),
        (
            "verify",
            "Needs checking",
            verify,
            "Some required checks are missing.",
        ),
        (
            "cleanup_candidate",
            "Possible cleanup",
            candidates,
            "Git checks passed; confirm usage first.",
        ),
    ] {
        write!(
            h.out,
            "<div class=\"metric {class}\"><span class=\"eyebrow\">{label}</span><strong>{count}</strong><span class=\"breakdown\">{explanation}</span></div>"
        )?;
    }
    h.raw("</div>")?;
    if view.selected_paths.len() != source.items.len() {
        write!(
            h.out,
            "<p class=\"view-status\">The saved view includes {} of {} reviewed entries. The summary covers the full review.</p>",
            view.selected_paths.len(),
            source.items.len()
        )?;
    }
    discovery_summary(h, &source.coverage)?;
    help::recommendations(
        h,
        Some((
            families.values().map(Vec::len).sum(),
            families
                .values()
                .flatten()
                .filter(|row| row.classification == "keep")
                .count(),
            families
                .values()
                .flatten()
                .filter(|row| row.classification == "verify")
                .count(),
            families
                .values()
                .flatten()
                .filter(|row| row.classification == "cleanup_candidate")
                .count(),
        )),
        &source.coverage,
    )?;
    overview_actions(h, true)?;
    section_end(h)?;
    section_start(h, "worktree-families", "Worktree list", "families-section")?;
    controls(h, true, view.selected_paths.len())?;
    list_shortcuts(h, true)?;
    let contexts: BTreeMap<_, _> = source
        .context
        .as_ref()
        .map(|c| c.items.iter().map(|r| (&r.path, r)).collect())
        .unwrap_or_default();
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
            h.raw("<td data-label=\"Status\"><span class=\"badge ")?;
            h.raw(match row.classification.as_str() {
                "keep" => "keep",
                "verify" => "verify",
                "cleanup_candidate" => "cleanup_candidate",
                _ => "",
            })?;
            h.raw("\">")?;
            help::icon(h, &row.classification)?;
            h.text(verdict_label(&row.classification))?;
            h.raw("</span><p class=\"primary-reason\">")?;
            match row.classification.as_str() {
                "keep" => h.text(row.decision.as_ref().and_then(|d| d.keep_reasons.first()).map_or("A retention reason was observed.", |r| super::keep_reason(r)))?,
                "verify" => h.raw("Required checks are missing or inconsistent. Open the details before deciding.")?,
                "cleanup_candidate" => h.raw("Git checks passed. Confirm this folder is no longer in use.")?,
                _ => h.raw("The status is unknown. Inspect the recorded evidence.")?,
            }
            h.raw("</p><details class=\"row-details\"><summary>Why this status?</summary><p>Recorded verdict: <code>")?;
            h.text(&row.classification)?;
            h.raw("</code></p>")?;
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
            h.raw("</details></td>")?;
            disk_cell(h, &row.path, &sizes)?;
            review_evidence(
                h,
                row,
                contexts
                    .get(&row.path)
                    .copied()
                    .filter(|c| c.common_dir == **common && c.head_oid == row.head),
                source.forge.as_ref(),
            )?;
            h.raw("</tr>")?;
        }
        family_end(h)?;
    }
    family_index(
        h,
        families
            .iter()
            .enumerate()
            .filter(|(_, (_, rows))| !rows.is_empty())
            .map(|(index, ((common, _, _), _))| (index, *common)),
    )?;
    if view.selected_paths.is_empty() {
        h.raw("<p class=\"empty\">No worktrees selected. Source totals, coverage and disk observations are retained.</p>")?;
    }
    section_end(h)?;
    help::guide(h, true)?;
    disk_overview(h, source.disk_usage.as_ref(), Some(&source.items))?;
    section_start_state(h, "coverage", "Scan limits", "panel", false)?;
    h.raw("<details class=\"panel\"><summary>Report provenance and saved filters</summary><p>HTML groups rows by historical Git family. The saved view grouping is <code>")?;
    h.text(&view.group_by)?;
    h.raw(
        "</code>. Filters select rows only; coverage and disk totals retain the source scope.</p>",
    )?;
    write!(
        h.out,
        "<p>Source totals: {} worktrees. ",
        source.items.len()
    )?;
    counts(h, source.items.iter())?;
    write!(
        h.out,
        "</p><p>Selected totals: {} worktrees. ",
        view.selected_paths.len()
    )?;
    counts(h, families.values().flat_map(|rows| rows.iter().copied()))?;
    h.raw("</p><p>Policy: ")?;
    h.text(&source.policy_version)?;
    h.raw(". Deterministic verdicts; no model decision.</p>")?;
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
    coverage(h, &source.coverage, &source.not_collected)?;
    h.raw("<details class=\"panel\"><summary>Shared and descriptive evidence</summary><p>Shared stashes and descriptive context are source observations; they do not change a verdict or prove integration.</p>")?;
    h.json(&source.shared_stashes)?;
    h.json(&source.context)?;
    h.json(&source.local_inventory_refs)?;
    h.raw("</details>")?;
    section_end(h)?;
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
        h.raw("</a>")?;
        h.copy_button(common)?;
        h.raw("</li>")?;
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
        "<section class=\"family\" id=\"family-{index}\" aria-labelledby=\"family-title-{index}\"><header class=\"family-header\"><div><h2 id=\"family-title-{index}\" class=\"mono\">",
    )?;
    repository_label(h, common)?;
    h.raw("</h2><details class=\"family-details\"><summary>Repository details</summary><p class=\"mono\">")?;
    h.path(common)?;
    h.raw("</p>")?;
    h.copy_button(common)?;
    write!(
        h.out,
        "<p class=\"identity muted\">Historical device {} / inode {}. Current identity is not established by grouping.</p></details></div><div class=\"family-summary\"><span class=\"visible-count\">{selected}</span> visible / {selected} selected / {source_count} source rows",
        identity.device, identity.inode
    )?;
    if let Some(rows) = sizes.get(&(common, true)) {
        for size in rows {
            h.raw("<p>Shared Git: ")?;
            disk_size(h, Some(size))?;
            h.raw("</p>")?;
        }
    }
    h.raw("</div></header><div class=\"table-wrap\"><table class=\"review-table\"><caption>Recorded entries. Sizes may overlap.</caption><thead><tr><th scope=\"col\">Folder</th><th scope=\"col\">Branch</th><th scope=\"col\">Status</th><th scope=\"col\">Disk observations</th><th scope=\"col\">Checks and details</th></tr></thead><tbody>")
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
    row_start_state(h, path, branch, verdict, sizes, None)
}
fn row_start_state(
    h: &mut Html,
    path: &BytePath,
    branch: Option<&BytePath>,
    verdict: &str,
    sizes: &MeasurementIndex<'_>,
    state: Option<&str>,
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
    h.raw("\"")?;
    if let Some(state) = state {
        h.raw(" data-state=\"")?;
        h.text(state)?;
        h.raw("\"")?;
    }
    h.raw(">")
}
fn path_cell(h: &mut Html, path: &BytePath, head: Option<&str>) -> fmt::Result {
    h.raw("<td data-label=\"Folder\"><span class=\"path mono\">")?;
    h.path(path)?;
    h.raw("</span>")?;
    h.copy_button(path)?;
    h.raw("<details class=\"row-details\"><summary>Commit identity</summary><span class=\"head mono\">HEAD ")?;
    h.text(head.unwrap_or("UNKNOWN"))?;
    h.raw("</span></details></td>")
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
fn byte_size(h: &mut Html, bytes: u64) -> fmt::Result {
    const UNITS: [&str; 7] = ["o", "Ko", "Mo", "Go", "To", "Po", "Eo"];
    write!(h.out, "<span class=\"byte-size\" title=\"{bytes} octets\">")?;
    if bytes < 1000 {
        write!(h.out, "{bytes} o")?;
    } else {
        let value = u128::from(bytes);
        let mut divisor = 1000_u128;
        let mut unit = 1;
        while value >= divisor * 1000 && unit + 1 < UNITS.len() {
            divisor *= 1000;
            unit += 1;
        }
        // Integer rounding preserves precision up to u64::MAX.
        let mut hundredths = (value * 100 + divisor / 2) / divisor;
        if hundredths >= 100_000 && unit + 1 < UNITS.len() {
            divisor *= 1000;
            unit += 1;
            hundredths = (value * 100 + divisor / 2) / divisor;
        }
        write!(
            h.out,
            "{},{:02} {}",
            hundredths / 100,
            hundredths % 100,
            UNITS[unit]
        )?;
    }
    h.raw("</span>")
}
fn disk_size(h: &mut Html, row: Option<&DiskMeasurement>) -> fmt::Result {
    let Some(row) = row else {
        return h.raw("Not measured; size UNKNOWN");
    };
    if !row.requested {
        return h.raw("Not requested; outside disk collection selection");
    }
    if let Some(bytes) = &row.bytes {
        h.raw("<strong>")?;
        byte_size(h, bytes.allocated)?;
        h.raw(" allocated</strong><small>")?;
        byte_size(h, bytes.logical)?;
        h.raw(" logical</small>")?;
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
    h.raw("<td class=\"size\" data-label=\"Measured size\">")?;
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
fn gap_item(h: &mut Html, gap: &Gap) -> fmt::Result {
    h.raw("<li class=\"path-gap\"><div class=\"path-actions\"><code>")?;
    h.path(&gap.path)?;
    h.raw("</code>")?;
    h.copy_button(&gap.path)?;
    h.raw("</div><p class=\"gap-reason\">")?;
    h.text(&gap.reason)?;
    h.raw("</p></li>")
}
fn web_link(h: &mut Html, label: &str, url: &str) -> fmt::Result {
    h.raw("<a href=\"")?;
    h.text(url)?;
    h.raw("\" target=\"_blank\" rel=\"noopener noreferrer\">")?;
    h.text(label)?;
    h.raw("</a>")
}
fn forge_navigation(
    h: &mut Html,
    row: &ReviewItem,
    context: Option<&WorktreeContext>,
    forge: Option<&ForgeExport>,
) -> fmt::Result {
    h.raw("<div class=\"forge-navigation\"><strong>GitHub / GitLab</strong><p class=\"muted\">Server not checked.</p>")?;
    if let Some(remote) = context.and_then(|c| c.remote.as_ref()) {
        write!(
            h.out,
            "<details class=\"forge-remotes\"><summary>Repository links ({} remotes)</summary>",
            remote.remotes.len()
        )?;
        for record in &remote.remotes {
            let links = crate::forge_links::links(record, row.head.as_deref());
            if links.is_empty() {
                continue;
            }
            h.raw("<div class=\"remote-record\"><span class=\"mono\">")?;
            h.path(&record.name)?;
            h.raw(" · ")?;
            h.text(&record.provider)?;
            h.raw("</span><div class=\"forge-links\">")?;
            for (label, url) in links {
                web_link(h, label, &url)?;
            }
            h.raw("</div><details><summary>Local tracking</summary><p class=\"mono\">")?;
            h.text(&record.host)?;
            h.raw("/")?;
            h.text(&record.project)?;
            h.raw("</p><p class=\"muted\">")?;
            h.raw(match record.head_matches_tracking {
                Some(true) => "HEAD matches the local tracking ref",
                Some(false) => "HEAD differs from the local tracking ref",
                None => "Publication unknown; no matching local tracking observation",
            })?;
            if let Some(reference) = &record.tracking_ref {
                h.raw(": <code>")?;
                h.path(reference)?;
                h.raw("</code>")?;
            }
            h.raw(". Server not checked.</p></details></div>")?;
        }
        if remote.remotes.is_empty() {
            h.raw("<p class=\"muted\">No supported remote link collected.</p>")?;
        }
        if !remote.gaps.is_empty() {
            h.raw("<details><summary>Remote collection gaps</summary>")?;
            list(h, &remote.gaps)?;
            h.raw("</details>")?;
        }
        write!(
            h.out,
            "<details><summary>Link provenance</summary><p>Constructed from local remote configuration; branch and commit pages may be unavailable. Local observation: Unix ms {}..{}. No fetch or server check.</p></details>",
            remote.started_unix_ms, remote.finished_unix_ms
        )?;
        h.raw("</details>")?;
    } else {
        h.raw(
            "<p class=\"muted\">Remote links not collected. Run review with context enabled.</p>",
        )?;
    }
    if row.requests.is_empty() {
        h.raw("<p class=\"muted\">No associated PR/MR in the imported evidence. Use --forge FILE to include a capture; absence is not established.</p>")?;
    }
    for request in &row.requests {
        h.raw("<div class=\"request-record\"><p>")?;
        let label = format!(
            "{} #{}",
            if request.provider == "gitlab" {
                "MR"
            } else {
                "PR"
            },
            request.iid
        );
        if let Some(url) = crate::forge_links::request_url(request) {
            web_link(h, &label, &url)?;
        } else {
            h.text(&label)?;
            h.raw(" (URL unavailable)")?;
        }
        h.raw(" · captured state: <strong>")?;
        h.text(&request.state)?;
        h.raw("</strong></p><details><summary>PR/MR details</summary><p>")?;
        h.text(request.title.as_deref().unwrap_or("Title not captured"))?;
        h.raw("</p><p>Updated at: ")?;
        h.text(request.updated_at.as_deref().unwrap_or("not captured"))?;
        h.raw(". State comes from the imported capture, not a live server check.</p>")?;
        if let Some(source) = forge.and_then(|f| f.sources.get(request.source_index)) {
            h.raw("<p>Source: ")?;
            h.path(&source.raw_path)?;
            h.raw("<br>SHA-256: <code>")?;
            h.text(&source.sha256)?;
            h.raw("</code></p>")?;
            write!(
                h.out,
                "<p>Receipt verified locally: {}. Capture interval: ",
                source.receipt_verified
            )?;
            match (source.started_unix_ms, source.finished_unix_ms) {
                (Some(start), Some(end)) => {
                    date(h, start)?;
                    h.raw(" to ")?;
                    date(h, end)?;
                }
                _ => h.raw("not captured")?,
            }
            h.raw(".</p>")?;
            list(h, &source.gaps)?;
        }
        h.raw("</details></div>")?;
    }
    h.raw("</div>")
}
fn review_evidence(
    h: &mut Html,
    row: &ReviewItem,
    context: Option<&WorktreeContext>,
    forge: Option<&ForgeExport>,
) -> fmt::Result {
    h.raw("<td data-label=\"Checks and details\">")?;
    h.raw("<details class=\"row-details\"><summary>Inspect evidence")?;
    let gaps = row.unknowns.len() + row.worktree_state.as_ref().map_or(0, |s| s.gaps.len());
    if gaps > 0 {
        write!(h.out, " ({gaps} gaps)")?;
    }
    h.raw("</summary>")?;
    forge_navigation(h, row, context, forge)?;
    h.raw("<dl class=\"evidence\"><dt>Commit subject</dt><dd class=\"mono\">")?;
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
fn measurement_summary(h: &mut Html, usage: Option<&DiskUsage>) -> fmt::Result {
    if let Some(usage) = usage {
        h.raw("<details class=\"panel\"><summary>Measurement scope and totals</summary><p>Measurement roots combine working directories and shared Git directories. They are not a count of worktrees.</p>")?;
        h.raw("<p class=\"space-coverage\">")?;
        h.text(&usage.scope_summary())?;
        h.raw(".</p>")?;
        let complete = usage
            .measurements
            .iter()
            .filter(|m| m.complete && m.bytes.is_some())
            .count();
        let partial = usage
            .measurements
            .iter()
            .filter(|m| !m.complete && m.bytes.is_some())
            .count();
        let missing = usage
            .measurements
            .iter()
            .filter(|m| m.requested && m.bytes.is_none())
            .count();
        write!(
            h.out,
            "<p class=\"space-coverage\"><strong>{complete}/{} roots fully measured</strong> · {partial} partial; {missing} not measured. Missing sizes remain unknown.</p></details>",
            usage.measurements.len()
        )?;
        let mut largest: Vec<_> = usage
            .measurements
            .iter()
            .filter(|m| m.kind == DiskKind::Worktree)
            .filter_map(|m| m.bytes.as_ref().map(|b| (m, b.allocated)))
            .collect();
        largest.sort_by(|(a, ab), (b, bb)| bb.cmp(ab).then_with(|| a.path.cmp(&b.path)));
        if !largest.is_empty() {
            h.raw("<details class=\"space-largest\"><summary>Largest observed worktree sizes (up to 3)</summary><p class=\"muted\">Start the size review here. Unmeasured worktrees may be larger. Row sizes may overlap and are not added together. Check each verdict before planning cleanup.</p><ol>")?;
            for (row, allocated) in largest.into_iter().take(3) {
                h.raw("<li><div class=\"path-actions\"><code>")?;
                h.path(&row.path)?;
                h.raw("</code>")?;
                h.copy_button(&row.path)?;
                h.raw("</div><span class=\"space-entry-size\">")?;
                byte_size(h, allocated)?;
                h.raw(if row.complete {
                    " allocated · Complete measurement"
                } else {
                    " allocated · Partial observation"
                })?;
                h.raw("</span></li>")?;
            }
            h.raw("</ol></details>")?;
        }
    }
    Ok(())
}
fn disk_overview(
    h: &mut Html,
    usage: Option<&DiskUsage>,
    items: Option<&[ReviewItem]>,
) -> fmt::Result {
    section_start_state(h, "disk", "Disk observations", "panel", false)?;
    dashboard::render(h, usage, items)?;
    h.raw("<p class=\"muted\">Space freed by cleanup is unknown. APFS clones and snapshots can share blocks; measured allocation does not predict freed space.</p>")?;
    measurement_summary(h, usage)?;
    h.raw("<details class=\"cleanup-boundaries\"><summary>Before planning cleanup</summary>")?;
    boundary_notice(h)?;
    h.raw("</details>")?;
    h.raw("<p class=\"muted\">All recorded source roots are listed. Review and browser filters do not narrow the disk collection selection.</p>")?;
    if let Some(usage) = usage {
        h.raw("<p>")?;
        h.text(&usage.scope_summary())?;
        h.raw(".</p>")?;
        h.raw("<div class=\"disk-totals\"><div><span class=\"eyebrow\">Deduplicated allocated</span><strong>")?;
        byte_size(h, usage.unique_bytes.allocated)?;
        h.raw("</strong></div><div><span class=\"eyebrow\">Deduplicated logical</span><strong>")?;
        byte_size(h, usage.unique_bytes.logical)?;
        h.raw("</strong></div></div><p class=\"muted\">Decimal units (1 Ko = 1000 octets). Display rounded to two decimals; exact counts are available on hover or in the details.</p>")?;
        write!(
            h.out,
            "<details><summary>Exact totals (octets)</summary><dl class=\"evidence\"><dt>Deduplicated allocated</dt><dd>{}</dd><dt>Deduplicated logical</dt><dd>{}</dd></dl></details><p>{} measurements, ",
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
            h.raw("<div class=\"copyable-details\"><details><summary><span class=\"disk-title\">")?;
            h.raw(if row.kind == DiskKind::GitCommon {
                "Shared Git"
            } else {
                "Worktree"
            })?;
            h.raw("</span><code class=\"disk-path\">")?;
            h.path(&row.path)?;
            h.raw("</code></summary><div class=\"size\">")?;
            disk_size(h, Some(row))?;
            h.raw("</div><ul class=\"coverage-list\">")?;
            for gap in &row.gaps {
                gap_item(h, gap)?;
            }
            write!(
                h.out,
                "</ul><p>Additional disk gaps: {}</p>",
                row.gaps_omitted
            )?;
            h.json(row)?;
            h.raw("</details>")?;
            h.copy_button(&row.path)?;
            h.raw("</div>")?;
        }
        if let Some(storage) = &usage.storage {
            h.raw(
                "<details class=\"volume-source\"><summary>Captured volume statistics</summary>",
            )?;
            h.json(storage)?;
            h.raw("</details>")?;
        }
    } else {
        h.raw("<p>Not measured; sizes UNKNOWN. Use scan or review --disk-usage to collect measurements.</p>")?;
    }
    h.raw("<p class=\"footnote\">Hidden and ignored files are included in captured measurements; symlink targets are not followed. Row and family sizes may overlap. The deduplicated total uses device/inode across measured requested roots. Shared Git is included when requested. APFS shared extents and snapshots are not deduplicated. These figures are not reclaimable space and do not affect a verdict.</p>")?;
    section_end(h)
}
fn coverage(h: &mut Html, coverage: &Coverage, not_collected: &[String]) -> fmt::Result {
    help::limits(h, coverage, not_collected)
}
