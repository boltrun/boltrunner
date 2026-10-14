use sha2::{Digest, Sha256};
use std::{env, fs, path::Path};

fn hash_path(root: &Path, path: &Path, digest: &mut Sha256) {
    println!("cargo:rerun-if-changed={}", path.display());
    if path.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for entry in entries {
            hash_path(root, &entry, digest);
        }
    } else {
        let name = path.strip_prefix(root).unwrap().to_str().unwrap();
        let bytes = fs::read(path).unwrap();
        digest.update(name.as_bytes());
        digest.update([0]);
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
}

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = Path::new(&manifest).parent().unwrap().parent().unwrap();
    let registry_path = root.join("assets/maps.json");
    let registry: serde_json::Value =
        serde_json::from_slice(&fs::read(&registry_path).unwrap()).unwrap();
    let maps = registry["maps"].as_array().expect("maps must be an array");
    assert!(!maps.is_empty(), "at least one map is required");
    let default = registry["default"].as_str().expect("default map id");
    let mut ids = std::collections::HashSet::new();
    let mut generated = format!("pub static MAPS: [MapAsset; {}] = [\n", maps.len());
    let mut default_index = None;
    for (index, map) in maps.iter().enumerate() {
        let id = map["id"].as_str().expect("map id");
        assert!(
            !id.is_empty()
                && id
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_'),
            "invalid map id"
        );
        assert!(ids.insert(id), "duplicate map id");
        if id == default {
            default_index = Some(index);
        }
        let directory = root.join("assets").join(id).join("built");
        let metadata = directory.join("map.json");
        let collision = directory.join("collision.bin");
        let render = directory.join("render.bin");
        generated.push_str(&format!("MapAsset::new({id:?}, include_str!({metadata:?}), include_bytes!({collision:?}), include_bytes!({render:?})),\n"));
    }
    generated.push_str(&format!(
        "];\npub const DEFAULT_MAP_INDEX: usize = {};\n",
        default_index.expect("default map must be registered")
    ));
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("map_assets.rs"),
        generated,
    )
    .unwrap();
    let mut digest = Sha256::new();
    hash_path(root, &registry_path, &mut digest);
    for map in maps {
        for file in ["map.json", "collision.bin", "render.bin"] {
            hash_path(
                root,
                &root
                    .join("assets")
                    .join(map["id"].as_str().unwrap())
                    .join("built")
                    .join(file),
                &mut digest,
            );
        }
    }
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        "crates/shared/Cargo.toml",
        "crates/shared/build.rs",
        "crates/server/Cargo.toml",
        "crates/client/Cargo.toml",
        "crates/shared/src",
        "crates/server/src",
        "crates/client/src",
        "vendor/aeronet_websocket",
    ] {
        hash_path(root, &root.join(path), &mut digest);
    }
    let build: String = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    println!("cargo:rustc-env=BOLTRUNNER_SIMULATION_BUILD={build}");
}
