use std::process::{Command, Stdio};

pub fn execute(command: &str) {
    Command::new("/usr/bin/bash")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok();
}

pub fn print_selection(value: &str) {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{}", value).ok();
    handle.flush().ok();
}
