//! Renders an Elden Ring character's head (src/model.rs, the book's software FLVER renderer) for a Wizard101
//! dialogue portrait (round 18 G3; tools/w101_npc_portraits.py puts it on Wizard101's blue portrait background).
//! Usage: npc_portrait <decompressed parts folder> <out.png> <m|f> <hide bits a.b.c|-> <hair id|-> <hair r,g,b>
//!        <centre height,view height|-> <yaw> <size> [armour stems ...]
//! IDLE=<idle_pose.tsv> poses the arms down as in the book; FULL=<height> draws head to feet in a size x height
//! picture instead (the marker previews' stand-ins).
#![allow(dead_code)]

#[path = "../../../src/model.rs"]
mod model;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let hide = if a[4] == "-" {
        0
    } else if a[4] == "all" {
        u128::MAX
    } else {
        a[4].split('.').filter_map(|n| n.parse::<u32>().ok()).fold(0u128, |m, n| m | (1u128 << n))
    };
    let rgb: Vec<f32> = a[6].split(',').filter_map(|v| v.parse::<f32>().ok()).map(|v| v / 255.0).collect();
    let view = (a[7] != "-").then(|| {
        let v: Vec<f32> = a[7].split(',').filter_map(|v| v.parse().ok()).collect();
        [v[0], v[1]]
    });
    let size: u32 = a[9].parse().unwrap();
    let look = model::Look {
        male: a[3] == "m",
        armour: a[10..].to_vec(),
        hide,
        weapon: None,
        hair: a[5].parse().ok(),
        hair_colour: [rgb[0], rgb[1], rgb[2]],
        item: None,
    };
    let mut r = model::Renderer::new(a[1].clone().into());
    if let Some(p) = std::env::var_os("IDLE") {
        r = r.with_idle(p.into());
    }
    // FULL=<height px>: head to feet, `size` wide (to stand the character behind a marker preview).
    let (framing, wh) = match std::env::var("FULL").ok().and_then(|h| h.parse::<u32>().ok()) {
        Some(h) => (model::Framing::Full, [size, h]),
        None => (model::Framing::Portrait, [size, size]),
    };
    let request = model::Request { look, framing, yaw: a[8].parse().unwrap(), size: wh, fast: false, view };
    r.request(0, request);
    let frame = loop {
        if let Some(f) = r.take(0) {
            break f;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    image::RgbaImage::from_raw(frame.size[0], frame.size[1], frame.rgba).unwrap().save(&a[2]).unwrap();
}
