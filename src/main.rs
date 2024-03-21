use fzf_wrapped::{run_with_output, Fzf};
use std::fs;
use std::fs::File;
use std::io;
use std::io::{Error, Read};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use tmux_interface::{HasSession, NewSession, SwitchClient, Tmux};
use yaml_rust::YamlLoader;

const DIRS: &str = "session-directories.yaml";

fn get_home_dir() -> String {
    simple_home_dir::home_dir()
        .expect("Could not determine home directory")
        .display()
        .to_string()
}

fn get_sub_dirs_mul_layer(
    out_dirs: &mut Vec<String>,
    dir: PathBuf,
    layers: i8,
) -> io::Result<Vec<String>> {
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
                    let basename = &path.display().to_string();
                    // println!("{}", basename);
                    out_dirs.push(basename.clone());
                    let sub_results = get_sub_dirs_mul_layer(out_dirs, path, layers - 1)?;
                    results.extend(sub_results);
                }
            }
        }
    }
    Ok(results)
}

fn add_to_dirs(out_dirs: &mut Vec<String>, dir: PathBuf) -> io::Result<Vec<String>> {
    // println!("{:?}", dir);
    out_dirs.push(dir.to_str().unwrap().to_string());
    Ok(out_dirs.to_vec())
}

fn parse(out_dirs: Arc<Mutex<Vec<String>>>, home: String, dir_yaml: String) -> Result<(), Error> {
    // let mut file = File::open(dir_yaml)?;
    let mut file = match File::open(dir_yaml) {
        Err(why) => panic!("\ncouldn't open {}: {}\n", DIRS, why),
        Ok(file) => file,
    };
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    // let docs = YamlLoader::load_from_str(&contents).unwrap();
    let docs = match YamlLoader::load_from_str(&contents) {
        Err(why) => panic!("\ncouldn't parse {}: {}\n", DIRS, why),
        Ok(docs) => docs,
    };
    let doc: &yaml_rust::Yaml;
    if docs.is_empty() {
        panic!("\nuh oh {} is completely empty!\nconsider adding some directories in the format:\n\ndirectories:\n  - name: <directory excluding home path>\n    layers: <number of layers>\n", DIRS)
    } else {
        doc = &docs[0];
    }
    let directories = &doc["directories"].as_vec().unwrap_or_else(|| panic!("\nyikes, there doesn't seem to be any entries in {}!\nmake sure you have are following the format:\n\ndirectories:\n  - name: <directory excluding home path>\n    layers: <number of layers>\n", DIRS));

    
    let mut handles: Vec<JoinHandle<Result<Vec<String>, Error>>> = vec![];

    for entry in directories.iter() {
        let name = entry["name"].as_str().unwrap().to_string();
        let layers = entry["layers"].as_i64().unwrap() as i8;
        let home_clone = home.clone();
        let out_dirs_clone = Arc::clone(&out_dirs);
        let handle = thread::spawn(move || {
            let cur_dir_path = PathBuf::from(&format!("{}{}", home_clone, name));
            let result: Result<Vec<String>, Error>;
            if layers == 0 {
                result = add_to_dirs(&mut out_dirs_clone.lock().unwrap(), cur_dir_path);
            } else {
                result = get_sub_dirs_mul_layer(
                    &mut out_dirs_clone.lock().unwrap(),
                    cur_dir_path,
                    layers,
                );
            }
            result
        });
        handles.push(handle);
    }
    for handle in handles {
        let result = handle.join().unwrap();
        result?;
    }
    Ok(())
}

fn tmux_session(users_selection: String) {
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

fn main() {
    let home = get_home_dir() + "/";
    let orig_out_dirs = Arc::new(Mutex::new(Vec::new()));
    let path_to_dir_list = home.to_owned() + DIRS;
    // println!("path to dir list {}", path_to_dir_list);
    let _ = parse(Arc::clone(&orig_out_dirs), home, path_to_dir_list);
    let out_dirs = orig_out_dirs.lock().unwrap().clone();

    let users_selection: String =
        run_with_output(Fzf::default(), out_dirs).expect("Something went awry!");
    if users_selection.is_empty() {
        std::process::exit(0)
    }
    tmux_session(users_selection);
}
