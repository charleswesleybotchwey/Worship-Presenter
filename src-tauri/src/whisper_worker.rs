//! Whisper in its own process. whisper.cpp is native code: if it ever crashes (an
//! unsupported processor, a damaged model, a bug), only this helper process ends and
//! the presentation keeps running; the app logs it and starts the helper again.
//!
//! The helper is this same program started with `--whisper-worker <model>`. The app
//! sends it phrases (a u32 sample count, then 16 kHz mono f32 samples, little-endian);
//! it answers each with one JSON line: {"text": "..."} or {"error": "..."}. Its first
//! line is {"ready": true} once the model is loaded.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// Does this processor have what the Whisper build needs (AVX2, FMA, F16C)?
pub fn cpu_ok() -> Result<(), String> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if !(std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma") && std::is_x86_feature_detected!("f16c")) {
            return Err("This computer's processor is too old for Whisper (it needs AVX2, found on Intel processors from 2013 and AMD from 2015). Use Vosk in Settings → Scripture recognition.".into());
        }
    }
    Ok(())
}

pub fn encode(samples: &[f32]) -> Vec<u8> {
    let mut b = Vec::with_capacity(4 + samples.len() * 4);
    b.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    for s in samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

pub fn decode(r: &mut impl Read) -> std::io::Result<Vec<f32>> {
    let mut n = [0u8; 4];
    r.read_exact(&mut n)?;
    let n = u32::from_le_bytes(n) as usize;
    if n > 16_000 * 60 {
        return Err(std::io::Error::other("phrase too long"));
    }
    let mut raw = vec![0u8; n * 4];
    r.read_exact(&mut raw)?;
    Ok(raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
}

/// The helper's own loop (this process does nothing else).
pub fn worker_main(model: &str, prompt: &str) -> ! {
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
    let out = std::io::stdout();
    let say = |v: serde_json::Value| {
        let mut o = out.lock();
        let _ = writeln!(o, "{v}");
        let _ = o.flush();
    };
    if let Err(e) = cpu_ok() {
        say(serde_json::json!({ "error": e }));
        std::process::exit(2);
    }
    let ctx = match WhisperContext::new_with_params(model, WhisperContextParameters::default()) {
        Ok(c) => c,
        Err(e) => {
            say(serde_json::json!({ "error": format!("The Whisper model could not be loaded ({e}). Download it again in Settings.") }));
            std::process::exit(2);
        }
    };
    let mut state = match ctx.create_state() {
        Ok(s) => s,
        Err(e) => {
            say(serde_json::json!({ "error": format!("Whisper could not start ({e}).") }));
            std::process::exit(2);
        }
    };
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 8) as i32;
    say(serde_json::json!({ "ready": true }));
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    loop {
        let Ok(audio) = decode(&mut input) else { std::process::exit(0) };
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_n_threads(threads);
        params.set_no_context(true);
        params.set_single_segment(true);
        params.set_suppress_blank(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_initial_prompt(prompt);
        if state.full(params, &audio).is_err() {
            say(serde_json::json!({ "text": "" }));
            continue;
        }
        let mut text = String::new();
        // confidence: the average probability of the words (special tokens left out)
        let (mut sum, mut n) = (0.0f32, 0u32);
        for i in 0..state.full_n_segments() {
            if let Some(seg) = state.get_segment(i) {
                text.push_str(&seg.to_str_lossy().unwrap_or_default());
                for k in 0..seg.n_tokens() {
                    if let Some(t) = seg.get_token(k) {
                        let word = t.to_str_lossy().unwrap_or_default();
                        if !word.starts_with("[_") && !word.starts_with("<|") {
                            sum += t.token_probability();
                            n += 1;
                        }
                    }
                }
            }
        }
        let conf = if n > 0 { Some(sum / n as f32) } else { None };
        say(serde_json::json!({ "text": text.trim(), "conf": conf }));
    }
}

/// The app's side: one helper process, restarted if it ends.
pub struct Worker {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Worker {
    pub fn start(model: &Path) -> Result<Worker, String> {
        cpu_ok()?;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut cmd = Command::new(exe);
        cmd.arg("--whisper-worker").arg(model);
        Self::start_with(cmd)
    }

    fn start_with(mut cmd: Command) -> Result<Worker, String> {
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // no console window
        }
        let mut child = cmd.spawn().map_err(|e| format!("Whisper could not start ({e})."))?;
        let stdin = child.stdin.take().ok_or("Whisper's input is not available.")?;
        let mut stdout = BufReader::new(child.stdout.take().ok_or("Whisper's output is not available.")?);
        let mut line = String::new();
        stdout.read_line(&mut line).map_err(|e| e.to_string())?;
        let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap_or_default();
        if v["ready"] != true {
            let _ = child.kill();
            return Err(v["error"].as_str().map(str::to_string).unwrap_or_else(|| "Whisper stopped while loading its model.".into()));
        }
        Ok(Worker { child, stdin, stdout })
    }

    /// The words in one phrase, with a confidence (0–1). An error means the helper has
    /// ended (start another).
    pub fn transcribe(&mut self, audio: &[f32]) -> Result<(String, Option<f32>), String> {
        self.stdin.write_all(&encode(audio)).and_then(|_| self.stdin.flush()).map_err(|_| "Whisper stopped unexpectedly.".to_string())?;
        let mut line = String::new();
        let n = self.stdout.read_line(&mut line).map_err(|_| "Whisper stopped unexpectedly.".to_string())?;
        if n == 0 {
            return Err("Whisper stopped unexpectedly.".into());
        }
        let v: serde_json::Value = serde_json::from_str(line.trim()).map_err(|_| "Whisper sent something unreadable.".to_string())?;
        Ok((v["text"].as_str().unwrap_or("").to_string(), v["conf"].as_f64().map(|c| c as f32)))
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    /// A helper that crashes in the middle of recognition (as native code can): the app
    /// gets an error it can handle, and carries on.
    #[cfg(unix)]
    #[test]
    fn a_crashing_helper_does_not_take_the_app_down() {
        let mut cmd = std::process::Command::new("sh");
        cmd.args(["-c", r#"echo '{"ready":true}'; head -c 64 >/dev/null; kill -SEGV $$"#]);
        let mut w = super::Worker::start_with(cmd).unwrap();
        let r = w.transcribe(&vec![0.1f32; 16_000]);
        assert_eq!(r, Err("Whisper stopped unexpectedly.".to_string()));
        // a helper that fails while loading reports its message
        let mut cmd = std::process::Command::new("sh");
        cmd.args(["-c", r#"echo '{"error":"The model is damaged."}'; exit 2"#]);
        assert_eq!(super::Worker::start_with(cmd).err().as_deref(), Some("The model is damaged."));
    }

    #[test]
    fn phrases_cross_the_pipe_intact() {
        let audio: Vec<f32> = (0..16_000).map(|i| (i as f32 / 100.0).sin() * 0.5).collect();
        let bytes = super::encode(&audio);
        assert_eq!(bytes.len(), 4 + 16_000 * 4);
        let back = super::decode(&mut std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(back, audio);
        let huge = (16_000u32 * 120).to_le_bytes();
        assert!(super::decode(&mut std::io::Cursor::new(huge.to_vec())).is_err(), "a damaged length is refused");
    }
}
