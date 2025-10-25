use fzf_wrapped::{run_with_output, Border, Color, Fzf};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Error, Read};
use std::ops::Deref;
use std::path::PathBuf;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::{env, fs};
use std::{io};
use tmux_interface::{start_server, NewSession, SwitchClient, Tmux};
use yaml_rust::YamlLoader;

// TODO: Add support for other directories outside of HOME
// TODO: Create new directories
// FIXME: printing session not found to command line when starting new session
// FIXME: Cant enter tmux if not already in session

const HOME: &str = env!("HOME_PATH");
const CONF: &str = env!("SESSION_PATH");

pub fn search() {
    let mut s_directories: BTreeSet<String> = BTreeSet::new();
    s_directories.insert(HOME.to_string());
    let orig_out_dirs: Arc<Mutex<BTreeSet<String>>> = Arc::new(Mutex::new(s_directories));
    let _ = parse_paths(Arc::clone(&orig_out_dirs), CONF);
    tmux_session(fzf_search(orig_out_dirs.lock().unwrap().deref().clone()));
}

fn parse_paths(out_dirs: Arc<Mutex<BTreeSet<String>>>, conf_path: &str) -> Result<(), Error> {
    let mut file = match File::open(conf_path) {
        Err(why) => {
            eprintln!("\ncouldn't open {}: {}\n", "session.yml", why);
            exit(1);
        }
        Ok(file) => file,
    };
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let docs = match YamlLoader::load_from_str(&contents) {
        Err(why) => {
            eprintln!("\ncouldn't parse {}: {}\n", "session config", why);
            exit(1);
        }
        Ok(docs) => docs,
    };
    let doc: &yaml_rust::Yaml;
    if docs.is_empty() {
        eprintln!("uh oh {} is completely empty!\nconsider adding some directories in the format:\n\ndirectories:\n  - name: <directory excluding home path>\n    layers: <number of layers>", conf_path);
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
    let mut handles: Vec<JoinHandle<Result<BTreeSet<String>, Error>>> = Vec::with_capacity(40);
    for entry in directories.iter() {
        let name_out = entry["name"].as_str();
        let search_path: String;
        match name_out {
            Some(name_from_entry) => search_path = name_from_entry.to_string(),
            None => {
                continue;
            }
        }
        let layers_out = entry["layers"].as_i64();
        let layers: i8;
        match layers_out {
            Some(layers_from_entry) => layers = layers_from_entry as i8,
            None => {
                continue;
            }
        }

        let out_dirs_clone = Arc::clone(&out_dirs);
        let handle = thread::spawn(move || {
            let path_slices = [HOME.as_bytes(), search_path.as_bytes()];
            let cur_dir_path = PathBuf::from(concat_bytez(&path_slices));
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
                    out_dirs.insert(basename.clone());
                    let sub_results = get_sub_dirs_mul_layer(out_dirs, path, layers - 1)?;
                    results.extend(sub_results);
                }
            }
        }
    }
    Ok(results)
}

fn concat_bytez(byte_slices: &[&[u8]]) -> String {
    let length = byte_slices.iter().map(|s| s.len()).sum();
    let mut buffer = Vec::with_capacity(length);
    for slice in byte_slices {
        buffer.extend_from_slice(slice);
    }
    String::from_utf8(buffer).expect("bad string")
}

fn fzf_search(out_dirs: BTreeSet<String>) -> String {
    let users_selection: String = run_with_output(
        Fzf::builder()
            .border(Border::Rounded)
            .border_label("session")
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

// TODO: be able to launch without going into tmux first
fn tmux_session(users_selection: String) {
    let (remaining, basename) = users_selection.rsplit_once('/').unwrap();
    let (_, parent) = remaining.rsplit_once('/').unwrap();

    let session_name: String = parent.to_string() + "/" + basename;

    // start_server!();
    // let _ = Tmux::new().add_command(
    //     NewSession::new()
    //         .detached()
    //         .session_name(&session_name)
    //         .start_directory(&users_selection),
    // );
    // Tmux::with_command(SwitchClient::new().target_session(&session_name))
    //     .status()
    //     .unwrap();
    // std::process::exit(0);

    start_server!();
    // let status = Tmux::with_command(Tmux::HasSession::new().target_session(&session_name))
    //     .status()
    //     .unwrap()
    //     .success();
    //if !status {
    Tmux::new()
        .add_command(
            NewSession::new()
                .detached()
                .session_name(&session_name)
                .start_directory(&users_selection),
        )
        .output()
        .unwrap();
    //}
    Tmux::with_command(SwitchClient::new().target_session(&session_name))
        .status()
        .unwrap();
    std::process::exit(0)
}
