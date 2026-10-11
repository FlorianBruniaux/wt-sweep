//! Offline volume gauges. Source totals never follow display filters.
use super::{Html, byte_size};
use crate::{
    disk::{
        DiskKind, DiskUsage,
        storage::{DeviceStorage, StorageUsage, percentage_tenths},
    },
    model::{BytePath, ReviewItem},
};
use std::{
    collections::BTreeSet,
    fmt::{self, Write},
};

fn valid_bytes(path: &BytePath) -> bool {
    let hex = path.hex.as_bytes();
    hex.len().is_multiple_of(2)
        && hex
            .iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
        && hex.chunks_exact(2).all(|pair| pair != b"00")
}
fn consistent(storage: &StorageUsage, usage: &DiskUsage) -> bool {
    if storage.schema != "wt-sweep.storage/1" {
        return false;
    }
    let mut identities = BTreeSet::new();
    let (mut logical, mut allocated) = (0u64, 0u64);
    for device in &storage.devices {
        if !identities.insert(device.device) {
            return false;
        }
        let (Some(l), Some(a)) = (
            logical.checked_add(device.bytes.logical),
            allocated.checked_add(device.bytes.allocated),
        ) else {
            return false;
        };
        logical = l;
        allocated = a;
        if let Some(volume) = &device.volume
            && [&volume.anchor_path]
                .into_iter()
                .chain(volume.mount_path.iter())
                .chain(volume.source.iter())
                .chain(volume.filesystem.iter())
                .any(|p| !valid_bytes(p))
        {
            return false;
        }
    }
    logical == usage.unique_bytes.logical && allocated == usage.unique_bytes.allocated
}
fn meter(h: &mut Html, label: &str, part: u64, capacity: u64, partial: bool) -> fmt::Result {
    let Some(tenths) = percentage_tenths(part, Some(capacity)) else {
        return Ok(());
    };
    let bounded = tenths.min(1000);
    h.raw("<div class=\"disk-gauge\"><span class=\"eyebrow\">")?;
    h.text(label)?;
    write!(
        h.out,
        "</span><strong>{}.{}%<span class=\"gauge-context\"> of volume capacity</span></strong>",
        tenths / 10,
        tenths % 10
    )?;
    write!(
        h.out,
        "<meter min=\"0\" max=\"100\" value=\"{}.{}\" aria-label=\"",
        bounded / 10,
        bounded % 10
    )?;
    h.text(label)?;
    write!(
        h.out,
        "\" aria-valuetext=\"{}.{}%",
        tenths / 10,
        tenths % 10
    )?;
    if partial {
        h.raw("; partial measurement")?;
    }
    if tenths > 1000 {
        h.raw("; exceeds reported capacity")?;
    }
    h.raw("\">")?;
    write!(h.out, "{}.{}%", tenths / 10, tenths % 10)?;
    h.raw("</meter>")?;
    if tenths > 1000 {
        h.raw(
            "<p class=\"muted\">Allocation exceeds reported capacity; the bar stops at 100%.</p>",
        )?;
    }
    h.raw("</div>")
}
fn card(h: &mut Html, device: &DeviceStorage, partial: bool) -> fmt::Result {
    write!(
        h.out,
        "<div class=\"volume-card\" data-device=\"{}\"><h4>Volume: ",
        device.device
    )?;
    let volume = device.volume.as_ref();
    if let Some(path) = volume.and_then(|v| v.mount_path.as_ref()) {
        h.path(path)?;
    } else {
        write!(h.out, "device {}", device.device)?;
    }
    h.raw("</h4><div class=\"volume-amounts\"><div><span class=\"eyebrow\">Measured allocation</span><strong>")?;
    byte_size(h, device.bytes.allocated)?;
    h.raw("</strong></div>")?;
    let capacity = volume
        .filter(|v| {
            v.identity_verified && v.anchor.device == device.device && v.method == "fstatfs"
        })
        .and_then(|v| v.capacity_bytes)
        .filter(|n| *n > 0);
    if let Some(capacity) = capacity {
        write!(
            h.out,
            "<div data-capacity-bytes=\"{capacity}\"><span class=\"eyebrow\">Volume capacity</span><strong>"
        )?;
        byte_size(h, capacity)?;
        h.raw("</strong></div><div><span class=\"eyebrow\">Available to you</span><strong>")?;
        let available = volume
            .and_then(|v| v.available_bytes)
            .filter(|n| *n <= capacity);
        if let Some(available) = available {
            byte_size(h, available)?;
        } else {
            h.raw("Not recorded or invalid")?;
        }
        h.raw("</strong></div></div><div class=\"volume-gauges\">")?;
        if let Some(unavailable) = available.and_then(|n| capacity.checked_sub(n)) {
            meter(
                h,
                "Space not available as a share of volume capacity",
                unavailable,
                capacity,
                false,
            )?;
        }
        meter(
            h,
            "Measured allocation as a share of volume capacity",
            device.bytes.allocated,
            capacity,
            partial,
        )?;
        h.raw("</div><p class=\"muted\">Space not available may include reservations or limits. Allocation can include shared APFS blocks; it does not predict space freed by removal.</p>")?;
    } else {
        h.raw("</div><p class=\"muted\">Capacity unavailable for this volume. Measured bytes are retained; no percentage can be calculated.</p>")?;
    }
    if let Some(volume) = volume {
        h.raw("<details class=\"volume-evidence\"><summary>Capture and limits</summary>")?;
        write!(
            h.out,
            "<p>Captured volume statistics: {}..{} Unix ms. No atomic snapshot.</p>",
            volume.started_unix_ms, volume.finished_unix_ms
        )?;
        for gap in &volume.gaps {
            h.raw("<p class=\"muted\">")?;
            h.text(gap)?;
            h.raw("</p>")?;
        }
        h.raw("</details>")?;
    } else if let Some(reason) = &device.unavailable_reason {
        h.raw("<p class=\"muted\">Capture limit: ")?;
        h.text(reason)?;
        h.raw("</p>")?;
    }
    h.raw("</div>")
}
pub(super) fn render(
    h: &mut Html,
    usage: Option<&DiskUsage>,
    items: Option<&[ReviewItem]>,
) -> fmt::Result {
    h.raw("<div class=\"disk-dashboard\" data-disk-dashboard><h3>Disk dashboard</h3><p class=\"dashboard-total\">Observed disk allocation: ")?;
    let measured = usage.is_some_and(|u| u.measurements.iter().any(|r| r.bytes.is_some()));
    let partial = usage.is_some_and(|u| {
        !u.complete
            || u.measurements
                .iter()
                .any(|r| r.requested && (!r.complete || r.bytes.is_none()))
    });
    if let Some(usage) = usage.filter(|_| measured) {
        byte_size(h, usage.unique_bytes.allocated)?;
    } else {
        h.raw("Not measured")?;
    }
    h.raw(".</p>")?;
    if partial {
        h.raw("<p class=\"muted\">Partial measurement; the inventory total remains unknown. Shared Git storage is included.</p>")?;
    } else if measured {
        h.raw("<p class=\"muted\">Recorded roots measured. Shared Git storage is included; discovery coverage is separate.</p>")?;
    }
    if let Some(items) = items {
        let (mut keep, mut candidate, mut verify, mut unknown) = (0, 0, 0, 0);
        for item in items {
            match item.classification.as_str() {
                "keep" => keep += 1,
                "cleanup_candidate" => candidate += 1,
                "verify" => verify += 1,
                _ => unknown += 1,
            }
        }
        write!(
            h.out,
            "<p class=\"cleanup-counts\"><strong data-cleanup-count=\"keep\">{keep}</strong> to keep · <strong data-cleanup-count=\"cleanup_candidate\">{candidate}</strong> cleanup candidates · <strong data-cleanup-count=\"verify\">{verify}</strong> to verify. Source counts; candidates still need a usage check.</p>"
        )?;
        if unknown > 0 {
            write!(h.out, "<p>{unknown} unrecognized source verdicts.</p>")?;
        }
    }
    match usage.and_then(|u| u.storage.as_ref().map(|s| (u, s))) {
        Some((usage, storage)) if consistent(storage, usage) && !measured => {
            h.raw("<p class=\"muted\">No measured device has captured capacity.</p>")?;
        }
        Some((usage, storage)) if consistent(storage, usage) => {
            let mut devices: Vec<_> = storage.devices.iter().collect();
            devices.sort_by_key(|d| (std::cmp::Reverse(d.bytes.allocated), d.device));
            if let Some(first) = devices.first() {
                card(h, first, partial)?;
            }
            if devices.len() > 1 {
                write!(
                    h.out,
                    "<details class=\"other-volumes\"><summary>{} other volume(s)</summary>",
                    devices.len() - 1
                )?;
                for device in &devices[1..] {
                    card(h, device, partial)?;
                }
                h.raw("</details><p class=\"muted\">Volume capacities are shown separately. APFS volumes may share the same storage.</p>")?;
            }
            if devices.is_empty() {
                h.raw("<p class=\"muted\">No measured device has captured capacity.</p>")?;
            }
        }
        Some((_, _)) => {
            h.raw("<p class=\"muted\">Captured volume data is unsupported or inconsistent; percentages are unavailable. The recorded sizes and worktree list remain available.</p>")?;
        }
        None if measured => {
            h.raw("<p class=\"muted\">Capacity was not recorded in this inventory. The measured size remains available; a new collection with --disk-usage is needed for percentages.</p>")?;
        }
        None => {
            h.raw("<p class=\"muted\">Sizes and capacity are unavailable in this inventory. A new collection with --disk-usage can capture them.</p>")?;
        }
    }
    if let Some(usage) = usage {
        h.raw("<div class=\"dashboard-coverage\"><h4>Measurement coverage</h4>")?;
        for (kind, label) in [
            (DiskKind::Worktree, "Working directories"),
            (DiskKind::GitCommon, "Shared Git directories"),
        ] {
            let (mut complete, mut partial, mut missing, mut unrequested) = (0, 0, 0, 0);
            for row in usage.measurements.iter().filter(|r| r.kind == kind) {
                if !row.requested {
                    unrequested += 1;
                } else if row.bytes.is_none() {
                    missing += 1;
                } else if row.complete {
                    complete += 1;
                } else {
                    partial += 1;
                }
            }
            write!(
                h.out,
                "<p class=\"space-coverage\">{label}: {complete} complete; {partial} partial; {missing} not measured; {unrequested} not requested.</p>"
            )?;
        }
        h.raw("</div>")?;
    }
    h.raw("</div>")
}
