mod args;
mod driver;
mod greet;
mod utils;

use crate::args::Args;
use crate::driver::{kill_pid, load_driver, unload_driver};
use crate::utils::check_pid;
use anyhow::anyhow;
use clap::Parser;
use std::ffi::CStr;
use std::ops::Not;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::mpsc::SyncSender;
use std::{fs, process};

fn init(sx: SyncSender<bool>) -> anyhow::Result<PathBuf> {
    let driver = include_bytes!("../driver/superman.sys");

    greet::greeting();

    ctrlc::set_handler(move || {
        println!("[+]Bye!");
        sx.send(true).unwrap();
    })?;

    let mut path = dirs::cache_dir().unwrap_or("C:\\Windows".into());
    path.push("Temp");

    if path.exists().not() {
        fs::create_dir_all(&path)?;
    }

    path.push("superman");

    if path.exists().not() {
        fs::write(&path, driver)?;
    }

    Ok(path)
}

fn main() {
    let args = Args::parse();

    if let Err(e) = try_main(&args) {
        eprintln!("{}", e);
        process::exit(1);
    }
}

fn try_main(args: &Args) -> anyhow::Result<()> {
    let (sx, rx) = mpsc::sync_channel(1);

    let path = init(sx)?;
    let service_name: &CStr = CStr::from_bytes_with_nul(b"superman\0").unwrap();

    let driver: (&Path, &CStr) = (&path, service_name);

    if check_pid(args.pid).not() {
        return Err(anyhow!("[-]Process not exists!"));
    }

    load_driver(driver)?;

    kill_pid(args, driver, rx)?;

    unload_driver(driver).unwrap();
    Ok(())
}
