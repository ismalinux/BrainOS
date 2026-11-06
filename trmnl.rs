use crate::framebuffer::{FramebufferConsole, TERMINAL_BACKGROUND, TERMINAL_FOREGROUND as BLUE};
use crate::input;
use crate::shell::Shell;

const COMMAND_CAPACITY: usize = 128;

pub fn run(screen: &mut FramebufferConsole) {
    let mut shell = Shell::new();
    screen.clear(TERMINAL_BACKGROUND);
    screen.write_at(1, 2, b"BRAIN OS // TRMNL CONSOLE", BLUE);
    screen.set_cursor(3, 0);

    loop {
        screen.write(b"user@Brain-OS:~$ ", BLUE);
        let mut command = [0u8; COMMAND_CAPACITY];
        let mut length = 0;
        loop {
            let character = input::read_char();
            match character {
                b'\x1B' if shell.escape_exits_console() => return,
                b'\x1B' => {}
                b'\n' => {
                    screen.put_char(b'\n', BLUE);
                    break;
                }
                b'\x08' if length > 0 => {
                    length -= 1;
                    screen.put_char(b'\x08', BLUE);
                }
                b'\x08' => {}
                0x20..=0x7E if length < COMMAND_CAPACITY - 1 => {
                    command[length] = character;
                    length += 1;
                    screen.put_char(character, BLUE);
                }
                _ => {}
            }
        }
        shell.record_command(&command[..length]);
        execute(screen, &mut shell, &command[..length]);
    }
}

fn execute(screen: &mut FramebufferConsole, shell: &mut Shell, command: &[u8]) {
    let command = trim_start(command);
    match command {
        b"help" => screen.write(b"Commands: help, echo, clear, apps, pwd, whoami\n", BLUE),
        b"clear" => {
            screen.clear(TERMINAL_BACKGROUND);
            screen.set_cursor(0, 0);
        }
        b"apps" => screen.write(
            b"TRMNL Console is available. Other apps are planned.\n",
            BLUE,
        ),
        b"pwd" => screen.write(b"/home/user\n", BLUE),
        b"whoami" => screen.write(b"user\n", BLUE),
        _ if command.is_empty() => {}
        _ => shell.execute(screen, command),
    }
}

fn trim_start(mut text: &[u8]) -> &[u8] {
    while text.first() == Some(&b' ') {
        text = &text[1..];
    }
    text
}
