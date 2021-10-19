#!/usr/bin/env bash

# this is just an ugly script to do some simple e2e test
# for locally running server/client
# TODO:
# add missing tests for
# - make request to endpoint without certificate
# - use invalid certificate
# - use multiple clients and verify that they only observe their own jobs

cargo build 

target/debug/rcmd_server tls-certs &> /dev/null &
server_pid=$!
sleep 2

assert_eq() {
  expected=$1
  got=$2
  if [[ "${expected}" == "${got}" ]]; then
    echo Ok
  else
    echo "Fail, expected ${expected}, got ${got}"
  fi
}

echo "TEST: create echo job"
output=$(target/debug/rcmd_client tls-certs localhost exec echo hi)
expected="0"
assert_eq "${expected}" "${output}"

echo "TEST: list jobs"
output=$(target/debug/rcmd_client tls-certs localhost list)
expected="0: echo hi"
assert_eq "${expected}" "${output}"

sleep 0.1
echo "TEST: status echo job"
output=$(target/debug/rcmd_client tls-certs localhost status 0)
expected="Completed { exit_code: 0 }"
assert_eq "${expected}" "${output}"

echo "TEST output echo job"
output=$(target/debug/rcmd_client tls-certs localhost output 0)
expected=$'___STDOUT___\nhi\n___STDERR___'
assert_eq "${expected}" "${output}"

echo "TEST delete echo job"
output=$(target/debug/rcmd_client tls-certs localhost delete 0)
expected="0 deleted"
assert_eq "${expected}" "${output}"

echo "TEST status deleted job"
output=$(target/debug/rcmd_client tls-certs localhost status 0)
expected="Job not found"
assert_eq "${expected}" "${output}"

echo "TEST output deleted job"
output=$(target/debug/rcmd_client tls-certs localhost output 0)
expected="Job not found"
assert_eq "${expected}" "${output}"

echo "TEST delete deleted job"
output=$(target/debug/rcmd_client tls-certs localhost delete 0)
expected="Job not found"
assert_eq "${expected}" "${output}"

echo "TEST: create sleep job"
output=$(target/debug/rcmd_client tls-certs localhost exec sleep 2)
expected="1"
assert_eq "${expected}" "${output}"

echo "TEST: status sleep job"
output=$(target/debug/rcmd_client tls-certs localhost status 1)
expected="Running"
assert_eq "${expected}" "${output}"

echo "TEST: create invalid job"
output=$(target/debug/rcmd_client tls-certs localhost exec aldjfiowed)
expected="2"
assert_eq "${expected}" "${output}"

echo "TEST: status invalid job"
output=$(target/debug/rcmd_client tls-certs localhost status 2)
expected="Error { msg: \"No such file or directory (os error 2)\" }"
assert_eq "${expected}" "${output}"

kill -2 $server_pid
wait $server_pid