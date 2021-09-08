#!/bin/bash
cd ../libp2p-benchmark
for i in $(seq 1 1 $1); do
  echo "Number: $i"
  ./target/release/libp2p-benchmark 0 $1 &> /dev/null &
done

