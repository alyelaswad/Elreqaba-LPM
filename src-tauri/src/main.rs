// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::{env, fs::File, io::Write};
use std::{thread, time,process::Command};
use sysinfo::{System};  


// struct ProcInfo {
//     id: String,
//     name: String,
//     cpu_usage: f32,
//     memory: u64,
//     status: String,
// }
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
    let mut found: bool = false;
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

fn ptable() {
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

    let args: Vec<String> = env::args().collect();
    if args.len() >= 3 {
        let file_path = &args[2];
        if file_path.ends_with(".csv") {
            let file = File::create(file_path);
            match file {
                // match is like a switch statement in c++
                Ok(mut file) => {
                    // Write CSV header
                    writeln!(
                        file,
                        "{},{},{},{},{}",
                        "PID", "Process Name", "CPU (%)", "Memory (KB)", "Status"
                    )
                    .unwrap();

                    for (id, process) in processes {
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

                    println!("Exported process table to: {}", file_path);
                }
                Err(e) => {
                    eprintln!("Failed to create file {}: {}", file_path, e);
                }
            }
        } else {
            eprintln!("Error: Please provide a .csv file path.");
        }
    } else {
        println!(
            "{:<10} {:<45} {:<10} {:<15} {:<10}",
            "PID", "Process Name", "CPU (%)", "Memory (KB)", "Status"
        );
        println!("{}", "-".repeat(75));
        for (id, process) in processes {
            println!(
                "{:<10} {:<45} {:<10.2} {:<15} {:<10}",
                id,
                process.name().to_string_lossy(),
                process.cpu_usage(),
                process.memory(),
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

fn get_os() {
    let os = env::consts::OS;
    println!("Your OS is: {}", os);
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

fn restart_if_failed(pid: u32, initial_pids: &Vec<(u32, String)>, current_pids: &Vec<(u32, String)>) {
    if initial_pids.iter().any(|(initial_pid, _)| *initial_pid == pid) {
        if !current_pids.iter().any(|(current_pid, _)| *current_pid == pid) {
            println!("Process {} has stopped. Restarting...", pid);
            let command = get_process_command(pid);
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
    }thread::sleep(time::Duration::from_secs(10));
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



fn main() {
    let initial_pids = get_pid_and_command();
    let args: Vec<String> = env::args().collect();
    for (pid, command) in &initial_pids { 
        println!("PID: {} - Command: {}", pid, command);
    }

    if args.len() < 2 {
        eprintln!("Usage: cargo run -- <command>");
        return;
    }

    match args[1].as_str() {
        "get_os" => get_os(),
        "ptable" => ptable(),
        "kill" => {
            if args.len() < 3 {
                eprintln!("Usage: cargo run -- kill <pid>");
                return;
            }
            kill_by_pid(args[2].to_string());
        }
        "track_process" => {
            if args.len() < 5 {
                eprintln!("Usage: cargo run -- track_process <pid> <output.csv> <duration_secs>");
                return;
            }
            let pid = args[2].to_string();
            let path = args[3].to_string();
            let duration = args[4].parse::<u64>().unwrap_or(0);
            if duration == 0 {
                eprintln!("Invalid duration. Please enter a positive integer.");
                return;
            }
            track_process(pid, path, duration);
        }
        "get_process_command" => {
            if args.len() < 3 {
                eprintln!("Usage: cargo run -- get_process_command <pid>");
                return;
            }
            let pid = args[2].parse::<u32>().unwrap_or(0);
            if pid == 0 {
                eprintln!("Invalid PID. Please enter a valid process ID.");
                return;
            }
            let command = get_process_command(pid);
            if !command.is_empty() {
                println!("Command for PID {}: {}", pid, command);
            } else {
                println!("Failed to retrieve command for PID {}", pid);
            }
        }
        "restart_if_failed" => {
            if args.len() < 3 {
                eprintln!("Usage: cargo run -- restart_if_failed <pid>");
                return;
            }
            let pid = args[2].parse::<u32>().unwrap_or(0);
            if pid == 0 {
                eprintln!("Invalid PID. Please enter a valid process ID.");
                return;
            }
            let current_pids = get_pid_and_command();
            restart_if_failed(pid, &initial_pids, &current_pids);
        }
        _ => todo!(), // Handle any other cases with a wildcard
    }
}    