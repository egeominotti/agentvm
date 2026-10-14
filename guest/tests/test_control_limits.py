"""Special files must never pin a guest worker indefinitely."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class FileLimitsTests(unittest.TestCase):
    def test_reading_a_fifo_is_rejected_without_waiting_for_a_writer(self):
        with tempfile.TemporaryDirectory() as directory:
            fifo = str(Path(directory, "pipe"))
            os.mkfifo(fifo)
            code = "import agent_files,sys\ntry:\n agent_files.dispatch('read', {'path':sys.argv[1]})\nexcept (ValueError,OSError):\n print('rejected')\n"
            out = subprocess.run([sys.executable, "-c", code, fifo], cwd=Path(__file__).resolve().parents[1],
                                 capture_output=True, text=True, timeout=2)
            self.assertEqual(out.stdout.strip(), "rejected")


if __name__ == "__main__":
    unittest.main()
