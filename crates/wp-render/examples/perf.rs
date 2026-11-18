//! How long the built-in NDI renderer takes per picture (cargo run --release -p wp-render --example perf).
use std::time::Instant;
fn main() {
    let mut r = wp_render::Renderer::new();
    let dir = std::env::temp_dir();
    for name in ["stream-lower-third-clean", "methodist-hymn-bilingual", "hymn-white-box", "scripture-reference-bar"] {
        let theme: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("crates/wp-core/themes/gallery/{name}.json")).unwrap()).unwrap();
        let mut times = vec![];
        for i in 0..12 {
            let c = wp_render::Content {
                primary: vec![format!("O for a thousand tongues to sing {i}"), "My great Redeemer's praise,".into()],
                second: vec!["Meenya ngyiramá apem matow ndwom".into(), "Meyi mo Pomfo ayɛw,".into()],
                reference: Some("John 3:16".into()),
                ..Default::default()
            };
            let t = Instant::now();
            std::hint::black_box(r.render(&theme, &c, 1920, 1080, &dir));
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let first = times[0];
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("{name:<28} first {first:>7.1} ms   then median {:>6.1} ms   worst {:>6.1} ms", times[6], times[11]);
        // the same theme with no words (background, shapes, boxes only)
        let c = wp_render::Content { hide_text: true, ..Default::default() };
        let t = Instant::now();
        std::hint::black_box(r.render(&theme, &c, 1920, 1080, &dir));
        let layers: Vec<String> = theme["layers"].as_array().map(|a| a.iter().map(|l| format!("{}{}", l["type"].as_str().unwrap_or("?"), if l["style"]["box"]["blur"].as_f64().unwrap_or(0.0) > 0.0 || l["blur"].as_f64().unwrap_or(0.0) > 0.0 { "+blur" } else { "" })).collect()).unwrap_or_default();
        println!("{:<28} without words  {:>6.1} ms   layers {:?}  background {}", "", t.elapsed().as_secs_f64() * 1000.0, layers, theme["background"]["kind"]);
    }
}
