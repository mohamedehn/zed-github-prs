# GitHub PR Review for Zed

Create and review GitHub pull requests from your terminal and from Zed, with a visual review window similar to the GitHub Pull Requests extension of VS Code: diffs, inline comments, review threads, "viewed" checkboxes, branch comparison. Authentication is handled by the GitHub CLI, so there are no tokens to configure.

Two ways to use it, both installed by the same program:

- **`zpr`**, a command you run in your project: it opens the review window in your browser.
- **The Agent panel in Zed**: ask the Agent to list, summarize, review or approve pull requests, or to open the review window.

## Installation

You need the [GitHub CLI](https://cli.github.com/), signed in with `gh auth login`. No Rust toolchain is needed.

1. Download the file for your platform from the [latest release](../../releases/latest).
2. Install it:

   - **Windows: double-click the downloaded `.exe`.** A window shows the result; press Enter to close it. If Windows SmartScreen warns about an unknown publisher (the file is not code-signed), click **More info**, then **Run anyway**.
   - **macOS and Linux:** raw binaries cannot be double-clicked, so run it once in a terminal, in the folder where you saved it:

   | Platform | Command |
   | --- | --- |
   | macOS, Apple Silicon | `chmod +x zpr-mcp-macos-aarch64 && xattr -d com.apple.quarantine zpr-mcp-macos-aarch64; ./zpr-mcp-macos-aarch64 install` |
   | macOS, Intel | `chmod +x zpr-mcp-macos-x86_64 && xattr -d com.apple.quarantine zpr-mcp-macos-x86_64; ./zpr-mcp-macos-x86_64 install` |
   | Linux | `chmod +x zpr-mcp-linux-x86_64 && ./zpr-mcp-linux-x86_64 install` |

`install` does three things, then you can delete the downloaded file:

- copies the program as `zpr` to a standard folder (`%LOCALAPPDATA%\zpr` on Windows, `~/.local/bin` on macOS and Linux);
- adds that folder to your `PATH`;
- registers the MCP server `github-pr-review` in your Zed `settings.json` (a backup is saved as `settings.json.zpr-backup`; your comments and other settings are kept).

**Close your terminal and open a new one** (the `PATH` only updates in new terminals). In Zed, open the Agent panel settings and check that `github-pr-review` is enabled (green dot).

On Windows you can also run `.\zpr-mcp-windows-x86_64.exe install` from a terminal.

**Updating:** download the new release file and install it the same way (double-click on Windows, `install` elsewhere).

## Quick start

In a terminal, inside a GitHub project, on the branch you are working on:

```sh
zpr          # your branch's pull request, or the "Create pull request" form
zpr dev      # same, but the pull request goes into the branch `dev`
```

A page opens in your browser:

- If your branch already has an open pull request, you land on it.
- If not, you get the **Create pull request** form.
- On the default branch (for example `main`), you get the list of open pull requests.

## Creating a pull request

1. Run `zpr` on your feature branch.
2. Pick the target branch, edit the title and the description, tick **Draft** if needed. The title comes from your commit message (or the branch name). The description comes from your repository's template (`.github/pull_request_template.md`) if there is one, otherwise from your commit messages.
3. Check your own changes below the form.
4. Click **Create pull request**. The window then shows the new pull request.

The **Create pull request** button is disabled, with the reason shown next to it, when:

- the branch is not on GitHub yet: click **Push branch to origin** (it runs `git push --set-upstream origin <branch>`);
- there are no commits between the target branch and yours: commit your changes first, GitHub refuses an empty pull request.

## Reviewing a pull request

- **Diffs**: side by side or unified, per file, with line numbers.
- **Comment on a line**: click its line number and write the comment. Comments stay pending until you click **Comment**, **Approve** or **Request changes** at the top. Each comment is checked against the diff first, so GitHub never rejects a bad position.
- **Threads**: reply or resolve under the line. Resolved threads are collapsed.
- **Viewed**: tick the checkbox next to a file (or **Viewed** in its header) when you are done. Viewed files are dimmed and counted (`3 / 12 files viewed`). On a pull request this syncs with GitHub.
- **Checks and description**: tabs next to **Files**.
- **Open in Zed**: opens your local copy of the file (run `zpr` from the project folder with the branch checked out; the `zed` command must be in your `PATH`, or set `ZPR_ZED_BIN` to its full path).

Approve and Request changes act on the real pull request and ask you to confirm.

## Comparing branches

Open **Compare branches** in the top bar, pick two branches, tags or commits, and click **Compare**. Tick **local checkout** to include commits you have not pushed. The checkboxes work there too (remembered in your browser).

## Using the Agent panel

Open a GitHub project in Zed, open the Agent panel and pick a model that supports tools. Ask in plain language; Zed may ask you to allow each tool call the first time.

- "List the open pull requests."
- "Summarize PR 124 and show the changed files."
- "Review the patch of `src/auth.rs` in PR 124."
- "Comment on line 42 of `src/auth.rs` in PR 124: please simplify this."
- "Approve PR 124."
- "Open the review UI." (same as `zpr`: your branch's pull request, or the create form)
- "Open the review UI against `dev`." (same as `zpr dev`)
- "Open PR 124 in the review UI."

| Tool | Description |
| --- | --- |
| `list_pull_requests` | List open pull requests |
| `get_pull_request` | Description, branches, stats, mergeable state |
| `list_changed_files` | Changed files with status and +/- counts |
| `get_file_patch` | Unified diff of one file |
| `submit_review` | Comment, approve or request changes, with validated inline comments |
| `open_pr_ui` | Open the review window: your branch (its PR or the create form, optionally into `target`), a PR, or two named branches |

All tools accept an optional `repo` (`owner/name`); by default the repository of the working directory is used.

## Troubleshooting

- `zpr: command not found`: open a new terminal.
- `zpr` shows nothing and waits: use `zpr --ui` instead.
- The server is not green in Zed: run `zed: open log` and read the last errors.
- "gh failed": run `gh auth status` in a terminal.
- "Could not detect the repository": run from a folder that is a GitHub repository, or add `--repo owner/name`.
- "branch not found locally or on origin": run `git fetch`.

**Manual Zed setup**, only if `install` could not edit your settings. Add this to `settings.json` (`zed: open settings`), with the full path of the program and forward slashes on Windows:

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

## Good to know

- **Security**: the review window is served on `127.0.0.1` only, on a random port, and every request needs a random token generated at startup. Nothing is exposed to your network.
- **Limits**: the viewed state and review threads cover the first 100 files and 100 threads of a pull request. Very large diffs can be truncated by GitHub. Comments are single-line (no ranges). Pull requests from forks cannot be created yet.
- The review window opens in your browser, not inside Zed. Zed extensions cannot add panels today.

## How it works

```
Zed Agent panel  <-- MCP (stdio) -->  zpr  -->  gh api  -->  GitHub
                                       |
                                       +-- local web page (127.0.0.1) <-- your browser
```

- `crates/zpr-core`: GitHub logic (pagination, GraphQL threads, diff mapping, branch comparison, pull request creation)
- `crates/zpr-mcp`: the program: MCP server, local web page (`assets/index.html`, no build step) and `install`
- `extension/`: experimental Zed extension (not required)

## Development

```sh
cargo test --workspace
cargo build --release -p zpr-mcp
```

Release: push a tag such as `v0.3.0`; the workflow attaches the `zpr-mcp-<os>-<arch>` binaries.

## Roadmap

- Syntax highlighting in diffs
- Multi-line comments and suggested changes
- Reviewers, labels and assignees when creating a pull request
- Checkout and merge from the UI
- Native Zed panel (requires a Zed fork or an upstream UI API for extensions)

## License

[Apache-2.0](LICENSE)
