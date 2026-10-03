import json
from pathlib import Path
import sys
import tempfile
import unittest
from run_negative_oracle import classify, executable_from_build, invoke
NAME = 'example_counterexample'
MARKER = 'changed bytes reused old output'
FAILED = f"running 1 test\ntest {NAME} ... \nthread '{NAME}' panicked at tests/example.rs:21:5:\nassertion `left == right` failed: {MARKER}\n  left: [111]\n right: [112]\nFAILED\nfailures:\n    {NAME}\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.01s\n"
PASSED = f'running 1 test\ntest {NAME} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'

class NegativeOracleTests(unittest.TestCase):

    def test_exact_named_assertion_is_counterexample_not_acceptance(self):
        self.assertEqual(classify(101, FAILED, NAME, MARKER), 'EXPECTED_FAIL')

    def test_panic_with_thread_id_keeps_exact_name_binding(self):
        text = FAILED.replace("' panicked", "' (12345) panicked")
        self.assertEqual(classify(101, text, NAME, MARKER), 'EXPECTED_FAIL')

    def test_remediation_is_not_misreported_as_unsafe(self):
        self.assertEqual(classify(0, PASSED, NAME, MARKER), 'COUNTEREXAMPLE_NOT_REPRODUCED')

    def test_compiler_failure_is_not_counterexample(self):
        self.assertNotEqual(classify(101, 'error: failed to run zig build', NAME, MARKER), 'EXPECTED_FAIL')

    def test_wrong_failure_marker_is_rejected(self):
        self.assertEqual(classify(101, FAILED.replace(MARKER, 'spawn failed'), NAME, MARKER), 'INVALID_ORACLE')

    def test_wrong_test_cannot_satisfy_expected_name(self):
        self.assertEqual(classify(101, FAILED.replace(NAME, 'other'), NAME, MARKER), 'INVALID_ORACLE')

    def test_zero_tests_cannot_be_green(self):
        self.assertEqual(classify(0, PASSED.replace('running 1 test', 'running 0 tests'), NAME, MARKER), 'INVALID_ORACLE')

    def test_ignored_tests_cannot_be_green(self):
        self.assertEqual(classify(101, FAILED.replace('0 ignored', '1 ignored'), NAME, MARKER), 'INVALID_ORACLE')

    def test_multiple_summaries_are_ambiguous(self):
        self.assertEqual(classify(101, FAILED + FAILED, NAME, MARKER), 'INVALID_ORACLE')

    def test_truncated_output_is_not_counterexample(self):
        self.assertEqual(classify(101, FAILED.split('test result:')[0], NAME, MARKER), 'INVALID_ORACLE')

    def test_timeout_and_signals_are_collector_failures(self):
        for code in (None, -9, -15):
            self.assertEqual(classify(code, FAILED, NAME, MARKER), 'COLLECTOR_FAILURE')
        self.assertEqual(classify(101, FAILED, NAME, MARKER, timed_out=True), 'COLLECTOR_FAILURE')

    def test_arbitrary_nonzero_exit_not_proof(self):
        for code in (1, 2, 127, 137):
            self.assertEqual(classify(code, FAILED, NAME, MARKER), 'INVALID_ORACLE')

    def test_missing_panic_is_not_the_expected_assertion(self):
        self.assertEqual(classify(101, FAILED.replace('panicked at', 'message from'), NAME, MARKER), 'INVALID_ORACLE')

    def test_build_failure_is_rejected_even_with_artifact_json(self):
        with self.assertRaises(ValueError):
            executable_from_build(101, '{"reason":"compiler-artifact"}', 'target')

    def test_successful_build_requires_one_existing_test_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / 'test'
            binary.write_text('fixture')
            binary.chmod(448)
            event = {'reason': 'compiler-artifact', 'target': {'name': 'target'}, 'profile': {'test': True}, 'executable': str(binary)}
            self.assertEqual(executable_from_build(0, json.dumps(event), 'target'), binary.resolve())
            with self.assertRaises(ValueError):
                executable_from_build(0, json.dumps(event), 'wrong-target')
            binary.unlink()
            with self.assertRaises(ValueError):
                executable_from_build(0, json.dumps(event), 'target')

    def test_command_failure_records_raw_log(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / 'run.log'
            code, text, timed_out = invoke([sys.executable, '-c', "print('build failed'); raise SystemExit(2)"], log, 2)
            self.assertEqual(code, 2)
            self.assertIn('build failed', text)
            self.assertFalse(timed_out)

    def test_timeout_terminates_process_group_and_never_proves_defect(self):
        with tempfile.TemporaryDirectory() as directory:
            code, text, timed_out = invoke([sys.executable, '-c', 'import time; time.sleep(10)'], Path(directory) / 'run.log', 0.03)
            self.assertTrue(timed_out)
            self.assertEqual(classify(code, text, NAME, MARKER, timed_out=timed_out), 'COLLECTOR_FAILURE')
if __name__ == '__main__':
    unittest.main()
