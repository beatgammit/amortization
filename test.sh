#!/usr/bin/env sh

_TEST_DB=test.sqlite
_TEST_LOAN=test
_TEST_APR=3.75
_TEST_BALANCE=213100
_TEST_TERM=30
_TEST_START=2016-04-01

_BIN=./target/debug/amortization

rm -f $_TEST_DB

cargo build

$_BIN init $_TEST_DB
$_BIN create $_TEST_DB $_TEST_LOAN --apr $_TEST_APR --balance $_TEST_BALANCE --term $_TEST_TERM --start $_TEST_START
$_BIN $_TEST_DB

rm -f $_TEST_DB
