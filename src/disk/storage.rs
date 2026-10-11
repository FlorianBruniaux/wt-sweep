//! Captured volume statistics and deterministic arithmetic, never cleanup evidence.
pub(super) mod native;
use super::DiskBytes;
use crate::model::{BytePath, Identity};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageUsage {
    pub schema: String,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub temporal_consistency: String,
    pub devices: Vec<DeviceStorage>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceStorage {
    pub device: u64,
    pub bytes: DiskBytes,
    pub volume: Option<VolumeSpace>,
    pub unavailable_reason: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeSpace {
    pub anchor: Identity,
    pub anchor_path: BytePath,
    pub mount_path: Option<BytePath>,
    pub source: Option<BytePath>,
    pub filesystem: Option<BytePath>,
    pub identity_verified: bool,
    pub capacity_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub method: String,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub gaps: Vec<String>,
}

/// Percentage in tenths, rounded half up without a float or narrowing cast.
pub fn percentage_tenths(part: u64, total: Option<u64>) -> Option<u128> {
    let total = u128::from(total.filter(|n| *n > 0)?);
    Some((u128::from(part) * 1000 + total / 2) / total)
}

pub(super) fn blocks_to_bytes(blocks: u64, block_size: u32) -> Option<u64> {
    if block_size == 0 || blocks == u64::MAX {
        return None;
    }
    blocks.checked_mul(u64::from(block_size))
}

/// Both counters are committed together; callers deduplicate inode identities first.
pub(super) fn add_unique(
    global: &mut DiskBytes,
    devices: &mut std::collections::BTreeMap<u64, DiskBytes>,
    device: u64,
    bytes: &DiskBytes,
) -> Result<(), &'static str> {
    let mut next_global = global.clone();
    let mut next_device = devices.get(&device).cloned().unwrap_or_default();
    super::add(&mut next_global, bytes)?;
    super::add(&mut next_device, bytes)?;
    *global = next_global;
    devices.insert(device, next_device);
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn percentages_keep_unknowns_and_large_values() {
        assert_eq!(percentage_tenths(100, Some(2000)), Some(50));
        assert_eq!(percentage_tenths(251, Some(100)), Some(2510));
        assert_eq!(percentage_tenths(u64::MAX, Some(u64::MAX)), Some(1000));
        assert_eq!(percentage_tenths(1, Some(16)), Some(63));
        assert_eq!(percentage_tenths(1, Some(0)), None);
        assert_eq!(percentage_tenths(1, None), None);
        assert_eq!(blocks_to_bytes(10, 4096), Some(40960));
        assert_eq!(blocks_to_bytes(u64::MAX - 1, 4096), None);
        assert_eq!(blocks_to_bytes(u64::MAX, 1), None);
        assert_eq!(blocks_to_bytes(10, 0), None);
    }

    #[test]
    fn device_sums_are_conserved_and_overflow_changes_nothing() {
        let mut total = DiskBytes::default();
        let mut devices = std::collections::BTreeMap::new();
        assert!(
            add_unique(
                &mut total,
                &mut devices,
                7,
                &DiskBytes {
                    logical: 5,
                    allocated: 4096
                }
            )
            .is_ok()
        );
        assert!(
            add_unique(
                &mut total,
                &mut devices,
                8,
                &DiskBytes {
                    logical: 7,
                    allocated: 8192
                }
            )
            .is_ok()
        );
        assert_eq!((total.logical, total.allocated), (12, 12288));
        assert_eq!(devices.values().map(|b| b.allocated).sum::<u64>(), 12288);
        let before = serde_json::to_value((&total, &devices)).expect("fixture");
        assert!(
            add_unique(
                &mut total,
                &mut devices,
                7,
                &DiskBytes {
                    logical: u64::MAX,
                    allocated: 1
                }
            )
            .is_err()
        );
        assert_eq!(
            serde_json::to_value((&total, &devices)).expect("fixture"),
            before
        );
        // A device overflow must likewise leave the global counter unchanged.
        devices.get_mut(&7).expect("device").allocated = u64::MAX;
        let before = serde_json::to_value((&total, &devices)).expect("fixture");
        assert!(
            add_unique(
                &mut total,
                &mut devices,
                7,
                &DiskBytes {
                    logical: 0,
                    allocated: 1
                }
            )
            .is_err()
        );
        assert_eq!(
            serde_json::to_value((&total, &devices)).expect("fixture"),
            before
        );
    }
}
