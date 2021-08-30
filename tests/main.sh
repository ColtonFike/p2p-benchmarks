#!/bin/bash
cd ../libp2p-benchmark
for i in $(seq 1 1 $1); do
  echo "Number: $i"
  ./target/debug/libp2p-benchmark 0 $1 &> /dev/null &
done

sleep 120
killall ./target/debug/libp2p-benchmark
