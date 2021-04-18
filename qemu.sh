#!/bin/bash

set -e

cargo build
qemu-system-x86_64 \
    -smp 2 \
    -accel hvf \
    -m 128 \
    -nographic \
    -bios ovmf-x64/OVMF_CODE-pure-efi.fd \
    -device driver=e1000,netdev=n0 \
    -netdev user,id=n0,tftp=target/x86_64-unknown-uefi/debug,bootfile=foobos.efi \
    2>&1 | \
    sed -e 's/.*DVD-ROM.*//' -e 's/.*CPUID.80000001H.ECX.svm.*//'

