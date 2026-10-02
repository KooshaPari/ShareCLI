"""Reject false-positive queue qualification logs; no Rust execution asserted."""
import unittest
from run_queue_lease_oracle import classify_positive

class QueueOracleTests(unittest.TestCase):
    expected = ["queue::tests::a", "queue::tests::b"]
    log = "running 2 tests\ntest queue::tests::a ... ok\ntest queue::tests::b ... ok\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n"
    def test_exact_inventory_passes(self):
        self.assertTrue(classify_positive(0, self.log, self.expected))
    def test_zero_tests_rejected(self):
        self.assertFalse(classify_positive(0, 'test result: ok. 0 passed; 0 failed; 0 ignored;', self.expected))
    def test_wrong_name_rejected(self):
        self.assertFalse(classify_positive(0, self.log.replace('::b', '::other'), self.expected))
    def test_ignored_rejected(self):
        self.assertFalse(classify_positive(0, self.log.replace('::b ... ok', '::b ... ignored'), self.expected))
    def test_duplicate_rejected(self):
        self.assertFalse(classify_positive(0, self.log.replace('::b', '::a'), self.expected))
    def test_missing_summary_rejected(self):
        self.assertFalse(classify_positive(0, self.log.split('test result:')[0], self.expected))
    def test_nonzero_rejected(self):
        for code in [1, 101, 127, -9, None]:
            self.assertFalse(classify_positive(code, self.log, self.expected))
    def test_timeout_rejected(self):
        self.assertFalse(classify_positive(0, self.log, self.expected, timed_out=True))
    def test_multiple_summaries_rejected(self):
        self.assertFalse(classify_positive(0, self.log + self.log, self.expected))
    def test_empty_inventory_rejected(self):
        self.assertFalse(classify_positive(0, '', []))

if __name__ == '__main__':
    unittest.main()
