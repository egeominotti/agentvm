@AGENTS.md

## Claude Code

- Skills for this repository: `agentvm-dev` (build, test, sandbox, drive a VM) and
  `agentvm-change` (a change from failing test to pushed commit). Use them instead of rediscovering
  the commands.
- The user writes in Italian, terse; answer in Italian. When away they expect the work finished,
  tested, committed and pushed, then a short report.
- Recommend one approach with its reason instead of listing options.
- The dashboard's look is Linear's: quiet surfaces, hairlines, one indigo accent, icons drawn for
  agentvm in `web/src/components/Icon.tsx` (never an icon library). Terminals use Catppuccin
  (Mocha dark, Latte light), as the user's Ghostty.
