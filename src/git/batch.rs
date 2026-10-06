//! Strict framing for finite Git object batches.
use crate::model::SweepError;
use std::path::Path;

#[derive(Debug)]
pub(crate) struct ObjectInfo {
    pub kind: String,
    pub size: usize,
}
#[derive(Debug)]
pub(crate) struct GitObject {
    pub kind: String,
    pub bytes: Vec<u8>,
}
fn invalid() -> SweepError {
    SweepError::new(
        "git_batch",
        "parse_batch",
        Path::new(""),
        "invalid object batch response",
    )
}
fn header(raw: &[u8], oid: &str) -> Result<(Result<ObjectInfo, String>, usize), SweepError> {
    let end = raw
        .iter()
        .take(160)
        .position(|b| *b == b'\n')
        .ok_or_else(invalid)?;
    let line = std::str::from_utf8(&raw[..end]).map_err(|_| invalid())?;
    let fields: Vec<_> = line.split(' ').collect();
    if fields
        .first()
        .is_none_or(|value| !value.eq_ignore_ascii_case(oid))
    {
        return Err(invalid());
    }
    if fields.len() == 2 && fields[1] == "missing" {
        return Ok((Err(format!("object {oid} missing")), end + 1));
    }
    if fields.len() != 3
        || !["blob", "tree", "commit", "tag"].contains(&fields[1])
        || fields[2].is_empty()
        || !fields[2].bytes().all(|b| b.is_ascii_digit())
    {
        return Err(invalid());
    }
    let size = fields[2].parse::<usize>().map_err(|_| invalid())?;
    Ok((
        Ok(ObjectInfo {
            kind: fields[1].into(),
            size,
        }),
        end + 1,
    ))
}

pub(crate) fn parse_object_info(
    mut raw: &[u8],
    oids: &[String],
) -> Result<Vec<Result<ObjectInfo, String>>, SweepError> {
    if oids.is_empty() || oids.len() > 128 {
        return Err(invalid());
    }
    let mut records = Vec::with_capacity(oids.len());
    for oid in oids {
        let (info, used) = header(raw, oid)?;
        records.push(info);
        raw = &raw[used..];
    }
    if !raw.is_empty() {
        return Err(invalid());
    }
    Ok(records)
}

pub(crate) fn parse_objects(
    mut raw: &[u8],
    oids: &[String],
) -> Result<Vec<Result<GitObject, String>>, SweepError> {
    if oids.is_empty() || oids.len() > 128 {
        return Err(invalid());
    }
    let mut records = Vec::with_capacity(oids.len());
    for oid in oids {
        let (info, used) = header(raw, oid)?;
        raw = &raw[used..];
        match info {
            Err(reason) => records.push(Err(reason)),
            Ok(info) => {
                let end = info.size.checked_add(1).ok_or_else(invalid)?;
                let payload = raw.get(..end).ok_or_else(invalid)?;
                if payload[info.size] != b'\n' {
                    return Err(invalid());
                }
                records.push(Ok(GitObject {
                    kind: info.kind,
                    bytes: payload[..info.size].to_vec(),
                }));
                raw = &raw[end..];
            }
        }
    }
    if !raw.is_empty() {
        return Err(invalid());
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::{ControlledGit, GitRead, run_bounded, run_bounded_inner};
    use std::{fs, time::Duration};

    #[test]
    fn real_batch_reads_binary_objects_and_preserves_missing_per_object() {
        for format in ["sha1", "sha256"] {
            let root = std::env::temp_dir().join(format!(
                "wt-sweep-batch-{}-{}-{format}",
                std::process::id(),
                crate::model::now_ms()
            ));
            fs::create_dir(&root).unwrap_or_else(|e| panic!("{e:?}"));
            let git = ControlledGit::default();
            let mut init = git.command(&root);
            init.args(["init", "-q", &format!("--object-format={format}")]);
            run_bounded(init, &root, "fixture_init", Duration::from_secs(10), 4096)
                .unwrap_or_else(|e| panic!("{e:?}"));
            let payload = b"binary\0data\nwith newlines\n";
            let mut hash = git.command(&root);
            hash.args(["hash-object", "-w", "--stdin"]);
            let oid = String::from_utf8(
                run_bounded_inner(
                    hash,
                    &root,
                    "fixture_blob",
                    Duration::from_secs(10),
                    4096,
                    false,
                    (payload, None),
                )
                .unwrap_or_else(|e| panic!("{e:?}")),
            )
            .unwrap_or_else(|e| panic!("{e:?}"))
            .trim()
            .to_owned();
            let oids = vec![oid.to_uppercase(), "1".repeat(oid.len()), oid.clone()];
            let before = git.spawned_processes();
            let info = git.read(&root, GitRead::ObjectInfo { oids: oids.clone() });
            let raw = git.read(&root, GitRead::Objects { oids: oids.clone() });
            fs::remove_dir_all(&root).unwrap_or_else(|e| panic!("{e:?}"));
            let info = parse_object_info(&info.unwrap_or_else(|e| panic!("{e:?}")), &oids)
                .unwrap_or_else(|e| panic!("{e:?}"));
            assert_eq!(
                info[0].as_ref().unwrap_or_else(|e| panic!("{e:?}")).size,
                payload.len()
            );
            assert_eq!(
                info[0].as_ref().unwrap_or_else(|e| panic!("{e:?}")).kind,
                "blob"
            );
            assert!(info[1].is_err());
            let objects = parse_objects(&raw.unwrap_or_else(|e| panic!("{e:?}")), &oids)
                .unwrap_or_else(|e| panic!("{e:?}"));
            for n in [0, 2] {
                let object = objects[n].as_ref().unwrap_or_else(|e| panic!("{e:?}"));
                assert_eq!(object.kind, "blob");
                assert_eq!(object.bytes, payload);
                crate::review::verify_object("blob", &oid, &object.bytes)
                    .unwrap_or_else(|e| panic!("{e:?}"));
            }
            assert!(objects[1].is_err());
            assert_eq!(git.spawned_processes() - before, 2);
        }
    }

    #[test]
    fn parser_rejects_wrong_order_truncation_overflow_and_extra_bytes() {
        let oid = "a".repeat(40);
        let ids = vec![oid.clone()];
        let valid = format!("{oid} blob 3\nabc\n");
        assert_eq!(
            parse_objects(valid.as_bytes(), &ids).unwrap_or_else(|e| panic!("{e:?}"))[0]
                .as_ref()
                .unwrap_or_else(|e| panic!("{e:?}"))
                .bytes,
            b"abc"
        );
        for invalid in [
            format!("{} blob 3\nabc\n", "b".repeat(40)),
            format!("{oid} blob 3\nab\n"),
            format!("{oid} blob 3\nabc!"),
            format!("{oid} unknown 3\nabc\n"),
            format!("{oid} blob 999999999999999999999999999\nabc\n"),
            format!("{valid}extra"),
        ] {
            assert!(
                parse_objects(invalid.as_bytes(), &ids).is_err(),
                "{invalid:?}"
            );
        }
        for invalid in [
            format!("{oid} blob -1\n"),
            format!("{oid} blob 3\nextra"),
            format!("{oid} missing\nextra"),
        ] {
            assert!(parse_object_info(invalid.as_bytes(), &ids).is_err());
        }
    }

    #[test]
    fn invalid_batch_input_never_spawns_git() {
        let git = ControlledGit::default();
        for oids in [
            vec![],
            vec!["HEAD".into()],
            vec!["0".repeat(40)],
            vec![format!("{}\nHEAD", "a".repeat(40))],
            vec!["a".repeat(40); 129],
        ] {
            assert!(git.read(Path::new("/"), GitRead::Objects { oids }).is_err());
        }
        assert_eq!(git.spawned_processes(), 0);
    }
}
