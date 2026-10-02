import copy
import unittest
from ci_queue_cleanup import BRANCH, SCOPES, cleanup, eligible, protected_head

REPO = "KooshaPari/ShareCLI"
HEAD, OLD = "a" * 40, "b" * 40
PR = {"number": 878, "state": "open", "merged": False,
      "head": {"ref": BRANCH, "sha": HEAD, "repo": {"full_name": REPO}}}
RUN = {"id": 12, "event": "pull_request", "head_branch": BRANCH,
       "head_repository": {"full_name": REPO}, "head_sha": OLD,
       "path": ".github/workflows/ci.yml", "status": "queued",
       "pull_requests": [{"number": 878}]}

class FakeAPI:
    def __init__(self, run=None, status="ahead", change_head=False):
        self.run = copy.deepcopy(run or RUN)
        self.status, self.change_head = status, change_head
        self.writes, self.pr_reads = [], 0
    def call(self, path, method="GET"):
        if method == "POST":
            self.writes.append(path)
            self.run.update(status="completed", conclusion="cancelled")
            return {}
        if path.startswith("/pulls/"):
            self.pr_reads += 1
            pr = copy.deepcopy(PR)
            if self.change_head and self.pr_reads > 1:
                pr["head"]["sha"] = "c" * 40
            return pr
        if path.startswith("/actions/runs?"):
            return {"workflow_runs": [copy.deepcopy(self.run)]}
        if path.startswith("/compare/"):
            return {"status": self.status, "merge_base_commit": {"sha": OLD}}
        return copy.deepcopy(self.run)

class SafetyTests(unittest.TestCase):
    def test_selection_positive(self):
        self.assertTrue(eligible(RUN, REPO, HEAD, 99))
    def test_excluded_identity_and_execution_classes(self):
        for key, value in [
            ("event", "push"), ("event", "workflow_dispatch"),
            ("head_branch", "main"), ("head_sha", HEAD),
            ("head_sha", "bad"), ("path", ".github/workflows/deploy.yml"),
            ("status", "completed"), ("id", 99),
            ("head_repository", {"full_name": "attacker/ShareCLI"}),
            ("pull_requests", [{"number": 876}]),
        ]:
            with self.subTest(key=key, value=value):
                run = copy.deepcopy(RUN)
                run[key] = value
                self.assertFalse(eligible(run, REPO, HEAD, 99))
    def test_closed_or_foreign_pr_is_refused(self):
        for mutation in [{"state": "closed"}, {"number": 1}, {"merged": True}]:
            with self.assertRaises(ValueError):
                protected_head(PR | mutation, REPO)
    def test_dry_run_performs_no_posts(self):
        api = FakeAPI()
        result = cleanup(api, REPO, 99, False)
        self.assertEqual(len(result["selected"]), 1)
        self.assertEqual(api.writes, [])
    def test_apply_confirms_only_stale_ancestor(self):
        api = FakeAPI()
        result = cleanup(api, REPO, 99, True)
        self.assertEqual(api.writes, ["/actions/runs/12/cancel"])
        self.assertEqual(result["confirmed_cancelled"], [12])
        self.assertEqual(result["qualification"], "NOT_PRODUCT_EVIDENCE")
    def test_diverged_or_newer_candidate_is_preserved(self):
        for status in ["diverged", "behind", "identical", None]:
            api = FakeAPI(status=status)
            self.assertEqual(cleanup(api, REPO, 99, True)["selected"], [])
            self.assertEqual(api.writes, [])
    def test_head_change_aborts_before_cancel(self):
        api = FakeAPI(change_head=True)
        result = cleanup(api, REPO, 99, True)
        self.assertTrue(result["head_changed"])
        self.assertEqual(api.writes, [])
    def test_missing_run_identity_fails_closed(self):
        for key in RUN:
            run = copy.deepcopy(RUN)
            del run[key]
            self.assertFalse(eligible(run, REPO, HEAD, 99))
    def test_wrong_repository_has_no_scope(self):
        self.assertFalse(eligible(RUN, "other/project", HEAD, 99))
    def test_cancellation_budget_is_enforced(self):
        api = FakeAPI()
        result = cleanup(api, REPO, 99, True, max_cancel=0)
        self.assertEqual(result["selected"], [])
        self.assertEqual(api.writes, [])

if __name__ == "__main__":
    unittest.main()
