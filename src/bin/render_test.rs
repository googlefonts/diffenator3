/// Debug rendering differences between fonts
use clap::Parser;
use diffenator3::render::renderer::Renderer;
use diffenator3::render::utils::{count_differences, make_same_size};
use diffenator3::render::{wordlists, DEFAULT_GRAY_FUZZ};
use diffenator3::setting::{parse_location, Setting};
use image::{Pixel, Rgba, RgbaImage};
use zeno::Command;

#[derive(Parser)]
struct Args {
    /// First font file
    font1: String,
    /// Second font file
    font2: String,

    /// Location in user space, in the form axis=123,other=456 (may be repeated)
    #[clap(long = "location")]
    location: Option<String>,

    /// Text to render
    text: String,
    /// Font size
    #[clap(short, long, default_value = "64.0")]
    size: f32,
    /// Script
    #[clap(short, long, default_value = "Latin")]
    script: String,

    /// Verbose debugging
    #[clap(short, long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();
    let data_a = std::fs::read(&args.font1).expect("Can't read font file");
    let mut dfont_a = diffenator3::dfont::DFont::new(&data_a);
    let data_b = std::fs::read(&args.font2).expect("Can't read font file");
    let mut dfont_b = diffenator3::dfont::DFont::new(&data_b);

    if let Some(location) = args.location {
        let loc = parse_location(&location).expect("Couldn't parse location");
        Setting::from_setting(loc)
            .set_on_fonts(&mut dfont_a, &mut dfont_b)
            .expect("Couldn't set location");
    }

    let direction = wordlists::get_script_direction(&args.script);
    let script_tag = wordlists::get_script_tag(&args.script);

    let mut renderer_a = Renderer::new(&dfont_a, args.size, direction, script_tag);
    let mut renderer_b = Renderer::new(&dfont_b, args.size, direction, script_tag);
    let (serialized_buffer_a, commands) =
        renderer_a.string_to_positioned_glyphs(&args.text).unwrap();
    let image_a = renderer_a.render_positioned_glyphs(&commands);
    if args.verbose {
        println!("Commands A: {}", to_svg(commands));
    }
    println!("Buffer A: {}", serialized_buffer_a);

    let (serialized_buffer_b, commands) =
        renderer_b.string_to_positioned_glyphs(&args.text).unwrap();
    let image_b = renderer_b.render_positioned_glyphs(&commands);
    if args.verbose {
        println!("Commands B: {}", to_svg(commands));
    }

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

    let differing_pixels = count_differences(image_a, image_b, DEFAULT_GRAY_FUZZ);
    println!("Pixel differences: {:.2?}", differing_pixels);
    println!("See output images: image_a.png, image_b.png, overlay.png");
}

fn to_svg(commands: Vec<Command>) -> String {
    let mut svg = String::new();
    for command in commands {
        match command {
            Command::MoveTo(p) => {
                svg.push_str(&format!("M {} {} ", p.x, p.y));
            }
            Command::LineTo(p) => {
                svg.push_str(&format!("L {} {} ", p.x, p.y));
            }
            Command::QuadTo(p1, p2) => {
                svg.push_str(&format!("Q {} {} {} {} ", p1.x, p1.y, p2.x, p2.y));
            }
            Command::CurveTo(p1, p2, p3) => {
                svg.push_str(&format!(
                    "C {} {} {} {} {} {} ",
                    p1.x, p1.y, p2.x, p2.y, p3.x, p3.y
                ));
            }
            Command::Close => {
                svg.push_str("Z  ");
            }
        }
    }
    svg
}
