#!/bin/bash

tmp_file=hv_output.txt
grep_content="Shutdown vm normally!"

cd arceos/ || exit


rm pflash.img
rm disk.img

make pflash_img
make disk_img

make payload
./update_disk.sh payload/skernel2/skernel2

# Create a helper script to feed periodic input to work around QEMU hang in pipe mode
#qemu是事件驱动型，qemu会默认认为没有输入就不会去检查其它设备（没有其它事件的情况）。导致卡死.... 目前这个也有几率卡死
 (while true; do echo " "; sleep 0.05; done) |  make run A=exercises/simple_hv/ BLK=y 2>/dev/null | tee $tmp_file

output=$(grep -a "$grep_content" $tmp_file | tail -n1  )

rm -rf $tmp_file 

if [[ -z "$output" ]]; then
    echo "simple_hv default"
    exit 1
else 
    echo "simple_hv pass"
    exit 0
fi
