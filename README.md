# dcal

Projects lifecycle management for [Claude Code](https://docs.anthropic.com/en/docs/claude-code).

With Claude Code it is so easy to just create a folder and start building something. However, because of that, after a while you end up with multiple folders/projects
that are hard to manage and know the status of each. That is why I created this tool. dcal is nothing more than a lightweight CLI tool
that helps track and work with multiple Claude Code projects at the same time.

## More specifically dcal:

- **Tracks projects** in a local registry with status, phase, and timestamps
- **Journals sessions automatically** by reading CC transcript files and summarizing them
- **Generates reengagement briefs** so you can pick up any project cold
- **Scaffolds new projects** from a plain-language idea, complete with a generated CLAUDE.md
- **Manages lifecycle**: pause, resume, search, inspect any project from one place

## Install

Requires Rust (stable) and Claude Code on PATH.

```
cargo install dcal
```

Or build from source:

```
git clone https://github.com/gkanellopoulos/dcal.git
cd dcal
cargo build --release
```

Set your Anthropic API key (needed for project creation and session sync):

```
export ANTHROPIC_API_KEY=sk-ant-...
```

dcal uses `claude-haiku-4-5` for intake and `claude-sonnet-4-5` for CLAUDE.md generation during `dcal new`. Session sync calls `claude -p` (your default Claude Code model). All models are configurable in `~/.dcal/config.yml`. Costs are minimal, a few cents per project creation and fractions of a cent per session sync.

## Usage

```
dcal init                        # one-time setup
dcal new                         # create a project from an idea
dcal new --model opus            # create and launch CC with a specific model
dcal list                        # see all projects
dcal sync                        # process unsynced CC sessions
dcal resume <project>            # get a brief, launch CC with context
dcal resume <project> --model opus  # resume with a specific model
dcal pause <project> --note "…"  # shelve a project
dcal onboard <path>              # import an existing CC project
dcal info <project>              # full project dashboard
dcal search <query>              # find projects by name
```

Run `dcal --help` for the full command list.

## How it works

dcal stores project data in `~/.dcal/`: a registry, per-project metadata, an append-only journal, and session records. It reads Claude Code's session transcripts from `~/.claude/projects/`, summarizes them via the Anthropic API, and writes structured journal entries.

No data leaves your machine except the API calls to Anthropic for summarization.

## License

MIT
