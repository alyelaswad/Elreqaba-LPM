#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::{env, fs::File, io::Write};
use std::{thread, time};
use cursive::Cursive;
use cursive::CursiveExt;
use cursive::views::{Dialog, TextView, LinearLayout, ScrollView, ListView};
use sysinfo::System;

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
                Ok(mut file) => {
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
        println!("[...]")
    }
}

fn get_os() {
    let os = env::consts::OS;
    println!("Your OS is: {}", os);
}

fn start_tui(processes: Vec<(String, String)>) {
    let mut siv = Cursive::new();

    // Create a ListView to display processes
    let mut list_view = ListView::new();

    for (pid, name) in processes {
        list_view.add_child(
            &format!("PID: {}", pid),
            TextView::new(format!("Name: {}", name)),
        );
    }

    siv.add_layer(
        Dialog::new()
            .title("System Processes")
            .content(ScrollView::new(list_view))
            .button("Quit", |s| s.quit()),
    );

    siv.run();
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
        "tui" => {
    let mut system = System::new_all();
    system.refresh_all();

    // Collect process data: PID and Name
    let processes: Vec<(String, String)> = system
        .processes()
        .iter()
        .map(|(pid, process)| (pid.to_string(), process.name().to_string_lossy().into_owned()))
        .collect();

    start_tui(processes); // Pass the process list to the TUI function
}

        _ => eprintln!("Unknown command: {}", args[1]),
    }
}
