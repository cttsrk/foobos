#!/bin/bash

set -e

cargo build
qemu-system-aarch64 \
    -M virt \
    -smp 2 \
    -m 128 \
    -cpu cortex-a53 \
    -nographic \
    -device driver=e1000,netdev=n0 \
    -netdev user,id=n0,tftp=target/x86_64-unknown-uefi/debug,bootfile=foobos.efi

    #2>&1 | sed \
    #-e 's/.*DVD-ROM.*/truncated DVD-ROM msg/' \
    #-e 's/.*CPUID.80000001H.ECX.svm.*/truncated CPUID msg/' \
    #-e 's/BdsDxe.*/truncated netboot msg/'

