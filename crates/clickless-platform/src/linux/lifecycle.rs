//! Native lifecycle ownership and bounded Settings-open requests.
use std::fs::{self, DirBuilder, File, OpenOptions, TryLockError};
use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt};
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;

fn runtime_directory() -> io::Result<PathBuf> {
    // SAFETY: geteuid takes no pointers and only reads the process identity.
    let uid = unsafe { libc::geteuid() };
    let directory = PathBuf::from(format!("/tmp/clickless-{uid}"));
    match DirBuilder::new().mode(0o700).create(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = fs::symlink_metadata(&directory)?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o777 != 0o700 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Clickless runtime directory must be private and owned by the effective user",
        ));
    }
    Ok(directory)
}

pub struct SingleInstance {
    _lock: File,
    directory: PathBuf,
}

impl SingleInstance {
    pub fn acquire() -> io::Result<Self> {
        let directory = runtime_directory()?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(directory.join("runtime.lock"))?;
        let metadata = lock.metadata()?;
        let owner = fs::symlink_metadata(&directory)?.uid();
        if !metadata.is_file()
            || metadata.uid() != owner
            || metadata.nlink() != 1
            || metadata.mode() & 0o777 != 0o600
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Clickless lock must be a private regular file owned by the effective user",
            ));
        }
        lock.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => io::Error::new(
                io::ErrorKind::AlreadyExists,
                "another Clickless instance is already running",
            ),
            TryLockError::Error(error) => error,
        })?;
        // Never unlink the lock inode. File drop and process exit unlock it.
        Ok(Self {
            _lock: lock,
            directory,
        })
    }
}

pub fn notify_open_settings() -> Result<(), String> {
    let send = || -> io::Result<()> {
        let socket = UnixDatagram::unbound()?;
        socket.set_nonblocking(true)?;
        socket.send_to(b"open", runtime_directory()?.join("settings.sock"))?;
        Ok(())
    };
    send().map_err(|error| format!("Settings request failed (error {error})"))
}

pub struct SettingsRequest<'a> {
    socket: UnixDatagram,
    path: PathBuf,
    _owner: &'a mut SingleInstance,
}

impl<'a> SettingsRequest<'a> {
    pub fn create(owner: &'a mut SingleInstance) -> Result<Self, String> {
        let path = owner.directory.join("settings.sock");
        let bind = || -> io::Result<UnixDatagram> {
            let socket = UnixDatagram::bind(&path)?;
            socket.set_nonblocking(true)?;
            Ok(socket)
        };
        let socket = match bind() {
            Ok(socket) => socket,
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                // Ownership lock is held. Only a stale socket may be removed.
                let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
                let uid = fs::symlink_metadata(&owner.directory)
                    .map_err(|error| error.to_string())?
                    .uid();
                if !metadata.file_type().is_socket() || metadata.uid() != uid {
                    return Err(
                        "Refusing to replace a non-socket or foreign Settings endpoint".into(),
                    );
                }
                fs::remove_file(&path).map_err(|error| error.to_string())?;
                bind().map_err(|error| format!("Settings socket bind failed (error {error})"))?
            }
            Err(error) => return Err(format!("Settings socket bind failed (error {error})")),
        };
        Ok(Self {
            socket,
            path,
            _owner: owner,
        })
    }

    /// Read at most one atomic request so clients cannot delay runtime ticks.
    pub fn poll(&self) -> bool {
        // One extra byte rejects oversized messages with a valid prefix.
        let mut buf = [0u8; 5];
        matches!(self.socket.recv(&mut buf), Ok(4)) && &buf[..4] == b"open"
    }

    pub fn signal(&self) -> Result<(), String> {
        notify_open_settings()
    }
}

impl Drop for SettingsRequest<'_> {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
