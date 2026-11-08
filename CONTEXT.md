# CONTEXT: repose

repose is a Rust desktop application that shows many Git repositories at once: a repository list
with branch and state, the selected repository's working-tree changes, its linear commit history,
read-only diffs and files, and staging and committing. It drives the installed `git` executable. Its one
Git network operation is Fetch, and only when the user clicks it; otherwise the only thing it sends
outward is a remote URL handed to the browser when the user asks. This file is the glossary; the blueprint lives in `.aidd/` (local only) and
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
the current branch with its upstream using these refs only. repose fetches only when the user
clicks Fetch, so the status line says "Saved refs; no fetch" until then, and a count can be out of
date until someone fetches. After a Fetch, that repository's line says "Fetched HH:MM".

**State badges**
: The marks on a repository row: changed-file count, ahead `↑n`, behind `↓n`, detached HEAD,
conflicts, and an error badge when `git status` failed. A row with no badges is clean and in step
with its upstream as far as saved refs show.

**Changes**
: The selected repository's working tree, in groups like VS Code: Merge Changes (conflicted files,
shown above the others only while there are any), Staged Changes (the index differs from HEAD) and
Changes (the working tree differs from the index, plus untracked files). Each file carries a letter
badge: M modified, A added, D deleted, R renamed, T type changed, U untracked, C conflicted. A file
staged with `git add -N` (intent to add) shows A.

**History**
: The linear log of the current branch, newest first, with an Outgoing changes group at the top for
commits not yet on the upstream. Each commit expands to the files it changed. With **Lanes** on,
History instead lists every branch and tag in topological order and draws each line of history as
a lane, with connectors where branches fork and merge.

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
: The first piece of work, now resolved: two small prototypes, one in a native Rust GUI and one in
Tauri, were measured and screenshotted, and Phillip chose eframe/egui. ADR-001 in
`docs/specs/2026-11-01-repose-v1/decisions.md` records it.
