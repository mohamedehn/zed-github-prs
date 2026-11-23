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

The simplest setup is a custom MCP server: no Rust toolchain needed.

### Quick start (recommended)

1. Install the [GitHub CLI](https://cli.github.com/) and run `gh auth login`.
2. Download the binary for your platform from the [latest release](../../releases/latest):

   | Platform | File |
   | --- | --- |
   | Windows | `zpr-mcp-windows-x86_64.exe` |
   | macOS (Apple Silicon) | `zpr-mcp-macos-aarch64` |
   | macOS (Intel) | `zpr-mcp-macos-x86_64` |
   | Linux | `zpr-mcp-linux-x86_64` |

3. macOS and Linux only: make it executable, for example `chmod +x ~/Downloads/zpr-mcp-macos-aarch64`. On macOS you may also need `xattr -d com.apple.quarantine <file>`.
4. In Zed, open the command palette, run `zed: open settings`, and add (use the full path to the file you downloaded; on Windows use forward slashes):

```json
{
  "context_servers": {
    "github-pr-review": {
      "command": "C:/Users/you/Downloads/zpr-mcp-windows-x86_64.exe",
      "args": []
    }
  }
}
```

5. Open the Agent panel, open its settings and check that `github-pr-review` is enabled (green dot).
6. Test it: open a GitHub project and ask "List the open pull requests."

### As a Zed dev extension

Requires Rust installed through [rustup](https://rustup.rs/). Clone this repository, run `zed: install dev extension` and select the `extension/` folder. The extension downloads the matching binary from the latest release. In the setup dialog, click **Configure Server**.

### Troubleshooting

- Server not green: run `zed: open log` and read the last errors.
- "gh failed" errors: run `gh auth status` in a terminal.
- "Could not detect the repository": open a folder that is a GitHub repository, or ask for it by name (`owner/name`).

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
