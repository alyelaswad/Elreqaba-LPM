// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::env;
use std::{thread, time};
use sysinfo::System;
// struct ProcInfo {
//     id: String,
//     name: String,
//     cpu_usage: f32,
//     memory: u64,
//     status: String,
// }

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

    match args[1].as_str() {
        "get_os" => get_os(),
        "ptable" => ptable(),
        _ => eprintln!("Unknown command: {}", args[1]),
    }
}
