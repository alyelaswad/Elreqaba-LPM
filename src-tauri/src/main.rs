// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;

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
        _ => eprintln!("Unknown command: {}", args[1]),
    }
}
