//! What the three verbs that reach a remote run with: Fetch, Pull and Push, each only from its
//! own user action (A1). None of them can prompt, open a sign-in window or start a program from a
//! repository's config, and none takes an argument from a caller.

use std::process::Command;

use super::commands::Verb;

/// The verbs that reach a remote. Nothing else in `Verb` may.
pub const NETWORK: [Verb; 3] = [Verb::Fetch, Verb::Pull, Verb::Push];

pub fn is_network(verb: Verb) -> bool {
	NETWORK.contains(&verb)
}

/// GIT_TERMINAL_PROMPT=0 (on every spawn) stops git's own prompt but not Git Credential Manager,
/// which can still open a sign-in window or a browser; GCM_INTERACTIVE and credential.interactive
/// keep it non-interactive. No askpass program and no ext:: transport either (Morticia L-2 on
/// c84df39), and OpenSSH may not ask through one even when it has no terminal
/// (SSH_ASKPASS_REQUIRE=never, Roger M-1). With no console and a null stdin, ssh has no terminal
/// to prompt on, so a passphrase or host-key question fails instead of waiting. core.sshCommand
/// and credential helpers are the user's own, as hooks are for a commit.
pub(super) fn non_interactive(cmd: &mut Command, verb: Verb) {
	if !is_network(verb) {
		return;
	}
	cmd.env("GCM_INTERACTIVE", "never")
		.env("SSH_ASKPASS_REQUIRE", "never")
		.env_remove("GIT_ASKPASS")
		.env_remove("SSH_ASKPASS")
		.args(["-c", "credential.interactive=false"])
		.args(["-c", "protocol.ext.allow=never"])
		.args(["-c", "core.askPass="]);
	match verb {
		// git pull has no --no-auto-maintenance, so the same two settings come as config. A
		// submodule is never updated by the merge's checkout either.
		Verb::Pull => {
			cmd.args(["-c", "maintenance.auto=false"])
				.args(["-c", "gc.auto=0"])
				.args(["-c", "submodule.recurse=false"]);
		}
		// The checked-out branch to the upstream it tracks and nothing else, whatever
		// push.default says; push.followTags cannot add tags (--no-follow-tags in its args).
		Verb::Push => {
			cmd.args(["-c", "push.default=upstream"]);
		}
		_ => {}
	}
}
