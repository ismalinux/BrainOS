# Brain OS Roadmap

## Boot and Kernel
- [x] Add an x86_64 long-mode assembly trampoline with a Multiboot2 header and identity-mapped boot memory.
- [x] Link a freestanding Rust ELF with page-aligned sections and a GRUB Multiboot2 menu entry.
- [x] Request and validate a Multiboot2 32-bit RGB framebuffer, then render the desktop, shell, and recovery screens through bounded framebuffer writes.
- [ ] Build the starter GRUB ISO and verify boot in QEMU (requires GRUB, xorriso, and QEMU).
- [ ] Target an approximately 2.20 GB release ISO using real system components, applications, browser runtime, fonts, and assets rather than padding.
- [ ] Add serial logging, CPU exception handlers, an IDT, and interrupt support.
- [ ] Add Multiboot memory-map handling and physical/virtual memory management.

## Security
- [ ] Define a threat model and privilege boundaries before adding untrusted applications or network-facing services.
- [ ] After process isolation, a VFS, file descriptors, and syscalls exist, implement Landlock-inspired least-privilege filesystem policies with denial tests. Linux Landlock itself is not directly usable by this custom kernel.

## Shell and Input
- [x] Add polled PS/2 keyboard input with basic US key mapping, Enter, and Backspace.
- [x] Add an editable framebuffer-rendered command line with the prompt `user@Brain-OS:~$`.
- [x] Add starter commands: `help`, `echo`, `clear`, `pwd`, `whoami`, and `apps`.
- [x] Add fixed-capacity shell commands for `printf`, `history`, echo flags, simulated jobs/processes, `times`, `ulimit`, `umask`, variable attributes, `bind`, `shopt`, aliases, and bounded `until` loops.
- [x] Recognize `sudo`/APT syntax and explain missing prerequisites without pretending package operations succeeded.
- [ ] Verify keyboard input and shell commands in QEMU.
- [ ] Add an interrupt-driven keyboard driver and improve the command parser.
- [ ] Add filesystem-backed `cd`, `export`, `unset`, and other stateful built-ins.
- [ ] Continue toward broader Bash compatibility, including `read`, `shift`, `wait`, script parsing, and real process/job control.
- [ ] Implement authenticated privilege separation and real package operations after user accounts, a VFS, networking/TLS, trusted time, and package signature verification exist.
- [ ] Add timer and keyboard drivers with interrupt-driven input.
- [ ] Add mouse input and a shared input-event layer.

## Network and Browser
- [ ] Add an Ethernet driver and network interfaces.
- [ ] Implement IPv4, ICMP, UDP, TCP, and DNS in dependency order.
- [ ] Add TLS using a maintained cryptographic library; provide a secure random source, certificate validation, and a trustworthy clock first.
- [ ] Establish userspace, filesystem, and socket APIs before integrating browser components.
- [ ] Build a browser in stages: HTML parsing and text layout, CSS subsets, then JavaScript through a maintained engine if platform support allows.
- [ ] Clearly document unsupported web standards; do not claim standards-complete browser support.

## Graphics and Desktop
- [ ] Verify framebuffer graphics and RGB channel order in QEMU at the requested resolution.
- [ ] Develop the desktop UI in a host-runnable Slint preview; integrate it only after graphics, input events, and a custom platform backend exist.
- [ ] Build a native window system and desktop shell with a taskbar, launcher, and workspaces.
- [ ] Add foundational applications: terminal, file manager, calculator, settings, and text editor.
- [ ] Add local user accounts and a sign-in flow appropriate to an offline OS.
- [ ] Investigate browser and media-player feasibility after storage, networking, graphics, and audio foundations exist.

## Polish
- [ ] Refine typography, icons, focus states, error handling, and keyboard navigation.
- [ ] Add automated build checks and documented emulator test steps.

COSMIC, GNOME, and KDE Plasma depend on substantial Linux kernel and userspace infrastructure. For this custom kernel, use them as design references unless a dedicated porting effort is planned.
