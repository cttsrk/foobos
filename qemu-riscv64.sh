#!/bin/bash

set -e

#cargo build --target .cargo/riscv64-unknown-uefi.json

qemu-system-risvc64 \
    -machine virt \
    -smp 2 \
    -m 512 \
    -nographic \
    -bios ovmf-x64/OVMF_CODE-pure-efi.fd \
    -device driver=e1000,netdev=n0 \
    -netdev user,id=n0,tftp=target/x86_64-unknown-uefi/debug,bootfile=foobos.efi
