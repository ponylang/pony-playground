#!/bin/sh

set -o errexit

ponyc --version

DIR="$(mktemp -d)"
mkdir "$DIR/main"
cd "$DIR/main"
cat > main.pony

ponyc --debug "$@"
printf '\377' # 255 in octal

if [ -f main.ll ]; then cat main.ll; fi
if [ -f main.s ]; then cat main.s; fi
rm -rf main.*
