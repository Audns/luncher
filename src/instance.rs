use std::os::unix::net::UnixListener;
use std::path::PathBuf;

pub struct SingleInstance {
    _listener: UnixListener,
    socket_path: PathBuf,
}

impl SingleInstance {
    pub fn try_acquire() -> std::io::Result<Option<Self>> {
        let socket_path = socket_path();

        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        match UnixListener::bind(&socket_path) {
            Ok(listener) => Ok(Some(Self {
                _listener: listener,
                socket_path,
            })),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                match std::os::unix::net::UnixStream::connect(&socket_path) {
                    Ok(_) => Ok(None),
                    Err(_) => {
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
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

fn socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(runtime_dir).join("luncher.lock")
}
