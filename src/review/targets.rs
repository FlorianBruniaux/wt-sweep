//! Explicit integration refs bound once to saved and live physical families.
use super::{error, full_ref};
use crate::local::{LocalHooks, NativeHooks};
use crate::model::{
    BytePath, Family, INVENTORY_SCHEMA, Inventory, SweepError, TARGET_MAP_SCHEMA, TargetMap,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Component;

type FamilyKey = (u64, u64, BytePath);

#[derive(Clone, Copy)]
pub enum TargetInput<'a> {
    Simple(&'a [BytePath]),
    Map(&'a TargetMap),
}

/// Private fields prevent mutation or unvalidated replacement of bound targets.
pub struct FamilyTargets {
    by_family: BTreeMap<FamilyKey, Vec<BytePath>>,
}
impl FamilyTargets {
    fn key(family: &Family) -> FamilyKey {
        (
            family.identity.device,
            family.identity.inode,
            family.common_dir.clone(),
        )
    }
    pub fn bind(inventory: &Inventory, input: TargetInput<'_>) -> Result<Self, SweepError> {
        Self::bind_with(inventory, input, &NativeHooks)
    }
    pub(crate) fn bind_with(
        inventory: &Inventory,
        input: TargetInput<'_>,
        hooks: &dyn LocalHooks,
    ) -> Result<Self, SweepError> {
        if inventory.schema != INVENTORY_SCHEMA {
            return Err(error("unsupported inventory schema"));
        }
        let mut identities = BTreeSet::new();
        let mut common_dirs = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let mut saved = BTreeMap::new();
        for family in &inventory.families {
            let common = family.common_dir.to_path()?;
            if !common.is_absolute()
                || common
                    .components()
                    .any(|c| matches!(c, Component::ParentDir))
            {
                return Err(error(
                    "family common directory must be an absolute path without parent traversal",
                ));
            }
            if !identities.insert((family.identity.device, family.identity.inode))
                || !common_dirs.insert(family.common_dir.clone())
            {
                return Err(error("duplicate or contradictory inventory family"));
            }
            for worktree in &family.worktrees {
                worktree.path.bytes()?;
                if !paths.insert(worktree.path.clone()) {
                    return Err(error("ambiguous duplicate worktree path in inventory"));
                }
            }
            saved.insert(Self::key(family), family);
        }
        let mut by_family = BTreeMap::new();
        match input {
            TargetInput::Simple(targets) => {
                let targets = normalized(targets)?;
                if !targets.is_empty() {
                    if inventory.families.len() != 1 {
                        return Err(error(
                            "nonempty simple targets require exactly one physical family; use an explicit target map",
                        ));
                    }
                    by_family.insert(Self::key(&inventory.families[0]), targets);
                }
            }
            TargetInput::Map(map) => {
                if map.schema != TARGET_MAP_SCHEMA {
                    return Err(error("unsupported target map schema"));
                }
                for entry in &map.families {
                    let path = entry.common_dir.to_path()?;
                    if !path.is_absolute()
                        || path.components().any(|c| matches!(c, Component::ParentDir))
                    {
                        return Err(error(
                            "target map common directory must be absolute without parent traversal",
                        ));
                    }
                    let key = (
                        entry.identity.device,
                        entry.identity.inode,
                        entry.common_dir.clone(),
                    );
                    if !saved.contains_key(&key) {
                        return Err(error(
                            "target map family path and identity do not match the saved inventory",
                        ));
                    }
                    let targets = normalized(&entry.targets)?;
                    if by_family.get(&key).is_some_and(|old| *old != targets) {
                        return Err(error("contradictory duplicate family target mapping"));
                    }
                    by_family.insert(key, targets);
                }
            }
        }
        // An explicit map is identity-bound before collection. Simple targets retain
        // unknown rows when a family has disappeared; existing controls still
        // prevent rebinding and output destinations still require current boundaries.
        if matches!(input, TargetInput::Map(_)) {
            for key in by_family.keys() {
                let family = saved
                    .get(key)
                    .ok_or_else(|| error("bound family missing from inventory"))?;
                let common = family.common_dir.to_path()?;
                if crate::local::history::directory_identity(&common, hooks)? != family.identity {
                    return Err(error(
                        "target map family identity differs from the current directory",
                    ));
                }
            }
        }
        Ok(Self { by_family })
    }
    pub fn for_family(&self, family: &Family) -> &[BytePath] {
        self.by_family
            .get(&Self::key(family))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}
fn normalized(targets: &[BytePath]) -> Result<Vec<BytePath>, SweepError> {
    let mut distinct = BTreeSet::new();
    for target in targets {
        full_ref(target)?;
        distinct.insert(target.clone());
        if distinct.len() > 128 {
            return Err(error(
                "more than 128 distinct integration targets for one family",
            ));
        }
    }
    Ok(distinct.into_iter().collect())
}
