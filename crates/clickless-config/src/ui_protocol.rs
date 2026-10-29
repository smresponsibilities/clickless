//! Bounded inherited-pipe messages for the separate UI process.
use crate::Config;
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
