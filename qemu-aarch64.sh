#!/bin/bash

set -e

cargo build --release --target .cargo/aarch64-unknown-uefi.json

qemu-system-aarch64 \
    -M virt \
    -smp 2 \
    -m 512 \
    -cpu cortex-a57 \
    -nographic \
    -bios firmware/edk2-aarch64-code.fd \
    -device driver=virtio-net,netdev=n0 \
    -netdev user,id=n0,tftp=target/aarch64-unknown-uefi/release,bootfile=foobos.efi
