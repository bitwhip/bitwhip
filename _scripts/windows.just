import 'common.just'

VcpkgArch := "x64-windows-static-md"

export VCPKG_ROOT := ProjectDir / "ext/vcpkg"

# Use our vcpkg openssl
export OPENSSL_NO_VENDOR := "1"

[no-cd]
install-deps:
    #!powershell
    $ProgressPreference = 'SilentlyContinue'
    $ErrorActionPreference = 'Stop'

    mkdir "{{ExtDir}}" -ErrorAction SilentlyContinue
    cd {{ExtDir}}

    # FFMPEG

    if (-not (Test-Path -Path "{{FFmpeg}}")) {
        echo "Downloading {{FFmpeg}}"
        wget "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/{{FFmpeg}}.zip" -outfile "{{FFmpeg}}.zip"
        7z x -y {{FFmpeg}}.zip
        del {{FFmpeg}}.zip
    }

    # OpenSSL

    if (-not (Test-Path -Path "vcpkg/installed/{{VcpkgArch}}/lib/openssl")) {
        echo "Downloading OpenSSL"
        rm -Recurse -Force \vcpkg -ErrorAction SilentlyContinue
        git clone --depth 1 https://github.com/Microsoft/vcpkg.git
        .\vcpkg\bootstrap-vcpkg.bat -disableMetrics
        .\vcpkg\vcpkg install "openssl[core]:{{VcpkgArch}}"
    }

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