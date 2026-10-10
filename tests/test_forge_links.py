"""Offline Git/forge links exercised through real disposable repositories."""
import json
import os
import subprocess
import unittest
from pathlib import Path

from inventory_support import BINARY, GIT, HistoryFixture, decode_path


class ForgeLinkTests(unittest.TestCase):
    def setUp(self):
        self.f = HistoryFixture()
        self.addCleanup(self.f.close)

    def cli(self, *args):
        before = self.f.target_manifest()
        result = subprocess.run([str(BINARY), *map(str, args)], env=self.f.env,
                                capture_output=True, timeout=40)
        self.assertEqual(before, self.f.target_manifest(), "read-only CLI changed the fixture")
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        return result

    def review(self, *args):
        output = self.f.root / f"links-{len(list(self.f.root.glob('links-*')))}"
        output.mkdir()
        self.cli("review", "--repo", self.f.principal, "--target", "refs/heads/main",
                 "--html", "--run-dir", output, *args)
        run = next(output.glob("run-*"))
        return json.loads((run / "review.json").read_text()), (run / "report.html").read_text(), (run / "report.txt").read_text()

    def git(self, *args):
        return self.f.git("-C", str(self.f.principal), *args)

    def context(self, report):
        return next(row for row in report["context"]["items"]
                    if decode_path(row["path"]) == os.fsencode(self.f.linked))

    def test_upstream_links_are_encoded_local_observations_not_push_claims(self):
        self.git("remote", "add", "fork", "git@github.com:example/project.git")
        self.git("update-ref", "refs/remotes/fork/review/été#1", self.f.base)
        self.git("config", "branch.topic.remote", "fork")
        self.git("config", "branch.topic.merge", "refs/heads/review/été#1")
        detached_path = self.f.root / "detached"
        self.git("worktree", "add", "--detach", str(detached_path), self.f.base)
        self.f.manifest_roots.add(os.fsencode(detached_path))
        report, html, text = self.review()
        remote = self.context(report)["remote"]["remotes"][0]
        self.assertEqual(remote["head_matches_tracking"], True)
        self.assertEqual(decode_path(remote["branch"]), "review/été#1".encode())
        for url in ("https://github.com/example/project",
                    "https://github.com/example/project/tree/review%2F%C3%A9t%C3%A9%231",
                    "https://github.com/example/project/commit/" + self.f.base):
            self.assertIn(f'href="{url}"', html)
            self.assertIn(url, text)
        self.assertIn("HEAD matches the local tracking ref", html)
        self.assertIn("Server not checked", html)
        self.assertIn("No associated PR/MR in the imported evidence", html)
        self.assertNotIn("Pushed: yes", html)
        detached = next(row for row in report["context"]["items"]
                        if decode_path(row["path"]) == os.fsencode(detached_path))
        self.assertIsNone(detached["remote"]["remotes"][0]["branch"])

    def import_requests(self, requests):
        raw = self.f.root / "capture.json"
        export = self.f.root / "forge.json"
        raw.write_text(json.dumps(requests))
        self.cli("forge", "ingerer", "--raw", raw, "--out", export)
        return export

    def test_fork_links_and_pr_point_to_different_projects(self):
        self.git("remote", "add", "origin", "https://github.com/contributor/project.git")
        self.git("remote", "add", "upstream", "https://github.com/maintainer/project.git")
        export = self.import_requests([{
            "number": 17, "state": "open", "title": "Fix <unsafe> title",
            "html_url": "https://github.com/maintainer/project/pull/17",
            "head": {"sha": self.f.base, "ref": "topic", "repo": {"full_name": "contributor/project"}},
            "base": {"ref": "main", "repo": {"full_name": "maintainer/project"}},
            "updated_at": "2026-10-01T12:00:00Z"}])
        report, html, _ = self.review("--forge", export)
        item = next(i for i in report["items"] if decode_path(i["path"]) == os.fsencode(self.f.linked))
        self.assertEqual(item["requests"][0]["iid"], 17)
        self.assertIn('href="https://github.com/contributor/project/tree/topic"', html)
        self.assertIn('href="https://github.com/maintainer/project/pull/17"', html)
        self.assertIn("Fix &lt;unsafe&gt; title", html)
        self.assertIn("captured state: <strong>open", html)
        self.assertIn("2026-10-01T12:00:00Z", html)
        self.assertIn("SHA-256", html)
        self.assertIn('rel="noopener noreferrer"', html)
        self.assertNotIn('href="https://github.com/contributor/project/pull/17"', html)
        disabled, no_html, _ = self.review("--forge", export, "--no-context")
        self.assertIn('href="https://github.com/maintainer/project/pull/17"', no_html)
        self.assertIn("Remote links not collected", no_html)
        self.assertEqual([(i["classification"], i["decision"]) for i in report["items"]],
                         [(i["classification"], i["decision"]) for i in disabled["items"]])

    def test_self_hosted_gitlab_requires_matching_import(self):
        self.git("remote", "add", "origin", "ssh://git@forge.example.invalid/team/sub/project.git")
        before, html, _ = self.review()
        self.assertEqual(self.context(before)["remote"]["remotes"], [])
        self.assertNotIn('href="https://forge.example.invalid', html)
        export = self.import_requests([{
            "iid": 7, "project_id": 17, "source_project_id": 17, "target_project_id": 17,
            "source_branch": "topic", "target_branch": "main", "sha": self.f.base,
            "state": "merged", "title": "Captured MR",
            "web_url": "https://forge.example.invalid/team/sub/project/-/merge_requests/7"}])
        report, html, _ = self.review("--forge", export)
        self.assertEqual(self.context(report)["remote"]["remotes"][0]["provider"], "gitlab")
        for suffix in ("/-/tree/topic", "/-/commit/" + self.f.base, "/-/merge_requests/7"):
            self.assertIn(f'href="https://forge.example.invalid/team/sub/project{suffix}"', html)
        self.assertIn("captured state: <strong>merged", html)

    def test_credentials_unsupported_remotes_and_no_context(self):
        self.git("remote", "add", "origin", "https://user:SYNTHETIC_SECRET@github.com/example/project.git")
        self.git("remote", "add", "unsafe", "https://github.com/example/../evil.git")
        self.git("remote", "add", "script", "javascript:alert(1)")
        report, html, text = self.review()
        for output in (json.dumps(report), html, text):
            self.assertNotIn("SYNTHETIC_SECRET", output)
            self.assertNotIn("javascript:", output)
        self.assertIn('href="https://github.com/example/project/tree/topic"', html)
        self.assertNotIn('href="https://github.com/example/../', html)
        self.assertTrue(self.context(report)["remote"]["gaps"])
        disabled, no_html, _ = self.review("--no-context")
        self.assertNotIn("context", disabled)
        self.assertIn("Remote links not collected", no_html)
        self.assertEqual([(i["classification"], i["decision"]) for i in report["items"]],
                         [(i["classification"], i["decision"]) for i in disabled["items"]])

    def test_divergent_tracking_ref_is_not_called_unpushed(self):
        self.git("remote", "add", "origin", "https://gitlab.com/team/project.git")
        # A real ancestor object is sufficient for a different local tracking OID.
        tree = self.git("rev-parse", "HEAD^{tree}").stdout.strip()
        if isinstance(tree, bytes):
            tree = tree.decode()
        parent = self.git("commit-tree", tree, "-m", "other tracking observation").stdout.strip()
        if isinstance(parent, bytes):
            parent = parent.decode()
        self.git("update-ref", "refs/remotes/origin/topic", parent)
        self.git("config", "branch.topic.remote", "origin")
        self.git("config", "branch.topic.merge", "refs/heads/topic")
        report, html, _ = self.review()
        remote = self.context(report)["remote"]["remotes"][0]
        self.assertFalse(remote["head_matches_tracking"])
        self.assertEqual(remote["tracking_oid"], parent)
        self.assertIn("HEAD differs from the local tracking ref", html)
        self.assertNotIn("Unpushed commits", html)


if __name__ == "__main__":
    unittest.main()
