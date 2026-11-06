use crate::cosmic_settings;
use crate::framebuffer::{FramebufferConsole, HEIGHT, WIDTH};
use crate::input;

const EXPLORE: u8 = 1;
const INSTALLED: u8 = 2;
const UPDATES: u8 = 4;
const UTILITIES: u8 = 8;
const WORK: u8 = 16;
const DEVELOPMENT: u8 = 32;
const GAMES: u8 = 64;
const SOCIALIZE: u8 = 128;
const VISIBLE_ROWS: usize = 5;

const CATEGORIES: [&[u8]; 8] = [
    b"Explore",
    b"Installed apps",
    b"Updates",
    b"Utilities",
    b"Work",
    b"Development",
    b"Games",
    b"Socialize",
];

struct AppEntry {
    name: &'static [u8],
    summary: &'static [u8],
    status: &'static [u8],
    groups: u8,
}

const APPS: [AppEntry; 20] = [
    AppEntry {
        name: b"TRMNL Console",
        summary: b"Built-in command line",
        status: b"Update demo",
        groups: EXPLORE | INSTALLED | UPDATES | DEVELOPMENT,
    },
    AppEntry {
        name: b"Text Editor",
        summary: b"Edit plain text",
        status: b"Native preview",
        groups: EXPLORE | UPDATES | WORK | DEVELOPMENT,
    },
    AppEntry {
        name: b"LibreOffice",
        summary: b"Docs, sheets, slides",
        status: b"Linux only",
        groups: EXPLORE | UPDATES | WORK,
    },
    AppEntry {
        name: b"COSMIC Files",
        summary: b"Browse files and folders",
        status: b"Native preview",
        groups: EXPLORE | UTILITIES,
    },
    AppEntry {
        name: b"COSMIC Settings",
        summary: b"Display and appearance",
        status: b"Built in",
        groups: EXPLORE | INSTALLED | UTILITIES,
    },
    AppEntry {
        name: b"Calculator",
        summary: b"Basic desktop calculator",
        status: b"Planned",
        groups: EXPLORE | UTILITIES,
    },
    AppEntry {
        name: b"Web Browser",
        summary: b"Web access requires networking",
        status: b"Planned",
        groups: EXPLORE | DEVELOPMENT,
    },
    AppEntry {
        name: b"Image Viewer",
        summary: b"View local images",
        status: b"Planned",
        groups: EXPLORE | UTILITIES,
    },
    AppEntry {
        name: b"Archive Manager",
        summary: b"Inspect compressed archives",
        status: b"Planned",
        groups: EXPLORE | UTILITIES,
    },
    AppEntry {
        name: b"Text Editor updates",
        summary: b"Update channel entry",
        status: b"Unavailable",
        groups: UPDATES | DEVELOPMENT,
    },
    AppEntry {
        name: b"LibreOffice updates",
        summary: b"Update channel entry",
        status: b"Unavailable",
        groups: UPDATES | WORK,
    },
    AppEntry {
        name: b"Slack",
        summary: b"Team messaging client",
        status: b"Linux only",
        groups: EXPLORE | WORK | SOCIALIZE,
    },
    AppEntry {
        name: b"Steam",
        summary: b"Game library and launcher",
        status: b"Linux only",
        groups: EXPLORE | GAMES,
    },
    AppEntry {
        name: b"Lutris",
        summary: b"Game library manager",
        status: b"Linux only",
        groups: EXPLORE | GAMES,
    },
    AppEntry {
        name: b"Visual Studio Code",
        summary: b"Code editor",
        status: b"Linux only",
        groups: EXPLORE | DEVELOPMENT,
    },
    AppEntry {
        name: b"Virtual Machine Mgr",
        summary: b"Manage virtual machines",
        status: b"Linux only",
        groups: EXPLORE | DEVELOPMENT | UTILITIES,
    },
    AppEntry {
        name: b"Firefox",
        summary: b"Web browser",
        status: b"Linux only",
        groups: EXPLORE | DEVELOPMENT | SOCIALIZE,
    },
    AppEntry {
        name: b"Discord",
        summary: b"Voice and text chat",
        status: b"Linux only",
        groups: EXPLORE | SOCIALIZE,
    },
    AppEntry {
        name: b"GIMP",
        summary: b"Image editor",
        status: b"Linux only",
        groups: EXPLORE | WORK,
    },
    AppEntry {
        name: b"Protontricks",
        summary: b"Steam compatibility helper",
        status: b"Linux only",
        groups: EXPLORE | GAMES | UTILITIES,
    },
];

pub fn run(screen: &mut FramebufferConsole) {
    let mut category = 0usize;
    let mut selected = 0usize;
    let mut message: &[u8] = b"Local catalog preview. Install/update service is unavailable.";

    loop {
        let colors = cosmic_settings::palette();
        screen.clear(colors.background);
        screen.fill_cells(0, 0, WIDTH, colors.panel);
        screen.write_at(0, 2, b"COSMIC STORE", colors.foreground);
        screen.write_at(0, 58, b"Updates 3", colors.foreground);

        for row in 1..HEIGHT {
            screen.fill_cells(row, 0, 19, colors.panel);
        }
        for (index, label) in CATEGORIES.iter().enumerate() {
            let row = 3 + index * 2;
            let color = if index == category {
                colors.accent
            } else {
                colors.muted
            };
            screen.write_at(row, 2, if index == category { b">" } else { b" " }, color);
            screen.write_at(row, 4, label, color);
        }

        screen.write_at(2, 22, category_title(category), colors.foreground);
        screen.write_at(4, 22, b"Applications", colors.muted);
        let total = count_apps(category);
        if total == 0 {
            screen.write_at(7, 24, b"No entries in this category.", colors.muted);
        } else {
            let first = selected.saturating_sub(VISIBLE_ROWS - 1);
            for visible_row in 0..VISIBLE_ROWS {
                let filtered_index = first + visible_row;
                let Some(app) = app_at(category, filtered_index) else {
                    break;
                };
                let row = 6 + visible_row * 3;
                let color = if filtered_index == selected {
                    colors.accent
                } else {
                    colors.foreground
                };
                screen.write_at(
                    row,
                    22,
                    if filtered_index == selected {
                        b">"
                    } else {
                        b" "
                    },
                    color,
                );
                screen.write_at(row, 25, app.name, color);
                screen.write_at(row + 1, 25, app.summary, colors.muted);
                screen.write_at(row + 1, 57, app.status, colors.muted);
            }
        }

        screen.write_at(21, 22, message, colors.muted);
        screen.write_at(
            HEIGHT - 2,
            22,
            b"A/D Category  W/S Browse  Enter Details  U Check  Esc Back",
            colors.muted,
        );

        match input::read_scancode() {
            0x1E => {
                category = category.saturating_sub(1);
                selected = 0;
                message = b"Category changed.";
            }
            0x20 => {
                category = (category + 1).min(CATEGORIES.len() - 1);
                selected = 0;
                message = b"Category changed.";
            }
            0x11 => selected = selected.saturating_sub(1),
            0x1F => selected = (selected + 1).min(count_apps(category).saturating_sub(1)),
            0x1C => {
                message = if category == 2 {
                    b"Update unavailable: no network or verified package manager."
                } else {
                    b"Install unavailable: no package manager or signed repository."
                };
            }
            0x16 => message = b"Update check unavailable: networking is not implemented.",
            0x01 => return,
            _ => {}
        }
    }
}

fn category_title(category: usize) -> &'static [u8] {
    match category {
        0 => b"Editor's choice",
        1 => b"Installed apps",
        2 => b"Updates",
        3 => b"Utilities",
        4 => b"Work",
        5 => b"Development",
        6 => b"Games",
        _ => b"Socialize",
    }
}

fn category_mask(category: usize) -> u8 {
    match category {
        0 => EXPLORE,
        1 => INSTALLED,
        2 => UPDATES,
        3 => UTILITIES,
        4 => WORK,
        5 => DEVELOPMENT,
        6 => GAMES,
        _ => SOCIALIZE,
    }
}

fn count_apps(category: usize) -> usize {
    let mask = category_mask(category);
    APPS.iter().filter(|app| app.groups & mask != 0).count()
}

fn app_at(category: usize, filtered_index: usize) -> Option<&'static AppEntry> {
    let mask = category_mask(category);
    APPS.iter()
        .filter(|app| app.groups & mask != 0)
        .nth(filtered_index)
}
