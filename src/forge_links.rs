//! Descriptive web navigation from local configuration and imported forge facts.
//! URLs never establish publication, integration, or cleanup eligibility.
use crate::model::{BytePath, ForgeRequest, RemoteContext, RemoteLink};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(crate) struct Repository {
    pub provider: String,
    pub host: String,
    pub project: String,
}

fn host_ok(host: &str) -> bool {
    let (dns, port) = host
        .split_once(':')
        .map_or((host, None), |(h, p)| (h, Some(p)));
    !dns.is_empty()
        && dns.len() <= 253
        && dns.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 63
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        && port.is_none_or(|p| p.parse::<u16>().is_ok_and(|p| p > 0))
}
fn project_ok(project: &str, provider: &str) -> bool {
    let parts: Vec<_> = project.split('/').collect();
    (parts.len() >= 2 && (provider != "github" || parts.len() == 2))
        && project.len() <= 2048
        && parts.iter().all(|p| {
            !p.is_empty()
                && *p != "."
                && *p != ".."
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
}
fn identity(url: &str) -> Option<(String, String)> {
    if url.len() > 4096
        || url
            .bytes()
            .any(|b| b <= 32 || b == 127 || b"\\?#%".contains(&b))
    {
        return None;
    }
    let (host, path) = if let Some((scheme, rest)) = url.split_once("://") {
        if !["https", "http", "ssh", "git"].contains(&scheme) {
            return None;
        }
        let (authority, path) = rest.split_once('/')?;
        let host = authority.rsplit('@').next()?;
        // An SSH transport port is not a web port. Only the default is understood.
        let host = if scheme == "ssh" {
            if let Some((name, port)) = host.split_once(':') {
                if port != "22" {
                    return None;
                }
                name
            } else {
                host
            }
        } else {
            host
        };
        (host, path)
    } else {
        let (authority, path) = url.split_once(':')?;
        let (user, host) = authority.split_once('@')?;
        if user.is_empty() {
            return None;
        }
        (host, path)
    };
    if !host_ok(host) {
        return None;
    }
    let path = path.trim_end_matches('/');
    let project = path.strip_suffix(".git").unwrap_or(path);
    Some((host.to_ascii_lowercase(), project.to_owned()))
}
impl Repository {
    fn from_remote(url: &str, hints: &BTreeSet<(String, String)>) -> Option<Self> {
        let (host, project) = identity(url)?;
        let provider = match host.as_str() {
            "github.com" => "github",
            "gitlab.com" => "gitlab",
            _ => {
                let providers: BTreeSet<_> = hints
                    .iter()
                    .filter(|(h, _)| h.eq_ignore_ascii_case(&host))
                    .map(|(_, p)| p.as_str())
                    .collect();
                if providers.len() != 1 {
                    return None;
                }
                *providers.first()?
            }
        };
        Self::checked(provider, &host, &project)
    }
    fn checked(provider: &str, host: &str, project: &str) -> Option<Self> {
        if !["github", "gitlab"].contains(&provider)
            || !host_ok(host)
            || !project_ok(project, provider)
            || (host.eq_ignore_ascii_case("github.com") && provider != "github")
            || (host.eq_ignore_ascii_case("gitlab.com") && provider != "gitlab")
        {
            return None;
        }
        Some(Self {
            provider: provider.into(),
            host: host.to_ascii_lowercase(),
            project: project.into(),
        })
    }
    fn url(&self) -> String {
        format!("https://{}/{}", self.host, self.project)
    }
}
fn encode(value: &[u8]) -> String {
    let mut out = String::new();
    for byte in value {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(byte) {
            out.push(char::from(*byte));
        } else {
            use std::fmt::Write;
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}
fn oid_ok(oid: &str) -> bool {
    matches!(oid.len(), 40 | 64)
        && oid.bytes().all(|b| b.is_ascii_hexdigit())
        && oid.bytes().any(|b| b != b'0')
}
/// Revalidate serialized metadata before it can become an HTML href.
pub fn links(remote: &RemoteLink, head: Option<&str>) -> Vec<(&'static str, String)> {
    let Some(repo) = Repository::checked(&remote.provider, &remote.host, &remote.project) else {
        return vec![];
    };
    let base = repo.url();
    let route = if repo.provider == "gitlab" { "/-" } else { "" };
    let mut out = vec![("Repository", base.clone())];
    if let Some(branch) = remote
        .branch
        .as_ref()
        .and_then(|b| b.bytes().ok())
        .filter(|b| {
            !b.is_empty()
                && b.len() <= 4096
                && std::str::from_utf8(b).is_ok()
                && !b.windows(2).any(|w| w == b"..")
                && b.split(|v| *v == b'/')
                    .all(|part| !part.is_empty() && part != b".")
                && !b.iter().any(|b| *b <= 32 || *b == 127)
        })
    {
        out.push(("Branch", format!("{base}{route}/tree/{}", encode(&branch))));
    }
    if let Some(head) = head.filter(|h| oid_ok(h)) {
        out.push(("Commit", format!("{base}{route}/commit/{head}")));
    }
    out
}
/// Imported URLs are accepted only when they agree with provider, host and request id.
pub fn request_url(request: &ForgeRequest) -> Option<String> {
    let url = request.url.as_deref()?;
    let marker = match request.provider.as_str() {
        "github" => "/pull/",
        "gitlab" => "/-/merge_requests/",
        _ => return None,
    };
    let (base, number) = url.rsplit_once(marker)?;
    if number != request.iid.to_string() || request.iid == 0 {
        return None;
    }
    let (host, project) = identity(base)?;
    if !request.host.as_ref()?.eq_ignore_ascii_case(&host) {
        return None;
    }
    if request.provider == "github" && request.project.as_deref() != Some(&project) {
        return None;
    }
    let repo = Repository::checked(&request.provider, &host, &project)?;
    Some(format!("{}{marker}{}", repo.url(), request.iid))
}

#[derive(Clone)]
struct Tracking {
    oid: String,
    remote: Vec<u8>,
    branch: Vec<u8>,
    reference: Vec<u8>,
}
#[derive(Clone, Default)]
pub(crate) struct Snapshot {
    remotes: Vec<(BytePath, Repository)>,
    refs: BTreeMap<Vec<u8>, Tracking>,
    pub gaps: Vec<String>,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
}
impl Snapshot {
    pub fn parse_remotes(&mut self, raw: &[u8], hints: &BTreeSet<(String, String)>) {
        let mut names: BTreeMap<Vec<u8>, Vec<&[u8]>> = BTreeMap::new();
        for field in raw.split(|b| *b == 0).filter(|f| !f.is_empty()) {
            let Some(split) = field.iter().position(|b| *b == b'\n') else {
                self.gaps
                    .push("Malformed remote configuration record".into());
                return;
            };
            let Some(name) = field[..split]
                .strip_prefix(b"remote.")
                .and_then(|k| k.strip_suffix(b".url"))
            else {
                self.gaps
                    .push("Ambiguous remote key framing; link omitted".into());
                continue;
            };
            names
                .entry(name.to_vec())
                .or_default()
                .push(&field[split + 1..]);
        }
        if names.len() > 32 {
            self.gaps.push("Remote count limit reached (32)".into());
        }
        for (name, urls) in names.into_iter().take(32) {
            if urls.len() != 1 {
                self.gaps
                    .push("Remote has multiple URLs; destination ambiguous".into());
                continue;
            }
            if let Some(repo) = std::str::from_utf8(urls[0])
                .ok()
                .and_then(|url| Repository::from_remote(url, hints))
            {
                self.remotes.push((BytePath::from_bytes(&name), repo));
            } else {
                self.gaps.push(
                    "Remote URL unsupported or forge provider unidentified; link omitted".into(),
                );
            }
        }
    }
    pub fn has_remotes(&self) -> bool {
        !self.remotes.is_empty()
    }
    pub fn parse_refs(&mut self, raw: &[u8]) -> Result<(), &'static str> {
        let mut refs = BTreeMap::new();
        for row in raw.split(|b| *b == b'\n').filter(|r| !r.is_empty()) {
            let fields: Vec<_> = row.split(|b| *b == 0).collect();
            if fields.len() != 6 || !fields[5].is_empty() {
                return Err("Malformed tracking-ref metadata");
            }
            let oid = std::str::from_utf8(fields[1])
                .ok()
                .filter(|o| oid_ok(o))
                .ok_or("Invalid tracking-ref OID")?;
            refs.insert(
                fields[0].to_vec(),
                Tracking {
                    oid: oid.into(),
                    remote: fields[2].to_vec(),
                    branch: fields[3].to_vec(),
                    reference: fields[4].to_vec(),
                },
            );
        }
        self.refs = refs;
        Ok(())
    }
    pub fn for_worktree(&self, branch: Option<&BytePath>, head: Option<&str>) -> RemoteContext {
        let branch = branch.and_then(|b| b.bytes().ok());
        let tracked = branch.as_ref().and_then(|b| self.refs.get(b));
        let mut remotes = Vec::new();
        for (name, repo) in &self.remotes {
            let upstream = tracked.filter(|t| {
                name.bytes().ok().as_deref() == Some(t.remote.as_slice())
                    && t.branch.starts_with(b"refs/heads/")
            });
            let remote_branch = upstream
                .map(|t| t.branch.as_slice())
                .or(branch.as_deref())
                .and_then(|b| b.strip_prefix(b"refs/heads/"));
            let tracking = upstream.and_then(|t| self.refs.get(&t.reference));
            remotes.push(RemoteLink {
                name: name.clone(),
                provider: repo.provider.clone(),
                host: repo.host.clone(),
                project: repo.project.clone(),
                branch: remote_branch.map(BytePath::from_bytes),
                tracking_ref: upstream
                    .filter(|t| !t.reference.is_empty())
                    .map(|t| BytePath::from_bytes(&t.reference)),
                tracking_oid: tracking.map(|t| t.oid.clone()),
                head_matches_tracking: tracking
                    .zip(head)
                    .map(|(t, h)| t.oid.eq_ignore_ascii_case(h)),
            });
        }
        RemoteContext {
            started_unix_ms: self.started_unix_ms,
            finished_unix_ms: self.finished_unix_ms,
            remotes,
            gaps: self.gaps.clone(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsafe_and_ambiguous_web_destinations() {
        for url in [
            "file:///tmp/a",
            "ext::command",
            "javascript:alert(1)",
            "https://github.com/a/../b",
            "https://github.com/a/b?token=x",
            "https://github.com/a/b#fragment",
            "https://github.com/a/%2e%2e",
            "https://github.com/a\\b/c",
            "https://github.com/a/b\n",
            "ssh://git@github.com:2222/a/b",
            "https://github.com@evil.invalid/a/b",
            "https://[::1]/a/b",
        ] {
            assert!(
                Repository::from_remote(url, &BTreeSet::new()).is_none(),
                "{url}"
            );
        }
        let hints = [
            ("host.invalid".into(), "gitlab".into()),
            ("host.invalid".into(), "github".into()),
        ]
        .into();
        assert!(Repository::from_remote("https://host.invalid/a/b", &hints).is_none());
        let mut snapshot = Snapshot::default();
        snapshot.parse_remotes(b"remote.origin.url\nhttps://github.com/a/b\0remote.origin.url\nhttps://github.com/c/d\0", &BTreeSet::new());
        assert!(!snapshot.has_remotes());
        assert_eq!(snapshot.gaps.len(), 1);
    }
    #[test]
    fn ambiguous_remote_key_framing_reports_a_gap() {
        let mut snapshot = Snapshot::default();
        snapshot.parse_remotes(
            b"remote.unusual\nname.url\nhttps://github.com/team/repo.git\0",
            &BTreeSet::new(),
        );
        assert!(!snapshot.has_remotes());
        assert_eq!(snapshot.gaps.len(), 1);
    }
    #[test]
    fn safe_remote_forms_discard_credentials() {
        for url in [
            "git@github.com:team/project.git",
            "ssh://git@github.com:22/team/project.git",
            "https://user:SECRET@github.com/team/project.git/",
        ] {
            let repo = Repository::from_remote(url, &BTreeSet::new()).unwrap();
            assert_eq!(repo.url(), "https://github.com/team/project");
        }
    }
    #[test]
    fn render_revalidates_metadata_and_preserves_branch_bytes() {
        let mut remote = RemoteLink {
            name: BytePath::from_bytes(b"origin"),
            provider: "gitlab".into(),
            host: "gitlab.com".into(),
            project: "team/sub/repo".into(),
            branch: Some(BytePath::from_bytes(b"topic/percent%#x")),
            tracking_ref: None,
            tracking_oid: None,
            head_matches_tracking: None,
        };
        let urls = links(&remote, Some(&"a".repeat(40)));
        assert_eq!(
            urls[1].1,
            "https://gitlab.com/team/sub/repo/-/tree/topic%2Fpercent%25%23x"
        );
        remote.branch = Some(BytePath::from_bytes(b"topic/\xff"));
        assert_eq!(links(&remote, None).len(), 1);
        remote.host = "gitlab.com\" onclick=\"evil".into();
        assert!(links(&remote, None).is_empty());
    }
    #[test]
    fn request_url_checks_provider_host_project_and_number() {
        let mut request = ForgeRequest {
            provider: "github".into(),
            host: Some("github.com".into()),
            project: Some("team/repo".into()),
            iid: 12,
            source_project: None,
            target_project: None,
            source_branch: None,
            target_branch: None,
            state: "closed".into(),
            title: None,
            head_oid: None,
            merge_oid: None,
            updated_at: None,
            url: Some("https://github.com/team/repo/pull/12".into()),
            source_index: 0,
        };
        assert_eq!(
            request_url(&request).as_deref(),
            Some("https://github.com/team/repo/pull/12")
        );
        for url in [
            "javascript:alert(1)",
            "https://github.com/team/repo/pull/13",
            "https://evil.invalid/team/repo/pull/12",
            "https://github.com/other/repo/pull/12",
            "https://github.com/team/repo/pull/12?x=y",
        ] {
            request.url = Some(url.into());
            assert!(request_url(&request).is_none(), "{url}");
        }
    }
    #[test]
    fn malformed_refs_do_not_produce_partial_tracking_claims() {
        let mut snapshot = Snapshot::default();
        assert!(
            snapshot
                .parse_refs(b"refs/heads/main\0bad\0\0\0\0\n")
                .is_err()
        );
        assert!(snapshot.refs.is_empty());
    }
}
