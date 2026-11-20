//! Prints what an EasyWorship 7 schedule holds (cargo run -p wp-core --example ews_probe -- file.ewsx).
fn main() {
    let p = std::env::args().nth(1).expect("a .ewsx file");
    let s = wp_core::import::ews::read(std::path::Path::new(&p)).unwrap();
    for it in &s.items {
        match it {
            wp_core::import::ews::EwsItem::Song { title, number, slides, .. } => {
                println!("SONG {title} (no. {number}), {} slides", slides.len());
                for (l, lines) in slides.iter().take(3) {
                    println!("   [{}] {}", l.as_deref().unwrap_or("?"), lines.join(" / "));
                }
            }
            other => println!("{other:?}"),
        }
    }
    println!("scripture options: {:?}", s.scripture);
}
