# GitHub PR Review for Zed

Review GitHub pull requests without leaving Zed. Ask the Agent panel to list PRs, read diffs, leave inline comments, approve or request changes. Authentication is handled by the GitHub CLI, so there are no tokens to configure.

## Features

- **List and inspect** open pull requests: description, branches, stats, mergeable state.
- **Read diffs** file by file, straight from GitHub.
- **Review**: submit `COMMENT`, `APPROVE` or `REQUEST_CHANGES` reviews.
- **Safe inline comments**: every comment is validated against the PR diff before it is sent, so GitHub never rejects a bad line position.
- **Zero credentials**: uses your existing `gh auth login` session.
- **Cross-platform**: Windows, macOS (Apple Silicon and Intel) and Linux.

## Requirements

- [GitHub CLI](https://cli.github.com/), signed in with `gh auth login`
- Zed with a model configured in the Agent panel

## Installation

### Option 1: Zed extension

Open the command palette, run `zed: install dev extension` and select the `extension/` folder of this repository. The extension downloads the matching `zpr-mcp` binary from the latest GitHub release.

### Option 2: Custom MCP server

1. Download `zpr-mcp` for your platform from the [Releases](../../releases) page.
2. Add it to your Zed `settings.json`:

```json
{
  "context_servers": {
    "github-pr-review": {
      "command": "/absolute/path/to/zpr-mcp",
      "args": []
    }
  }
}
```

3. Open the Agent panel and make sure the server is enabled.

## Usage

Open a GitHub repository in Zed and ask the Agent:

- "List the open pull requests."
- "Summarize PR 124 and show the changed files."
- "Review the patch of `src/auth.rs` in PR 124."
- "Comment on line 42 of `src/auth.rs` in PR 124: please simplify this."
- "Approve PR 124."

Approving and requesting changes act on the real pull request. Zed asks you to confirm tool calls; try them on a test repository first.

## Tools

| Tool | Description |
| --- | --- |
| `list_pull_requests` | List open pull requests |
| `get_pull_request` | Description, branches, stats, mergeable state |
| `list_changed_files` | Changed files with status and +/- counts |
| `get_file_patch` | Unified diff of one file |
| `submit_review` | Comment, approve or request changes, with validated inline comments |

All tools accept an optional `repo` (`owner/name`); by default the repository of the working directory is used.

## How it works

```
Zed Agent panel  <-- MCP (stdio) -->  zpr-mcp  -->  gh api  -->  GitHub
```

- `crates/zpr-core`: GitHub logic (pagination, diff line mapping, review submission)
- `crates/zpr-mcp`: MCP server exposing the tools
- `extension/`: Zed extension that downloads and starts `zpr-mcp`

## Development

```sh
cargo test --workspace
cargo build --release -p zpr-mcp
cd extension && cargo build --target wasm32-wasip2
```

Releases: push a tag such as `v0.1.0`; the workflow attaches `zpr-mcp-<os>-<arch>` binaries.

## Roadmap

- Resolve review threads and mark files as viewed (GraphQL)
- CI check status
- Checkout and merge

## License

[Apache-2.0](LICENSE)
