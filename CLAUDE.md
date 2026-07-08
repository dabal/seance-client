# Seance

A modern, beautiful, cross-platform (Windows/Mac) VNC client. Built because every free VNC client is trash, not cross-platform, or both.

## Core differentiators

Two pillars.

**1. Creature-comfort input**, headlined by **clipboard-to-keystrokes**: paste from the local host anywhere on the remote — including login/lock screens where VNC clipboard sync (`ClientCutText`) can't work — by synthesizing `KeyEvent` messages from the local clipboard string. This must work even where no remote helper agent is running.

**2. Agent bridge (MCP)**: the Seance client exposes a local MCP *server* so any AI agent (Claude Code, Claude Desktop, Codex, etc. — MCP-agnostic) can see and drive the user's authenticated remote session under human supervision. It is agentless: it works against any VNC server via screenshot + input injection, with no host install. The human's live session is the authorization boundary; consent, live visibility, and audit are core, not settings. Full design in ROADMAP.md — this is the product-defining bet, not a side feature.

## Theming — scope is deliberately limited

The seance gimmick appears in exactly two places. Do not extend it further.

1. **Connecting messages**: a random quip from a list shown while connecting to a host. Examples: "Holding a seance...", "Consulting the shadows...", "Shining the crystal ball...". More in this vein are welcome.
2. **Palette**: attractive dark mode in hues of aubergine.

Everywhere else, use plain professional terminology: hosts are "hosts", sessions are "sessions", connections are "connections". No "spirits", no themed feature names, no rabbit hole.
