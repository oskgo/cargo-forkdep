use std::{path::Path, fs};
use anyhow::{Result, Context};

fn main() -> Result<()> {
    println!("Enter a github personal access token: ");
    let mut token = String::new();
    std::io::stdin().read_line(&mut token).context("failed to read user input")?;
    fs::write(Path::new("token.txt"), token).context("could not write token to file")?;
    Ok(())
}