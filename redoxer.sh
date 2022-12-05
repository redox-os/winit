#!/usr/bin/env bash

set -ex

if [ -n "$1" ]
then
    EXAMPLE="$1"
else
    EXAMPLE=window
fi

rm -rf target/redoxer
mkdir -p target/redoxer

redoxer install \
    --no-track \
    --path . \
    --example "${EXAMPLE}" \
    --root "target/redoxer"

cd target/redoxer

redoxer exec \
    --gui \
    --folder . \
    "./bin/${EXAMPLE}"
