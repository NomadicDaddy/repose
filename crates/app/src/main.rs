//! repose: the window. Flags: `--config <path>` overrides the config file location;
//! `--measure-startup [file]` writes the milliseconds from process start to the first frame that
//! shows repository rows into the file (default `repose-startup-ms.txt`), prints it, then exits.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod platform;
mod screens;
mod state;
mod theme;

use std::path::PathBuf;
use std::time::Instant;

use eframe::egui;
use repose_core::config;

struct App {
	started: Instant,
	/// When set, the startup measurement is written to this file at the first frame with rows.
	measure: Option<PathBuf>,
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
			state,
			platform: platform::current(),
		}
	}
}

impl eframe::App for App {
	fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
		let ctx = root.ctx().clone();
		self.state.pump(&ctx);
		screens::repo_list::show(root, &mut self.state, self.platform.as_ref());
		screens::detail::show(root, &self.state);
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
		if self.state.scanning {
			ctx.request_repaint_after(std::time::Duration::from_millis(100));
		}
	}
}

fn main() -> eframe::Result {
	let started = Instant::now();
	let args: Vec<String> = std::env::args().collect();
	let measure = args.iter().position(|a| a == "--measure-startup").map(|i| {
		args.get(i + 1)
			.map(PathBuf::from)
			.unwrap_or_else(|| PathBuf::from("repose-startup-ms.txt"))
	});
	let config_path = args
		.iter()
		.position(|a| a == "--config")
		.and_then(|i| args.get(i + 1))
		.map(PathBuf::from)
		.unwrap_or_else(config::default_path);
	let cfg = match config::load_or_create(&config_path) {
		Ok(c) => c,
		Err(e) => {
			eprintln!("repose: {e}");
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
		Box::new(move |cc| Ok(Box::new(App::new(cc, started, measure, cfg)))),
	)
}
