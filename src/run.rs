//! One owned directory per publication. A receipt is not a backup certificate.
use crate::{
    model::{Inventory, SweepError, now_ms},
    output::{RunDirectory, sha256},
};
use std::path::{Path, PathBuf};

pub struct Artifact<'a> {
    pub command: &'a str,
    pub name: &'a str,
    pub json: &'a [u8],
    pub report: &'a str,
    pub partial: bool,
}

struct NativePublicationIo;
impl PublicationIo for NativePublicationIo {}

pub fn publish(
    base: &Path,
    artifact: Artifact<'_>,
    boundaries: &Inventory,
) -> Result<PathBuf, SweepError> {
    publish_with_io(base, artifact, boundaries, &mut NativePublicationIo)
}
fn failed(
    directory: &RunDirectory,
    phase: PublicationPhase,
    receipt_visible: bool,
    message: impl std::fmt::Display,
) -> SweepError {
    SweepError::new(
        "run_output",
        phase.as_str(),
        &directory.path,
        format!(
            "run_path={} stage={} receipt_may_be_visible={} durability=unknown: {}",
            crate::model::BytePath::from_path(&directory.path).display(),
            phase.as_str(),
            receipt_visible,
            message
        ),
    )
}
fn write_artifact(
    directory: &RunDirectory,
    name: &str,
    bytes: &[u8],
    io: &mut dyn PublicationIo,
    write_phase: PublicationPhase,
    sync_phase: PublicationPhase,
    receipt_visible: bool,
) -> Result<(), SweepError> {
    let mut file = directory
        .open_artifact(name)
        .map_err(|e| failed(directory, write_phase, receipt_visible, e))?;
    let mut remaining = bytes;
    while !remaining.is_empty() {
        // Explicit loop preserves short writes while refusing Interrupted. A
        // signal latch alone never prevents publishing a partial observation.
        let count = io
            .write(&mut file, remaining, write_phase)
            .map_err(|e| failed(directory, write_phase, receipt_visible, e))?;
        if count == 0 || count > remaining.len() {
            return Err(failed(
                directory,
                write_phase,
                receipt_visible,
                "invalid or zero-length artifact write",
            ));
        }
        remaining = &remaining[count..];
    }
    io.sync(&file, sync_phase)
        .map_err(|e| failed(directory, sync_phase, receipt_visible, e))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationPhase {
    InventoryWrite,
    InventorySync,
    JsonWrite,
    JsonSync,
    ReportWrite,
    ReportSync,
    ArtifactsDirectorySync,
    ReceiptWrite,
    ReceiptSync,
    FinalDirectorySync,
    ParentDirectorySync,
}
impl PublicationPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InventoryWrite => "inventory_write",
            Self::InventorySync => "inventory_sync",
            Self::JsonWrite => "json_write",
            Self::JsonSync => "json_sync",
            Self::ReportWrite => "report_write",
            Self::ReportSync => "report_sync",
            Self::ArtifactsDirectorySync => "artifacts_directory_sync",
            Self::ReceiptWrite => "receipt_write",
            Self::ReceiptSync => "receipt_sync",
            Self::FinalDirectorySync => "final_directory_sync",
            Self::ParentDirectorySync => "parent_directory_sync",
        }
    }
}
pub trait PublicationIo {
    fn write(
        &mut self,
        file: &mut std::fs::File,
        bytes: &[u8],
        _phase: PublicationPhase,
    ) -> std::io::Result<usize> {
        std::io::Write::write(file, bytes)
    }
    fn sync(&mut self, file: &std::fs::File, _phase: PublicationPhase) -> std::io::Result<()> {
        file.sync_all()
    }
}
pub fn publish_with_io(
    base: &Path,
    artifact: Artifact<'_>,
    boundaries: &Inventory,
    io: &mut dyn PublicationIo,
) -> Result<PathBuf, SweepError> {
    publish_internal(base, artifact, boundaries, None, io)
}
fn publish_internal(
    base: &Path,
    artifact: Artifact<'_>,
    boundaries: &Inventory,
    inventory_json: Option<&[u8]>,
    io: &mut dyn PublicationIo,
) -> Result<PathBuf, SweepError> {
    use PublicationPhase::*;
    let limits = crate::output::PublicationLimits::default();
    if inventory_json.is_some() && artifact.name != "review.json" {
        return Err(SweepError::new(
            "run_output",
            "artifact_validation",
            base,
            "input inventory requires primary review.json; artifact names must not collide",
        ));
    }
    if artifact.json.len() > limits.json_bytes
        || artifact.report.len() > limits.report_bytes
        || inventory_json.is_some_and(|bytes| bytes.len() > limits.json_bytes)
    {
        return Err(SweepError::new(
            "run_output",
            "artifact_validation",
            base,
            "full artifact exceeds publication byte limit; rescan a smaller scope",
        ));
    }
    let started = now_ms();
    let directory = RunDirectory::create(base, boundaries)?;
    let run_id = directory
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            failed(
                &directory,
                JsonWrite,
                false,
                "invalid generated run identity",
            )
        })?;
    if let Some(bytes) = inventory_json {
        write_artifact(
            &directory,
            "inventory.json",
            bytes,
            io,
            InventoryWrite,
            InventorySync,
            false,
        )?;
    }
    write_artifact(
        &directory,
        artifact.name,
        artifact.json,
        io,
        JsonWrite,
        JsonSync,
        false,
    )?;
    write_artifact(
        &directory,
        "report.txt",
        artifact.report.as_bytes(),
        io,
        ReportWrite,
        ReportSync,
        false,
    )?;
    directory
        .verify()
        .map_err(|e| failed(&directory, ArtifactsDirectorySync, false, e))?;
    io.sync(directory.directory_file(), ArtifactsDirectorySync)
        .map_err(|e| failed(&directory, ArtifactsDirectorySync, false, e))?;
    let mut artifacts = Vec::new();
    if let Some(bytes) = inventory_json {
        artifacts.push(serde_json::json!({"name": "inventory.json", "sha256": sha256(bytes), "bytes": bytes.len()}));
    }
    artifacts.push(serde_json::json!({"name": artifact.name, "sha256": sha256(artifact.json), "bytes": artifact.json.len()}));
    artifacts.push(serde_json::json!({"name": "report.txt", "sha256": sha256(artifact.report.as_bytes()), "bytes": artifact.report.len()}));
    let receipt = serde_json::json!({
        "schema": "wt-sweep.run/1", "run_id": run_id, "command": artifact.command,
        "status": if artifact.partial { "partial" } else { "complete" },
        "started_unix_ms": started, "finished_unix_ms": now_ms(),
        "temporal_consistency": "not_guaranteed",
        "publication": { "meaning": "artifacts_synced_before_receipt",
            "completion": "requires_successful_publisher_return", "durability": "unknown_from_receipt_alone" },
        "artifacts": artifacts
    });
    let bytes = crate::output::bounded_json(
        &receipt,
        crate::output::PublicationLimits::default().metadata_reserve_bytes,
    )
    .map_err(|e| failed(&directory, ReceiptWrite, false, e))?;
    write_artifact(
        &directory,
        "run.json",
        &bytes,
        io,
        ReceiptWrite,
        ReceiptSync,
        true,
    )?;
    directory
        .verify()
        .map_err(|e| failed(&directory, FinalDirectorySync, true, e))?;
    io.sync(directory.directory_file(), FinalDirectorySync)
        .map_err(|e| failed(&directory, FinalDirectorySync, true, e))?;
    io.sync(directory.parent_file(), ParentDirectorySync)
        .map_err(|e| failed(&directory, ParentDirectorySync, true, e))?;
    Ok(directory.path)
}

pub fn publish_with_inventory(
    base: &Path,
    artifact: Artifact<'_>,
    boundaries: &Inventory,
    inventory_json: &[u8],
) -> Result<PathBuf, SweepError> {
    publish_with_inventory_io(
        base,
        artifact,
        boundaries,
        inventory_json,
        &mut NativePublicationIo,
    )
}
pub fn publish_with_inventory_io(
    base: &Path,
    artifact: Artifact<'_>,
    boundaries: &Inventory,
    inventory_json: &[u8],
    io: &mut dyn PublicationIo,
) -> Result<PathBuf, SweepError> {
    publish_internal(base, artifact, boundaries, Some(inventory_json), io)
}
