# GitHub PR Review for Zed

Review GitHub pull requests from Zed. Ask the Agent panel to list PRs, read diffs, leave inline comments, approve or request changes, or open a full visual review window with diffs, threads and branch comparison. Authentication is handled by the GitHub CLI, so there are no tokens to configure.

## Features

**In the Agent panel (chat)**
- **List and inspect** open pull requests: description, branches, stats, mergeable state.
- **Read diffs** file by file, straight from GitHub.
- **Review**: submit `COMMENT`, `APPROVE` or `REQUEST_CHANGES` reviews.
- **Safe inline comments**: every comment is validated against the PR diff before it is sent, so GitHub never rejects a bad line position.

**In the visual review window** (see [Visual review UI](#visual-review-ui))
- Create a pull request from your checked-out branch: target branch, title, description (from your PR template), draft, and a preview of the changes.
- Pull request list with search, description and CI checks.
- Unified or side-by-side diffs with line numbers, per file.
- Inline comments collected into a pending review, then submitted as Comment, Approve or Request changes.
- Existing review threads: reply, resolve and unresolve.
- Check off files as you review them ("Viewed" checkbox in the file list and in the file header, with an `N / M files viewed` counter). On a pull request it syncs with GitHub; in a branch comparison it is remembered in your browser.
- Compare any two branches or tags, on GitHub or in your local checkout (including unpushed branches).
- Open any file in Zed.

**General**
- **Zero credentials**: uses your existing `gh auth login` session.
- **Cross-platform**: Windows, macOS (Apple Silicon and Intel) and Linux.

## Requirements

- [GitHub CLI](https://cli.github.com/), signed in with `gh auth login`
- Zed with a model configured in the Agent panel

## Installation

Two steps, no Rust toolchain needed.

1. Install the [GitHub CLI](https://cli.github.com/) and run `gh auth login`.
2. Download the file for your platform from the [latest release](../../releases/latest) and run it once with `install`:

   | Platform | File | Command (in the folder where you downloaded it) |
   | --- | --- | --- |
   | Windows | `zpr-mcp-windows-x86_64.exe` | `.\zpr-mcp-windows-x86_64.exe install` |
   | macOS (Apple Silicon) | `zpr-mcp-macos-aarch64` | `chmod +x zpr-mcp-macos-aarch64 && xattr -d com.apple.quarantine zpr-mcp-macos-aarch64; ./zpr-mcp-macos-aarch64 install` |
   | macOS (Intel) | `zpr-mcp-macos-x86_64` | `chmod +x zpr-mcp-macos-x86_64 && xattr -d com.apple.quarantine zpr-mcp-macos-x86_64; ./zpr-mcp-macos-x86_64 install` |
   | Linux | `zpr-mcp-linux-x86_64` | `chmod +x zpr-mcp-linux-x86_64 && ./zpr-mcp-linux-x86_64 install` |

That's it. `install`:

- copies the program to a standard folder as `zpr` (`%LOCALAPPDATA%\zpr` on Windows, `~/.local/bin` on macOS and Linux);
- adds that folder to your `PATH`, so `zpr` works in any new terminal;
- registers the MCP server `github-pr-review` in your Zed `settings.json` (a backup is saved next to it as `settings.json.zpr-backup`; your comments and other settings are kept).

Then open a new terminal, and check in Zed (Agent panel, settings) that `github-pr-review` is enabled (green dot). You can delete the downloaded file.

**Updating:** download the new release file and run `install` again. It replaces the old program and keeps your settings.

**Manual setup** (only if `install` cannot edit your Zed settings): add this to `settings.json` (`zed: open settings`), using the full path of the program, with forward slashes on Windows:

```json
{
  "context_servers": {
    "github-pr-review": {
      "command": "C:/Users/you/AppData/Local/zpr/zpr.exe",
      "args": []
    }
  }
}
```

### Troubleshooting

- `zpr: command not found`: open a new terminal (the `PATH` only updates for new ones). On Windows, Git Bash and PowerShell both need a restart of the window.
- Server not green: run `zed: open log` and read the last errors.
- "gh failed" errors: run `gh auth status` in a terminal.
- "Could not detect the repository": open a folder that is a GitHub repository, or ask for it by name (`owner/name`).

## Usage

1. Open a GitHub repository as a project in Zed.
2. Open the Agent panel (`agent: toggle focus`) and pick a model that supports tools.
3. Ask in plain language. The Agent calls the PR tools for you; Zed may ask you to allow each tool call the first time.

Examples:

- "List the open pull requests."
- "Summarize PR 124 and show the changed files."
- "Review the patch of `src/auth.rs` in PR 124."
- "Comment on line 42 of `src/auth.rs` in PR 124: please simplify this."
- "Approve PR 124."
- "Open PR 124 in the review UI."

Approving and requesting changes act on the real pull request. Zed asks you to confirm tool calls; try them on a test repository first.

## Visual review UI

For a VS Code-like review experience, ask the Agent:

- "Open the review UI." The checked-out branch is detected: its open pull request opens, or, if it has none, the **Create pull request** form.
- "Open the review UI against `dev`." Same, but the pull request goes into `dev` instead of the default branch.
- "Open PR 124 in the review UI."
- "Compare `feature/login` with `main` in the review UI." (any two named branches, on GitHub)

A local page opens in your browser. You can also start it yourself, without the Agent:

```sh
zpr                       # checked-out branch: its PR, or the create form
zpr dev                   # same, with the pull request going into dev
zpr dev --repo owner/name # another repository
```

On the default branch, `zpr` opens the pull request list instead.

`zpr` is installed by the `install` step above. Run it from your project folder. Zed starts the same program with no arguments as its MCP server.

Creating a pull request (like in VS Code):

1. Check out your branch and run `zpr` (or ask the Agent to open the review UI).
2. Choose the target branch, edit the title and description, and tick Draft if needed. The title comes from your commit message (or branch name) and the description from your repository's pull request template (`.github/pull_request_template.md`), otherwise from your commit messages.
3. Review your own changes below the form, with the same diff view and checkboxes.
4. If the branch is not on GitHub yet, click "Push branch to origin" (it runs `git push --set-upstream origin <branch>`).
5. Click **Create pull request**. The window switches to the new pull request.

If the branch already has an open pull request into that target, it opens that pull request instead.

How to use it:

- **Comment on a line**: click its line number, write the comment, then "Add to review". Comments stay pending until you click Comment, Approve or Request changes at the top.
- **Threads**: reply or resolve directly under the line; resolved threads are collapsed.
- **Viewed**: tick the checkbox next to a file (or "Viewed" in its header) when you are done with it. Viewed files are dimmed and counted at the top of the file list.
- **Compare**: pick two branches (or type any tag or SHA) and click Compare. Tick "local checkout" for unpushed branches.
- **Open in Zed**: opens your local copy of the file. Run it from the project folder, with the branch checked out. It needs the `zed` command in your PATH (or set `ZPR_ZED_BIN` to its full path).

Security: the server listens on `127.0.0.1` only, on a random port, and every request needs a random token generated at startup. Nothing is exposed to your network.

Limits: the file viewed state and review threads show the first 100 files and 100 threads of a PR. Very large diffs can be truncated by GitHub. Comments cover single lines, not ranges.

## Tools

| Tool | Description |
| --- | --- |
| `list_pull_requests` | List open pull requests |
| `get_pull_request` | Description, branches, stats, mergeable state |
| `list_changed_files` | Changed files with status and +/- counts |
| `get_file_patch` | Unified diff of one file |
| `submit_review` | Comment, approve or request changes, with validated inline comments |
| `open_pr_ui` | Open the visual review UI: the checked-out branch (its PR, or the create form, optionally into `target`), a PR, or two named branches |

All tools accept an optional `repo` (`owner/name`); by default the repository of the working directory is used.

## How it works

```
Zed Agent panel  <-- MCP (stdio) -->  zpr-mcp  -->  gh api  -->  GitHub
                                         |
                                         +-- local web UI (127.0.0.1) <-- your browser
```

- `crates/zpr-core`: GitHub logic (pagination, GraphQL threads, diff line mapping, branch comparison, review submission)
- `crates/zpr-mcp`: MCP server, plus the local review UI (`assets/index.html`, no build step)
- `extension/`: experimental Zed extension (not required; the custom server setup above is the supported way)

## Development

```sh
cargo test --workspace
cargo build --release -p zpr-mcp
cd extension && cargo build --target wasm32-wasip2
```

Releases: push a tag such as `v0.1.0`; the workflow attaches `zpr-mcp-<os>-<arch>` binaries.

## Roadmap

- Syntax highlighting in diffs
- Multi-line comments and suggested changes
- Checkout and merge from the UI
- Native Zed panel (requires a Zed fork or an upstream UI API for extensions)

## License

[Apache-2.0](LICENSE)
