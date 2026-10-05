//! Pure selection over a complete judged review. Source evidence is never filtered.
use crate::{model::*, output::canonical_digest};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
};

fn error(message: &str) -> SweepError {
    SweepError::new("invalid_review_view", "review_view", Path::new(""), message)
}
fn absolute(path: &BytePath) -> Result<PathBuf, SweepError> {
    let value = path.to_path()?;
    if !value.is_absolute()
        || value
            .components()
            .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(error(
            "view paths must be absolute without parent traversal",
        ));
    }
    Ok(value)
}
pub fn validate_filters(filters: &ReviewFilters, group_by: &str) -> Result<(), SweepError> {
    if !matches!(group_by, "repo" | "directory") {
        return Err(error("group-by must be repo or directory"));
    }
    for path in [&filters.repository, &filters.path_prefix]
        .into_iter()
        .flatten()
    {
        absolute(path)?;
    }
    if let Some(branch) = &filters.branch {
        crate::review::full_ref(branch)?;
    }
    if filters
        .local_state
        .as_deref()
        .is_some_and(|s| !matches!(s, "clean" | "dirty" | "unknown"))
    {
        return Err(error("local-state must be clean, dirty or unknown"));
    }
    if filters
        .verdict
        .as_deref()
        .is_some_and(|s| !matches!(s, "keep" | "cleanup_candidate" | "verify"))
    {
        return Err(error("verdict must be keep, cleanup_candidate or verify"));
    }
    Ok(())
}
fn local_state(item: &ReviewItem) -> &'static str {
    if item.local_summary.as_ref().is_some_and(|s| {
        [
            &s.staged,
            &s.unstaged,
            &s.conflicts,
            &s.untracked,
            &s.ignored,
        ]
        .iter()
        .any(|c| c.observed > 0)
    }) {
        "dirty"
    } else if item
        .decision
        .as_ref()
        .is_some_and(|d| !d.missing_conditions.iter().any(|c| c == "C3"))
    {
        "clean"
    } else {
        "unknown"
    }
}
fn group_key(binding: &ReviewRowBinding, group_by: &str) -> Result<BytePath, SweepError> {
    if group_by == "repo" {
        return Ok(binding.common_dir.clone());
    }
    let path = absolute(&binding.path)?;
    Ok(BytePath::from_path(path.parent().unwrap_or(Path::new("/"))))
}
/// Both input digests and every historical row binding are checked before selection.
pub fn build(
    inventory: &Inventory,
    source: &Review,
    filters: ReviewFilters,
    group_by: &str,
) -> Result<ReviewView, SweepError> {
    validate_filters(&filters, group_by)?;
    if inventory.schema != INVENTORY_SCHEMA
        || source.schema != "wt-sweep.review/2"
        || source.policy_version != "triage/1"
    {
        return Err(error(
            "view requires current inventory/1 and review/2 with triage/1",
        ));
    }
    if canonical_digest(inventory)? != source.inventory_sha256
        || canonical_digest(source)? != source.digest_sha256
    {
        return Err(error("view source or inventory digest mismatch"));
    }
    let mut bindings = BTreeMap::new();
    let mut families = BTreeMap::new();
    for family in &inventory.families {
        absolute(&family.common_dir)?;
        if let Some(previous) = families.insert(family.common_dir.clone(), family.identity.clone())
            && previous != family.identity
        {
            return Err(error("ambiguous family identity in view source"));
        }
        for tree in &family.worktrees {
            absolute(&tree.path)?;
            let row = ReviewRowBinding {
                path: tree.path.clone(),
                common_dir: family.common_dir.clone(),
                identity: family.identity.clone(),
            };
            if bindings.insert(tree.path.clone(), row).is_some() {
                return Err(error("ambiguous duplicate worktree path in view source"));
            }
        }
    }
    let mut seen = BTreeSet::new();
    for item in &source.items {
        if !bindings.contains_key(&item.path) || !seen.insert(item.path.clone()) {
            return Err(error(
                "review rows do not uniquely match inventory bindings",
            ));
        }
        if let Some(branch) = &item.branch {
            branch.bytes()?;
        }
    }
    if seen.len() != bindings.len() {
        return Err(error("view source omits inventory rows"));
    }
    let mut selected_family = None;
    if let Some(repository) = &filters.repository {
        for row in bindings.values() {
            if row.common_dir == *repository || row.path == *repository {
                let family = (row.common_dir.clone(), row.identity.clone());
                if selected_family
                    .as_ref()
                    .is_some_and(|previous| previous != &family)
                {
                    return Err(error("repository filter has ambiguous family bindings"));
                }
                selected_family = Some(family);
            }
        }
    }
    let prefix = filters.path_prefix.as_ref().map(absolute).transpose()?;
    let mut selected = Vec::new();
    for item in &source.items {
        let binding = &bindings[&item.path];
        if filters.repository.is_some()
            && selected_family
                .as_ref()
                .is_none_or(|(path, id)| *path != binding.common_dir || *id != binding.identity)
        {
            continue;
        }
        if let Some(prefix) = &prefix
            && !absolute(&item.path)?.starts_with(prefix)
        {
            continue;
        }
        if filters
            .branch
            .as_ref()
            .is_some_and(|b| item.branch.as_ref() != Some(b))
        {
            continue;
        }
        if filters
            .local_state
            .as_deref()
            .is_some_and(|s| local_state(item) != s)
        {
            continue;
        }
        if filters
            .verdict
            .as_deref()
            .is_some_and(|s| item.classification != s)
        {
            continue;
        }
        selected.push((group_key(binding, group_by)?, item.path.clone()));
    }
    selected.sort();
    let mut view = ReviewView {
        schema: REVIEW_VIEW_SCHEMA.into(),
        source: source.clone(),
        row_bindings: bindings.into_values().collect(),
        selected_paths: selected.into_iter().map(|(_, path)| path).collect(),
        group_by: group_by.into(),
        filters,
        digest_sha256: String::new(),
    };
    view.digest_sha256 = canonical_digest(&view)?;
    Ok(view)
}
/// Rendering uses the same historical grouping as the serialized view.
pub fn groups(view: &ReviewView) -> Result<BTreeMap<BytePath, BytePath>, SweepError> {
    view.row_bindings
        .iter()
        .map(|row| Ok((row.path.clone(), group_key(row, &view.group_by)?)))
        .collect()
}
