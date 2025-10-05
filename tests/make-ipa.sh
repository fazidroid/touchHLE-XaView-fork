#!/bin/sh
set -x
rm -r TestApp.ipa Payload/
mkdir Payload
ln -s ../TestApp.app Payload/TestApp.app
zip -r TestApp.ipa Payload/
