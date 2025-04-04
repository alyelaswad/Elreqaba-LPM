use cursive::align::HAlign;
use cursive::traits::*;
use cursive::views::{Dialog, TextView, ScrollView};
use cursive::Cursive;
use cursive::CursiveExt;
use cursive::view::Nameable;
use cursive_table_view::{TableView, TableViewItem};
use std::cmp::Ordering;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use sysinfo::System;

#[derive(Clone, Debug)]
pub struct Process {
    pub pid: u32,
    pub cpu: f32,
    pub mem: f32,
    pub cmd: String,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum BasicColumn {
    PID,
    CPU,
    MEM,
    CMD,
}

impl TableViewItem<BasicColumn> for Process {
    fn to_column(&self, column: BasicColumn) -> String {
        match column {
            BasicColumn::PID => format!("{}", self.pid),
            BasicColumn::CPU => format!("{:.2}", self.cpu),
            BasicColumn::MEM => format!("{:.2}", self.mem),
            BasicColumn::CMD => self.cmd.clone(),
        }
    }

    fn cmp(&self, other: &Self, column: BasicColumn) -> Ordering {
        match column {
            BasicColumn::PID => self.pid.cmp(&other.pid),
            BasicColumn::CPU => self.cpu.partial_cmp(&other.cpu).unwrap_or(Ordering::Equal),
            BasicColumn::MEM => self.mem.partial_cmp(&other.mem).unwrap_or(Ordering::Equal),
            BasicColumn::CMD => self.cmd.cmp(&other.cmd),
        }
    }
}

static TUI_RUNNING: AtomicBool = AtomicBool::new(true);

#[derive(Copy, Clone)]
pub struct Config {
    pub update_every: NonZeroU32,
}

static CONFIG: Config = Config { update_every: NonZeroU32::new(1).unwrap() };

fn custom_theme_from_cursive(_siv: &Cursive) -> cursive::theme::Theme {
    cursive::theme::Theme::default()
}

pub fn display_tui(columns_to_display: Vec<String>, mut processes: Vec<Process>) {
    processes.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(Ordering::Equal));

    let mut siv = Cursive::default();
    let theme = custom_theme_from_cursive(&siv);
    siv.set_theme(theme);

    siv.add_global_callback('q', |s| {
        TUI_RUNNING.store(false, AtomicOrdering::SeqCst);
        s.quit();
    });

    let mut table = TableView::<Process, BasicColumn>::new();

    // Add columns based on `columns_to_display`
    for col_name in columns_to_display {
        match col_name.as_str() {
            "PID" => table = table.column(BasicColumn::PID, "PID", |c| c.align(HAlign::Right).width(6)),
            "CPU" => table = table.column(BasicColumn::CPU, "CPU %", |c| c.width(8).align(HAlign::Right)),
            "MEM" => table = table.column(BasicColumn::MEM, "MEM %", |c| c.width(8).align(HAlign::Right)),
            "CMD" => table = table.column(BasicColumn::CMD, "CMD", |c| c.align(HAlign::Left)),
            _ => println!("Invalid column: {}", col_name),
        }
    }

    table.set_items(processes);

    let table_with_default = table.default_column(BasicColumn::CPU);

    let scrollable_table = ScrollView::new(table_with_default.with_name("table").full_screen());

    // Add the table layer to the UI without the update status layer
    siv.add_layer(Dialog::around(scrollable_table).title("Processes"));

    siv.set_fps(CONFIG.update_every.into());
    siv.run();
}
