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
use std::sync::atomic::Ordering::{Acquire, Release};

const DRIVER: &[u8] = include_bytes!("../driver/superman.sys");
static DRIVER_PATH: OnceCell<PathBuf> = OnceCell::new();
static EXIT: AtomicBool = AtomicBool::new(false);

#[ctor]
fn init() {
    greet::greeting();

    ctrlc::set_handler(|| {
        if EXIT.load(Acquire).not() {
            println!("[+]Bye!");
            EXIT.store(true, Release);
        }
    })
    .unwrap();

    let mut path = dirs::cache_dir().or(Some("C:\\Windows".into())).unwrap();
    path.push("Temp");

    if path.exists().not() {
        fs::create_dir_all(&path).unwrap();
    }

    path.push("superman");
    DRIVER_PATH.set(path).unwrap();
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
    unload_delete_driver(DRIVER_PATH.get().unwrap()).unwrap();
}
