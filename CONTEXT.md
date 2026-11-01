# CONTEXT: repose

repose is a Rust desktop application that shows many Git repositories at once: a repository list
with branch and state, the selected repository's working-tree changes, its linear commit history,
read-only diffs and files, and staging and committing. It drives the installed `git` executable and
never contacts a remote. This file is the glossary; the blueprint lives in `.aidd/` (local only) and
in `docs/specs/2026-11-01-repose-v1/`.

## Glossary

**Root**
: A folder repose scans for repositories, such as `D:\applications`. Roots are listed in the
config file and scanned to a fixed depth. A root is not itself a repository unless it has a
`.git` entry.

**Repository**
: A folder with a `.git` directory or a `.git` file (a worktree or submodule pointer). A
repository nested inside another repository's tree is its own repository row.
_Avoid_: "repo" in user-facing text; the screens say "repository" and "repositories".

**Worktree**
: A checkout linked to a main repository through a `.git` file whose `gitdir:` points into the
main repository's `.git/worktrees/`. Worktrees found under any root, including
`D:\applications.worktrees`, are listed beneath their main repository, collapsed by default.

**Submodule**
: A nested checkout registered in a parent's `.gitmodules`. Submodules are not rows; the parent
shows a count.

**Saved refs**
: The remote-tracking refs already in `.git` (`refs/remotes/...`). Ahead and behind counts compare
the current branch with its upstream using these refs only. repose never fetches, so the status
line says "Saved refs; no fetch" and a count can be out of date until the user fetches elsewhere.

**State badges**
: The marks on a repository row: changed-file count, ahead `↑n`, behind `↓n`, detached HEAD,
conflicts, and an error badge when `git status` failed. A row with no badges is clean and in step
with its upstream as far as saved refs show.

**Changes**
: The selected repository's working tree, in two groups like VS Code: Staged Changes (the index
differs from HEAD) and Changes (the working tree differs from the index, plus untracked files).
Each file carries a letter badge: M modified, A added, D deleted, R renamed, U untracked, C
conflicted.

**History**
: The linear log of the current branch, newest first, with an Outgoing changes group at the top for
commits not yet on the upstream. Each commit expands to the files it changed. There are no drawn
branch lanes in the first release.

**Diff**
: A read-only comparison rendered from `git diff` output: working tree against index, index
against HEAD, or one commit against its first parent. It can be shown side by side or inline.

**Viewer**
: The read-only pane that shows a file at HEAD or at a commit, with syntax highlighting. Editing
happens in VS Code through the Open in VS Code action.

**Live refresh**
: The file-system watcher that re-reads a repository's status when its files or its `.git`
metadata change, debounced so a build writing thousands of files produces one refresh. The Refresh
button forces a full rescan.

**External action**
: A shortcut that leaves the app: Open folder in Explorer, Open in VS Code, Open a terminal here,
and Copy path, hash or remote URL (which also opens the remote page in the browser).

**Toolkit decision**
: The first piece of work. Two small prototypes, one in a native Rust GUI and one in Tauri, are
measured and screenshotted, and Phillip chooses. The decision is recorded in
`docs/specs/2026-11-01-repose-v1/decisions.md` before any screen is built.
