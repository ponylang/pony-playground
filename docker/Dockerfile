FROM ghcr.io/ponylang/ponyc:release

# Package self-tests are typechecked on import and add unused dependencies.
RUN packages="$(dirname "$(dirname "$(readlink -f "$(command -v ponyc)")")")/packages" \
    && test -d "$packages" \
    && find "$packages" -type f -name '_test.pony' -delete \
    && find "$packages" -path "$packages/pony_test" -prune -o \
       -type f -name '_test_*.pony' -exec rm {} + \
    && rm "$packages/json/_stream_test.pony" \
          "$packages/files/_non_root_test.pony"

COPY bin/compile.sh bin/evaluate.sh /usr/local/bin/

ENV CC=gcc
WORKDIR /tmp
