//! Renders the spellbook's character model (src/model.rs) to PNGs from a folder of decompressed parts files.
//! Usage: model_preview <parts folder> <out folder> [armour stems ...]
#![allow(dead_code)]

#[path = "../../../src/model.rs"]
mod model;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(&args[2]);
    std::fs::create_dir_all(&out).unwrap();
    let armour: Vec<String> = args[3..].iter().filter(|a| !a.starts_with("hide=")).cloned().collect();
    let hide = args[3..].iter().find_map(|a| a.strip_prefix("hide=")).map_or(0u128, |h| {
        h.split('.').filter_map(|n| n.parse::<u32>().ok()).fold(0, |m, n| m | (1u128 << n))
    });
    let mut r = model::Renderer::new(args[1].clone().into());
    let look = model::Look { male: true, armour, hide, weapon: Some(200), hair: Some(1), hair_colour: [0.25, 0.17, 0.1] };
    let jobs: [(&str, model::Framing, f32, [u32; 2]); 4] = [("full_0", model::Framing::Full, 0.0f32, [495u32, 727u32]), ("full_45", model::Framing::Full, 0.8, [495, 727]),
                ("full_180", model::Framing::Full, 3.14, [495, 727]), ("portrait", model::Framing::Portrait, 0.25, [240, 240])];
    for (name, framing, yaw, size) in jobs {
        r.request(0, model::Request { look: look.clone(), framing, yaw, size, fast: false });
        let t = std::time::Instant::now();
        let frame = loop {
            if let Some(f) = r.take(0) { break f; }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        println!("{name}: {} ms", t.elapsed().as_millis());
        image::RgbaImage::from_raw(frame.size[0], frame.size[1], frame.rgba).unwrap().save(out.join(format!("{name}.png"))).unwrap();
    }
}
