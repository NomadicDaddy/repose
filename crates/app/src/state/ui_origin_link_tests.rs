//! header-origin-link: the origin line in the repository header opens the page Open remote page
//! opens, from a click or from Enter after Tab reaches it, with the pointer cursor and the page
//! as its tooltip on hover. Other remotes, and an origin with no page, stay plain text.

use std::path::PathBuf;
use std::time::SystemTime;

use eframe::egui::{self, CursorIcon, Event, Key, OutputCommand, Pos2, Rect};
use repose_core::config::Config;
use repose_core::git::remote::RemoteEntry;

use super::AppState;
use super::ui_harness::{Headless, SCREEN, click, key, repo, tab, text_at, texts};
use crate::screens::{detail, repo_list};

const PAGE: &str = "https://example.invalid/team/thing";
const ORIGIN: &str = "origin · https://example.invalid/team/thing.git";
const MIRROR: &str = "mirror · https://mirror.example.invalid/team/thing.git";

fn with_remotes(origin_url: &str) -> (AppState, PathBuf) {
	let mut state = AppState::new(Config::default());
	let row = repo("origin-link", SystemTime::now());
	let path = row.path.clone();
	state.requested_selection = Some(path.clone());
	assert!(state.merge_discovered(vec![row]), "the row is selected");
	state.remotes.insert(
		path.clone(),
		Ok(vec![
			RemoteEntry {
				name: "origin".into(),
				url: origin_url.into(),
				push_url: None,
			},
			RemoteEntry {
				name: "mirror".into(),
				url: "https://mirror.example.invalid/team/thing.git".into(),
				push_url: None,
			},
		]),
	);
	(state, path)
}

/// One frame at `time`, returning what it painted and what it asked the platform for.
fn run(
	ctx: &egui::Context,
	state: &mut AppState,
	events: Vec<Event>,
	time: f64,
) -> (Vec<egui::epaint::ClippedShape>, egui::PlatformOutput) {
	let input = egui::RawInput {
		screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
		time: Some(time),
		events,
		..Default::default()
	};
	let mut out = ctx.run_ui(input, |root| {
		repo_list::show(root, state, &Headless);
		detail::show(root, state, &Headless);
	});
	out.textures_delta.clear();
	(out.shapes, out.platform_output)
}

fn opened(output: &egui::PlatformOutput) -> Vec<String> {
	output
		.commands
		.iter()
		.filter_map(|c| match c {
			OutputCommand::OpenUrl(open) => Some(open.url.clone()),
			_ => None,
		})
		.collect()
}

#[test]
fn clicking_the_origin_line_opens_its_page_with_a_pointer_and_a_tooltip() {
	let (mut state, _) = with_remotes("https://example.invalid/team/thing.git");
	let ctx = egui::Context::default();
	let (shapes, _) = run(&ctx, &mut state, Vec::new(), 0.0);
	assert!(
		texts(&shapes).iter().any(|t| t == ORIGIN),
		"the displayed text is kept"
	);
	let at =
		text_at(&shapes, ORIGIN).unwrap_or_else(|| panic!("origin line: {:?}", texts(&shapes)));
	let mut tip = false;
	let mut pointer = false;
	// One move onto the line, then frames with the pointer resting there while time passes, so
	// egui's tooltip delay runs out.
	for i in 1..=20 {
		let events = if i == 1 {
			vec![Event::PointerMoved(at)]
		} else {
			Vec::new()
		};
		let (shapes, out) = run(&ctx, &mut state, events, f64::from(i) * 0.1);
		pointer |= out.cursor_icon == CursorIcon::PointingHand;
		tip |= text_at(&shapes, &format!("Open {PAGE}")).is_some();
	}
	assert!(pointer, "the pointer cursor shows over the link");
	assert!(tip, "the tooltip names the page that will open");
	let (_, out) = run(&ctx, &mut state, click(at), 2.5);
	assert_eq!(
		opened(&out),
		[PAGE],
		"a click opens origin's page, as Open remote page does"
	);
	let (shapes, _) = run(&ctx, &mut state, Vec::new(), 3.0);
	let at =
		text_at(&shapes, MIRROR).unwrap_or_else(|| panic!("mirror line: {:?}", texts(&shapes)));
	let (_, out) = run(&ctx, &mut state, click(at), 3.5);
	assert!(opened(&out).is_empty(), "only origin is a link");
}

#[test]
fn an_origin_with_no_page_stays_plain_text() {
	for (url, line) in [
		(r"D:\repos\thing", r"origin · D:\repos\thing"),
		("http://localhost?who=a@b", "origin · http://localhost"),
	] {
		let (mut state, _) = with_remotes(url);
		let ctx = egui::Context::default();
		let (shapes, _) = run(&ctx, &mut state, Vec::new(), 0.0);
		let at =
			text_at(&shapes, line).unwrap_or_else(|| panic!("{line:?} in {:?}", texts(&shapes)));
		let (_, out) = run(&ctx, &mut state, click(at), 0.5);
		assert!(
			opened(&out).is_empty(),
			"{url} has no page and no click action"
		);
		assert_ne!(
			out.cursor_icon,
			CursorIcon::PointingHand,
			"{url}: no link cursor"
		);
	}
}

/// Tab walks the header controls in order; once the origin link holds focus, Enter opens it.
#[test]
fn tab_reaches_the_origin_link_and_enter_opens_it() {
	let (mut state, _) = with_remotes("https://example.invalid/team/thing.git");
	let ctx = egui::Context::default();
	run(&ctx, &mut state, Vec::new(), 0.0);
	let mut time = 0.0;
	let mut focused = false;
	for _ in 0..12 {
		time += 0.1;
		let (_, out) = run(&ctx, &mut state, tab(), time);
		focused = out.events.iter().any(|e| {
			matches!(e, egui::output::OutputEvent::FocusGained(w)
				if w.role == egui::Role::Link && w.label.as_deref() == Some(ORIGIN))
		});
		if focused {
			break;
		}
	}
	assert!(focused, "Tab did not reach the origin link in 12 presses");
	let (_, out) = run(&ctx, &mut state, key(Key::Enter), time + 0.1);
	assert_eq!(
		opened(&out),
		[PAGE],
		"Enter on the focused link opens origin's page"
	);
}

/// header-remote-url-wraps-at-separator: an origin URL far wider than the header wraps only after
/// a "/", never inside a name, and stays one link: a click on its second line opens its page.
#[test]
fn a_long_origin_wraps_at_separators_and_stays_one_link() {
	use eframe::egui::epaint::Shape;

	let segments = "a-rather-long-folder-name/".repeat(12);
	let url = format!("https://example.invalid/{segments}thing.git");
	let page = format!("https://example.invalid/{segments}thing");
	let (mut state, _) = with_remotes(&url);
	let ctx = egui::Context::default();
	let (shapes, _) = run(&ctx, &mut state, Vec::new(), 0.0);
	fn origin(shape: &Shape) -> Option<(Pos2, std::sync::Arc<egui::Galley>)> {
		match shape {
			Shape::Vec(inner) => inner.iter().find_map(origin),
			Shape::Text(t) if t.galley.text().starts_with("origin · https://") => {
				Some((t.pos, t.galley.clone()))
			}
			_ => None,
		}
	}
	let Some((pos, galley)) = shapes.iter().find_map(|c| origin(&c.shape)) else {
		panic!("no origin line in {:?}", texts(&shapes));
	};
	let lines: Vec<&str> = galley.text().split('\n').collect();
	assert!(lines.len() > 1, "a URL this long wraps: {lines:?}");
	for line in &lines[..lines.len() - 1] {
		assert!(
			line.ends_with('/'),
			"a line breaks only after a separator: {line:?}"
		);
	}
	assert_eq!(
		galley.text().replace('\n', ""),
		format!("origin · {url}"),
		"nothing is lost or added"
	);
	assert_eq!(
		galley.rows.len(),
		lines.len(),
		"egui adds no wrap of its own"
	);
	let second = &galley.rows[1];
	let at = pos + second.pos.to_vec2() + egui::vec2(8.0, second.row.size.y / 2.0);
	let (_, out) = run(&ctx, &mut state, click(at), 0.5);
	assert_eq!(opened(&out), [page], "the second line is the same link");
}
