// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // the Whisper helper process (speech recognition in its own process, so a crash in
    // native code never closes the presentation)
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--whisper-worker") {
        worship_presenter_lib::whisper_worker(args.get(2).map(String::as_str).unwrap_or(""));
    }
    worship_presenter_lib::run()
}
