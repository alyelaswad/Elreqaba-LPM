use cursive::align::HAlign;
use cursive::traits::*;
use cursive::views::{Dialog, TextView, ScrollView, LinearLayout, DummyView, SelectView, EditView};
use cursive::Cursive;
use cursive::CursiveExt;
use cursive::view::Nameable;
use cursive_table_view::{TableView, TableViewItem};
use sysinfo::{ProcessStatus, System};
use std::cmp::Ordering;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::thread;
use std::time::Duration;
use lazy_static::lazy_static;
use num_cpus;
use sysinfo::{Pid, Signal};
use std::io::{BufRead, BufReader};
use std::fs::File;
use std::fs::read_to_string;
use std::collections::HashMap;
use cursive::theme::{BaseColor, Color, ColorStyle, Palette, PaletteColor, Theme, Effect, Style};
use cursive::utils::markup::StyledString;

#[derive(Clone, Debug)]
pub struct Process {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub user: Option<String>,
    pub cpu: f32,
    pub mem: f32,
    pub cmd: String,
    pub start_time: u64,
    pub process_state: ProcessStatus,
    pub priority: i32,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum BasicColumn {
    PID,
    PPID,
    USER,
    CPU,
    MEM,
    CMD,
    START,
    STATUS,
    PRIORITY,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum FilterType {
    PID,
    PPID,
    USER,
    STATUS,
}

impl TableViewItem<BasicColumn> for Process {
    fn to_column(&self, column: BasicColumn) -> String {
        match column {
            BasicColumn::PID => format!("{}", self.pid),
            BasicColumn::PPID => self.ppid.map_or("N/A".to_string(), |ppid| ppid.to_string()),
            BasicColumn::USER => self.user.clone().unwrap_or_else(|| "N/A".to_string()),
            BasicColumn::CPU => format!("{:.2}", self.cpu),
            BasicColumn::MEM => format!("{:.2}", self.mem/1024.0),
            BasicColumn::CMD => self.cmd.clone(),
            BasicColumn::START => {
                // Convert start_time to a readable format
                let datetime = chrono::DateTime::from_timestamp(self.start_time as i64, 0)
                    .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
                format!("{}", datetime.format("%H:%M:%S"))
            }
            BasicColumn::STATUS => format!("{:?}", self.process_state),
            BasicColumn::PRIORITY => format!("{}", self.priority),
        }
    }

    fn cmp(&self, other: &Self, column: BasicColumn) -> Ordering {
        match column {
            BasicColumn::PID => self.pid.cmp(&other.pid),
            BasicColumn::PPID => self.ppid.cmp(&other.ppid),
            BasicColumn::USER => self.user.cmp(&other.user),
            BasicColumn::CPU => self.cpu.partial_cmp(&other.cpu).unwrap_or(Ordering::Equal),
            BasicColumn::MEM => self.mem.partial_cmp(&other.mem).unwrap_or(Ordering::Equal),
            BasicColumn::CMD => self.cmd.cmp(&other.cmd),
            BasicColumn::START => self.start_time.cmp(&other.start_time),
            BasicColumn::STATUS => format!("{:?}", self.process_state).cmp(&format!("{:?}", other.process_state)),
            BasicColumn::PRIORITY => self.priority.cmp(&other.priority),
        }
    }
}

// Atomic flags as static variables
static TUI_RUNNING: AtomicBool = AtomicBool::new(true);
static UPDATES_PAUSED: AtomicBool = AtomicBool::new(false);

// Add filter state tracking
#[derive(Clone)]
struct FilterState {
    filter_type: Option<FilterType>,
    filter_value: String,
}

impl Default for FilterState {
    fn default() -> Self {
        FilterState {
            filter_type: None,
            filter_value: String::new(),
        }
    }
}

// Create a singleton for the System and filter state
lazy_static! {
    static ref SYSTEM: Mutex<System> = Mutex::new(System::new_all());
    static ref CURRENT_FILTER: Mutex<FilterState> = Mutex::new(FilterState::default());
}

#[derive(Clone)]
struct SysStats {
    cpu_freq: u64,
    cpu_name: String,
    cpu_temp: f32,
    cpu_cores_num: usize,
    uptime: u64,
    mem_total: u64,
    user_proc_count: usize,
}

impl Default for SysStats {
    fn default() -> Self {
        SysStats {
            cpu_freq: 0,
            cpu_name: String::new(),
            cpu_temp: 0.0,
            cpu_cores_num: num_cpus::get(),
            uptime: 0,
            mem_total: 0,
            user_proc_count: 0,
        }
    }
}

// Add a static flag to track if the tree view is open
static TREE_VIEW_OPEN: AtomicBool = AtomicBool::new(false);

fn act_on_selected_process<F>(siv: &mut Cursive, action: F, action_name: &str)
where
    F: Fn(&sysinfo::Process) -> bool + Send + 'static + Clone,
{
    if let Some(table) = siv.find_name::<TableView<Process, BasicColumn>>("table") {
        if let Some(selected_row) = table.item() {
            if let Some(process) = table.borrow_item(selected_row) {
                let pid = process.pid;
                let cmd = process.cmd.clone();
                let action_name = action_name.to_string();
                
                siv.add_layer(
                    Dialog::text(format!("Are you sure you want to {} process {} ({})?", action_name, pid, cmd))
                        .button("Yes", move |s| {
                            s.pop_layer();
                            let sink = s.cb_sink().clone();
                            let action_clone = action.clone();
                            let cmd_clone = cmd.clone();
                            let action_name_clone = action_name.clone();
                            
                            thread::spawn(move || {
                                let mut system = SYSTEM.lock().unwrap();
                                system.refresh_processes_specifics(
                                    sysinfo::ProcessesToUpdate::Some(&[Pid::from(pid as usize)]),
                                    true,
                                    sysinfo::ProcessRefreshKind::everything(),
                                );

                                if let Some(sys_proc) = system.process(Pid::from(pid as usize)) {
                                    let result = action_clone(sys_proc);
                                    let msg = if result {
                                        format!("{} signal sent to PID {} ({})", action_name_clone, pid, cmd_clone)
                                    } else {
                                        format!("Failed to send {} signal to PID {} ({})", action_name_clone, pid, cmd_clone)
                                    };
                                    
                                    thread::sleep(Duration::from_millis(100));
                                    system.refresh_all();
                                    drop(system);

                                    let sink_clone = sink.clone();
                                    sink.send(Box::new(move |s| {
                                        s.add_layer(Dialog::info(msg).button("OK", |s| { s.pop_layer(); }));
                                        
                                        thread::spawn(move || {
                                            thread::sleep(Duration::from_millis(200));
                                            sink_clone.send(Box::new(move |s| {
                                                if let Some(mut table_view) = s.find_name::<TableView<Process, BasicColumn>>("table") {
                                                    table_view.set_items(get_processes());
                                                }
                                            })).ok();
                                        });
                                    })).ok();
                                } else {
                                    sink.send(Box::new(move |s| {
                                        s.add_layer(Dialog::info(format!("Process {} ({}) not found.", pid, cmd_clone)));
                                    })).ok();
                                }
                            });
                        })
                        .button("No", |s| { s.pop_layer(); })
                );
            }
        } else {
            siv.add_layer(Dialog::info("No process selected. Please select a process first."));
        }
    }
}

// Helper functions for nice value management
fn get_process_nice(pid: u32) -> Option<i32> {
    match read_to_string(format!("/proc/{}/stat", pid)) {
        Ok(contents) => {
            let fields: Vec<&str> = contents.split_whitespace().collect();
            fields.get(18).and_then(|&nice| nice.parse::<i32>().ok())
        }
        Err(_) => None
    }
}

fn get_current_user() -> Option<String> {
    std::process::Command::new("whoami")
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn needs_sudo(nice_value: i32, process_user: &Option<String>, current_nice: Option<i32>) -> bool {
    let current_user = get_current_user();
    nice_value < 0 || 
        process_user.as_ref().map(|u| Some(u) != current_user.as_ref()).unwrap_or(true) ||
        current_nice.map(|n| nice_value < n).unwrap_or(false)
}

fn execute_renice(pid: u32, nice_value: i32, needs_sudo: bool) -> std::io::Result<std::process::Output> {
    let mut cmd = if needs_sudo {
        let mut c = std::process::Command::new("sudo");
        c.arg("renice");
        c
    } else {
        std::process::Command::new("renice")
    };
    
    cmd.arg(format!("{}", nice_value))
        .arg("-p")
        .arg(format!("{}", pid))
        .output()
}

fn create_nice_values_list() -> Vec<(String, i32)> {
    (-20..=19)
        .map(|n| (format!("{:3} {}", n, 
            if n < 0 { "(Higher Priority - Requires Root)" }
            else if n > 0 { "(Lower Priority)" }
            else { "(Default)" }), n))
        .collect()
}

fn handle_priority_change(
    s: &mut Cursive,
    pid: u32,
    cmd: &str,
    nice_value: i32,
    current_nice: Option<i32>,
    needs_sudo: bool
) {
    match execute_renice(pid, nice_value, needs_sudo) {
        Ok(output) if output.status.success() => {
            thread::sleep(Duration::from_millis(100));
            match get_process_nice(pid) {
                Some(new_nice) if new_nice == nice_value => {
                    show_success_dialog(s, current_nice.unwrap_or(0), new_nice, pid, cmd, needs_sudo);
                }
                Some(new_nice) => {
                    show_verification_failed_dialog(s, current_nice.unwrap_or(0), nice_value, new_nice, needs_sudo);
                }
                None => {
                    s.add_layer(Dialog::info("Failed to verify new nice value").button("OK", |s| { s.pop_layer(); }));
                }
            }
        }
        Ok(output) => {
            show_error_dialog(s, &String::from_utf8_lossy(&output.stderr), needs_sudo);
        }
        Err(e) => {
            show_error_dialog(s, &e.to_string(), needs_sudo);
        }
    }
}

fn show_success_dialog(s: &mut Cursive, old_nice: i32, new_nice: i32, pid: u32, cmd: &str, used_sudo: bool) {
    s.add_layer(Dialog::info(format!(
        "Priority successfully changed from {} to {} for process {} ({})\nUsed {} renice",
        old_nice, new_nice, pid, cmd,
        if used_sudo { "sudo" } else { "regular" }
    )).button("OK", |s| {
        s.pop_layer();
        if let Some(mut table_view) = s.find_name::<TableView<Process, BasicColumn>>("table") {
            let updated_processes = get_processes();
            table_view.set_items(updated_processes);
        }
    }));
}

fn show_verification_failed_dialog(s: &mut Cursive, old_nice: i32, requested_nice: i32, current_nice: i32, needs_sudo: bool) {
    s.add_layer(Dialog::info(format!(
        "Priority change verification failed.\nPrevious: {}\nRequested: {}\nCurrent: {}\n{}",
        old_nice, requested_nice, current_nice,
        if needs_sudo && current_nice != requested_nice {
            "This might be due to insufficient permissions. Try with sudo."
        } else {
            "The change was not applied as expected."
        }
    )).button("OK", |s| { s.pop_layer(); }));
}

fn show_error_dialog(s: &mut Cursive, error_msg: &str, needs_sudo: bool) {
    s.add_layer(Dialog::info(format!(
        "Failed to change priority: {}\nNote: {} privileges are required for this operation.",
        error_msg,
        if needs_sudo { "Root" } else { "Sufficient" }
    )).button("OK", |s| { s.pop_layer(); }));
}

fn renice_process(siv: &mut Cursive) {
    if let Some(table) = siv.find_name::<TableView<Process, BasicColumn>>("table") {
        if let Some(selected_row) = table.item() {
            if let Some(process) = table.borrow_item(selected_row) {
                let pid = process.pid;
                let cmd = process.cmd.clone();
                let process_user = process.user.clone();
                let current_nice = get_process_nice(pid);
                
                let dialog = Dialog::around(
                    LinearLayout::vertical()
                        .child(TextView::new(format!(
                            "Change priority for process {} ({}) owned by {}\nCurrent nice value: {}", 
                            pid, cmd, process_user.as_ref().map(|s| s.as_str()).unwrap_or("unknown"),
                            current_nice.unwrap_or(0))))
                        .child(DummyView)
                        .child(TextView::new("Select nice value:"))
                        .child(ScrollView::new(
                            SelectView::new()
                                .with_all(create_nice_values_list())
                                .on_submit({
                                    let process_user = process_user.clone();
                                    let cmd = cmd.clone();
                                    move |s, &nice_value| {
                                        let needs_sudo = needs_sudo(nice_value, &process_user, current_nice);
                                        handle_priority_change(s, pid, &cmd, nice_value, current_nice, needs_sudo);
                                    }
                                })
                        ).fixed_height(15))
                )
                .title("Change Process Priority")
                .button("Cancel", |s| { s.pop_layer(); });
                
                siv.add_layer(dialog);
            }
        } else {
            siv.add_layer(Dialog::info("No process selected. Please select a process first."));
        }
    }
}

fn get_processes() -> Vec<Process> {
    let mut system = SYSTEM.lock().unwrap();
    system.refresh_all();

    let processes: Vec<Process> = system
        .processes()
        .iter()
        .map(|(pid, process)| {
            let ppid = process.parent().map(|p| p.as_u32());
            
            let user = match process.user_id() {
                Some(uid) => {
                    let uid_value = **uid;
                    match users::get_user_by_uid(uid_value) {
                        Some(user) => Some(user.name().to_string_lossy().into_owned()),
                        None => Some(format!("uid:{}", uid_value))
                    }
                },
                None => Some("unknown".to_string())
            };

            // Get the nice value directly from /proc/[pid]/stat
            let priority = get_process_nice(pid.as_u32()).unwrap_or(0);
            
            Process {
                pid: pid.as_u32(),
                ppid,
                user,
                cpu: process.cpu_usage(),
                mem: process.memory() as f32 / 1024.0,
                cmd: process.name().to_string_lossy().into_owned(),
                start_time: process.start_time(),
                process_state: process.status(),
                priority,
            }
        })
        .collect();

    processes
}

fn verify_nice_value(pid: u32) -> Option<i32> {
    // Use the same method as get_process_nice to verify the nice value
    get_process_nice(pid)
}

// Add this function to get real-time CPU frequency
fn get_cpu_frequencies() -> Vec<f64> {
    let mut frequencies = Vec::new();
    let cpu_count = num_cpus::get();

    for cpu_id in 0..cpu_count {
        let freq_file = format!("/sys/devices/system/cpu/cpu{}/cpufreq/scaling_cur_freq", cpu_id);
        if let Ok(file) = File::open(&freq_file) {
            let reader = BufReader::new(file);
            if let Some(Ok(line)) = reader.lines().next() {
                if let Ok(freq) = line.trim().parse::<u64>() {
                    // Convert KHz to GHz
                    frequencies.push(freq as f64 / 1_000_000.0);
                }
            }
        }
    }

    // Fallback to /proc/cpuinfo if cpufreq is not available
    if frequencies.is_empty() {
        if let Ok(file) = File::open("/proc/cpuinfo") {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if line.starts_with("cpu MHz") {
                        if let Some(freq_str) = line.split(':').nth(1) {
                            if let Ok(freq) = freq_str.trim().parse::<f64>() {
                                frequencies.push(freq / 1000.0); // Convert MHz to GHz
                            }
                        }
                    }
                }
            }
        }
    }

    frequencies
}

fn get_system_info() -> String {
    let mut system = SYSTEM.lock().unwrap();
    system.refresh_all();

    // Get average CPU frequency across all cores
    let freq = std::fs::read_to_string("/proc/cpuinfo")
        .map(|content| {
            let freqs: Vec<f64> = content.lines()
                .filter(|line| line.starts_with("cpu MHz"))
                .filter_map(|line| line.split(':').nth(1)?.trim().parse().ok())
                .collect();
            freqs.iter().sum::<f64>() / freqs.len().max(1) as f64
        })
        .unwrap_or_default();

    // Get CPU name
    let cpu_name = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|content| {
            content.lines()
                .find(|line| line.starts_with("model name"))
                .and_then(|line| line.split(':').nth(1))
                .map(|name| name.trim().to_string())
        })
        .unwrap_or_else(|| "Unknown CPU".to_string());

    let cpu_usage = system.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / system.cpus().len() as f32;
    let total_memory = system.total_memory() as f32 / 1024.0/ 1024.0/ 1024.0;
    let used_memory = system.used_memory() as f32 / 1024.0/ 1024.0/ 1024.0;
    let available_memory = system.available_memory() as f32 / 1024.0/ 1024.0/ 1024.0;
    let uptime = System::uptime();

    format!(
        "CPU Information:\n\
         - Name: {}\n\
         - Current Frequency: {:.0} MHz\n\
         - CPU Usage: {:.1}%\n\
         - Physical Cores: {}\n\
         - Logical Cores: {}\n\
         \n\
         Memory Usage:\n\
         - Total: {:.2} GB\n\
         - Used: {:.2} GB\n\
         - Available: {:.2} GB\n\
         - Usage: {:.1}%\n\
         \n\
         System Information:\n\
         - Number of Processes: {}\n\
         - System Uptime: {}d {}h {}m {}s",
        cpu_name,
        freq,
        cpu_usage,
        num_cpus::get_physical(),
        num_cpus::get(),
        total_memory,
        used_memory,
        available_memory,
        (used_memory / total_memory) * 100.0,
        system.processes().len(),
        uptime / 86400,
        (uptime % 86400) / 3600,
        (uptime % 3600) / 60,
        uptime % 60
    )
}

// Function to get a single-line, full-width system info bar
fn get_system_info_bar(width: usize) -> StyledString {
    let mut system = SYSTEM.lock().unwrap();
    system.refresh_all();
    let cpu_name = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|content| {
            content.lines()
                .find(|line| line.starts_with("model name"))
                .and_then(|line| line.split(':').nth(1))
                .map(|name| name.trim().to_string())
        })
        .unwrap_or_else(|| "Unknown CPU".to_string());
    let cpu_usage = system.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / system.cpus().len() as f32;
    let total_memory = system.total_memory() as f32 / 1024.0 / 1024.0;
    let used_memory = system.used_memory() as f32 / 1024.0 / 1024.0;
    let swap = system.total_swap() as f32 / 1024.0 / 1024.0;
    let uptime = System::uptime();
    let process_count = system.processes().len();
    let freq = std::fs::read_to_string("/proc/cpuinfo")
        .map(|content| {
            let freqs: Vec<f64> = content.lines()
                .filter(|line| line.starts_with("cpu MHz"))
                .filter_map(|line| line.split(':').nth(1)?.trim().parse().ok())
                .collect();
            freqs.iter().sum::<f64>() / freqs.len().max(1) as f64
        })
        .unwrap_or_default();
    let temp = 20.0; // Placeholder, you can add real temp reading if available
    let info = format!(
        "CPU: {} | Usage: {:.1}% | Freq: {:.0}MHz | Mem: {:.1}/{:.1}MB | Swap: {:.1}MB | Uptime: {}d {:02}h {:02}m | Procs: {}",
        cpu_name,
        cpu_usage,
        freq,
        used_memory,
        total_memory,
        swap,
        uptime / 86400,
        (uptime % 86400) / 3600,
        (uptime % 3600) / 60,
        process_count
    );
    let padded = if info.len() < width {
        let mut s = info;
        s.extend(std::iter::repeat(' ').take(width - s.len()));
        s
    } else {
        info.chars().take(width).collect()
    };
    StyledString::styled(
        padded,
        Style::from(ColorStyle::new(Color::Dark(BaseColor::Magenta), Color::Dark(BaseColor::Black))).combine(Effect::Bold)
    )
}

// Function to get a styled keybindings bar
fn get_keybindings_bar() -> StyledString {
    use cursive::utils::markup::StyledString;
    let mut bar = StyledString::new();

    // Helper to style shortcut keys
    let key = |k: &str| StyledString::styled(
        k,
        Style::from(ColorStyle::new(Color::Light(BaseColor::Cyan), Color::Dark(BaseColor::Magenta))).combine(Effect::Bold)
    );
    let text = |t: &str| StyledString::styled(
        t,
        Style::from(ColorStyle::new(Color::Light(BaseColor::White), Color::Dark(BaseColor::Magenta)))
    );

    bar.append_plain("┃ ");
    bar.append(key("Exit <q>"));
    bar.append_plain("  ");
    bar.append(key("Pause/Unpause <u>"));
    bar.append_plain("  ");
    bar.append(key("Process Tree <t>"));
    bar.append_plain("  ");
    bar.append(key("Kill <k>"));
    bar.append_plain("  ");
    bar.append(key("Filter <f>"));
    bar.append_plain("  ");
    bar.append(key("Change Nice <n>"));
    bar.append_plain("  ");
    bar.append(key("Help <h>"));
    bar.append_plain(" ┃");

    bar
}

// Custom theme for a modern TUI look
fn custom_theme() -> Theme {
    let mut theme = Theme::default();
    theme.palette[PaletteColor::Background] = Color::Dark(BaseColor::Black);
    theme.palette[PaletteColor::View] = Color::Dark(BaseColor::Black);
    theme.palette[PaletteColor::Primary] = Color::Light(BaseColor::White);
    theme.palette[PaletteColor::TitlePrimary] = Color::Light(BaseColor::Magenta);
    theme.palette[PaletteColor::Highlight] = Color::Dark(BaseColor::Magenta);
    theme.palette[PaletteColor::HighlightText] = Color::Light(BaseColor::White);
    theme.palette[PaletteColor::Secondary] = Color::Dark(BaseColor::Magenta);
    theme
}

#[derive(Clone)]
struct ProcessNode {
    process: Process,
    children: Vec<ProcessNode>,
}

fn build_process_tree(processes: &[Process]) -> Vec<ProcessNode> {
    // Create a map of pid to process
    let mut pid_map: std::collections::HashMap<u32, Vec<Process>> = std::collections::HashMap::new();
    
    // Group processes by their parent pid
    for process in processes {
        let ppid = process.ppid.unwrap_or(0);
        pid_map.entry(ppid).or_default().push(process.clone());
    }
    
    // Build tree starting from root processes (those with ppid 0 or 1)
    let root_processes = pid_map.remove(&0).unwrap_or_default();
    let mut tree = Vec::new();
    
    for process in root_processes {
        tree.push(build_process_subtree(process, &mut pid_map));
    }
    
    // Handle any remaining processes (in case of orphaned processes)
    for (_, remaining_processes) in pid_map.iter() {
        for process in remaining_processes {
            tree.push(ProcessNode {
                process: process.clone(),
                children: Vec::new(),
            });
        }
    }
    
    tree
}

fn build_process_subtree(process: Process, pid_map: &mut std::collections::HashMap<u32, Vec<Process>>) -> ProcessNode {
    let mut node = ProcessNode {
        process,
        children: Vec::new(),
    };
    
    if let Some(children) = pid_map.remove(&node.process.pid) {
        for child in children {
            node.children.push(build_process_subtree(child, pid_map));
        }
        // Sort children by PID
        node.children.sort_by_key(|child| child.process.pid);
    }
    
    node
}

fn format_process_tree(node: &ProcessNode, prefix: &str, is_last: bool, output: &mut String) {
    let branch = if is_last { "└── " } else { "├── " };
    let next_prefix = if is_last { "    " } else { "│   " };
    
    // Format current process - only show name and PID
    output.push_str(&format!("{}{}{} ({})\n", 
        prefix, 
        branch, 
        node.process.cmd,
        node.process.pid
    ));
    
    // Format children
    for (i, child) in node.children.iter().enumerate() {
        let is_last_child = i == node.children.len() - 1;
        format_process_tree(child, &format!("{}{}", prefix, next_prefix), is_last_child, output);
    }
}

fn show_process_tree_fullscreen(siv: &mut Cursive) {
    let processes = get_processes();
    let tree = build_process_tree(&processes);
    let mut tree_text = String::new();
    for (i, root) in tree.iter().enumerate() {
        format_process_tree(root, "", i == tree.len() - 1, &mut tree_text);
    }
    let tree_view = ScrollView::new(
        TextView::new(tree_text)
            .with_name("tree_content")
            .full_width()
    ).full_screen();
    siv.add_fullscreen_layer(tree_view.with_name("tree_layer"));
    TREE_VIEW_OPEN.store(true, AtomicOrdering::SeqCst);
}

fn close_process_tree_fullscreen(siv: &mut Cursive) {
    // Remove the top layer (tree view)
    siv.pop_layer();
    TREE_VIEW_OPEN.store(false, AtomicOrdering::SeqCst);
}

pub fn display_tui(columns_to_display: Vec<String>, _initial_processes: Vec<Process>) {
    TUI_RUNNING.store(true, AtomicOrdering::SeqCst);
    UPDATES_PAUSED.store(false, AtomicOrdering::SeqCst);
    
    {
        let mut system = SYSTEM.lock().unwrap();
        system.refresh_all();
        thread::sleep(Duration::from_millis(250));
    }
    
    let initial_accurate_processes = get_processes();
    let processes = Arc::new(Mutex::new(initial_accurate_processes));
    let mut siv = Cursive::default();
    let theme = custom_theme();
    siv.set_theme(theme);

    // Get terminal width
    let width = siv.screen_size().x.max(80) as usize;
    // Top bar
    let sysinfo_block = TextView::new(get_system_info_block(width)).with_name("sysinfo_block").fixed_height(4);
    // Bottom bar
    let bottom_bar = TextView::new(get_keybindings_bar()).with_name("bottom_bar").fixed_height(1);

    siv.add_global_callback('q', |s| {
        TUI_RUNNING.store(false, AtomicOrdering::SeqCst);
        s.quit();
    });
    
    // Toggle updates
    siv.add_global_callback('u', |s| {
        let current_paused = UPDATES_PAUSED.load(AtomicOrdering::SeqCst);
        let new_state = !current_paused;
        UPDATES_PAUSED.store(new_state, AtomicOrdering::SeqCst);
        let status = if new_state { "PAUSED" } else { "Running" };
        s.call_on_name("main_dialog", |view: &mut Dialog| {
            view.set_title(format!("Processes ({}) - Press 'u' to toggle updates, 'q' to quit", status));
        });
    });

    siv.add_global_callback('s', |s| {
        show_system_info_dialog(s);
    });
    
    let mut table = TableView::<Process, BasicColumn>::new()
        .on_sort(|_siv, _column, _order| {
        });

    for col_name in columns_to_display {
        match col_name.as_str() {
            "PID" => table = table.column(BasicColumn::PID, "PID", |c| c.align(HAlign::Right).width(6)),
            "PPID" => table = table.column(BasicColumn::PPID, "PPID", |c| c.align(HAlign::Right).width(6)),
            "USER" => table = table.column(BasicColumn::USER, "OWNER", |c| c.align(HAlign::Left).width(10)),
            "CPU" => table = table.column(BasicColumn::CPU, "CPU %", |c| c.width(8).align(HAlign::Right)),
            "MEM" => table = table.column(BasicColumn::MEM, "MEM %", |c| c.width(8).align(HAlign::Right)),
            "NI" => table = table.column(BasicColumn::PRIORITY, "PRI", |c| c.align(HAlign::Right).width(4)),
            "CMD" => table = table.column(BasicColumn::CMD, "CMD", |c| c.align(HAlign::Right).width(30)),
            "START" => table = table.column(BasicColumn::START, "STARTED", |c| c.align(HAlign::Left).width(10)),
            "STATUS" => table = table.column(BasicColumn::STATUS, "STATE", |c| c.align(HAlign::Left).width(15)),
            _ => println!("Invalid column: {}", col_name),
        }
    }

    {
        let processes_guard = processes.lock().unwrap();
        let mut sorted_processes = processes_guard.clone();
        sorted_processes.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(Ordering::Equal));
        table.set_items(sorted_processes);
    }
    
    table.sort_by(BasicColumn::CPU, Ordering::Greater);
    
    table.set_on_submit(|siv, row, _| {
        if let Some(process) = siv.find_name::<TableView<Process, BasicColumn>>("table")
            .unwrap()
            .borrow_item(row) 
        {
            siv.add_layer(
                Dialog::around(TextView::new(format!(
                    "PID: {}\nCommand: {}\nCPU Usage: {:.2}%\nMemory: {:.2} MB\nStatus: {:?}\nNice Value: {}",
                    process.pid, process.cmd, process.cpu, process.mem, process.process_state, process.priority
                )))
                .title("Process Details")
                .button("Close", |s| { s.pop_layer(); })
            );
        }
    });

    let table_with_name = table.with_name("table").full_screen();
    let scrollable_table = ScrollView::new(table_with_name);

    // Compose the main layout with system info block, table, and bottom bar
    let main_layout = LinearLayout::vertical()
        .child(sysinfo_block)
        .child(scrollable_table)
        .child(bottom_bar);

    siv.add_fullscreen_layer(main_layout);

    siv.add_global_callback('h', move |s| {
        s.add_layer(
            Dialog::around(TextView::new(
                "Controls:\n\
                 - Click column headers to sort\n\
                 - 'u' to toggle updates (pause/resume)\n\
                 - 'q' to quit\n\
                 - 's' to show system information\n\
                 - 'K' to kill the selected process\n\
                 - 'P' to pause the selected process\n\
                 - 'R' to resume the selected process\n\
                 - 'N' to change process priority (nice value)\n\
                 - 'f' to filter/clear filter processes\n\
                 - 't' to show process tree\n\
                 - 'h' for help"
            ))
            .title("Help")
            .button("Close", |s| { s.pop_layer(); })
        );
    });
    // Kill process
siv.add_global_callback('k', |s| {
    act_on_selected_process(s, |proc| proc.kill(), "Kill");
});

// Pause process (SIGSTOP)
siv.add_global_callback('p', |s| {
    act_on_selected_process(s, |proc| proc.kill_with(Signal::Stop).unwrap_or(false), "Pause");
});

// Resume process (SIGCONT)
siv.add_global_callback('r', |s| {
    act_on_selected_process(s, |proc| proc.kill_with(Signal::Continue).unwrap_or(false), "Resume");
});

// Add this near the other key bindings in display_tui
siv.add_global_callback('n', |s| {
    renice_process(s);
});

    // Add the 't' key binding for process tree
    siv.add_global_callback('t', |s| {
        if TREE_VIEW_OPEN.load(AtomicOrdering::SeqCst) {
            close_process_tree_fullscreen(s);
        } else {
            show_process_tree_fullscreen(s);
        }
    });

    // Add filter key binding
    siv.add_global_callback('f', |s| {
        let has_filter = {
            let filter_state = CURRENT_FILTER.lock().unwrap();
            filter_state.filter_type.is_some()
        };
        
        if has_filter {
            clear_filter(s);
        } else {
            show_filter_dialog(s);
        }
    });

    let processes_clone = Arc::clone(&processes);
    let sink = siv.cb_sink().clone();
    
    thread::spawn(move || {
        while TUI_RUNNING.load(AtomicOrdering::SeqCst) {
            thread::sleep(Duration::from_secs(1));
            if !UPDATES_PAUSED.load(AtomicOrdering::SeqCst) {
                let updated_processes = get_processes();
                {
                    let mut processes_guard = processes_clone.lock().unwrap();
                    *processes_guard = updated_processes;
                }
                let processes_for_closure = Arc::clone(&processes_clone);
                sink.send(Box::new(move |s| {
                    if let Some(mut table_view) = s.find_name::<TableView<Process, BasicColumn>>("table") {
                        let current_processes = processes_for_closure.lock().unwrap().clone();
                        
                        // Apply current filter if one exists
                        let filtered_processes = {
                            let filter_state = CURRENT_FILTER.lock().unwrap();
                            if let Some(filter_type) = filter_state.filter_type {
                                filter_processes(&current_processes, filter_type, &filter_state.filter_value)
                            } else {
                                current_processes
                            }
                        };
                        
                        table_view.set_items(filtered_processes);
                    }
                    // Update system info bar
                    let width = s.screen_size().x.max(80) as usize;
                    if let Some(mut sysinfo_view) = s.find_name::<TextView>("sysinfo_block") {
                        sysinfo_view.set_content(get_system_info_block(width));
                    }
                })).ok();
            }
        }
    });
    
    siv.run();
    
    TUI_RUNNING.store(false, AtomicOrdering::SeqCst);
    thread::sleep(Duration::from_millis(100));
}

// Add this function to create a real-time system info dialog
fn show_system_info_dialog(siv: &mut Cursive) {
    let content = TextView::new(get_system_info()).with_name("sysinfo_content");
    let dialog = Dialog::around(content)
        .title("System Information")
        .button("Close", |s| {
            UPDATES_PAUSED.store(false, AtomicOrdering::SeqCst);
            s.pop_layer();
        });
    
    siv.add_layer(dialog);
    UPDATES_PAUSED.store(true, AtomicOrdering::SeqCst);
    
    let sink = siv.cb_sink().clone();
    thread::spawn(move || {
        while UPDATES_PAUSED.load(AtomicOrdering::SeqCst) {
            sink.send(Box::new(|s| {
                if let Some(mut view) = s.find_name::<TextView>("sysinfo_content") {
                    view.set_content(get_system_info());
                }
            })).ok();
            thread::sleep(Duration::from_millis(500));
        }
    });
}

fn filter_processes(processes: &[Process], filter_type: FilterType, filter_value: &str) -> Vec<Process> {
    processes.iter()
        .filter(|process| {
            match filter_type {
                FilterType::PID => {
                    if let Ok(pid) = filter_value.parse::<u32>() {
                        process.pid == pid
                    } else {
                        false
                    }
                },
                FilterType::PPID => {
                    if let Ok(ppid) = filter_value.parse::<u32>() {
                        process.ppid.map_or(false, |p| p == ppid)
                    } else {
                        false
                    }
                },
                FilterType::USER => process.user.as_ref().map_or(false, |user| user.contains(filter_value)),
                FilterType::STATUS => format!("{:?}", process.process_state).contains(filter_value),
            }
        })
        .cloned()
        .collect()
}

fn show_filter_dialog(siv: &mut Cursive) {
    let dialog = Dialog::around(
        LinearLayout::vertical()
            .child(TextView::new("Select filter type:"))
            .child(SelectView::new()
                .item("PID", FilterType::PID)
                .item("PPID", FilterType::PPID)
                .item("User", FilterType::USER)
                .item("Status", FilterType::STATUS)
                .on_submit(move |s, &filter_type| {
                    s.pop_layer();
                    show_filter_value_dialog(s, filter_type);
                }))
    )
    .title("Filter Processes")
    .button("Cancel", |s| { s.pop_layer(); });
    
    siv.add_layer(dialog);
}

fn show_filter_value_dialog(siv: &mut Cursive, filter_type: FilterType) {
    let dialog = Dialog::around(
        LinearLayout::vertical()
            .child(TextView::new(format!("Enter filter value for {:?}:", filter_type)))
            .child(DummyView)
            .child(EditView::new()
                .with_name("filter_value")
                .fixed_width(20))
    )
    .title("Enter Filter Value")
    .button("Apply", move |s| {
        if let Some(mut view) = s.find_name::<EditView>("filter_value") {
            let filter_value = view.get_content().to_string();
            if let Some(mut table_view) = s.find_name::<TableView<Process, BasicColumn>>("table") {
                let current_processes = get_processes();
                let filtered_processes = filter_processes(&current_processes, filter_type, &filter_value);
                table_view.set_items(filtered_processes);
                
                // Update the current filter state
                let mut filter_state = CURRENT_FILTER.lock().unwrap();
                filter_state.filter_type = Some(filter_type);
                filter_state.filter_value = filter_value;
            }
            s.pop_layer();
        }
    })
    .button("Cancel", |s| { s.pop_layer(); });
    
    siv.add_layer(dialog);
}

fn clear_filter(siv: &mut Cursive) {
    if let Some(mut table_view) = siv.find_name::<TableView<Process, BasicColumn>>("table") {
        let current_processes = get_processes();
        table_view.set_items(current_processes);
        
        // Clear the filter state
        {
            let mut filter_state = CURRENT_FILTER.lock().unwrap();
            filter_state.filter_type = None;
            filter_state.filter_value.clear();
        }
    }
}

fn get_system_info_block(width: usize) -> StyledString {
    let mut system = SYSTEM.lock().unwrap();
    system.refresh_all();

    let cpu_name = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|content| {
            content.lines()
                .find(|line| line.starts_with("model name"))
                .and_then(|line| line.split(':').nth(1))
                .map(|name| name.trim().to_string())
        })
        .unwrap_or_else(|| "Unknown CPU".to_string());

    let cpu_usage = system.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / system.cpus().len() as f32;
    let total_memory = system.total_memory() as f32 / 1024.0 / 1024.0;
    let used_memory = system.used_memory() as f32 / 1024.0 / 1024.0;
    let swap = system.total_swap() as f32 / 1024.0 / 1024.0;
    let uptime = System::uptime();
    let process_count = system.processes().len();
    let freq = std::fs::read_to_string("/proc/cpuinfo")
        .map(|content| {
            let freqs: Vec<f64> = content.lines()
                .filter(|line| line.starts_with("cpu MHz"))
                .filter_map(|line| line.split(':').nth(1)?.trim().parse().ok())
                .collect();
            freqs.iter().sum::<f64>() / freqs.len().max(1) as f64
        })
        .unwrap_or_default();
    let physical_cores = num_cpus::get_physical();
    let logical_cores = num_cpus::get();

    let lines = vec![
        format!("CPU Name: {:<30} | Freq: {:>6.0} MHz | Usage: {:>5.1}% | Cores: {} ({} phys)", cpu_name, freq, cpu_usage, logical_cores, physical_cores),
        format!("Memory: {:>7.0}/{:<7.0} MB | Swap: {:>7.0} MB", used_memory, total_memory, swap),
        format!("Uptime: {}d {:02}h {:02}m | Procs: {}", uptime / 86400, (uptime % 86400) / 3600, (uptime % 3600) / 60, process_count),
    ];

    let centered = lines
        .into_iter()
        .map(|line| {
            let pad = (width.saturating_sub(line.len())) / 2;
            format!("{}{}", " ".repeat(pad), line)
        })
        .collect::<Vec<_>>()
        .join("\n");

    StyledString::styled(
        centered,
        Style::from(ColorStyle::new(Color::Dark(BaseColor::Magenta), Color::Dark(BaseColor::Black))).combine(Effect::Bold)
    )
}