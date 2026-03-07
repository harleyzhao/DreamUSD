use dreamusd_ui::app::DreamUsdApp;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();
    let initial_scene = std::env::args_os().nth(1).map(PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("DreamUSD")
            .with_inner_size([1280.0, 800.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "DreamUSD",
        options,
        Box::new(move |cc| Ok(Box::new(DreamUsdApp::new(cc, initial_scene.clone())))),
    )
}
