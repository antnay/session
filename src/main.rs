use fzf_wrapped::{run_with_output, Fzf};
use std::fs;
use std::fs::File;
use std::io;
use std::io::{Error, Read};
use tmux_interface::{HasSession, NewSession, SwitchClient, Tmux};
use yaml_rust::YamlLoader;

const DIRS: &str = "session-directories.yaml";

fn get_home_dir() -> String {
    simple_home_dir::home_dir()
        .expect("Could not determine home directory")
        .display()
        .to_string()
}

fn get_sub_dirs(out_dirs: &mut Vec<String>, dir: &str, layers: i8) -> io::Result<Vec<String>> {
    let mut results = Vec::new();
    if layers == 0 {
        return Ok(out_dirs.to_vec());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            let file_name = entry.file_name();
            if let Some(name_str) = file_name.to_str() {
                if !name_str.starts_with('.') {
                    let basename = path.display().to_string();
                    out_dirs.push(basename.clone());
                    let sub_results = get_sub_dirs(out_dirs, &basename, layers - 1)?;
                    results.extend(sub_results);
                }
            }
        }
    }
    Ok(results)
}

fn parse(out_dirs: &mut Vec<String>, home: String, dir_yaml: String) -> Result<Vec<String>, Error> {
    let mut file = File::open(dir_yaml)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let docs = YamlLoader::load_from_str(&contents).unwrap();
    let doc = &docs[0];

    let directories = &doc["directories"].as_vec().unwrap();

    for entry in directories.iter() {
        let name = entry["name"].as_str().unwrap();
        let layers = entry["layers"].as_i64().unwrap() as i8;
        let cur_dir = &format!("{}{}", home, name);
        // println!("{}", cur_dir);
        get_sub_dirs(out_dirs, cur_dir, layers).expect("Something went awry");
    }
    Ok(out_dirs.to_vec())
}

fn main() {
    let home = get_home_dir() + "/";
    let mut out_dirs: Vec<String> = Vec::new();
    let path_to_dir_list = home.to_owned() + DIRS;
    // println!("{}", path_to_dir_list);
    let _ = parse(&mut out_dirs, home, path_to_dir_list);

    let users_selection: String =
        run_with_output(Fzf::default(), out_dirs).expect("Something went wrong!");
    if users_selection.is_empty() {
        std::process::exit(0)
    }

    let (remaining, basename) = users_selection.rsplit_once('/').unwrap();
    let (_, parent) = remaining.rsplit_once('/').unwrap();
    let session_name = format!("{}/{}", parent, basename);

    let status = Tmux::with_command(HasSession::new().target_session(&session_name))
        .status()
        .unwrap()
        .success();

    if !status {
        Tmux::new()
            .add_command(
                NewSession::new()
                    .detached()
                    .session_name(&session_name)
                    .start_directory(&users_selection),
            )
            .output()
            .unwrap();
    }
    Tmux::with_command(SwitchClient::new().target_session(&session_name))
        .status()
        .unwrap();
    std::process::exit(0)
}
