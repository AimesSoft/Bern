//! The average color of a picture, for `Shape(fill_from:)`: read and
//! decoded once per address off the main thread; windows draw again when
//! it is known.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

enum Average {
    Pending,
    Known(Option<[u8; 3]>),
}

static AVERAGES: LazyLock<Mutex<HashMap<String, Average>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The average color of the picture at `url` (a file or an http(s)
/// address); none until it is read, or when it cannot be.
pub(crate) fn average(url: &str) -> Option<gpui::Rgba> {
    if url.is_empty() {
        return None;
    }
    let mut averages = AVERAGES.lock().ok()?;
    match averages.get(url) {
        Some(Average::Known(color)) => {
            return color.map(|[r, g, b]| gpui::Rgba {
                r: f32::from(r) / 255.0,
                g: f32::from(g) / 255.0,
                b: f32::from(b) / 255.0,
                a: 1.0,
            });
        }
        Some(Average::Pending) => return None,
        None => {}
    }
    averages.insert(url.to_string(), Average::Pending);
    let url = url.to_string();
    std::thread::spawn(move || {
        let color = read(&url).and_then(|bytes| mean(&bytes));
        if let Ok(mut averages) = AVERAGES.lock() {
            averages.insert(url, Average::Known(color));
        }
        crate::host::redraw();
    });
    None
}

fn read(url: &str) -> Option<Vec<u8>> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() == "file" {
        return std::fs::read(parsed.to_file_path().ok()?).ok();
    }
    use futures::AsyncReadExt;
    use http_client::HttpClient;
    let client = reqwest_client::ReqwestClient::new();
    let request = http_client::Request::get(url)
        .body(http_client::AsyncBody::default())
        .ok()?;
    futures::executor::block_on(async {
        let mut response = client.send(request).await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        let mut bytes = Vec::new();
        response.body_mut().read_to_end(&mut bytes).await.ok()?;
        Some(bytes)
    })
}

fn mean(bytes: &[u8]) -> Option<[u8; 3]> {
    let picture = image::load_from_memory(bytes)
        .ok()?
        .thumbnail(24, 24)
        .into_rgb8();
    let count = u64::from(picture.width()) * u64::from(picture.height());
    if count == 0 {
        return None;
    }
    let mut sum = [0u64; 3];
    for pixel in picture.pixels() {
        for (total, channel) in sum.iter_mut().zip(pixel.0) {
            *total += u64::from(channel);
        }
    }
    Some(sum.map(|total| (total / count) as u8))
}
