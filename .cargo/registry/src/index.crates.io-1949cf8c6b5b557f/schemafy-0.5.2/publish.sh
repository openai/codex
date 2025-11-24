#!/bin/sh

(./version.sh $@ &&
    (cd schemafy_core && cargo publish) &&
    sleep 15 &&
    (cd schemafy_lib && cargo publish) &&
    sleep 15 &&
    cargo publish)
