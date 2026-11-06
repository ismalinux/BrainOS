# Brain OS

Brain OS is a from-scratch 32-bit x86 operating-system project written in C and assembly. Its current kernel boots through GRUB Multiboot v1 and provides a VGA text terminal with polled PS/2 keyboard input.

The starter shell displays `user@Brain-OS:~$` and supports `help`, `echo`, `clear`, `pwd`, `whoami`, and `apps`. The authenticated login, graphical desktop, and graphical applications are not implemented yet.

## Build

GCC, NASM, and GNU binutils are needed to compile the kernel ELF:

```sh
make kernel
```

To create a bootable GRUB ISO and run it in QEMU, install GRUB PC BIOS tools, xorriso, and QEMU, then run:

```sh
make iso
make run
```

The ISO target creates `build/brainos.iso`. The current starter image is small; the planned approximately 2.20 GB release image should contain real system components and apps, not filler data. See [ROADMAP.md](ROADMAP.md) for implemented and planned milestones.
