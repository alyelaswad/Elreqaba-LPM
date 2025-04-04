use std::{env, fs::File, io::Write};
use std::{thread, time};
use sysinfo::{System, Process};
mod TUI;
use TUI::display_tui;
use std::io;

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
                        "PID", "Process Name", "CPU (%)", "Memory (KB)", "Status"
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
            "PID", "Process Name", "CPU (%)", "Memory (KB)", "Status"
        );
        println!("{}", "-".repeat(75));
        for (id, process) in &processes {
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

fn tui() {
    let mut system = sysinfo::System::new_all();
    system.refresh_all();

    // Convert sysinfo::Process to TUI::Process
    let processes: Vec<TUI::Process> = system
        .processes()
        .iter()
        .map(|(_, process)| TUI::Process {
            pid: process.pid().as_u32(),
            cpu: process.cpu_usage(),
            mem: process.memory() as f32 / 1024.0, // Convert memory to MB
            cmd: process.name().to_string_lossy().into_owned(),
        })
        .collect();

    // Define which columns to display in the TUI
    let columns_to_display = vec!["PID".into(), "CPU".into(), "MEM".into(), "CMD".into()];

    // Display the TUI
    TUI::display_tui(columns_to_display, processes);
}


fn main() {
    loop {
        let mut command = String::new();

        println!("Enter command (or type 'exit' to quit):");
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
                let file_path = parts.get(1).map(|s| *s);
                ptable(file_path);
            }
            Some(&"kill") => {
                if let Some(&pid) = parts.get(1) {
                    kill_by_pid(pid.to_string());
                } else {
                    eprintln!("Usage: kill <pid>");
                }
            }
            Some(&"tui") => tui(),
            Some(cmd) => eprintln!("Unknown command: {}", cmd),
            None => continue,
        }
    }
}
