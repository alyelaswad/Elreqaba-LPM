use std::{env, fs::File, io::Write};
use std::{thread, time};
use std::process::Command;
use sysinfo::System;
mod TUI;
use std::io;
use users::get_user_by_uid;
use nix::sys::signal::{kill, Signal};
use chrono::{Local}; 
use sysinfo::{Pid};

fn log_by_pid(pid_str: String) {
    let mut system = System::new_all();
    system.refresh_all();
    thread::sleep(time::Duration::from_secs(1));
    system.refresh_all();

    let pid = match pid_str.parse::<u32>() {
        Ok(pid) => Pid::from(pid as usize),
        Err(_) => {
            println!("Invalid PID string: {}", pid_str);
            return;
        }
    };

    let processes: Vec<_> = system
        .processes()
        .iter()
        .map(|(id, process)| (id.to_string(), process))
        .collect();

    let mut found = false;
    let mut log = vec![];

    for (id, process) in processes {
        if id == pid_str {
            found = true;
            let creation_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            log.push(format!("[{}] Process {} created", creation_time, pid));

            let mut last_state = process.status().to_string();

            loop {
                thread::sleep(time::Duration::from_secs(1));
                system.refresh_all(); 
                if let Some(process) = system.process(pid) {
                    let current_state = process.status().to_string();

                    if current_state != last_state {
                        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                        log.push(format!("[{}] Status of Process {} changed to: {}", timestamp, pid, current_state));
                        last_state = current_state;
                        break;
                    }
                } else {
                    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                    log.push(format!("[{}] Process {} terminated", timestamp, pid));
                    break;
                }
            }
            break;
        }
    }

    if !found {
        println!("The Process was not found, recheck the PID");
    } else {
        for entry in log {
            println!("{}", entry);
        }
    }
}
fn kill_by_pid(pid: String) {
    let mut system = System::new_all();
    system.refresh_all();
    thread::sleep(time::Duration::from_secs(1));
    system.refresh_all();

    let processes: Vec<_> = system
        .processes()
        .iter()
        .map(|(id, process)| (id.to_string(), process))
        .collect();

    let mut found = false;
    for (id, process) in processes {
        if id == pid {
            process.kill();
            found = true;
            println!(
                " {} was killed, PID: {}",
                process.name().to_string_lossy(),
                id
            );
            break;
        }
    }
    if !found {
        println!("The Process was not found, recheck the PID");
    }
}
fn ptable(file_path: Option<&str>) {
    let mut system = System::new_all();

    system.refresh_all();
    thread::sleep(time::Duration::from_secs(1));
    system.refresh_all();

    let mut processes: Vec<_> = system
        .processes()
        .iter()
        .map(|(id, process)| (id.to_string(), process))
        .collect();

    processes.sort_by(|a, b| b.1.cpu_usage().partial_cmp(&a.1.cpu_usage()).unwrap());

    if let Some(path) = file_path {
        if path.ends_with(".csv") {
            match File::create(path) {
                Ok(mut file) => {
                    writeln!(
                        file,
                        "{},{},{},{},{}",
                        "PID", "Process Name", "CPU (%)", "Memory (MB)", "Status"
                    )
                    .unwrap();

                    for (id, process) in &processes {
                        writeln!(
                            file,
                            "{},{},{:.2},{},{}",
                            id,
                            process.name().to_string_lossy(),
                            process.cpu_usage(),
                            process.memory(),
                            format!("{:?}", process.status())
                        )
                        .unwrap();
                    }

                    println!("Exported process table to: {}", path);
                }
                Err(e) => {
                    eprintln!("Failed to create file {}: {}", path, e);
                }
            }
        } else {
            eprintln!("Error: Please provide a .csv file path.");
        }
    } else {
        println!(
            "{:<10} {:<45} {:<10} {:<15} {:<10}",
            "PID", "Process Name", "CPU (%)", "Memory (MB)", "Status"
        );
        println!("{}", "-".repeat(75));
        for (id, process) in &processes {
            println!(
                "{:<10} {:<45} {:<10.2} {:<15} {:<10}",
                id,
                process.name().to_string_lossy(),
                process.cpu_usage(),
                process.memory()/1024,
                format!("{:?}", process.status())
            );
        }
    }
}
fn track_process(pid: String, path: String, duration_secs: u64) {
    let mut system = System::new_all();
    let mut file = match File::create(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to create file {}: {}", path, e);
            return;
        }
    };

    writeln!(file, "Timestamp,CPU (%),Memory (KB)").unwrap();

    let start_time = time::Instant::now();
    let mut total_cpu: f32 = 0.0;
    let mut total_memory: u64 = 0;
    let mut count: u64 = 0;

    while start_time.elapsed().as_secs() < duration_secs {
        system.refresh_all();
        if let Some(process) = system.processes().get(&pid.parse().unwrap()) {
            total_cpu += process.cpu_usage();
            total_memory += process.memory();
            count += 1;

            writeln!(
                file,
                "{}, {:.2}, {}",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
                process.cpu_usage(),
                process.memory()
            )
            .unwrap();
        } else {
            println!("Process {} not found. Stopping monitoring.", pid);
            break;
        }
        thread::sleep(time::Duration::from_secs(1));
    }
    if count > 0 {
        let avg_cpu = total_cpu / count as f32;
        let avg_memory = total_memory / count;

        println!(
            "Tracking complete. Data saved to {}",
            path
        );
        println!("Average CPU Usage: {:.2}%", avg_cpu);
        println!("Average Memory Usage: {} KB", avg_memory);
    } else {
        println!("No data collected. The process may not have been available.");
    }
}
fn get_process_command(pid: u32) -> String {
  
    let output = Command::new("ps")
        .arg("-p")
        .arg(pid.to_string())
        .arg("-o")
        .arg("command=")  
        .output()
        .expect("Failed to fetch process command");

    if output.status.success() {
        
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        String::new()
    }
}
fn get_pid_and_command() -> Vec<(u32, String)> {
    let mut system = System::new_all();
    system.refresh_all();

    let mut pid_and_commands = Vec::new();

    for (pid, process) in system.processes() {
        let cmd = process.cmd().iter()
            .map(|arg| arg.to_string_lossy().into_owned()) 
            .collect::<Vec<String>>()
            .join(" ");
        pid_and_commands.push((
            pid.as_u32(), 
            if cmd.is_empty() { "[no command]".to_string() } else { cmd }
        ));
    }

    pid_and_commands
}
fn restart_if_failed(pid: u32, initial_pids: &Vec<(u32, String)>, current_pids: &Vec<(u32, String)>) {
    if initial_pids.iter().any(|(initial_pid, _)| *initial_pid == pid) {
        if !current_pids.iter().any(|(current_pid, _)| *current_pid == pid) {
            println!("Process {} has stopped. Restarting...", pid);

            let command = initial_pids.iter()
                .find(|(initial_pid, _)| *initial_pid == pid)
                .map(|(_, cmd)| cmd.clone())
                .unwrap_or_else(|| String::new());

            if !command.is_empty() {
                let child = Command::new("xterm")
                    .arg("-e")
                    .arg(command.clone())
                    .spawn();

                match child {
                    Ok(child_proc) => {
                        println!("Restarted process with new PID: {}", child_proc.id());
                    }
                    Err(e) => {
                        eprintln!("Failed to restart process: {}", e);
                    }
                }
            } else {
                eprintln!("Could not retrieve command for PID {}", pid);
            }
        } else {
            println!("Process {} is already running.", pid);
        }
    } else {
        println!("PID {} not found in the initial table.", pid);
    }

    thread::sleep(time::Duration::from_secs(10));
}
fn pause_process(pid: u32) -> bool {
use nix::unistd::Pid;

    kill(Pid::from_raw(pid as i32), Signal::SIGSTOP).is_ok()
}
fn resume_process(pid: u32) -> bool {
    use nix::unistd::Pid;

    kill(Pid::from_raw(pid as i32), Signal::SIGCONT).is_ok()
}
fn get_os() {
    let os = env::consts::OS;
    println!("Your OS is: {}", os);
}
fn tui() {
    let mut system = sysinfo::System::new_all();
    system.refresh_all();

    thread::sleep(std::time::Duration::from_millis(500));
    system.refresh_all();

    let processes: Vec<TUI::Process> = system
        .processes()
        .iter()
        .map(|(pid, process)| {
            let ppid = process.parent().map(|p| p.as_u32());
            let user = match process.user_id() {
                Some(uid) => {
                    let uid_value = **uid;
                    
                    match get_user_by_uid(uid_value) {
                        Some(user) => Some(user.name().to_string_lossy().into_owned()),
                        None => Some(format!("uid:{}", uid_value))
                    }
                },
                None => Some("unknown".to_string())
            };

            // Get the nice value for the process using ps command
            let priority = std::process::Command::new("ps")
                .arg("-o")
                .arg("nice")
                .arg("-p")
                .arg(format!("{}", pid.as_u32()))
                .output()
                .ok()
                .and_then(|output| {
                    if output.status.success() {
                        String::from_utf8_lossy(&output.stdout)
                            .lines()
                            .nth(1)
                            .and_then(|line| line.trim().parse::<i32>().ok())
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            
            TUI::Process {
                pid: pid.as_u32(),
                ppid,
                user,
                cpu: process.cpu_usage(),
                mem: process.memory() as f32 / 1024.0, // Convert memory to MB
                cmd: process.name().to_string_lossy().into_owned(),
                start_time: process.start_time(),
                process_state: process.status(),
                priority,
            }
        })
        .collect();

    // Define which columns to display in the TUI
    let columns_to_display = vec![
        "PID".into(),
        "PPID".into(),
        "USER".into(),
        "CPU".into(),
        "MEM".into(),
        "NI".into(),  // Changed from "PRIORITY" to "NI" to match the column definition
        "CMD".into(),
        "START".into(),
        "STATUS".into(),
    ];

    // Display the TUI
    TUI::display_tui(columns_to_display, processes);
}
fn change_niceness(pid: u32, niceness: i32) {
    let output = Command::new("renice")
        .arg(niceness.to_string())
        .arg("-p")
        .arg(pid.to_string())
        .output()
        .expect("Failed to change niceness");

    if output.status.success() {
        println!("Changed niceness for PID {} to {}", pid, niceness);
    } else {
        eprintln!(
            "Failed to change niceness for PID {}: {}",
            pid,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
fn main() {
    let initial_pids = get_pid_and_command();

    loop {
        let mut command = String::new();

        print!("Enter command (or type 'exit' to quit): ");
        let _ = io::stdout().flush(); // Make sure prompt appears before input
        let _ = io::stdin().read_line(&mut command);

        let command = command.trim();

        if command.eq_ignore_ascii_case("exit") {
            println!("Exited!");
            break;
        }

        let parts: Vec<&str> = command.split_whitespace().collect();

        match parts.get(0) {
            Some(&"get_os") => get_os(),

            Some(&"ptable") => {
                let file_path = parts.get(1).copied();
                ptable(file_path);
            }
            Some(&"change_nice") => {
                if parts.len() < 3 {
                    eprintln!("Usage: change_nice <pid> <niceness>");
                } else {
                    let pid = parts[1].parse::<u32>().unwrap_or(0);
                    let niceness = parts[2].parse::<i32>().unwrap_or(0);
                    if pid == 0 {
                        eprintln!("Invalid PID.");
                    } else {
                        change_niceness(pid, niceness);
                    }
                }
            }
            Some(&"kill") => {
                if let Some(&pid) = parts.get(1) {
                    kill_by_pid(pid.to_string());
                } else {
                    eprintln!("Usage: kill <pid>");
                }
            }
            Some(&"log") => {
                if let Some(&pid) = parts.get(1) {
                    log_by_pid(pid.to_string());
                } else {
                    eprintln!("Usage: log <pid>");
                }
            }
            Some(&"pause") => {
                if let Some(&pid_str) = parts.get(1) {
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        if pause_process(pid) {
                            println!("Paused process with PID {}", pid);
                        } else {
                            eprintln!("Failed to pause process with PID {}", pid);
                        }
                    } else {
                        eprintln!("Invalid PID.");
                    }
                } else {
                    eprintln!("Usage: pause <pid>");
                }
            }

            Some(&"resume") => {
                if let Some(&pid_str) = parts.get(1) {
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        if resume_process(pid) {
                            println!("Resumed process with PID {}", pid);
                        } else {
                            eprintln!("Failed to resume process with PID {}", pid);
                        }
                    } else {
                        eprintln!("Invalid PID.");
                    }
                } else {
                    eprintln!("Usage: resume <pid>");
                }
            }
            Some(&"track_process") => {
                if parts.len() < 4 {
                    eprintln!("Usage: track_process <pid> <output.csv> <duration_secs>");
                } else {
                    let pid = parts[1].to_string();
                    let path = parts[2].to_string();
                    let duration = parts[3].parse::<u64>().unwrap_or(0);
                    if duration == 0 {
                        eprintln!("Invalid duration. Please enter a positive integer.");
                    } else {
                        track_process(pid, path, duration);
                    }
                }
            }

            Some(&"get_process_command") => {
                if let Some(&pid_str) = parts.get(1) {
                    let pid = pid_str.parse::<u32>().unwrap_or(0);
                    if pid == 0 {
                        eprintln!("Invalid PID. Please enter a valid process ID.");
                    } else {
                        let command = get_process_command(pid);
                        if !command.is_empty() {
                            println!("Command for PID {}: {}", pid, command);
                        } else {
                            println!("Failed to retrieve command for PID {}", pid);
                        }
                    }
                } else {
                    eprintln!("Usage: get_process_command <pid>");
                }
            }

            Some(&"restart_if_failed") => {
                if let Some(&pid_str) = parts.get(1) {
                    let pid = pid_str.parse::<u32>().unwrap_or(0);
                    if pid == 0 {
                        eprintln!("Invalid PID. Please enter a valid process ID.");
                    } else {
                        let current_pids = get_pid_and_command();
                        restart_if_failed(pid, &initial_pids, &current_pids);
                    }
                } else {
                    eprintln!("Usage: restart_if_failed <pid>");
                }
            }

            Some(&"tui") => tui(),

            Some(cmd) => eprintln!("Unknown command: {}", cmd),

            None => continue,
        }
    }
}
