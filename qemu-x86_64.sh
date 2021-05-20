#!/bin/bash

set -e

cargo build --release --target x86_64-unknown-uefi

qemu-system-x86_64 \
    -smp 2 \
    -accel hvf \
    -m 512 \
    -nographic \
    -bios firmware/OVMF_CODE-pure-efi.fd \
    -device driver=e1000,netdev=n0 \
    -netdev user,id=n0,tftp=target/x86_64-unknown-uefi/release,bootfile=foobos.efi \

    #2>&1 | sed \
    #-e 's/.*DVD-ROM.*/truncated QEMU DVD-ROM msg/' \
    #-e 's/.*CPUID.80000001H.ECX.svm.*/truncated QEMU CPUID msg/' \
    #-e 's/BdsDxe.*/truncated QEMU netboot msg/'

