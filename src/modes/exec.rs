use crate::config::Scripts;
use crate::executor;

pub fn run(name: &str) {
    let scripts = Scripts::load();
    if let Some(entry) = scripts.entries.get(name) {
        executor::execute(&entry.command);
    }
}
