"""Classify actual tmux stderr without hiding operational failures."""
import importlib
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))


class TerminalErrorsTests(unittest.TestCase):
    def test_missing_server_means_empty_sessions_but_permission_errors_do_not(self):
        terminal = importlib.import_module("agent_terminal")
        cases = [
            ("error connecting to /tmp/tmux-0/default (No such file or directory)\n", True),
            ("no server running on /tmp/tmux-0/default\n", True),
            ("no sessions\n", True),
            ("error connecting to /tmp/tmux-0/default (Permission denied)\n", False),
            ("error connecting to /tmp/tmux-0/default (Connection refused)\n", False),
            ("unknown command: list-sessions", False),
            ("permission denied: no server running", False),
            ("error connecting to /tmp/tmux-0/default (Permission denied)\nerror connecting to /tmp/tmux-0/default (No such file or directory)", False),
        ]
        for message, expected in cases:
            with self.subTest(message=message):
                self.assertEqual(terminal.missing_server(message), expected)


if __name__ == "__main__":
    unittest.main()
