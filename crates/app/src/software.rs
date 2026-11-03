//! CPU renderer for machines without a GPU (virtual machines, Windows Server).
//! egui_software_backend rasterizes on the CPU and presents through
//! softbuffer (GDI on Windows), which there is many times faster than
//! Direct3D on the WARP software rasterizer. Dialogs open inside the main
//! window, file dialogs are not parented to it.

use egui_software_backend::{SoftwareBackend, SoftwareBackendAppConfiguration};
use storage::AppPaths;

use crate::app::App;
use crate::screenshot::CliOptions;

struct SoftwareApp {
    app: App,
}

impl egui_software_backend::App for SoftwareApp {
    fn update(&mut self, ctx: &egui::Context, backend: &mut SoftwareBackend) {
        let cpu_usage = if self.app.cli.bench {
            backend.set_capture_frame_time(true);
            backend.last_frame_time().map(|d| d.as_secs_f32())
        } else {
            None
        };
        self.app.run_frame(ctx, None, cpu_usage);
    }
}

impl Drop for SoftwareApp {
    // The backend has no exit hook; the app is dropped when the window closes.
    fn drop(&mut self) {
        self.app.shutdown();
    }
}

pub fn run(viewport: egui::ViewportBuilder, rt: tokio::runtime::Runtime, paths: AppPaths, cli: CliOptions) -> Result<(), String> {
    let settings = SoftwareBackendAppConfiguration::new().viewport_builder(viewport);
    let mut state = Some((rt, paths, cli));
    let result = egui_software_backend::run_app_with_software_backend(settings, |ctx| {
        // Called once, when the window exists.
        let (rt, paths, cli) = state.take().unwrap_or_else(|| panic!("software renderer created the app twice"));
        ctx.set_embed_viewports(true);
        SoftwareApp { app: App::new(&ctx, rt, paths, cli) }
    });
    result.map_err(|e| e.to_string())
}
