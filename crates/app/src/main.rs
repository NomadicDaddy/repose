//! repose: the window. Flags: `--config <path>` overrides the config file location;
//! `--measure-startup [file]` writes the milliseconds from process start to the first frame that
//! shows repository rows into the file (default `repose-startup-ms.txt`), prints it, then exits.
//! `--measure-diff <file>` writes one tab-separated line, `diff-loaded`, the repository, the path,
//! the hunk count and the milliseconds since start, once the
//! open diff has loaded with at least one hunk, and keeps running, so the idle memory sample is
//! taken with a diff on screen.
//! `--select <repository path>` opens on that repository and
//! `--view changes|history|files|branches` on that pane, so a shortcut or a script can land where
//! the user wants; `--file <path>` opens that file's working-tree diff there, `--inline` in
//! inline mode; `--open <path>` opens the file read-only as HEAD holds it. `--scale <factor>`
//! multiplies the window's zoom on top of the desktop's own scaling (so 1.25 on a 100% desktop
//! renders as 125% would, and 0.8333 on a 150% desktop too), for eyesight and for the scaling
//! captures in the records. Without `--select`, a start from inside a repository selects that
//! repository when it is in the list.
//! `--help` (or `-h`) prints every flag to stderr and exits 0; a `--view` that names no pane is
//! refused with exit code 2, as a bad `--scale` is.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod diff_rows;
mod file_tree;
mod highlight;
mod platform;
mod screens;
mod state;
mod theme;

use std::path::{Path, PathBuf};
use std::time::Instant;

use eframe::egui;
use repose_core::config;

struct App {
	started: Instant,
	/// When set, the startup measurement is written to this file at the first frame with rows.
	measure: Option<PathBuf>,
	/// When set, a line is written here once the open diff has loaded; then it is cleared.
	diff_loaded: Option<PathBuf>,
	state: state::AppState,
	platform: Box<dyn platform::Platform>,
}

impl App {
	fn new(
		cc: &eframe::CreationContext<'_>,
		started: Instant,
		measure: Option<PathBuf>,
		cfg: config::Config,
	) -> Self {
		theme::apply(&cc.egui_ctx);
		let mut state = state::AppState::new(cfg);
		state.start_scan(&cc.egui_ctx);
		Self {
			started,
			measure,
			diff_loaded: None,
			state,
			platform: platform::current(),
		}
	}
}

impl eframe::App for App {
	fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
		let ctx = root.ctx().clone();
		self.state.pump(&ctx);
		self.state.tick(&ctx);
		// The bottom row goes first so it spans the window under both panes.
		screens::footer::show(root);
		screens::repo_list::show(root, &mut self.state, self.platform.as_ref());
		screens::detail::show(root, &mut self.state, self.platform.as_ref());
		if let Some(out) = &self.measure
			&& !self.state.repos.is_empty()
		{
			let line = format!(
				"first-frame-with-rows-ms {}\n",
				self.started.elapsed().as_millis()
			);
			let _ = std::fs::write(out, &line);
			print!("{line}");
			std::process::exit(0);
		}
		if let Some(out) = &self.diff_loaded
			&& let Some(d) = &self.state.diff
			&& let state::DiffContent::Files(files) = &d.content
		{
			let hunks: usize = files.iter().map(|f| f.hunks.len()).sum();
			if hunks > 0 {
				let line = format!(
					"diff-loaded\t{}\t{}\t{hunks}\t{}\n",
					d.open.repo.display(),
					d.open.display,
					self.started.elapsed().as_millis()
				);
				let _ = std::fs::write(out, line);
				self.diff_loaded = None;
			}
		}
		if self.state.scanning {
			ctx.request_repaint_after(std::time::Duration::from_millis(100));
		}
	}
}

/// The repository the process was started in: the nearest ancestor of the current directory
/// that holds a `.git` folder or file (a linked worktree has the file). The selection it asks
/// for lands only when discovery lists that path; otherwise the first row is selected as usual.
fn repo_at_cwd() -> Option<PathBuf> {
	let cwd = std::env::current_dir().ok()?;
	cwd.ancestors()
		.find(|dir| dir.join(".git").exists())
		.map(Path::to_path_buf)
}

/// A small window that shows a startup error and exits when closed, so a double-click never
/// fails silently.
fn error_window(message: String) -> eframe::Result {
	struct ErrorApp(String);
	impl eframe::App for ErrorApp {
		fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
			egui::CentralPanel::default().show(root, |ui| {
				ui.heading("repose could not start");
				ui.add_space(8.0);
				ui.label(&self.0);
				ui.add_space(12.0);
				if ui.button("Close").clicked() {
					ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
				}
			});
		}
	}
	let options = eframe::NativeOptions {
		viewport: egui::ViewportBuilder::default()
			.with_inner_size([560.0, 240.0])
			.with_title("repose"),
		..Default::default()
	};
	eframe::run_native(
		"repose-error",
		options,
		Box::new(move |cc| {
			theme::apply(&cc.egui_ctx);
			Ok(Box::new(ErrorApp(message)))
		}),
	)
}

fn main() -> eframe::Result {
	let started = Instant::now();
	let args: Vec<String> = std::env::args().collect();
	if args.iter().skip(1).any(|a| a == "--help" || a == "-h") {
		eprint!("{}", cli::HELP);
		std::process::exit(0);
	}
	let measure = args.iter().position(|a| a == "--measure-startup").map(|i| {
		args.get(i + 1)
			.map(PathBuf::from)
			.unwrap_or_else(|| PathBuf::from("repose-startup-ms.txt"))
	});
	let diff_loaded = args
		.iter()
		.position(|a| a == "--measure-diff")
		.and_then(|i| args.get(i + 1))
		.map(PathBuf::from);
	let config_path = args
		.iter()
		.position(|a| a == "--config")
		.and_then(|i| args.get(i + 1))
		.map(PathBuf::from)
		.unwrap_or_else(config::default_path);
	let select = args
		.iter()
		.position(|a| a == "--select")
		.and_then(|i| args.get(i + 1))
		// `.` and relative paths resolve against the current folder; the raw value is kept if
		// that fails.
		.map(|s| std::path::absolute(s).unwrap_or_else(|_| PathBuf::from(s)))
		.or_else(repo_at_cwd);
	// --view with no value after it is refused too, as an empty name, rather than opening Changes.
	let view_value = args
		.iter()
		.position(|a| a == "--view")
		.map(|i| args.get(i + 1).map_or("", String::as_str));
	let view = match cli::view(view_value) {
		Ok(view) => view,
		Err(message) => {
			eprintln!("repose: {message}");
			let _ = error_window(message);
			std::process::exit(2);
		}
	};
	let file = args
		.iter()
		.position(|a| a == "--file")
		.and_then(|i| args.get(i + 1))
		.cloned();
	let inline = args.iter().any(|a| a == "--inline");
	let open = args
		.iter()
		.position(|a| a == "--open")
		.and_then(|i| args.get(i + 1))
		.cloned();
	// A zoom factor multiplied into the desktop's own scaling. A value that is not a number
	// between 0.5 and 4 is refused out loud rather than run at normal zoom without a word
	// (Morticia Info on 4b39403).
	let scale = args.iter().position(|a| a == "--scale").map(|i| {
		args.get(i + 1)
			.and_then(|s| s.parse::<f32>().ok())
			.filter(|s| (0.5..=4.0).contains(s))
			.ok_or_else(|| args.get(i + 1).cloned().unwrap_or_default())
	});
	let scale = match scale {
		None => None,
		Some(Ok(s)) => Some(s),
		Some(Err(given)) => {
			let message = format!("--scale needs a number between 0.5 and 4, not {given:?}");
			eprintln!("repose: {message}");
			let _ = error_window(message);
			std::process::exit(2);
		}
	};
	let cfg = match config::load_or_create(&config_path) {
		Ok(c) => c,
		Err(e) => {
			// The release build has no console, so the message goes in a window; the process
			// still exits non-zero so a script or a terminal sees the failure.
			eprintln!("repose: {e}");
			let _ = error_window(format!("{e}\n\nConfig file: {}", config_path.display()));
			std::process::exit(2);
		}
	};
	let options = eframe::NativeOptions {
		viewport: egui::ViewportBuilder::default()
			.with_inner_size([1440.0, 900.0])
			.with_min_inner_size([960.0, 600.0])
			.with_title("repose"),
		..Default::default()
	};
	eframe::run_native(
		"repose",
		options,
		Box::new(move |cc| {
			if let Some(scale) = scale {
				cc.egui_ctx.set_zoom_factor(scale);
			}
			let mut app = App::new(cc, started, measure, cfg);
			app.diff_loaded = diff_loaded;
			app.state.view = view;
			app.state.requested_selection = select;
			app.state.requested_diff = file.map(|f| (f, inline));
			app.state.requested_file = open;
			Ok(Box::new(app))
		}),
	)
}
