use cursive::align::HAlign;
use cursive::traits::*;
use cursive::views::{Dialog, TextView, ScrollView};
use cursive::Cursive;
use cursive::CursiveExt;
use cursive::view::Nameable;
use cursive_table_view::{TableView, TableViewItem};
use sysinfo::{ProcessStatus, System};
use std::cmp::Ordering;
use std::sync::{Arc, Mutex, Once};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::thread;
use std::time::Duration;
use lazy_static::lazy_static;

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
}

impl TableViewItem<BasicColumn> for Process {
    fn to_column(&self, column: BasicColumn) -> String {
        match column {
            BasicColumn::PID => format!("{}", self.pid),
            BasicColumn::PPID => self.ppid.map_or("N/A".to_string(), |ppid| ppid.to_string()),
            BasicColumn::USER => self.user.clone().unwrap_or_else(|| "N/A".to_string()),
            BasicColumn::CPU => format!("{:.2}", self.cpu),
            BasicColumn::MEM => format!("{:.2}", self.mem),
            BasicColumn::CMD => self.cmd.clone(),
            BasicColumn::START => {
                // Convert start_time to a readable format
                let datetime = chrono::DateTime::from_timestamp(self.start_time as i64, 0)
                    .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
                format!("{}", datetime.format("%H:%M:%S"))
            }
            BasicColumn::STATUS => format!("{:?}", self.process_state),
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
        }
    }
}

pub static TUI_RUNNING: AtomicBool = AtomicBool::new(true);
pub static UPDATES_PAUSED: AtomicBool = AtomicBool::new(false);

// Create a singleton System instance to maintain state between calls
lazy_static! {
    static ref SYSTEM: Mutex<System> = Mutex::new(System::new_all());
    static ref INIT: Once = Once::new();
}

fn get_processes() -> Vec<Process> {
    INIT.call_once(|| {
        let mut system = SYSTEM.lock().unwrap();
        system.refresh_all();
        drop(system);
        thread::sleep(Duration::from_millis(250));
    });
    
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
            
            Process {
                pid: pid.as_u32(),
                ppid,
                user,
                cpu: process.cpu_usage(),
                mem: process.memory() as f32 / 1024.0, // Convert memory to MB
                cmd: process.name().to_string_lossy().into_owned(),
                start_time: process.start_time(),
                process_state: process.status(),
            }
        })
        .collect();

    processes
}

fn custom_theme_from_cursive(_siv: &Cursive) -> cursive::theme::Theme {
    cursive::theme::Theme::default()
}

pub fn display_tui(columns_to_display: Vec<String>, _initial_processes: Vec<Process>) {
    // Initialize the system and get accurate initial process data
    // This ensures we have a baseline for CPU usage calculations
    let initial_accurate_processes = get_processes();
    
    // Create a shared state for processes that can be accessed by multiple threads
    let processes = Arc::new(Mutex::new(initial_accurate_processes));
    
    // Set up TUI
    let mut siv = Cursive::default();
    let theme = custom_theme_from_cursive(&siv);
    siv.set_theme(theme);

    siv.add_global_callback('q', |s| {
        TUI_RUNNING.store(false, AtomicOrdering::SeqCst);
        s.quit();
    });

    // Add toggle for pausing/resuming updates
    siv.add_global_callback('u', |s| {
        let current_paused = UPDATES_PAUSED.load(AtomicOrdering::SeqCst);
        let new_state = !current_paused;
        UPDATES_PAUSED.store(new_state, AtomicOrdering::SeqCst);
        
        // Update the title to show the current state
        let status = if new_state { "PAUSED" } else { "Running" };
        s.call_on_name("main_dialog", |view: &mut Dialog| {
            view.set_title(format!("Processes ({}) - Press 'u' to toggle updates, 'q' to quit", status));
        });
    });

    let mut table = TableView::<Process, BasicColumn>::new()
        .on_sort(|_siv, _column, _order| {
            // Sorting is handled automatically by cursive_table_view
        });

    for col_name in columns_to_display {
        match col_name.as_str() {
            "PID" => table = table.column(BasicColumn::PID, "PID", |c| c.align(HAlign::Right).width(6)),
            "PPID" => table = table.column(BasicColumn::PPID, "PPID", |c| c.align(HAlign::Right).width(6)),
            "USER" => table = table.column(BasicColumn::USER, "USER", |c| c.align(HAlign::Left).width(10)),
            "CPU" => table = table.column(BasicColumn::CPU, "CPU %", |c| c.width(8).align(HAlign::Right)),
            "MEM" => table = table.column(BasicColumn::MEM, "MEM MB", |c| c.width(8).align(HAlign::Right)),
            "CMD" => table = table.column(BasicColumn::CMD, "CMD", |c| c.align(HAlign::Right).width(30)),
            "START" => table = table.column(BasicColumn::START, "START TIME", |c| c.align(HAlign::Left).width(10)),
            "STATUS" => table = table.column(BasicColumn::STATUS, "STATUS", |c| c.align(HAlign::Left).width(15)),
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
    
    table.set_on_submit(|siv, _row, index| {
        let table_view = siv.find_name::<TableView<Process, BasicColumn>>("table").unwrap();
        
        let process = table_view.borrow_item(index).unwrap().clone();
        
        let details = format!(
            "PID: {}\nCommand: {}\nCPU Usage: {:.2}%\nMemory: {:.2} MB\nStatus: {:?}",
            process.pid, process.cmd, process.cpu, process.mem, process.process_state
        );
        
        siv.add_layer(
            Dialog::around(TextView::new(details))
                .title("Process Details")
                .button("Close", |s| { s.pop_layer(); })
        );
    });

    let table_with_name = table.with_name("table").full_screen();
    let scrollable_table = ScrollView::new(table_with_name);

    siv.add_layer(
        Dialog::around(scrollable_table)
            .title("Processes (Running) - Press 'u' to toggle updates, 'q' to quit")
            .with_name("main_dialog")
    );

    siv.add_global_callback('h', move |s| {
        s.add_layer(
            Dialog::around(TextView::new(
                "Controls:\n\
                 - Click column headers to sort\n\
                 - 'u' to toggle updates (pause/resume)\n\
                 - 'q' to quit\n\
                 - 'h' for help"
            ))
            .title("Help")
            .button("Close", |s| { s.pop_layer(); })
        );
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
                        table_view.set_items(current_processes);
                    }
                })).ok();
            }
        }
    });
    siv.run();
}
