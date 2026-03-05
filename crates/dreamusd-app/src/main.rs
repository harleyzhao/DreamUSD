use dreamusd_ui::app::DreamUsdApp;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

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
        Box::new(|_cc| Ok(Box::new(DreamUsdApp::default()))),
    )
}
