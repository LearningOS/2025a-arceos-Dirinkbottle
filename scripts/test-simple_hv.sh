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

 (while true; do echo " "; sleep 0.05; done) 2>/dev/null |  make run A=exercises/simple_hv/ BLK=y 2>/dev/null | tee $tmp_file

output=$(grep -a "$grep_content" $tmp_file | tail -n1  )

rm -rf $tmp_file 

if [[ -z "$output" ]]; then
    echo "simple_hv default"
    exit 1
else 
    echo "simple_hv pass"
    exit 0
fi
