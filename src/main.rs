// use fzf_wrapped::run_with_output;
// use fzf_wrapped::Fzf;
use std::fs;

const HOME: &str = "/home/anthony/";

fn main() {
    let directories_1 = vec![format!("{}obsidian-vaults", HOME)];
    let directories_2 = vec![format!("{}development", HOME), format!("{}school", HOME)];

    let mut dirs = Vec::new();
    for dir in &directories_1 {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    if entry.file_type().unwrap().is_dir() {
                        dirs.push(entry.path().display().to_string());
                    }
                }
            }
        }
    }
    for path in dirs {
        println!("Name: {}", path);
    }

    // let paths = fs::read_dir("./").unwrap();
    //
    // for path in paths {
    //     println!("Name: {}", path.unwrap().path().display())
    // }
    // let users_selection =
    //     run_with_output(Fzf::default(), directories).expect("Something went wrong!");
}
