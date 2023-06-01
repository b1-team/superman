mod args;
mod driver;
mod greet;
mod utils;

use crate::args::Args;
use crate::driver::{kill_pid, load_driver, unload_delete_driver};
use crate::utils::check_pid;
use anyhow::anyhow;
use clap::Parser;
use ctor::{ctor, dtor};
use once_cell::sync::OnceCell;
use std::ffi::CString;
use std::fs;
use std::ops::Not;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Release;

const DRIVER: &[u8] = include_bytes!("../driver/superman.sys");
static DRIVER_PATH: OnceCell<PathBuf> = OnceCell::new();
static EXIT: AtomicBool = AtomicBool::new(false);

#[ctor]
fn init() {
    greet::greeting();

    let _ = ctrlc::set_handler(|| {
        println!("[+]Bye!");
        EXIT.store(true, Release);
    });

    let mut path = dirs::cache_dir().unwrap();
    path.push("Temp/superman");
    let _ = DRIVER_PATH.set(path);
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let driver_path = DRIVER_PATH.get().unwrap().to_owned();

    if check_pid(args.pid).not() {
        return Err(anyhow!("[-]Process not exists!"));
    }

    if driver_path.exists().not() {
        fs::write(&driver_path, DRIVER)?;
    }

    load_driver(CString::new(driver_path.to_str().unwrap())?.as_c_str())?;

    kill_pid(args)?;

    Ok(())
}

#[dtor]
fn exit() {
    let _ = unload_delete_driver(DRIVER_PATH.get().unwrap());
}
