/// Debug rendering differences between fonts
use clap::Parser;
use diffenator3::render::renderer::Renderer;
use diffenator3::render::utils::{count_differences, make_same_size};
use diffenator3::render::wordlists;
use image::{Pixel, Rgba, RgbaImage};

#[derive(Parser)]
struct Args {
    /// First font file
    font1: String,
    /// Second font file
    font2: String,
    /// Text to render
    text: String,
    /// Font size
    #[clap(short, long, default_value = "40.0")]
    size: f32,
    /// Script
    #[clap(short, long, default_value = "Latin")]
    script: String,
}

fn main() {
    let args = Args::parse();
    let data_a = std::fs::read(&args.font1).expect("Can't read font file");
    let dfont_a = diffenator3::dfont::DFont::new(&data_a);
    let data_b = std::fs::read(&args.font2).expect("Can't read font file");
    let dfont_b = diffenator3::dfont::DFont::new(&data_b);
    let direction = wordlists::get_script_direction(&args.script);
    let script_tag = wordlists::get_script_tag(&args.script);

    let mut renderer_a = Renderer::new(&dfont_a, args.size, direction, script_tag);
    let mut renderer_b = Renderer::new(&dfont_b, args.size, direction, script_tag);
    let (serialized_buffer_a, commands) =
        renderer_a.string_to_positioned_glyphs(&args.text).unwrap();
    let image_a = renderer_a.render_positioned_glyphs(&commands);
    println!("Buffer A: {}", serialized_buffer_a);

    let (serialized_buffer_b, commands) =
        renderer_b.string_to_positioned_glyphs(&args.text).unwrap();
    let image_b = renderer_b.render_positioned_glyphs(&commands);
    println!("Buffer B: {}", serialized_buffer_b);

    let (mut image_a, mut image_b) = make_same_size(image_a, image_b);
    image::imageops::flip_vertical_in_place(&mut image_a);
    image::imageops::flip_vertical_in_place(&mut image_b);

    image_a.save("image_a.png").expect("Can't save");
    image_b.save("image_b.png").expect("Can't save");

    // Make an overlay image
    let mut overlay = RgbaImage::new(image_a.width(), image_a.height());
    for (x, y, pixel) in overlay.enumerate_pixels_mut() {
        let pixel_a = image_a.get_pixel(x, y);
        let pixel_b = image_b.get_pixel(x, y);
        let mut a_green = Rgba([0, pixel_a.0[0], 0, 128]);
        let b_red = Rgba([pixel_b.0[0], 0, 0, 128]);
        a_green.blend(&b_red);
        *pixel = a_green;
    }
    overlay.save("overlay.png").expect("Can't save");

    let differing_pixels = count_differences(image_a, image_b);
    println!("Pixel differences: {:.2?}", differing_pixels);
    println!("See output images: image_a.png, image_b.png, overlay.png");
}