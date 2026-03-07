use dreamusd_core::{DisplayMode, HydraEngine, Stage};
use image::RgbaImage;
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let shadows_enabled = match args.next() {
        Some(flag) if flag == "--no-shadows" => false,
        Some(first) => {
            let scene_path = PathBuf::from(first);
            let output_path = PathBuf::from(
                args.next()
                    .ok_or("usage: render_once [--no-shadows] <scene.usd[a|c]> <output.png>")?,
            );
            return render(scene_path, output_path, true);
        }
        None => {
            return Err(
                "usage: render_once [--no-shadows] <scene.usd[a|c]> <output.png>".into(),
            );
        }
    };
    let scene_path = PathBuf::from(
        args.next()
            .ok_or("usage: render_once [--no-shadows] <scene.usd[a|c]> <output.png>")?,
    );
    let output_path = PathBuf::from(
        args.next()
            .ok_or("usage: render_once [--no-shadows] <scene.usd[a|c]> <output.png>")?,
    );

    render(scene_path, output_path, shadows_enabled)
}

fn render(
    scene_path: PathBuf,
    output_path: PathBuf,
    shadows_enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {

    let stage = Stage::open(&scene_path)?;
    let hydra = HydraEngine::create(&stage)?;
    hydra.set_display_mode(DisplayMode::SmoothShaded)?;
    hydra.set_enable_lighting(true)?;
    hydra.set_enable_shadows(shadows_enabled)?;
    hydra.set_msaa(true)?;
    hydra.set_camera([8.0, 6.0, 12.0], [0.0, 2.0, 0.0], [0.0, 1.0, 0.0])?;
    for _ in 0..3 {
        hydra.render(1280, 720)?;
    }

    let (rgba, width, height) = hydra.get_framebuffer()?;
    let image = RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or("failed to construct RGBA image from Hydra framebuffer")?;
    image.save(&output_path)?;

    println!(
        "rendered {} -> {} ({}x{})",
        scene_path.display(),
        output_path.display(),
        width,
        height
    );

    Ok(())
}
