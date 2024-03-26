use fzf_wrapped::{run_with_output, Border, Color, Fzf};
use std::collections::BTreeSet;
use std::fs;
use std::fs::File;
use std::io;
use std::io::{Error, Read};
use std::ops::Deref;
use std::path::PathBuf;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use tmux_interface::{start_server, HasSession, NewSession, SwitchClient, Tmux};
use yaml_rust::YamlLoader;

// TODO: Add support for other directories outside of HOME

const DIRS: &str = "session-directories.yaml";

fn main() {
    let home = get_home_dir() + "/";
    // let orig_out_dirs = Arc::new(Mutex::new(vec![home.to_owned()]));
    let mut s_directories: BTreeSet<String> = BTreeSet::new();
    s_directories.insert(home.clone());
    let orig_out_dirs: Arc<Mutex<BTreeSet<String>>> = Arc::new(Mutex::new(s_directories));
    let path_to_dir_list = home.to_owned() + DIRS;
    let _ = parse(Arc::clone(&orig_out_dirs), home, path_to_dir_list);
    // let out_dirs = orig_out_dirs.lock().unwrap().clone();
    tmux_session(fzf_search(orig_out_dirs.lock().unwrap().deref().clone()));
}

fn get_home_dir() -> String {
    simple_home_dir::home_dir()
        .expect("Could not determine home directory")
        .display()
        .to_string()
}

fn parse(
    out_dirs: Arc<Mutex<BTreeSet<String>>>,
    home: String,
    dir_yaml: String,
) -> Result<(), Error> {
    let mut file = match File::open(dir_yaml) {
        Err(why) => {
            eprintln!("\ncouldn't open {}: {}\n", DIRS, why);
            exit(1);
        }
        Ok(file) => file,
    };
    let mut contents = String::new();

    file.read_to_string(&mut contents)?;
    let docs = match YamlLoader::load_from_str(&contents) {
        Err(why) => {
            eprintln!("\ncouldn't parse {}: {}\n", DIRS, why);
            exit(1);
        }
        Ok(docs) => docs,
    };
    let doc: &yaml_rust::Yaml;
    if docs.is_empty() {
        eprintln!("uh oh {} is completely empty!\nconsider adding some directories in the format:\n\ndirectories:\n  - name: <directory excluding home path>\n    layers: <number of layers>", DIRS);
        exit(1);
    } else {
        doc = &docs[0];
    }
    let directories;
    match doc["directories"].as_vec() {
        Some(dir) => directories = dir,
        None => {
            eprintln!("yikes, there doesn't seem to be any entries in 'directories'!\nmake sure you are following the format:\n\ndirectories:\n  - name: <directory excluding home path>\n    layers: <number of layers>");
            exit(1);
        }
    }
    let mut handles: Vec<JoinHandle<Result<BTreeSet<String>, Error>>> = vec![];
    for entry in directories.iter() {
        let name_out = entry["name"].as_str();
        let name: String;
        match name_out {
            Some(name_from_entry) => name = name_from_entry.to_string(),
            None => {
                eprintln!(
                    "oh shoot, an entry is missing a value for 'name' in {}! Skipping.",
                    DIRS
                );
                continue;
            }
        }
        let layers_out = entry["layers"].as_i64();
        let layers: i8;
        match layers_out {
            Some(layers_from_entry) => layers = layers_from_entry as i8,
            None => {
                eprintln!(
                    "aw man, an entry is missing a value for 'layers' in {}! Skipping.",
                    DIRS
                );
                continue;
            }
        }

        let home_clone = home.clone();
        let out_dirs_clone = Arc::clone(&out_dirs);
        let handle = thread::spawn(move || {
            let cur_dir_path = PathBuf::from(&format!("{}{}", home_clone, name));
            let result: Result<BTreeSet<String>, Error>;
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

fn get_sub_dirs_mul_layer(
    out_dirs: &mut BTreeSet<String>,
    dir: PathBuf,
    layers: i8,
) -> io::Result<BTreeSet<String>> {
    let mut results = BTreeSet::new();
    if layers == 0 {
        return Ok(out_dirs.deref().clone());
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
                    out_dirs.insert(basename.clone());
                    let sub_results = get_sub_dirs_mul_layer(out_dirs, path, layers - 1)?;
                    results.extend(sub_results);
                }
            }
        }
    }
    Ok(results)
}

fn fzf_search(out_dirs: BTreeSet<String>) -> String {
    let users_selection: String = run_with_output(
        Fzf::builder()
            .border(Border::Rounded)
            .border_label("Sessionizer")
            .color(Color::Dark)
            .build()
            .unwrap(),
        out_dirs,
    )
    .expect("Something went awry with fzf!");
    if users_selection.is_empty() {
        std::process::exit(0)
    }
    users_selection
}

fn add_to_dirs(out_dirs: &mut BTreeSet<String>, dir: PathBuf) -> io::Result<BTreeSet<String>> {
    // println!("{:?}", dir);
    out_dirs.insert(dir.to_str().unwrap().to_string());
    Ok(out_dirs.deref().clone())
}

fn tmux_session(users_selection: String) {
    let (remaining, basename) = users_selection.rsplit_once('/').unwrap();
    let (_, parent) = remaining.rsplit_once('/').unwrap();
    let session_name = format!("{}/{}", parent, basename);
    start_server!();
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
