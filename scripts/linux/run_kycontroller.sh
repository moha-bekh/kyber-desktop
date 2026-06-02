#!/bin/bash

BASE_DIR=$(realpath $(dirname $0))

export PATH=$BASE_DIR/bin:$PATH
export LD_LIBRARY_PATH=$BASE_DIR/lib:$BASE_DIR/lib/x86_64-linux-gnu:$BASE_DIR/lib64

kycontroller "$@"
