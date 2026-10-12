// Print what bern-image makes of a picture: cargo run -p bern-image --example probe -- <file> [headroom]
fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("a file");
    let headroom = args.next().map(|h| h.parse().expect("a number"));
    let data = std::fs::read(&path).expect("readable");
    println!("kind: {:?}", bern_image::kind(&data));
    let started = std::time::Instant::now();
    match bern_image::decode(&data, headroom, bern_image::Primaries::Bt709) {
        Ok(Some(p)) => {
            let at = |x: u32, y: u32| {
                let i = ((y * p.width + x) * 4) as usize;
                format!(
                    "{:.3} {:.3} {:.3}",
                    p.pixels[i],
                    p.pixels[i + 1],
                    p.pixels[i + 2]
                )
            };
            println!(
                "{}x{} peak {:.3} in {:?}",
                p.width,
                p.height,
                p.peak,
                started.elapsed()
            );
            println!("top left quarter: {}", at(p.width / 4, p.height / 3));
            println!("top right quarter: {}", at(3 * p.width / 4, p.height / 3));
            println!("ramp middle: {}", at(p.width / 2, 5 * p.height / 6));
        }
        Ok(None) => println!("not HDR"),
        Err(error) => println!("error: {error}"),
    }
}
