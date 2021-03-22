#!/bin/bash

systemctl stop monkey

cp ./target/release/monkey /home/monkey
chown -R monkey:monkey /home/monkey
setcap 'cap_net_bind_service=+ep' /home/monkey/monkey

systemctl start monkey