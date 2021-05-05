# Summary

This is an operating system development ("OsDev") project shadowing
gamozolabs' FuzzOS as a learning experience.

Attributes inherited from the parents project are: Written in Rust with minor
assembler inclusions, no third party dependencies at runtime, support for
64-bit x86, 64-bit arm and attempted support for 64-bit risc-v.

# Building

### Requirements

To build this, you need Rust beta or nightly toolchain.

### Building

To build x86, just run `cargo build`.

# Usage

To run this in a VM, Qemu and Bash are required. Just run the shell scripts
called `qemu-x86_64.sh` or `qemu-aarch64.sh` to build FoobOS and launch it in
a Qemu virtual machine.

Both scripts use Qemu's built-in TFTP- and DHCP-emulation to PXE-boot. There
is no disk support at all.
