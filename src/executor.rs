pub fn execute(command: &str) {
    std::process::Command::new("/usr/bin/bash")
        .arg("-c")
        .arg(command)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok();
}

pub fn print_selection(value: &str) {
    println!("{}", value);
}
