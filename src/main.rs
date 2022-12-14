use anyhow::{anyhow, Result};
use cargo::{
    core::{PackageSet, SourceMap, Workspace},
    ops::{generate_lockfile, load_pkg_lockfile},
    util::{config::Config, important_paths::find_root_manifest_for_wd},
};
use clap::Parser;
use git2::Repository;

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
use toml_edit::{Document, InlineTable, Item, Table};
use webbrowser::open;

#[derive(Parser, Debug)]
#[clap(name = "cargo")]
#[clap(bin_name = "cargo")]
enum Cargo {
    Forkdep(Forkdep),
}

#[derive(clap::Args, Debug)]
#[clap(author, version, about, long_about = None)]
struct Forkdep {
    dependency: String,

    #[clap(long, value_parser)]
    manifest_path: Option<PathBuf>,
}

fn main() -> Result<()> {
    let Cargo::Forkdep(args) = Cargo::parse();
    let config = Config::default()?;
    let manifest_path: PathBuf = args
        .manifest_path
        .map(Ok)
        .unwrap_or_else(|| find_root_manifest_for_wd(&std::env::current_dir()?))?;
    let workspace = Workspace::new(&manifest_path, &config)?;
    let repo = get_repo(&workspace, &args.dependency)?;
    let mut manifest = read_manifest(&manifest_path)?;
    let patch_dir = manifest_path
        .parent()
        .ok_or_else(|| anyhow!("could not find parent directory of manifest"))?;
    let dep_path = make_local_copy(&repo, patch_dir, &args.dependency)?;
    insert_patch(&mut manifest, &dep_path, args.dependency)?;
    fs::write(manifest_path, manifest.to_string())?;
    Ok(())
}

fn make_local_copy(url: &str, dir: &Path, dep_name: &str) -> Result<PathBuf> {
    let new_url = fork_repo(url)?;
    let root_repo = Repository::open(dir)?;
    let mut submodule =
        root_repo.submodule(&new_url, Path::new(&format!("patches/{dep_name}")), false)?;
    submodule.clone(None)?;
    Ok(submodule.path().to_owned())
}

fn fork_repo(url: &str) -> Result<String> {
    let repo = url
        .split('/')
        .last()
        .ok_or_else(|| anyhow!("could not parse url {}", url))?;
    if open(url).is_err() {
        println!("fork the repository at {}", url);
    }
    let mut owner = String::new();
    println!("Enter the name of the owner of the fork: ");
    std::io::stdin().read_line(&mut owner)?;
    let owner = owner.trim();
    Ok(format!("https://www.github.com/{owner}/{repo}"))
}

fn insert_patch(manifest: &mut Document, path: &Path, dep: String) -> Result<()> {
    let patch = manifest
        .as_table_mut()
        .entry("patch")
        .or_insert_with(|| Item::Table(Table::new()))
        .as_table_mut()
        .ok_or_else(|| anyhow!("patch is not a Table"))?;
    patch.set_implicit(true);
    let crates_io = patch
        .entry("crates-io")
        .or_insert_with(|| Item::Table(Table::new()))
        .as_table_mut()
        .ok_or_else(|| anyhow!("crates-io is not a Table"))?;
    let dependency = crates_io
        .entry(&dep)
        .or_insert_with(|| Item::Value(InlineTable::new().into()))
        .as_inline_table_mut()
        .ok_or_else(|| anyhow!("dependency is not an InlineTable"))?;
    let path_entry = dependency
        .entry("path")
        .or_insert_with(|| InlineTable::new().into());
    *path_entry = path
        .to_str()
        .ok_or_else(|| anyhow!("Could not write patch path to file"))?
        .into();
    Ok(())
}

fn read_manifest(manifest_path: &Path) -> Result<toml_edit::Document> {
    let data = fs::read_to_string(&manifest_path)?;
    Ok(data.parse()?)
}

fn get_repo(workspace: &Workspace, dependency: &str) -> Result<String> {
    let config = workspace.config();
    let lockfile = match load_pkg_lockfile(workspace)? {
        Some(lockfile) => lockfile,
        None => {
            generate_lockfile(workspace)?;
            load_pkg_lockfile(workspace)?.ok_or_else(|| anyhow!("Failed to generate lockfile"))?
        }
    };
    for package in workspace.members() {
        let package_id = package.package_id();
        for (dep_id, _) in lockfile
            .deps(package_id)
            .filter(|(id, _)| id.name().as_str() == dependency)
        {
            let mut sources = SourceMap::new();
            sources.insert(dep_id.source_id().load(config, &HashSet::new())?);
            let deps = [dep_id];
            let pkg_set = PackageSet::new(&deps, sources, config)?;
            let package = pkg_set.get_one(dep_id)?;
            if let Some(repo) = &package.manifest().metadata().repository {
                return Ok(repo.clone());
            }
        }
    }
    Err(anyhow!("Could not find use of dependency {}", dependency))
}
