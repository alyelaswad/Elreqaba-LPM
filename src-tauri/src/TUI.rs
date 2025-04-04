use cursive::Cursive;
use cursive::views::{Dialog, TextView, LinearLayout};
use cursive::theme::{Color, PaletteColor, Theme};
use cursive::align::HAlign;

#[derive(Clone)]
struct SysStats {
    cpu_usage: u32,
    mem_usage: u32,
    total_mem: u32,
}

fn custom_theme(siv: &Cursive) -> Theme {
    let mut theme = siv.current_theme().clone();
    theme.palette[PaletteColor::Background] = Color::TerminalDefault;
    theme.palette[PaletteColor::View] = Color::TerminalDefault;
    theme.palette[PaletteColor::Primary] = Color::Rgb(255, 255, 255);
    theme.palette[PaletteColor::HighlightText] = Color::Rgb(0, 0, 0);
    theme.palette[PaletteColor::Highlight] = Color::Rgb(0, 230, 118);
    theme
}

fn display_tui() {
    // Initialize the Cursive UI
    let mut siv = Cursive::default();
    let theme = custom_theme(&siv);
    siv.set_theme(theme);

    // Example system stats
    let sys_stats = SysStats {
        cpu_usage: 30,   // Example value for CPU usage
        mem_usage: 2048, // Example value for memory usage in MB
        total_mem: 8192, // Example total memory in MB
    };

    // Display system stats in the UI
    siv.add_layer(
        Dialog::new()
            .title("System Stats")
            .content(
                LinearLayout::vertical()
                    .child(TextView::new(format!("CPU Usage: {}%", sys_stats.cpu_usage)))
                    .child(TextView::new(format!("Memory Usage: {}/{} MB", sys_stats.mem_usage, sys_stats.total_mem))),
            )
            .button("Quit", |s| s.quit()),
    );

    // Run the event loop
    siv.run();
}

fn main() {
    display_tui();
}
