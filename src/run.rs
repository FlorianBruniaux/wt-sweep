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

pub fn publish(
    base: &Path,
    artifact: Artifact<'_>,
    boundaries: &Inventory,
) -> Result<PathBuf, SweepError> {
    let started = now_ms();
    let directory = RunDirectory::create(base, boundaries)?;
    let run_id = directory
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| {
            SweepError::new(
                "run_output",
                "publish_run",
                base,
                "invalid generated run identity",
            )
        })?;
    directory.write(artifact.name, artifact.json)?;
    directory.write("report.txt", artifact.report.as_bytes())?;
    directory.verify_and_sync()?;
    let receipt = serde_json::json!({
        "schema": "wt-sweep.run/1",
        "run_id": run_id,
        "command": artifact.command,
        "status": if artifact.partial { "partial" } else { "complete" },
        "started_unix_ms": started,
        "finished_unix_ms": now_ms(),
        "temporal_consistency": "not_guaranteed",
        "artifacts": [
            {"name": artifact.name, "sha256": sha256(artifact.json), "bytes": artifact.json.len()},
            {"name": "report.txt", "sha256": sha256(artifact.report.as_bytes()), "bytes": artifact.report.len()}
        ]
    });
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|e| {
        SweepError::new(
            "run_output",
            "serialize_run",
            &directory.path,
            e.to_string(),
        )
    })?;
    directory.write("run.json", &bytes)?;
    directory.verify_and_sync()?;
    Ok(directory.path)
}
