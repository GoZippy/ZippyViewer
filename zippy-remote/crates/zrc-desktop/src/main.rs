use zrc_desktop::ZrcDesktopApp;

fn main() -> eframe::Result<()> {
    // Init tracing
    tracing_subscriber::fmt::init();
    
    // Create runtime for background tasks
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    let handle = runtime.handle().clone();

    // Define native options
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "Zippy Remote Control",
        native_options,
        Box::new(|cc| Box::new(ZrcDesktopApp::new(cc, handle))),
    )
}
