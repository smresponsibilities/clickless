//! Bounded inherited-pipe messages for the separate UI process.
use super::Config;
use std::io::{BufRead, Read, Write};

pub fn read_line(reader: &mut impl BufRead) -> Result<String, String> {
    let mut line = String::new();
    reader
        .take(257)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    if line.len() > 256 || !line.ends_with('\n') {
        return Err("UI pipe closed or invalid command".into());
    }
    Ok(line.trim().to_string())
}

pub fn read_config(reader: &mut impl BufRead) -> Result<Config, String> {
    let size = read_line(reader)?
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
    if size == 0 || size > 65_536 {
        return Err("UI configuration exceeds 64 KiB".into());
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    Config::parse(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

pub fn write_config(writer: &mut impl Write, config: &Config) -> Result<(), String> {
    let text = config.to_toml();
    writeln!(writer, "{}", text.len()).map_err(|e| e.to_string())?;
    writer
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())
}

/// Bounded output keeps a stalled companion off the capture thread.
pub struct UiWriter {
    pending: std::sync::mpsc::SyncSender<Vec<u8>>,
    alive: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl UiWriter {
    pub fn new(mut writer: impl Write + Send + 'static) -> Result<Self, String> {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        let (pending, messages) = mpsc::sync_channel::<Vec<u8>>(8);
        let alive = Arc::new(AtomicBool::new(true));
        let running = alive.clone();
        std::thread::Builder::new()
            .name("settings-output".into())
            .spawn(move || {
                while let Ok(bytes) = messages.recv() {
                    if !running.load(Ordering::SeqCst)
                        || writer
                            .write_all(&bytes)
                            .and_then(|()| writer.flush())
                            .is_err()
                    {
                        break;
                    }
                }
                running.store(false, Ordering::SeqCst);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { pending, alive })
    }

    pub fn send(&self, bytes: Vec<u8>) -> Result<(), String> {
        if self.is_closed() {
            return Err("Settings output is closed".into());
        }
        if bytes.len() > 65_800 || self.pending.try_send(bytes).is_err() {
            self.alive.store(false, std::sync::atomic::Ordering::SeqCst);
            return Err("Settings output is stalled or disconnected".into());
        }
        Ok(())
    }

    pub fn is_closed(&self) -> bool {
        !self.alive.load(std::sync::atomic::Ordering::SeqCst)
    }
}

pub struct Replies {
    pub responses: std::sync::mpsc::Receiver<String>,
    pub updates: std::sync::Arc<std::sync::Mutex<Option<Config>>>,
    pub alive: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub show: std::sync::Arc<std::sync::atomic::AtomicU8>,
}

pub fn read_replies(mut reader: impl BufRead + Send + 'static) -> Replies {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    };
    let (response_tx, responses) = mpsc::sync_channel(1);
    let updates = Arc::new(std::sync::Mutex::new(None));
    let update_tx = updates.clone();
    let alive = Arc::new(AtomicBool::new(true));
    let running = alive.clone();
    let show = Arc::new(AtomicU8::new(0));
    let show_requested = show.clone();
    std::thread::spawn(move || {
        while let Ok(line) = read_line(&mut reader) {
            if line == "show" || line == "show home" {
                show_requested.store(if line == "show home" { 2 } else { 1 }, Ordering::SeqCst);
            } else if line == "state" {
                match read_config(&mut reader) {
                    Ok(config) => {
                        if let Ok(mut update) = update_tx.lock() {
                            *update = Some(config);
                        }
                    }
                    Err(_) => break,
                }
            } else if line == "OK" || line.starts_with("ERR ") {
                if response_tx.send(line).is_err() {
                    break;
                }
            } else {
                break;
            }
        }
        running.store(false, Ordering::SeqCst);
    });
    Replies {
        responses,
        updates,
        alive,
        show,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stalled_writer_does_not_block_seed_or_runtime_messages() {
        use std::sync::mpsc;
        use std::time::Duration;
        struct Stalled {
            entered: Option<mpsc::Sender<()>>,
            release: mpsc::Receiver<()>,
        }
        impl Write for Stalled {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if let Some(entered) = self.entered.take() {
                    entered.send(()).unwrap();
                    self.release.recv().unwrap();
                }
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let (entered, started) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let (result, done) = mpsc::channel();
        let runtime = std::thread::spawn(move || {
            let writer = UiWriter::new(Stalled {
                entered: Some(entered),
                release: gate,
            })
            .unwrap();
            writer.send(b"seed".to_vec()).unwrap();
            started.recv_timeout(Duration::from_secs(2)).unwrap();
            for _ in 0..8 {
                writer.send(b"state".to_vec()).unwrap();
            }
            let rejected = writer.send(b"quit".to_vec()).is_err();
            result.send(rejected && writer.is_closed()).unwrap();
        });
        let verdict = done.recv_timeout(Duration::from_secs(2));
        let _ = release.send(());
        runtime.join().unwrap();
        assert!(
            verdict.unwrap(),
            "full pipe must not block runtime or seed sends"
        );
    }

    #[test]
    fn writer_failure_closes_connection() {
        struct Failed;
        impl Write for Failed {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "injected",
                ))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let writer = UiWriter::new(Failed).unwrap();
        writer.send(b"seed".to_vec()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !writer.is_closed() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(writer.is_closed());
        assert!(writer.send(b"state".to_vec()).is_err());
    }

    #[test]
    fn roundtrip_and_reject_unbounded_frames() {
        let config = Config::default();
        let mut bytes = Vec::new();
        write_config(&mut bytes, &config).unwrap();
        assert_eq!(read_config(&mut bytes.as_slice()).unwrap(), config);
        for mut invalid in [b"65537\n".as_slice(), b"0\n", b"12\nshort", b"nonsense\n"] {
            assert!(read_config(&mut invalid).is_err());
        }
        assert!(read_line(&mut vec![b'a'; 300].as_slice()).is_err());
    }

    #[test]
    fn state_updates_do_not_consume_apply_acknowledgements() {
        let mut bytes = b"show\nstate\n".to_vec();
        let mut config = Config::default();
        write_config(&mut bytes, &config).unwrap();
        bytes.extend_from_slice(b"state\n");
        config.enabled = false;
        write_config(&mut bytes, &config).unwrap();
        bytes.extend_from_slice(b"OK\n");
        let replies = read_replies(std::io::Cursor::new(bytes));
        assert_eq!(
            replies
                .responses
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            "OK"
        );
        assert_eq!(replies.updates.lock().unwrap().as_ref(), Some(&config));
        assert_eq!(replies.show.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
