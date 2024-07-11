import 'common.just'

export OPENSSL_NO_VENDOR := "1"

[no-cd]
install-deps:
    #!/bin/sh
    set -e

    mkdir -p "{{ExtDir}}"
    cd {{ExtDir}}

    # FFMPEG

    if [ ! -d "{{FFmpeg}}" ]; then
        echo "Downloading {{FFmpeg}}"
        wget "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/{{FFmpeg}}.tar.xz"
        tar -xf {{FFmpeg}}.tar.xz
        rm {{FFmpeg}}.tar.xz
    fi

    # OpenSSL

    if [ ! -d "openssl" ]; then
        echo "Downloading OpenSSL"
        git clone --depth 1 https://github.com/openssl/openssl.git
        cd openssl
        ./config
        make
        cd ..
    fi

[no-cd]
build *param:
    cargo build -vvv --release -- {{param}}

[no-cd]
build-debug *param:
    cargo build -- {{param}}

[no-cd]
run *param:
    cargo run --release -- {{param}}

[no-cd]
test *param:
    cargo test -- {{param}}

[no-cd]
clippy:
    cargo clippy

[no-cd]
debug *param:
    cargo run -- {{param}}

[no-cd]
profile *param:
    cargo run --profile profile -- {{param}}
