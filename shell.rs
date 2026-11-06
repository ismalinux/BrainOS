use crate::ata_pio::{self, AtaError};
use crate::cmos;
use crate::framebuffer::{FramebufferConsole, TERMINAL_BACKGROUND, TERMINAL_FOREGROUND as BLUE};

const HISTORY_CAPACITY: usize = 50;
const TEXT_CAPACITY: usize = 128;
const NAME_CAPACITY: usize = 32;
const VARIABLE_CAPACITY: usize = 16;
const TASK_CAPACITY: usize = 8;
const OPTION_CAPACITY: usize = 8;
const OVERRIDE_CAPACITY: usize = 8;
const BINDING_CAPACITY: usize = 8;
const MAX_UNTIL_ITERATIONS: usize = 32;
const MAX_ULIMIT: u32 = 1_000_000;

const ATTR_READONLY: u8 = 1;
const ATTR_INTEGER: u8 = 2;
const ATTR_LOCAL: u8 = 4;
const ATTR_EXPORT: u8 = 8;

#[derive(Clone, Copy)]
struct FixedText {
    bytes: [u8; TEXT_CAPACITY],
    length: usize,
}

impl FixedText {
    const EMPTY: Self = Self {
        bytes: [0; TEXT_CAPACITY],
        length: 0,
    };

    fn set(&mut self, value: &[u8]) -> bool {
        if value.len() > TEXT_CAPACITY {
            return false;
        }
        self.bytes[..value.len()].copy_from_slice(value);
        self.length = value.len();
        true
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}

#[derive(Clone, Copy)]
struct Variable {
    name: [u8; NAME_CAPACITY],
    name_length: usize,
    value: FixedText,
    attributes: u8,
    active: bool,
}

impl Variable {
    const EMPTY: Self = Self {
        name: [0; NAME_CAPACITY],
        name_length: 0,
        value: FixedText::EMPTY,
        attributes: 0,
        active: false,
    };

    fn matches(&self, name: &[u8]) -> bool {
        self.active && self.name_length == name.len() && &self.name[..self.name_length] == name
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum TaskState {
    Empty,
    Running,
    Stopped,
    Done,
}

#[derive(Clone, Copy)]
struct SimulatedTask {
    job_id: u8,
    pid: u16,
    state: TaskState,
    ticks: u32,
    name: FixedText,
}

impl SimulatedTask {
    const EMPTY: Self = Self {
        job_id: 0,
        pid: 0,
        state: TaskState::Empty,
        ticks: 0,
        name: FixedText::EMPTY,
    };
}

#[derive(Clone, Copy)]
struct ShellOption {
    name: &'static [u8],
    enabled: bool,
}

#[derive(Clone, Copy)]
struct CommandOverride {
    name: FixedText,
    target: FixedText,
    active: bool,
}

impl CommandOverride {
    const EMPTY: Self = Self {
        name: FixedText::EMPTY,
        target: FixedText::EMPTY,
        active: false,
    };
}

#[derive(Clone, Copy)]
struct KeyBinding {
    key: FixedText,
    action: FixedText,
    active: bool,
}

impl KeyBinding {
    const EMPTY: Self = Self {
        key: FixedText::EMPTY,
        action: FixedText::EMPTY,
        active: false,
    };
}

pub struct Shell {
    history: [FixedText; HISTORY_CAPACITY],
    history_next: usize,
    history_count: usize,
    history_serial: u32,
    variables: [Variable; VARIABLE_CAPACITY],
    tasks: [SimulatedTask; TASK_CAPACITY],
    next_pid: u16,
    next_job_id: u8,
    uptime_ticks: u32,
    ulimit: u32,
    umask: u16,
    options: [ShellOption; OPTION_CAPACITY],
    overrides: [CommandOverride; OVERRIDE_CAPACITY],
    bindings: [KeyBinding; BINDING_CAPACITY],
}

impl Shell {
    pub const fn new() -> Self {
        Self {
            history: [FixedText::EMPTY; HISTORY_CAPACITY],
            history_next: 0,
            history_count: 0,
            history_serial: 0,
            variables: [Variable::EMPTY; VARIABLE_CAPACITY],
            tasks: [SimulatedTask::EMPTY; TASK_CAPACITY],
            next_pid: 100,
            next_job_id: 1,
            uptime_ticks: 0,
            ulimit: 256,
            umask: 0o022,
            options: [
                ShellOption {
                    name: b"checkwinsize",
                    enabled: true,
                },
                ShellOption {
                    name: b"cmdhist",
                    enabled: true,
                },
                ShellOption {
                    name: b"expand_aliases",
                    enabled: true,
                },
                ShellOption {
                    name: b"nullglob",
                    enabled: false,
                },
                ShellOption {
                    name: b"nocaseglob",
                    enabled: false,
                },
                ShellOption {
                    name: b"dotglob",
                    enabled: false,
                },
                ShellOption {
                    name: b"histappend",
                    enabled: true,
                },
                ShellOption {
                    name: b"extglob",
                    enabled: false,
                },
            ],
            overrides: [CommandOverride::EMPTY; OVERRIDE_CAPACITY],
            bindings: [KeyBinding::EMPTY; BINDING_CAPACITY],
        }
    }

    pub fn record_command(&mut self, command: &[u8]) {
        if command.is_empty() {
            return;
        }
        let entry = &mut self.history[self.history_next];
        let length = command.len().min(TEXT_CAPACITY);
        entry.bytes[..length].copy_from_slice(&command[..length]);
        entry.length = length;
        self.history_next = (self.history_next + 1) % HISTORY_CAPACITY;
        self.history_count = (self.history_count + 1).min(HISTORY_CAPACITY);
        self.history_serial = self.history_serial.wrapping_add(1);
        self.uptime_ticks = self.uptime_ticks.saturating_add(1);
        for task in &mut self.tasks {
            if task.state == TaskState::Running {
                task.ticks = task.ticks.saturating_add(1);
            }
        }
    }

    pub fn execute(&mut self, screen: &mut FramebufferConsole, line: &[u8]) {
        self.dispatch(screen, trim_start(line), 0, true);
    }

    fn dispatch(
        &mut self,
        screen: &mut FramebufferConsole,
        line: &[u8],
        depth: usize,
        allow_override: bool,
    ) {
        if depth > 4 {
            screen.write(b"shell: command nesting limit exceeded\n", BLUE);
            return;
        }
        let (name, arguments) = split_word(line);
        if name.is_empty() {
            return;
        }

        if let Some(equal) = name.iter().position(|&byte| byte == b'=') {
            if is_name(&name[..equal]) {
                self.assign(screen, &name[..equal], &name[equal + 1..], 0);
                return;
            }
        }

        if name == b"command" {
            self.command_builtin(screen, arguments, depth);
            return;
        }
        if name == b"builtin" {
            let (builtin_name, args) = split_word(arguments);
            self.dispatch_named(screen, builtin_name, args, depth + 1, false);
            return;
        }
        if allow_override && self.option_enabled(b"expand_aliases") {
            if let Some(target) = self.override_target(name) {
                let mut target_copy = FixedText::EMPTY;
                target_copy.set(target);
                self.dispatch_named(screen, target_copy.as_bytes(), arguments, depth + 1, false);
                return;
            }
        }
        self.dispatch_named(screen, name, arguments, depth, false);
    }

    fn dispatch_named(
        &mut self,
        screen: &mut FramebufferConsole,
        name: &[u8],
        args: &[u8],
        depth: usize,
        allow_override: bool,
    ) {
        match name {
            b"echo" => self.echo(screen, args),
            b"printf" => self.printf(screen, args),
            b"history" => self.print_history(screen),
            b"kill" => self.kill(screen, args),
            b"jobs" => self.jobs(screen),
            b"bg" => self.resume_job(screen, args, false),
            b"fg" => self.resume_job(screen, args, true),
            b"times" => self.times(screen),
            b"ulimit" => self.ulimit_command(screen, args),
            b"umask" => self.umask_command(screen, args),
            b"declare" | b"typeset" => self.declare(screen, args, 0),
            b"local" => self.declare(screen, args, ATTR_LOCAL),
            b"readonly" => self.declare(screen, args, ATTR_READONLY),
            b"unset" => self.unset(screen, args),
            b"bind" => self.bind(screen, args),
            b"shopt" => self.shopt(screen, args),
            b"until" => self.until_loop(screen, args, depth),
            b"spawn" => self.spawn(screen, args),
            b"sudo" => self.sudo(screen, args),
            b"apt" => self.apt(screen, args),
            b"commands" => self.list_commands(screen),
            b"uname" => self.uname(screen, args),
            b"uptime" => self.uptime(screen),
            b"date" => self.date(screen),
            b"disk" => self.disk(screen, args),
            b"nvram" => self.nvram(screen, args),
            b"mem" => self.memory(screen),
            b"drivers" => screen.write(
                b"PS/2 polling; framebuffer RGB32; IDE primary-master PIO read-only; CMOS RTC/NVRAM read-only\n",
                BLUE,
            ),
            b"id" => screen.write(
                b"uid=unknown(user) gid=unknown(user) [offline profile]\n",
                BLUE,
            ),
            b"hostname" => screen.write(b"Brain-OS\n", BLUE),
            b"type" => self.command_type(screen, args),
            b"alias" => self.alias(screen, args),
            b"unalias" => self.unalias(screen, args),
            b"test" => screen.write(
                if self.condition_true(args) {
                    b"true\n"
                } else {
                    b"false\n"
                },
                BLUE,
            ),
            b"set" => self.set_command(screen, args),
            b"export" => self.declare(screen, args, ATTR_EXPORT),
            b"ps" => self.jobs(screen),
            b"ls" | b"cat" | b"df" | b"mount" => screen.write(
                b"filesystem: no VFS or mounted filesystem is available\n",
                BLUE,
            ),
            b"free" => self.memory(screen),
            b"reboot" | b"poweroff" => screen.write(
                b"power control: ACPI/reset support is not implemented\n",
                BLUE,
            ),
            b"true" => {}
            b"false" => screen.write(b"false: condition is false\n", BLUE),
            b"help" => screen.write(b"Commands: help, echo, clear, apps, pwd, whoami\n", BLUE),
            b"apps" => screen.write(
                b"TRMNL Console is available. Other apps are planned.\n",
                BLUE,
            ),
            b"clear" => {
                screen.clear(TERMINAL_BACKGROUND);
                screen.set_cursor(0, 0);
            }
            b"pwd" => screen.write(b"/home/user\n", BLUE),
            b"whoami" => screen.write(b"user\n", BLUE),
            _ if allow_override => {
                if let Some(target) = self.override_target(name) {
                    let mut target_copy = FixedText::EMPTY;
                    target_copy.set(target);
                    self.dispatch_named(screen, target_copy.as_bytes(), args, depth + 1, false);
                } else {
                    self.unknown_command(screen, name);
                }
            }
            _ => self.unknown_command(screen, name),
        }
    }

    fn unknown_command(&self, screen: &mut FramebufferConsole, name: &[u8]) {
        screen.write(b"Brain OS: command not found: ", BLUE);
        screen.write(name, BLUE);
        screen.put_char(b'\n', BLUE);
    }

    fn sudo(&self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        if trim_start(arguments).is_empty() {
            screen.write(b"sudo: usage: sudo COMMAND [ARGUMENTS...]\n", BLUE);
            return;
        }
        screen.write(
            b"sudo: unavailable; BrainOS has no user authentication or privilege model. Command not run.\n",
            BLUE,
        );
    }

    fn apt(&self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let (action, rest) = split_word(trim_start(arguments));
        match action {
            b"update" => screen.write(
                b"apt: update unavailable; networking, TLS, package indexes, and signature verification are not implemented.\n",
                BLUE,
            ),
            b"install" => {
                let (package, _) = split_word(trim_start(rest));
                if package.is_empty() {
                    screen.write(b"apt: install requires a package name\n", BLUE);
                } else {
                    screen.write(
                        b"apt: install unavailable; BrainOS has no package manager, VFS, or signed repository support.\n",
                        BLUE,
                    );
                }
            }
            b"upgrade" => screen.write(
                b"apt: upgrade unavailable; BrainOS has no package database or update mechanism.\n",
                BLUE,
            ),
            _ => screen.write(
                b"apt: supported syntax is update, install PACKAGE, or upgrade; package operations are not implemented.\n",
                BLUE,
            ),
        }
    }

    fn list_commands(&self, screen: &mut FramebufferConsole) {
        screen.write(
            b"Shell: echo printf history help clear apps pwd whoami commands\n",
            BLUE,
        );
        screen.write(b"System: uname uptime date id hostname mem drivers\n", BLUE);
        screen.write(b"Storage: disk [LBA] nvram [INDEX] (read-only)\n", BLUE);
        screen.write(
            b"State: declare typeset local readonly export unset set test until\n",
            BLUE,
        );
        screen.write(b"Jobs: spawn jobs kill bg fg times ulimit umask\n", BLUE);
        screen.write(
            b"Config: bind shopt alias unalias builtin command sudo apt reboot poweroff\n",
            BLUE,
        );
    }

    fn uname(&self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let (flag, _) = split_word(trim_start(arguments));
        if flag == b"-a" {
            screen.write(b"Brain-OS brainos 0.1.0 x86_64 bare-metal\n", BLUE);
        } else {
            screen.write(b"Brain-OS\n", BLUE);
        }
    }

    fn uptime(&self, screen: &mut FramebufferConsole) {
        screen.write(b"Uptime: ", BLUE);
        write_u32(screen, self.uptime_ticks, 10);
        screen.write(b" shell ticks (not wall-clock time)\n", BLUE);
    }

    fn date(&self, screen: &mut FramebufferConsole) {
        match cmos::read_datetime() {
            Ok(datetime) => {
                write_u32(screen, u32::from(datetime.year), 10);
                screen.put_char(b'-', BLUE);
                write_two_digits(screen, datetime.month);
                screen.put_char(b'-', BLUE);
                write_two_digits(screen, datetime.day);
                screen.put_char(b' ', BLUE);
                write_two_digits(screen, datetime.hour);
                screen.put_char(b':', BLUE);
                write_two_digits(screen, datetime.minute);
                screen.put_char(b':', BLUE);
                write_two_digits(screen, datetime.second);
                screen.put_char(b'\n', BLUE);
            }
            Err(cmos::CmosError::UpdateTimeout) => {
                screen.write(b"date: RTC update timed out\n", BLUE)
            }
            Err(cmos::CmosError::InvalidBcd) => {
                screen.write(b"date: RTC returned invalid BCD data\n", BLUE)
            }
        }
    }

    fn nvram(&self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let argument = trim_start(arguments);
        if argument.is_empty() {
            screen.write(b"CMOS config (read-only): ", BLUE);
            for index in 0x10..=0x17 {
                if index != 0x10 {
                    screen.put_char(b' ', BLUE);
                }
                match cmos::read_legacy_config(index) {
                    Some(value) => write_hex_byte(screen, value),
                    None => screen.write(b"??", BLUE),
                }
            }
            screen.put_char(b'\n', BLUE);
            screen.write(
                b"Syntax: nvram INDEX (decimal 16..63); writes disabled\n",
                BLUE,
            );
            return;
        }

        let (index_text, _) = split_word(argument);
        let Some(index) = parse_u32(index_text).and_then(|value| u8::try_from(value).ok()) else {
            screen.write(b"nvram: expected an index from 16 to 63\n", BLUE);
            return;
        };
        match cmos::read_legacy_config(index) {
            Some(value) => {
                screen.write(b"CMOS[", BLUE);
                write_u32(screen, u32::from(index), 10);
                screen.write(b"] = 0x", BLUE);
                write_hex_byte(screen, value);
                screen.put_char(b'\n', BLUE);
            }
            None => screen.write(b"nvram: allowed read-only indexes are 16..63\n", BLUE),
        }
    }

    fn disk(&self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let (lba_text, _) = split_word(trim_start(arguments));
        let lba = if lba_text.is_empty() {
            0
        } else if let Some(value) = parse_u32(lba_text) {
            value
        } else {
            screen.write(b"disk: expected a decimal LBA sector number\n", BLUE);
            return;
        };

        match ata_pio::read_sector(lba) {
            Ok(sector) => {
                screen.write(b"IDE primary master, LBA ", BLUE);
                write_u32(screen, lba, 10);
                screen.write(b" (first 32 bytes)\n", BLUE);
                for row in 0..2 {
                    write_u32(screen, (row * 16) as u32, 16);
                    screen.write(b": ", BLUE);
                    for byte in &sector[row * 16..row * 16 + 16] {
                        write_hex_byte(screen, *byte);
                        screen.put_char(b' ', BLUE);
                    }
                    screen.put_char(b'\n', BLUE);
                }
                screen.write(b"Read-only PIO; sector size 512 bytes\n", BLUE);
            }
            Err(AtaError::NoDevice) => screen.write(
                b"disk: no primary-master IDE disk; attach one to QEMU\n",
                BLUE,
            ),
            Err(AtaError::Timeout) => screen.write(b"disk: ATA device timed out\n", BLUE),
            Err(AtaError::DeviceFault) => screen.write(b"disk: ATA device fault\n", BLUE),
            Err(AtaError::DeviceError(code)) => {
                screen.write(b"disk: ATA error register 0x", BLUE);
                write_hex_byte(screen, code);
                screen.put_char(b'\n', BLUE);
            }
            Err(AtaError::LbaOutOfRange) => screen.write(b"disk: LBA exceeds 28-bit range\n", BLUE),
        }
    }

    fn memory(&self, screen: &mut FramebufferConsole) {
        let summary = crate::memory_map::current();
        if summary.usable_bytes == 0 {
            screen.write(b"mem: Multiboot2 memory map unavailable\n", BLUE);
            return;
        }
        let usable_mib = (summary.usable_bytes / (1024 * 1024)).min(u64::from(u32::MAX)) as u32;
        screen.write(b"RAM reported usable: ", BLUE);
        write_u32(screen, usable_mib, 10);
        screen.write(b" MiB across ", BLUE);
        write_u32(screen, summary.usable_regions, 10);
        screen.write(b" regions\n", BLUE);
        screen.write(
            b"Identity map limit: 4096 MiB; physical allocator: not implemented\n",
            BLUE,
        );
    }

    fn command_type(&self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let (name, _) = split_word(trim_start(arguments));
        if name.is_empty() {
            screen.write(b"type: expected a command name\n", BLUE);
        } else if let Some(target) = self.override_target(name) {
            screen.write(name, BLUE);
            screen.write(b" is aliased to ", BLUE);
            screen.write(target, BLUE);
            screen.put_char(b'\n', BLUE);
        } else if is_builtin(name) {
            screen.write(name, BLUE);
            screen.write(b" is a shell builtin\n", BLUE);
        } else {
            screen.write(name, BLUE);
            screen.write(b" not found\n", BLUE);
        }
    }

    fn alias(&mut self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let assignment = trim_start(arguments);
        if assignment.is_empty() {
            for entry in &self.overrides {
                if entry.active {
                    screen.write(entry.name.as_bytes(), BLUE);
                    screen.write(b"='", BLUE);
                    screen.write(entry.target.as_bytes(), BLUE);
                    screen.write(b"'\n", BLUE);
                }
            }
            return;
        }
        let (assignment, _) = split_word(assignment);
        let Some(equal) = assignment.iter().position(|&byte| byte == b'=') else {
            screen.write(b"alias: expected NAME=BUILTIN\n", BLUE);
            return;
        };
        self.set_override(screen, &assignment[..equal], &assignment[equal + 1..]);
    }

    fn unalias(&mut self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let (name, _) = split_word(trim_start(arguments));
        if let Some(entry) = self
            .overrides
            .iter_mut()
            .find(|entry| entry.active && entry.name.as_bytes() == name)
        {
            *entry = CommandOverride::EMPTY;
        } else {
            screen.write(b"unalias: no such alias\n", BLUE);
        }
    }

    fn set_command(&mut self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let arguments = trim_start(arguments);
        if arguments.is_empty() {
            for variable in &self.variables {
                if variable.active {
                    screen.write(variable.name_view(), BLUE);
                    screen.write(b"=", BLUE);
                    screen.write(variable.value.as_bytes(), BLUE);
                    screen.put_char(b'\n', BLUE);
                }
            }
        } else {
            self.dispatch(screen, arguments, 1, true);
        }
    }

    fn echo(&self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let mut rest = trim_start(arguments);
        let mut newline = true;
        let mut escapes = false;
        loop {
            let (word, tail) = split_word(rest);
            if word.is_empty() || word[0] != b'-' || word == b"-" {
                break;
            }
            match word {
                b"-n" => newline = false,
                b"-e" => escapes = true,
                b"-ne" | b"-en" => {
                    newline = false;
                    escapes = true;
                }
                _ => break,
            }
            rest = trim_start(tail);
        }
        if escapes {
            write_escaped(screen, rest);
        } else {
            write_expanded(screen, rest, &self.variables);
        }
        if newline {
            screen.put_char(b'\n', BLUE);
        }
    }

    fn printf(&self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let (format, mut values) = split_word(trim_start(arguments));
        if format.is_empty() {
            return;
        }
        let mut index = 0;
        while index < format.len() {
            match format[index] {
                b'\\' if index + 1 < format.len() => {
                    index += 1;
                    match format[index] {
                        b'n' => screen.put_char(b'\n', BLUE),
                        b't' => screen.put_char(b'\t', BLUE),
                        b'r' => screen.put_char(b'\r', BLUE),
                        b'\\' => screen.put_char(b'\\', BLUE),
                        byte => screen.put_char(byte, BLUE),
                    }
                }
                b'%' if index + 1 < format.len() => {
                    index += 1;
                    if format[index] == b'%' {
                        screen.put_char(b'%', BLUE);
                    } else {
                        let (value, tail) = split_word(trim_start(values));
                        values = tail;
                        match format[index] {
                            b's' => write_expanded(screen, value, &self.variables),
                            b'd' => match parse_i64(value) {
                                Some(number) => write_i64(screen, number, 10),
                                None => screen.write(b"0", BLUE),
                            },
                            b'x' => match parse_i64(value) {
                                Some(number) => write_i64(screen, number, 16),
                                None => screen.write(b"0", BLUE),
                            },
                            _ => {
                                screen.put_char(b'%', BLUE);
                                screen.put_char(format[index], BLUE);
                            }
                        }
                    }
                }
                byte => screen.put_char(byte, BLUE),
            }
            index += 1;
        }
    }

    fn print_history(&self, screen: &mut FramebufferConsole) {
        let first = (self.history_next + HISTORY_CAPACITY - self.history_count) % HISTORY_CAPACITY;
        let first_serial = self
            .history_serial
            .saturating_sub(self.history_count as u32);
        for offset in 0..self.history_count {
            let index = (first + offset) % HISTORY_CAPACITY;
            write_u32(screen, first_serial + offset as u32 + 1, 10);
            screen.write(b"  ", BLUE);
            screen.write(self.history[index].as_bytes(), BLUE);
            screen.put_char(b'\n', BLUE);
        }
    }

    fn spawn(&mut self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let name = trim_start(arguments);
        if name.is_empty() {
            screen.write(b"spawn: provide a simulated task name\n", BLUE);
            return;
        }
        let Some(slot) = self
            .tasks
            .iter_mut()
            .find(|task| task.state == TaskState::Empty || task.state == TaskState::Done)
        else {
            screen.write(b"spawn: task table is full\n", BLUE);
            return;
        };
        let mut task_name = FixedText::EMPTY;
        if !task_name.set(name) {
            screen.write(b"spawn: task name is too long\n", BLUE);
            return;
        }
        *slot = SimulatedTask {
            job_id: self.next_job_id,
            pid: self.next_pid,
            state: TaskState::Stopped,
            ticks: 0,
            name: task_name,
        };
        write_u32(screen, u32::from(self.next_job_id), 10);
        screen.write(b" (", BLUE);
        write_u32(screen, u32::from(self.next_pid), 10);
        screen.write(b") stopped  ", BLUE);
        screen.write(name, BLUE);
        screen.put_char(b'\n', BLUE);
        self.next_job_id = self.next_job_id.wrapping_add(1).max(1);
        self.next_pid = self.next_pid.wrapping_add(1).max(100);
    }

    fn kill(&mut self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let (pid_text, _) = split_word(trim_start(arguments));
        let Some(pid) = parse_u32(pid_text) else {
            screen.write(b"kill: expected a simulated process id\n", BLUE);
            return;
        };
        if pid > u32::from(u16::MAX) {
            screen.write(b"kill: no active simulated process with pid ", BLUE);
            write_u32(screen, pid, 10);
            screen.put_char(b'\n', BLUE);
            return;
        }
        if let Some(task) = self.tasks.iter_mut().find(|task| {
            task.pid == pid as u16
                && task.state != TaskState::Empty
                && task.state != TaskState::Done
        }) {
            task.state = TaskState::Done;
            screen.write(b"[", BLUE);
            write_u32(screen, u32::from(task.job_id), 10);
            screen.write(b"] terminated simulated pid ", BLUE);
            write_u32(screen, pid, 10);
            screen.put_char(b'\n', BLUE);
        } else {
            screen.write(b"kill: no active simulated process with pid ", BLUE);
            write_u32(screen, pid, 10);
            screen.put_char(b'\n', BLUE);
        }
    }

    fn jobs(&self, screen: &mut FramebufferConsole) {
        let mut found = false;
        for task in &self.tasks {
            if task.state == TaskState::Empty || task.state == TaskState::Done {
                continue;
            }
            found = true;
            screen.write(b"[", BLUE);
            write_u32(screen, u32::from(task.job_id), 10);
            screen.write(b"] ", BLUE);
            screen.write(
                match task.state {
                    TaskState::Running => b"Running" as &[u8],
                    TaskState::Stopped => b"Stopped",
                    _ => b"Done",
                },
                BLUE,
            );
            screen.write(b"  ", BLUE);
            screen.write(task.name.as_bytes(), BLUE);
            screen.write(b" (pid ", BLUE);
            write_u32(screen, u32::from(task.pid), 10);
            screen.write(b")\n", BLUE);
        }
        if !found {
            screen.write(b"jobs: no simulated jobs\n", BLUE);
        }
    }

    fn resume_job(&mut self, screen: &mut FramebufferConsole, arguments: &[u8], foreground: bool) {
        let argument = trim_start(arguments);
        let requested_id = if argument.is_empty() || argument == b"%+" {
            self.tasks
                .iter()
                .rev()
                .find(|task| task.state == TaskState::Stopped || task.state == TaskState::Running)
                .map(|task| task.job_id)
        } else {
            let number = argument.strip_prefix(b"%").unwrap_or(argument);
            parse_u32(number).map(|value| value as u8)
        };
        let Some(job_id) = requested_id else {
            screen.write(b"job: no matching simulated job\n", BLUE);
            return;
        };
        if let Some(task) = self.tasks.iter_mut().find(|task| {
            task.job_id == job_id && task.state != TaskState::Empty && task.state != TaskState::Done
        }) {
            task.state = TaskState::Running;
            screen.write(
                if foreground {
                    b"[foreground] "
                } else {
                    b"[background] "
                },
                BLUE,
            );
            screen.write(task.name.as_bytes(), BLUE);
            screen.write(b" is running in the simulator\n", BLUE);
        } else {
            screen.write(b"job: no active simulated job\n", BLUE);
        }
    }

    fn times(&self, screen: &mut FramebufferConsole) {
        screen.write(b"shell ", BLUE);
        write_u32(screen, self.uptime_ticks, 10);
        screen.write(b" ticks; simulated tasks ", BLUE);
        let mut total = 0u32;
        for task in &self.tasks {
            total = total.saturating_add(task.ticks);
        }
        write_u32(screen, total, 10);
        screen.write(b" ticks\n", BLUE);
    }

    fn ulimit_command(&mut self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let argument = trim_start(arguments);
        if argument.is_empty() || argument == b"-n" {
            write_u32(screen, self.ulimit, 10);
            screen.write(b" (simulated open-file limit)\n", BLUE);
            return;
        }
        let value_text = if argument.starts_with(b"-n ") {
            trim_start(&argument[3..])
        } else {
            argument
        };
        let Some(value) = parse_u32(value_text) else {
            screen.write(b"ulimit: expected a non-negative integer\n", BLUE);
            return;
        };
        if value > MAX_ULIMIT {
            screen.write(b"ulimit: exceeds kernel simulation maximum\n", BLUE);
            return;
        }
        self.ulimit = value;
        screen.write(b"ulimit set to ", BLUE);
        write_u32(screen, value, 10);
        screen.put_char(b'\n', BLUE);
    }

    fn umask_command(&mut self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let value = trim_start(arguments);
        if value.is_empty() {
            screen.write(b"0", BLUE);
            write_u32(screen, u32::from(self.umask), 8);
            screen.write(b" (simulated)\n", BLUE);
            return;
        }
        let Some(mask) = parse_octal(value) else {
            screen.write(b"umask: expected an octal mask from 0000 to 0777\n", BLUE);
            return;
        };
        if mask > 0o777 {
            screen.write(b"umask: mask is out of range\n", BLUE);
            return;
        }
        self.umask = mask as u16;
        screen.write(b"umask set to 0", BLUE);
        write_u32(screen, mask, 8);
        screen.put_char(b'\n', BLUE);
    }

    fn declare(&mut self, screen: &mut FramebufferConsole, arguments: &[u8], defaults: u8) {
        let mut attrs = defaults;
        let mut rest = trim_start(arguments);
        loop {
            let (word, tail) = split_word(rest);
            match word {
                b"-i" => attrs |= ATTR_INTEGER,
                b"-r" => attrs |= ATTR_READONLY,
                b"-x" => {}
                _ => break,
            }
            rest = trim_start(tail);
        }
        if rest.is_empty() {
            for variable in &self.variables {
                if variable.active {
                    screen.write(variable.name_view(), BLUE);
                    screen.write(b"=", BLUE);
                    screen.write(variable.value.as_bytes(), BLUE);
                    screen.put_char(b'\n', BLUE);
                }
            }
            return;
        }

        let (assignment, _) = split_word(rest);
        let (name, value) = match assignment.iter().position(|&byte| byte == b'=') {
            Some(equal) => (&assignment[..equal], Some(&assignment[equal + 1..])),
            None => (assignment, None),
        };
        if !is_name(name) {
            screen.write(b"declare: invalid variable name\n", BLUE);
            return;
        }
        let existing_attrs = self
            .find_variable(name)
            .map(|index| self.variables[index].attributes)
            .unwrap_or(0);
        if let Some(value) = value {
            self.assign(screen, name, value, attrs | existing_attrs);
        } else if let Some(index) = self.find_variable(name) {
            self.variables[index].attributes |= attrs;
        } else {
            self.assign(screen, name, b"", attrs);
        }
    }

    fn assign(&mut self, screen: &mut FramebufferConsole, name: &[u8], value: &[u8], attrs: u8) {
        if !is_name(name) || name.len() > NAME_CAPACITY || value.len() > TEXT_CAPACITY {
            screen.write(b"variable: name or value exceeds fixed capacity\n", BLUE);
            return;
        }
        if attrs & ATTR_INTEGER != 0 && !value.is_empty() && parse_i64(value).is_none() {
            screen.write(
                b"variable: integer attribute requires a decimal integer\n",
                BLUE,
            );
            return;
        }
        let slot = self
            .find_variable(name)
            .or_else(|| self.variables.iter().position(|variable| !variable.active));
        let Some(index) = slot else {
            screen.write(b"variable: variable table is full\n", BLUE);
            return;
        };
        let entry = &mut self.variables[index];
        if entry.active && entry.attributes & ATTR_READONLY != 0 && entry.value.as_bytes() != value
        {
            screen.write(b"variable: cannot modify readonly variable\n", BLUE);
            return;
        }
        entry.name = [0; NAME_CAPACITY];
        entry.name[..name.len()].copy_from_slice(name);
        entry.name_length = name.len();
        if !entry.value.set(value) {
            screen.write(b"variable: value too long\n", BLUE);
            return;
        }
        entry.attributes |= attrs;
        entry.active = true;
    }

    fn unset(&mut self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let (name, _) = split_word(trim_start(arguments));
        if let Some(index) = self.find_variable(name) {
            if self.variables[index].attributes & ATTR_READONLY != 0 {
                screen.write(b"unset: cannot unset readonly variable\n", BLUE);
            } else {
                self.variables[index] = Variable::EMPTY;
            }
        }
    }

    fn find_variable(&self, name: &[u8]) -> Option<usize> {
        self.variables
            .iter()
            .position(|variable| variable.matches(name))
    }

    fn bind(&mut self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let argument = trim_start(arguments);
        if argument.is_empty() || argument == b"-p" {
            for binding in &self.bindings {
                if binding.active {
                    screen.write(binding.key.as_bytes(), BLUE);
                    screen.write(b"=", BLUE);
                    screen.write(binding.action.as_bytes(), BLUE);
                    screen.put_char(b'\n', BLUE);
                }
            }
            return;
        }
        let (assignment, _) = split_word(argument);
        let Some(equal) = assignment.iter().position(|&byte| byte == b'=') else {
            screen.write(b"bind: expected KEY=ACTION\n", BLUE);
            return;
        };
        let key = &assignment[..equal];
        let action = &assignment[equal + 1..];
        if key.is_empty()
            || action.is_empty()
            || key.len() > TEXT_CAPACITY
            || action.len() > TEXT_CAPACITY
        {
            screen.write(b"bind: invalid or oversized binding\n", BLUE);
            return;
        }
        if key != b"ESC" || (action != b"exit" && action != b"ignore") {
            screen.write(
                b"bind: supported mappings are ESC=exit and ESC=ignore\n",
                BLUE,
            );
            return;
        }
        let slot = self
            .bindings
            .iter()
            .position(|binding| binding.active && binding.key.as_bytes() == key)
            .or_else(|| self.bindings.iter().position(|binding| !binding.active));
        let Some(index) = slot else {
            screen.write(b"bind: binding table is full\n", BLUE);
            return;
        };
        let mut binding = KeyBinding::EMPTY;
        binding.key.set(key);
        binding.action.set(action);
        binding.active = true;
        self.bindings[index] = binding;
        screen.write(b"bind: escape mapping updated\n", BLUE);
    }

    pub fn escape_exits_console(&self) -> bool {
        self.bindings
            .iter()
            .find(|binding| binding.active && binding.key.as_bytes() == b"ESC")
            .is_none_or(|binding| binding.action.as_bytes() != b"ignore")
    }

    fn option_enabled(&self, name: &[u8]) -> bool {
        self.options
            .iter()
            .find(|option| option.name == name)
            .is_some_and(|option| option.enabled)
    }

    fn shopt(&mut self, screen: &mut FramebufferConsole, arguments: &[u8]) {
        let (flag, rest) = split_word(trim_start(arguments));
        if flag == b"-s" || flag == b"-u" {
            let enabled = flag == b"-s";
            let (name, _) = split_word(trim_start(rest));
            if let Some(option) = self.options.iter_mut().find(|option| option.name == name) {
                option.enabled = enabled;
                screen.write(b"shopt: option updated\n", BLUE);
            } else {
                screen.write(b"shopt: unknown option\n", BLUE);
            }
        } else {
            for option in &self.options {
                screen.write(if option.enabled { b"on  " } else { b"off " }, BLUE);
                screen.write(option.name, BLUE);
                screen.put_char(b'\n', BLUE);
            }
        }
    }

    fn command_builtin(&mut self, screen: &mut FramebufferConsole, arguments: &[u8], depth: usize) {
        let (first, rest) = split_word(trim_start(arguments));
        if first == b"-v" {
            let (name, _) = split_word(trim_start(rest));
            if let Some(target) = self.override_target(name) {
                screen.write(b"override -> ", BLUE);
                screen.write(target, BLUE);
                screen.put_char(b'\n', BLUE);
            } else if is_builtin(name) {
                screen.write(b"shell builtin\n", BLUE);
            } else {
                screen.write(b"not found\n", BLUE);
            }
            return;
        }
        if first == b"map" {
            let (name, tail) = split_word(trim_start(rest));
            let (target, _) = split_word(trim_start(tail));
            self.set_override(screen, name, target);
            return;
        }
        if first.is_empty() {
            screen.write(
                b"command: expected a command name or map NAME BUILTIN\n",
                BLUE,
            );
            return;
        }
        self.dispatch_named(screen, first, rest, depth + 1, false);
    }

    fn set_override(&mut self, screen: &mut FramebufferConsole, name: &[u8], target: &[u8]) {
        if !is_name(name)
            || !is_builtin(target)
            || name.len() > TEXT_CAPACITY
            || target.len() > TEXT_CAPACITY
        {
            screen.write(
                b"command: map requires a valid name and builtin target\n",
                BLUE,
            );
            return;
        }
        let slot = self
            .overrides
            .iter()
            .position(|entry| entry.active && entry.name.as_bytes() == name)
            .or_else(|| self.overrides.iter().position(|entry| !entry.active));
        let Some(index) = slot else {
            screen.write(b"command: override table is full\n", BLUE);
            return;
        };
        let mut entry = CommandOverride::EMPTY;
        entry.name.set(name);
        entry.target.set(target);
        entry.active = true;
        self.overrides[index] = entry;
        screen.write(b"command: override installed\n", BLUE);
    }

    fn override_target(&self, name: &[u8]) -> Option<&[u8]> {
        self.overrides
            .iter()
            .find(|entry| entry.active && entry.name.as_bytes() == name)
            .map(|entry| entry.target.as_bytes())
    }

    fn until_loop(&mut self, screen: &mut FramebufferConsole, arguments: &[u8], depth: usize) {
        let arguments = trim_start(arguments);
        let Some(do_index) = find_token(arguments, b"do") else {
            screen.write(
                b"until: syntax is until CONDITION do COMMAND [done]\n",
                BLUE,
            );
            return;
        };
        let condition = trim_start(&arguments[..do_index]);
        let mut body = trim_start(&arguments[do_index + 2..]);
        if body.ends_with(b"done") {
            body = trim_end(&body[..body.len() - 4]);
        }
        if body.is_empty() {
            screen.write(b"until: missing loop body\n", BLUE);
            return;
        }
        for _ in 0..MAX_UNTIL_ITERATIONS {
            if self.condition_true(condition) {
                return;
            }
            self.dispatch(screen, body, depth + 1, true);
            self.uptime_ticks = self.uptime_ticks.saturating_add(1);
        }
        screen.write(b"until: stopped at safe iteration limit\n", BLUE);
    }

    fn condition_true(&self, condition: &[u8]) -> bool {
        let condition = trim_start(condition);
        if condition == b"true" {
            return true;
        }
        if condition == b"false" || condition.is_empty() {
            return false;
        }
        let condition = condition.strip_prefix(b"test ").unwrap_or(condition);
        let (left, rest) = split_word(condition);
        let (operator, operator_rest) = split_word(trim_start(rest));
        let (right, _) = split_word(trim_start(operator_rest));
        if left == b"-n" {
            let (operand, _) = split_word(rest);
            return !operand.is_empty();
        }
        if left == b"-z" {
            let (operand, _) = split_word(rest);
            return operand.is_empty();
        }
        if left == b"uptime" {
            let current = i64::from(self.uptime_ticks);
            return compare_numbers_with_left(current, operator, right);
        }
        match operator {
            b"-eq" => compare_numbers(left, right, |a, b| a == b),
            b"-ne" => compare_numbers(left, right, |a, b| a != b),
            b"-gt" => compare_numbers(left, right, |a, b| a > b),
            b"-ge" => compare_numbers(left, right, |a, b| a >= b),
            b"-lt" => compare_numbers(left, right, |a, b| a < b),
            b"-le" => compare_numbers(left, right, |a, b| a <= b),
            b"==" => left == right,
            b"!=" => left != right,
            _ => left == b"true",
        }
    }
}

impl Variable {
    fn name_view(&self) -> &[u8] {
        &self.name[..self.name_length]
    }
}

fn is_builtin(name: &[u8]) -> bool {
    matches!(
        name,
        b"echo"
            | b"printf"
            | b"history"
            | b"kill"
            | b"jobs"
            | b"bg"
            | b"fg"
            | b"times"
            | b"ulimit"
            | b"umask"
            | b"declare"
            | b"typeset"
            | b"local"
            | b"readonly"
            | b"unset"
            | b"bind"
            | b"shopt"
            | b"builtin"
            | b"command"
            | b"until"
            | b"spawn"
            | b"sudo"
            | b"apt"
            | b"commands"
            | b"uname"
            | b"uptime"
            | b"date"
            | b"disk"
            | b"nvram"
            | b"mem"
            | b"drivers"
            | b"id"
            | b"hostname"
            | b"type"
            | b"alias"
            | b"unalias"
            | b"test"
            | b"set"
            | b"export"
            | b"ps"
            | b"ls"
            | b"cat"
            | b"df"
            | b"mount"
            | b"free"
            | b"reboot"
            | b"poweroff"
            | b"help"
            | b"apps"
            | b"pwd"
            | b"whoami"
            | b"clear"
    )
}

fn is_name(name: &[u8]) -> bool {
    let Some((&first, rest)) = name.split_first() else {
        return false;
    };
    (first == b'_' || first.is_ascii_alphabetic())
        && rest
            .iter()
            .all(|byte| *byte == b'_' || byte.is_ascii_alphanumeric())
}

fn trim_start(mut text: &[u8]) -> &[u8] {
    while text.first().is_some_and(u8::is_ascii_whitespace) {
        text = &text[1..];
    }
    text
}

fn trim_end(mut text: &[u8]) -> &[u8] {
    while text.last().is_some_and(u8::is_ascii_whitespace) {
        text = &text[..text.len() - 1];
    }
    text
}

fn split_word(text: &[u8]) -> (&[u8], &[u8]) {
    let text = trim_start(text);
    let end = text
        .iter()
        .position(u8::is_ascii_whitespace)
        .unwrap_or(text.len());
    (&text[..end], &text[end..])
}

fn find_token(text: &[u8], token: &[u8]) -> Option<usize> {
    let mut offset = 0;
    while offset < text.len() {
        while offset < text.len() && text[offset].is_ascii_whitespace() {
            offset += 1;
        }
        if offset == text.len() {
            break;
        }
        let start = offset;
        while offset < text.len() && !text[offset].is_ascii_whitespace() {
            offset += 1;
        }
        if &text[start..offset] == token {
            return Some(start);
        }
    }
    None
}

fn parse_i64(text: &[u8]) -> Option<i64> {
    if text.is_empty() {
        return None;
    }
    let (negative, digits) = if text[0] == b'-' {
        (true, &text[1..])
    } else {
        (false, text)
    };
    let mut value = 0i64;
    for &byte in digits {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(i64::from(byte - b'0'))?;
    }
    if negative {
        value.checked_neg()
    } else {
        Some(value)
    }
}

fn parse_u32(text: &[u8]) -> Option<u32> {
    let value = parse_i64(text)?;
    u32::try_from(value).ok()
}

fn parse_octal(text: &[u8]) -> Option<u32> {
    if text.is_empty() {
        return None;
    }
    let mut value = 0u32;
    for &byte in text {
        if !(b'0'..=b'7').contains(&byte) {
            return None;
        }
        value = value.checked_mul(8)?.checked_add(u32::from(byte - b'0'))?;
    }
    Some(value)
}

fn compare_numbers(left: &[u8], right: &[u8], predicate: impl FnOnce(i64, i64) -> bool) -> bool {
    match (parse_i64(left), parse_i64(right)) {
        (Some(left), Some(right)) => predicate(left, right),
        _ => false,
    }
}

fn compare_numbers_with_left(left: i64, operator: &[u8], right: &[u8]) -> bool {
    let Some(right) = parse_i64(right) else {
        return false;
    };
    match operator {
        b"-eq" => left == right,
        b"-ne" => left != right,
        b"-gt" => left > right,
        b"-ge" => left >= right,
        b"-lt" => left < right,
        b"-le" => left <= right,
        _ => false,
    }
}

fn write_expanded(
    screen: &mut FramebufferConsole,
    text: &[u8],
    variables: &[Variable; VARIABLE_CAPACITY],
) {
    let mut index = 0;
    while index < text.len() {
        if text[index] == b'$' {
            let start = index + 1;
            let mut end = start;
            while end < text.len() && (text[end] == b'_' || text[end].is_ascii_alphanumeric()) {
                end += 1;
            }
            if end > start {
                if let Some(variable) = variables
                    .iter()
                    .find(|variable| variable.matches(&text[start..end]))
                {
                    screen.write(variable.value.as_bytes(), BLUE);
                }
                index = end;
                continue;
            }
        }
        screen.put_char(text[index], BLUE);
        index += 1;
    }
}

fn write_escaped(screen: &mut FramebufferConsole, text: &[u8]) {
    let mut index = 0;
    while index < text.len() {
        if text[index] == b'\\' && index + 1 < text.len() {
            index += 1;
            match text[index] {
                b'n' => screen.put_char(b'\n', BLUE),
                b't' => screen.put_char(b'\t', BLUE),
                b'\\' => screen.put_char(b'\\', BLUE),
                b'c' => return,
                byte => {
                    screen.put_char(b'\\', BLUE);
                    screen.put_char(byte, BLUE);
                }
            }
        } else {
            screen.put_char(text[index], BLUE);
        }
        index += 1;
    }
}

fn write_u32(screen: &mut FramebufferConsole, value: u32, radix: u32) {
    let mut digits = [0u8; 16];
    let mut cursor = digits.len();
    let mut number = value;
    loop {
        let digit = (number % radix) as u8;
        cursor -= 1;
        digits[cursor] = if digit < 10 {
            b'0' + digit
        } else {
            b'a' + digit - 10
        };
        number /= radix;
        if number == 0 {
            break;
        }
    }
    screen.write(&digits[cursor..], BLUE);
}

fn write_two_digits(screen: &mut FramebufferConsole, value: u8) {
    screen.put_char(b'0' + value / 10, BLUE);
    screen.put_char(b'0' + value % 10, BLUE);
}

fn write_hex_byte(screen: &mut FramebufferConsole, value: u8) {
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    screen.put_char(DIGITS[(value >> 4) as usize], BLUE);
    screen.put_char(DIGITS[(value & 0x0F) as usize], BLUE);
}

fn write_i64(screen: &mut FramebufferConsole, value: i64, radix: u32) {
    if value < 0 && radix == 10 {
        screen.put_char(b'-', BLUE);
    }
    let magnitude = value.unsigned_abs();
    let mut digits = [0u8; 16];
    let mut cursor = digits.len();
    let mut number = magnitude;
    loop {
        let digit = (number % u64::from(radix)) as u8;
        cursor -= 1;
        digits[cursor] = if digit < 10 {
            b'0' + digit
        } else {
            b'a' + digit - 10
        };
        number /= u64::from(radix);
        if number == 0 {
            break;
        }
    }
    screen.write(&digits[cursor..], BLUE);
}
