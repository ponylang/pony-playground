#!/bin/sh

set -o errexit

ponyc --version

DIR="$(mktemp -d)"
mkdir "$DIR/main"
cd "$DIR/main"
cat > main.pony

if [ "$1" = "--pass=asm" ]; then
  ponyc --debug
  printf '\377' # 255 in octal
  objdump -d main
else
  ponyc --debug "$@"
  printf '\377' # 255 in octal
  cat main.main.ll
fi
rm -rf main.*
