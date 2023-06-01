use sysinfo::{Pid, PidExt, ProcessExt, System, SystemExt};

/// Check if process exists
pub fn check_pid(pid: u32) -> bool {
    let mut system = System::new();
    system.refresh_process(Pid::from_u32(pid))
}

/// Get process name by pid
pub fn get_process_name(pid: u32) -> String {
    let mut system = System::new();
    system.refresh_processes();
    system
        .process(Pid::from_u32(pid))
        .unwrap()
        .name()
        .to_owned()
}

/// Get process pid by name
pub fn get_process_pid(name: &str) -> Option<u32> {
    let mut system = System::new();
    system.refresh_processes();
    let x = system
        .processes_by_exact_name(name)
        .next()
        .map(|x| x.pid().as_u32());

    x
}
