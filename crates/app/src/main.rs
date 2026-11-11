//! repose: the window. Flags: `--config <path>` overrides the config file location;
//! `--measure-startup [file]` writes the milliseconds from process start to the first frame that
//! shows repository rows into the file (default `repose-startup-ms.txt`), prints it, then exits.
//! `--measure-diff <file>` writes one tab-separated line, `diff-loaded`, the repository, the path,
//! the hunk count and the milliseconds since start, once the
//! open diff has loaded with at least one hunk, and keeps running, so the idle memory sample is
//! taken with a diff on screen.
//! `--measure-lanes <file>` turns lanes on, scrolls History through 100 rows and writes the
//! frame times (measure_lanes.rs), then exits.
//! `--select <repository path>` opens on that repository and
//! `--view changes|history|files|branches` on that pane, so a shortcut or a script can land where
//! the user wants; `--file <path>` opens that file's working-tree diff there, `--inline` in
//! inline mode; `--open <path>` opens the file read-only as HEAD holds it. `--scale <factor>`
//! multiplies the window's zoom on top of the desktop's own scaling (so 1.25 on a 100% desktop
//! renders as 125% would, and 0.8333 on a 150% desktop too), for eyesight and for the scaling
//! captures in the records. Without `--select`, a start from inside a repository selects that
//! repository when it is in the list.
//! `--help` (or `-h`) prints every flag to stderr and exits 0; a `--view` that names no pane is
//! refused with exit code 2, as a bad `--scale` is. The flags are read in cli.rs, from the same
//! table `--help` prints.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod diff_rows;
mod file_tree;
mod highlight;
mod lanes;
mod measure_lanes;
mod platform;
mod screens;
mod state;
mod syntax_palette;
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
	/// Whether Windows' animation setting has been asked for yet; see `ui`.
	motion_asked: bool,
	/// `--measure-lanes`: the frame-time probe, while it runs.
	lanes_probe: Option<measure_lanes::LanesProbe>,
}

impl App {
	fn new(
		cc: &eframe::CreationContext<'_>,
		started: Instant,
		measure: Option<PathBuf>,
		cfg: config::Config,
	) -> Self {
		theme::apply(&cc.egui_ctx, cfg.theme);
		let mut state = state::AppState::new(cfg);
		state.start_scan(&cc.egui_ctx);
		Self {
			started,
			measure,
			diff_loaded: None,
			state,
			platform: platform::current(),
			motion_asked: false,
			lanes_probe: None,
		}
	}
}

impl eframe::App for App {
	fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
		if let Some(probe) = &mut self.lanes_probe {
			probe.inject(raw);
		}
	}

	fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
		let ctx = root.ctx().clone();
		theme::begin_frame(&ctx);
		if let Some(probe) = &mut self.lanes_probe {
			probe.begin();
		}
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
		// Windows' animation setting is asked for only once the first rows are drawn, or the first
		// scan found none, so its pwsh never competes with startup: started from App::new it took
		// the a3b9a3a gate's startup from about 750 ms to 1074 ms.
		if !self.motion_asked && (!self.state.repos.is_empty() || !self.state.scanning) {
			self.motion_asked = true;
			theme::follow_reduced_motion(&ctx);
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
		if let Some(probe) = &mut self.lanes_probe
			&& let Some(report) = probe.end(&self.state, &ctx)
		{
			print!("{report}");
			// The file is the evidence: a failed write exits non-zero, as an error run does.
			let written = std::fs::write(probe.out(), &report);
			if let Err(e) = &written {
				eprintln!("repose: could not write {}: {e}", probe.out().display());
			}
			std::process::exit(i32::from(written.is_err() || report.starts_with("error")));
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
		viewport: with_icon(
			egui::ViewportBuilder::default()
				.with_inner_size([560.0, 240.0])
				.with_title("repose"),
		),
		..Default::default()
	};
	eframe::run_native(
		"repose-error",
		options,
		Box::new(move |cc| {
			// The config may be what failed, so the error window follows the Windows theme.
			theme::apply(&cc.egui_ctx, config::Theme::System);
			Ok(Box::new(ErrorApp(message)))
		}),
	)
}

/// The program icon at 256 px (program-icon), from assets/icon/make-icon.sh.
const ICON_PNG: &[u8] = include_bytes!("../../../assets/icon/repose-256.png");

/// The window's own icon, for its title bar (program-icon): the mark the executable also carries,
/// decoded with eframe's PNG loader. A PNG that will not decode leaves the default icon rather
/// than stopping repose from starting.
fn with_icon(viewport: egui::ViewportBuilder) -> egui::ViewportBuilder {
	match eframe::icon_data::from_png_bytes(ICON_PNG) {
		Ok(icon) => viewport.with_icon(icon),
		Err(_) => viewport,
	}
}

fn main() -> eframe::Result {
	let started = Instant::now();
	let args = match cli::parse(&std::env::args().collect::<Vec<_>>()) {
		cli::Parsed::Help => {
			eprint!("{}", cli::help());
			std::process::exit(0);
		}
		cli::Parsed::Refused(message) => {
			eprintln!("repose: {message}");
			let _ = error_window(message);
			std::process::exit(2);
		}
		cli::Parsed::Run(args) => args,
	};
	let config_path = args.config.unwrap_or_else(config::default_path);
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
		viewport: with_icon(
			egui::ViewportBuilder::default()
				.with_inner_size([1440.0, 900.0])
				.with_min_inner_size([960.0, 600.0])
				.with_title("repose"),
		),
		..Default::default()
	};
	eframe::run_native(
		"repose",
		options,
		Box::new(move |cc| {
			if let Some(scale) = args.scale {
				cc.egui_ctx.set_zoom_factor(scale);
			}
			let mut app = App::new(cc, started, args.measure_startup, cfg);
			app.diff_loaded = args.measure_diff;
			// `--lanes` starts the session with lanes on, as the History toggle would.
			app.state.lanes = args.lanes;
			// Pins are saved to the config file this session loaded, `--config` included.
			app.state.config_path = Some(config_path);
			if let Some(out) = args.measure_lanes {
				app.state.lanes = true;
				app.lanes_probe = Some(measure_lanes::LanesProbe::new(out, args.select.clone()));
			}
			app.state.view = args.view;
			app.state.start_on(args.select.or_else(repo_at_cwd));
			app.state.requested_diff = args.file.map(|f| (f, args.inline));
			app.state.requested_file = args.open;
			Ok(Box::new(app))
		}),
	)
}

#[cfg(test)]
mod icon_tests {
	/// program-icon clause 3: the window icon decodes, at the 256 px the executable's largest frame
	/// has, so with_icon never falls back to the default without a test failing.
	#[test]
	fn the_window_icon_decodes() {
		let icon =
			eframe::icon_data::from_png_bytes(super::ICON_PNG).unwrap_or_else(|e| panic!("{e}"));
		assert_eq!((icon.width, icon.height), (256, 256));
	}
}
