use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const RELEASE_TARGETS: [&str; 6] = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
];

const SHIPPED_CRATES: [&str; 2] = ["tinyusd-capi", "tinyusd-py"];

const LICENSE_PREFIXES: [&str; 6] = ["LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT", "UNLICENSE"];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    crate_dir().join("../..").canonicalize().unwrap()
}

fn bless() -> bool {
    std::env::var_os("TINYUSD_BLESS").is_some_and(|v| v != "0")
}

fn collect(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(root, &path, out);
        } else {
            out.insert(path.strip_prefix(root).unwrap().to_path_buf(), fs::read(&path).unwrap());
        }
    }
}

fn check_dir(dir: &Path, expected: &BTreeMap<PathBuf, Vec<u8>>) {
    let mut actual = BTreeMap::new();
    if dir.exists() {
        collect(dir, dir, &mut actual);
    }
    if &actual == expected {
        return;
    }
    if bless() {
        if dir.exists() {
            fs::remove_dir_all(dir).unwrap();
        }
        for (rel, bytes) in expected {
            let path = dir.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        return;
    }
    let changed: Vec<_> = expected
        .keys()
        .chain(actual.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|k| expected.get(*k) != actual.get(*k))
        .take(10)
        .collect();
    panic!(
        "{} is out of date (differs at {changed:?}); run `TINYUSD_BLESS=1 cargo test -p tinyusd-capi`",
        dir.display()
    );
}

#[test]
fn c_header_is_current() {
    let config = cbindgen::Config::from_file(crate_dir().join("cbindgen.toml")).unwrap();
    let bindings = cbindgen::Builder::new()
        .with_crate(crate_dir())
        .with_config(config)
        .generate()
        .unwrap();
    let mut header = Vec::new();
    bindings.write(&mut header);
    let path = repo_root().join("include/tinyusd/tinyusd.h");
    if fs::read(&path).ok().as_deref() == Some(header.as_slice()) {
        return;
    }
    if bless() {
        fs::write(&path, header).unwrap();
        return;
    }
    panic!("include/tinyusd/tinyusd.h is out of date; run `TINYUSD_BLESS=1 cargo test -p tinyusd-capi`");
}

fn metadata(target: &str) -> Value {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--filter-platform",
            target,
        ])
        .current_dir(repo_root())
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).unwrap()
}

struct Package {
    name: String,
    version: String,
    license: String,
    repository: String,
    dir: PathBuf,
}

fn shipped_packages() -> BTreeMap<(String, String), Package> {
    let mut out = BTreeMap::new();
    for target in RELEASE_TARGETS {
        let meta = metadata(target);
        let packages: BTreeMap<&str, &Value> = meta["packages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (p["id"].as_str().unwrap(), p))
            .collect();
        let nodes: BTreeMap<&str, &Value> = meta["resolve"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| (n["id"].as_str().unwrap(), n))
            .collect();
        let mut stack: Vec<&str> = packages
            .iter()
            .filter(|(_, p)| SHIPPED_CRATES.contains(&p["name"].as_str().unwrap()))
            .map(|(id, _)| *id)
            .collect();
        let mut seen = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            for dep in nodes[id]["deps"].as_array().unwrap() {
                let normal = dep["dep_kinds"].as_array().unwrap().iter().any(|k| k["kind"].is_null());
                let pkg = packages[dep["pkg"].as_str().unwrap()];
                let proc_macro = pkg["targets"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|t| t["kind"].as_array().unwrap().iter().any(|k| k == "proc-macro"));
                if normal && !proc_macro {
                    stack.push(dep["pkg"].as_str().unwrap());
                }
            }
        }
        for id in seen {
            let p = packages[id];
            if p["source"].is_null() {
                continue;
            }
            let name = p["name"].as_str().unwrap().to_owned();
            let version = p["version"].as_str().unwrap().to_owned();
            out.entry((name.clone(), version.clone())).or_insert_with(|| Package {
                name,
                version,
                license: p["license"].as_str().unwrap_or("").to_owned(),
                repository: p["repository"].as_str().unwrap_or("").to_owned(),
                dir: Path::new(p["manifest_path"].as_str().unwrap())
                    .parent()
                    .unwrap()
                    .to_path_buf(),
            });
        }
    }
    out
}

fn license_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file())
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy().to_uppercase();
            LICENSE_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
        })
        .collect();
    files.sort();
    files
}

fn files_for(package: &Package) -> Vec<(String, PathBuf)> {
    let mut files = Vec::new();
    let mut dir = Some(package.dir.as_path());
    for _ in 0..3 {
        let Some(d) = dir else { break };
        let found = license_files(d);
        if !found.is_empty() {
            files.extend(
                found
                    .into_iter()
                    .map(|f| (f.file_name().unwrap().to_string_lossy().into_owned(), f)),
            );
            break;
        }
        dir = d.parent();
    }
    if package.name.starts_with("openusd") {
        let mut dir = Some(package.dir.as_path());
        while let Some(d) = dir {
            let vendor = d.join("vendor/OpenUSD");
            if vendor.is_dir() {
                for f in license_files(&vendor) {
                    files.push((format!("OpenUSD-{}", f.file_name().unwrap().to_string_lossy()), f));
                }
                break;
            }
            dir = d.parent();
        }
    }
    files
}

#[test]
fn third_party_licenses_are_current() {
    let packages = shipped_packages();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for p in packages.values() {
        *counts.entry(p.name.as_str()).or_default() += 1;
    }
    let mut expected = BTreeMap::new();
    let mut index = String::from(
        "Third-party code shipped in tinyusd's libraries and Python wheels.\n\
         Each directory holds that project's license files, copied unchanged.\n\n",
    );
    let mut missing = Vec::new();
    for p in packages.values() {
        let dir = if counts[p.name.as_str()] > 1 {
            format!("{}-{}", p.name, p.version)
        } else {
            p.name.clone()
        };
        let files = files_for(p);
        if files.is_empty() {
            missing.push(format!("{} {}", p.name, p.version));
            continue;
        }
        for (name, path) in &files {
            expected.insert(PathBuf::from(&dir).join(name), fs::read(path).unwrap());
        }
        index.push_str(&format!(
            "{dir}\n    version: {}\n    license: {}\n",
            p.version, p.license
        ));
        if !p.repository.is_empty() {
            index.push_str(&format!("    source: {}\n", p.repository));
        }
        let listed: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        index.push_str(&format!("    files: {}\n\n", listed.join(", ")));
    }
    assert!(missing.is_empty(), "no license files found for: {missing:?}");
    expected.insert(PathBuf::from("INDEX.txt"), index.into_bytes());
    check_dir(&repo_root().join("licenses"), &expected);
}
