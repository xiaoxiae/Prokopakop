#!/bin/bash

cargo build --release
cp target/release/prokopakop target/release/prokopakop-$(git rev-parse HEAD)
