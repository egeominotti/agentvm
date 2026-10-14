import json
import pathlib
import subprocess
import tempfile
import unittest

GUEST = pathlib.Path(__file__).resolve().parents[1]


class AccountDependencies(unittest.TestCase):
    def dependencies(self, selection):
        with tempfile.TemporaryDirectory() as directory:
            spec = pathlib.Path(directory) / "task.json"
            spec.write_text(json.dumps({"agent": selection}))
            result = subprocess.run(["bash", str(GUEST / "agentvm-install"),
                                     "--account-dependencies", str(spec)],
                                    text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout)

    def test_goose_chatgpt_installs_the_pinned_nested_codex_cli(self):
        self.assertEqual(self.dependencies({"kind": "goose", "auth": "account",
                                            "account_kind": "chatgpt"}),
                         [{"kind": "codex", "package": "@openai/codex", "version": "0.162.1"}])

    def test_goose_claude_and_api_modes_need_no_additional_downloads(self):
        for selection in [{"kind": "goose", "auth": "account"},
                          {"kind": "goose", "auth": "account", "account_kind": "claude"},
                          {"kind": "goose", "auth": "api", "provider": "openai"},
                          {"kind": "goose"}, {"kind": "gemini", "auth": "account"}]:
            with self.subTest(selection=selection):
                self.assertEqual(self.dependencies(selection), [])


if __name__ == "__main__":
    unittest.main()
