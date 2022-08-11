use anyhow::{anyhow, Result};
use cargo::{
    core::{package_id, EitherManifest, Manifest, PackageSet, SourceId, SourceMap, Dependency, Package, Workspace},
    ops::{generate_lockfile, load_pkg_lockfile},
    sources::{GitSource, RegistrySource},
    util::{config::Config, important_paths::find_root_manifest_for_wd, toml::TomlManifest},
};
use clap::Parser;
use toml_edit::Item;
use std::{
    collections::HashSet,
    path::{Path, PathBuf}, fs, str::FromStr,
};

#[derive(Parser, Debug)] // requires `derive` feature
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
        .map(|v| Ok(v))
        .unwrap_or_else(|| find_root_manifest_for_wd(&std::env::current_dir()?))?;
    let mut workspace = Workspace::new(&manifest_path, &config)?;
    let repo = get_repo(&mut workspace, &args.dependency)?;
    let mut manifest = dbg!(read_manifest(manifest_path)?);
    manifest["patch"]["crates.io"][args.dependency]["git"] = <Item as FromStr>::from_str(&repo)?;
    Ok(())
}

fn read_manifest(manifest_path: PathBuf) -> Result<toml_edit::Document> {
    let data = fs::read_to_string(&manifest_path)?;
    Ok(data.parse()?)
}

fn get_repo(workspace: &Workspace, dependency: &str) -> Result<String> {
    let config = workspace.config();
    let lockfile = match load_pkg_lockfile(&*workspace)? {
        Some(lockfile) => lockfile,
        None => {
            generate_lockfile(&*workspace)?;
            load_pkg_lockfile(&*workspace)?.ok_or_else(|| anyhow!("Failed to generate lockfile"))?
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
                return Ok(repo.clone())
            }
        }
    }
    Err(anyhow!("Could not find use of dependency {}", dependency))
}
