use std::path::{Path, PathBuf};
use std::{env, fs};

/// Commands the UI may call. Each window's capability file grants a subset.
const COMMANDS: &[&str] = &[
    "get_state",
    "update_settings",
    "preview_pack",
    "market_list",
    "market_install",
    "remove_pack",
    "open_packs_folder",
    "open_link",
    "show_settings",
    "arrange_overlay",
    "quit_app",
];

fn main() {
    embed_builtin_sounds();
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}

/// Compile every file under `sounds/` into the binary as `BUILTIN_FILES`.
fn embed_builtin_sounds() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("sounds");
    println!("cargo:rerun-if-changed=sounds");
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    files.sort();
    let mut out = String::from("pub static BUILTIN_FILES: &[(&str, &[u8])] = &[\n");
    for rel in files {
        let abs = root.join(&rel);
        out += &format!(
            "    ({rel:?}, include_bytes!({:?})),\n",
            abs.display().to_string()
        );
    }
    out += "];\n";
    let dest = Path::new(&env::var("OUT_DIR").unwrap()).join("builtin_sounds.rs");
    fs::write(dest, out).unwrap();
}

fn collect(root: &Path, dir: &Path, files: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, files);
        } else {
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            files.push(rel);
        }
    }
}
