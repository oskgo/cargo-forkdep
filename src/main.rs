use anyhow::{anyhow, Result};
use cargo::{
    core::{package_id, EitherManifest, Manifest, PackageSet, SourceId, SourceMap},
    ops::{generate_lockfile, load_pkg_lockfile},
    sources::{GitSource, RegistrySource},
    util::{config::Config, important_paths::find_root_manifest_for_wd, toml::TomlManifest},
};
use clap::Parser;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
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
    let mut workspace = cargo::core::Workspace::new(&manifest_path, &config)?;
    let config = workspace.config().to_owned();
    let lockfile = match load_pkg_lockfile(&workspace)? {
        Some(lockfile) => lockfile,
        None => {
            generate_lockfile(&workspace)?;
            load_pkg_lockfile(&workspace)?.ok_or_else(|| anyhow!("Failed to generate lockfile"))?
        }
    };
    let dep_repo: Option<String> = None;
    for package in workspace.members_mut() {
        let package_id = package.package_id();
        for (dep, _) in lockfile
            .deps(package_id)
            .filter(|(id, _)| id.name().as_str() == &args.dependency)
        {
            let mut sources = SourceMap::new();
            sources.insert(dep.source_id().load(config, &HashSet::new())?);
            let deps = [dep];
            let pkg_set = PackageSet::new(&deps, sources, config)?;
            let package = pkg_set.get_one(dep)?;
            package.manifest().metadata().repository.as_ref().ok_or_else(|| anyhow!("No repository in manifest of {}", dep.name()))?;
        }
    }
    Ok(())
}
