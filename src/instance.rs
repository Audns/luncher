use std::os::unix::net::UnixListener;
use std::path::PathBuf;

pub struct SingleInstance {
    // Keeps the socket alive — dropped when program exits,
    // which closes and removes the lock
    _listener: UnixListener,
    socket_path: PathBuf,
}

impl SingleInstance {
    /// Try to acquire the single-instance lock.
    /// Returns Ok(Some(guard)) if we are the only instance.
    /// Returns Ok(None) if another instance is already running.
    pub fn try_acquire() -> std::io::Result<Option<Self>> {
        let socket_path = socket_path();

        // Ensure parent dir exists
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        match UnixListener::bind(&socket_path) {
            Ok(listener) => Ok(Some(Self {
                _listener: listener,
                socket_path,
            })),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                // Socket exists — check if the other process is actually alive
                // by trying to connect to it
                match std::os::unix::net::UnixStream::connect(&socket_path) {
                    Ok(_) => {
                        // Another live instance is running
                        Ok(None)
                    }
                    Err(_) => {
                        // Stale socket — remove it and try again
                        std::fs::remove_file(&socket_path)?;
                        let listener = UnixListener::bind(&socket_path)?;
                        Ok(Some(Self {
                            _listener: listener,
                            socket_path,
                        }))
                    }
                }
            }
            Err(e) => Err(e),
        }
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        // Clean up socket on exit
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

fn socket_path() -> PathBuf {
    // Use XDG_RUNTIME_DIR if available (Wayland standard),
    // fall back to /tmp
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(runtime_dir).join("luncher.lock")
}
