use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use wt_sweep::model::BytePath;
use wt_sweep::mounts::{MountBackend, MountLimits, MountRecord, enumerate_with};

struct Backend {
    counts: RefCell<VecDeque<io::Result<usize>>>,
    reads: RefCell<VecDeque<io::Result<Vec<MountRecord>>>>,
    capacities: RefCell<Vec<usize>>,
}
impl MountBackend for Backend {
    fn count(&self) -> io::Result<usize> {
        self.counts
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| panic!("unexpected count"))
    }
    fn read(&self, capacity: usize) -> io::Result<Vec<MountRecord>> {
        self.capacities.borrow_mut().push(capacity);
        self.reads
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| panic!("unexpected read"))
    }
}
fn record(path: &[u8]) -> MountRecord {
    MountRecord {
        path: BytePath::from_bytes(path),
        source: BytePath::from_bytes(b"/dev/synthetic"),
        filesystem: BytePath::from_bytes(b"apfs"),
        flags: 0,
        filesystem_type: 17,
        filesystem_subtype: 0,
    }
}
fn backend(counts: &[usize], reads: Vec<Vec<MountRecord>>) -> Backend {
    Backend {
        counts: RefCell::new(counts.iter().copied().map(Ok).collect()),
        reads: RefCell::new(reads.into_iter().map(Ok).collect()),
        capacities: RefCell::new(Vec::new()),
    }
}

#[test]
fn returns_exact_nonempty_mounts_without_filtering_hidden_or_remote_filesystems() {
    let mut remote = record(b"/hidden/network");
    remote.filesystem = BytePath::from_bytes(b"smbfs");
    let expected = vec![record(b"/"), remote];
    let source = backend(&[2, 2, 2, 2], vec![expected.clone(), expected.clone()]);
    let table = enumerate_with(&source, MountLimits::default())
        .unwrap_or_else(|e| panic!("enumerate: {e}"));
    assert_eq!(table.records, expected);
    assert_eq!(
        table.temporal_consistency,
        "observations_equal_snapshot_unknown"
    );
    assert_eq!(*source.capacities.borrow(), vec![3, 3]);
}

#[test]
fn retries_a_saturated_buffer_and_growing_count_without_losing_mounts() {
    let records = vec![record(b"/"), record(b"/other")];
    let source = backend(
        &[1, 2, 2, 2, 2, 2],
        vec![records.clone(), records.clone(), records.clone()],
    );
    let table = enumerate_with(&source, MountLimits::default())
        .unwrap_or_else(|e| panic!("enumerate: {e}"));
    assert_eq!(table.records, records);
    assert_eq!(*source.capacities.borrow(), vec![2, 3, 3]);
}

#[test]
fn preserves_non_utf8_and_whitespace_bytes_in_every_string_field() {
    let mut raw = record(b"/mount \n\xff");
    raw.source = BytePath::from_bytes(b"server:/name\xfe\t");
    raw.filesystem = BytePath::from_bytes(b"custom\xfd");
    let source = backend(&[1, 1, 1, 1], vec![vec![raw.clone()], vec![raw.clone()]]);
    let table = enumerate_with(&source, MountLimits::default())
        .unwrap_or_else(|e| panic!("enumerate: {e}"));
    assert_eq!(table.records, vec![raw]);
}

#[test]
fn mount_replacement_with_the_same_count_requires_new_matching_observations() {
    let before = record(b"/");
    let mut after = before.clone();
    after.source = BytePath::from_bytes(b"/dev/replaced");
    let source = backend(
        &[1, 1, 1, 1, 1, 1],
        vec![vec![before], vec![after.clone()], vec![after.clone()]],
    );
    let table = enumerate_with(&source, MountLimits::default())
        .unwrap_or_else(|e| panic!("enumerate: {e}"));
    assert_eq!(table.records, vec![after]);
    assert_eq!(source.capacities.borrow().len(), 3);
}

#[test]
fn count_and_read_errors_preserve_errno_and_never_become_empty_tables() {
    for errno in [libc::EPERM, libc::EACCES, libc::EIO] {
        let source = backend(&[], vec![]);
        source
            .counts
            .borrow_mut()
            .push_back(Err(io::Error::from_raw_os_error(errno)));
        let error = enumerate_with(&source, MountLimits::default())
            .err()
            .unwrap_or_else(|| panic!("count error ignored"));
        assert_eq!(error.raw_os_error(), Some(errno));
        let source = backend(&[1], vec![]);
        source
            .reads
            .borrow_mut()
            .push_back(Err(io::Error::from_raw_os_error(errno)));
        let error = enumerate_with(&source, MountLimits::default())
            .err()
            .unwrap_or_else(|| panic!("read error ignored"));
        assert_eq!(error.raw_os_error(), Some(errno));
    }
}

#[test]
fn refuses_zero_oversized_malformed_and_duplicate_mount_tables() {
    let zero = backend(&[0], vec![]);
    assert!(enumerate_with(&zero, MountLimits::default()).is_err());
    let oversized = backend(&[4096], vec![]);
    assert!(enumerate_with(&oversized, MountLimits::default()).is_err());
    assert!(oversized.capacities.borrow().is_empty());
    for records in [
        vec![record(b"relative")],
        vec![record(b"/nul\0path")],
        vec![record(b"/"), record(b"/")],
    ] {
        let source = backend(&[records.len()], vec![records]);
        assert!(enumerate_with(&source, MountLimits::default()).is_err());
    }
}

#[test]
fn persistent_truncation_or_changed_counts_exhaust_the_attempt_budget() {
    let records = vec![record(b"/"), record(b"/other")];
    let source = backend(&[1, 2, 1, 2], vec![records.clone(), records]);
    let error = enumerate_with(
        &source,
        MountLimits {
            max_records: 4,
            max_attempts: 2,
        },
    )
    .err()
    .unwrap_or_else(|| panic!("truncation ignored"));
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert_eq!(source.capacities.borrow().len(), 2);
}

#[test]
fn overflowing_backend_result_and_invalid_limits_are_rejected() {
    let source = backend(&[1], vec![vec![record(b"/"), record(b"/a"), record(b"/b")]]);
    assert!(enumerate_with(&source, MountLimits::default()).is_err());
    for limits in [
        MountLimits {
            max_records: 0,
            max_attempts: 8,
        },
        MountLimits {
            max_records: usize::MAX,
            max_attempts: 8,
        },
        MountLimits {
            max_records: 4,
            max_attempts: 0,
        },
    ] {
        let source = backend(&[], vec![]);
        assert!(enumerate_with(&source, limits).is_err());
    }
}

#[test]
fn equal_mounts_in_different_kernel_order_stabilize() {
    let expected = vec![record(b"/"), record(b"/other")];
    let source = backend(
        &[2, 2, 2, 2],
        vec![
            expected.clone(),
            vec![expected[1].clone(), expected[0].clone()],
        ],
    );
    let table = enumerate_with(&source, MountLimits::default())
        .unwrap_or_else(|e| panic!("enumerate: {e}"));
    assert_eq!(table.records, expected);
}

#[test]
fn same_count_churn_and_final_count_errors_remain_unknown() {
    let first = record(b"/");
    let mut second = first.clone();
    second.flags = 1;
    let source = backend(&[1, 1, 1, 1], vec![vec![first], vec![second]]);
    assert_eq!(
        enumerate_with(
            &source,
            MountLimits {
                max_records: 4,
                max_attempts: 2
            }
        )
        .err()
        .unwrap_or_else(|| panic!("churn ignored"))
        .kind(),
        io::ErrorKind::WouldBlock
    );
    let source = backend(&[1], vec![vec![record(b"/")]]);
    source
        .counts
        .borrow_mut()
        .push_back(Err(io::Error::from_raw_os_error(libc::EIO)));
    assert_eq!(
        enumerate_with(&source, MountLimits::default())
            .err()
            .unwrap_or_else(|| panic!("final count ignored"))
            .raw_os_error(),
        Some(libc::EIO)
    );
}

/// This is a table-only syscall smoke, never a walk or stat of mounted content.
#[cfg(target_os = "macos")]
#[test]
fn native_macos_table_is_nonempty_and_contains_root_without_traversing_volumes() {
    let table = wt_sweep::mounts::enumerate().unwrap_or_else(|e| panic!("native mount table: {e}"));
    assert!(!table.records.is_empty());
    assert!(
        table
            .records
            .iter()
            .any(|record| record.path == BytePath::from_bytes(b"/"))
    );
    assert_eq!(
        table.temporal_consistency,
        "observations_equal_snapshot_unknown"
    );
    println!(
        "native table-only smoke observed {} mounts",
        table.records.len()
    );
}
