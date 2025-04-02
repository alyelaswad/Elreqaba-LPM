// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::{env, fs::File, io::Write};
use std::{thread, time};
use sysinfo::System;
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
    let found: bool = false;
    for (id, process) in processes {
        if id == pid {
            process.kill();
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

fn get_os() {
    let os = env::consts::OS;
    println!("Your OS is: {}", os);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: cargo run -- <command>");
        return;
    }
    if args[1].as_str() == "kill" && args.len() < 3 {
        eprintln!("Usage: cargo run -- kill <pid>");
        return;
    }

    match args[1].as_str() {
        "get_os" => get_os(),
        "ptable" => ptable(),
        "kill" => kill_by_pid(args[2].to_string()),
        _ => eprintln!("Unknown command: {}", args[1]),
    }
}
