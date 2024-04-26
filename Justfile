set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

BaseFile := os()

run *param:
    just -f _scripts/{{BaseFile}}.just run {{param}}

test *param:
    just -f _scripts/{{BaseFile}}.just test {{param}}

build *param:
    just -f _scripts/{{BaseFile}}.just build {{param}}

build-debug *param:
    just -f _scripts/{{BaseFile}}.just build-debug {{param}}

debug *param:
    just -f _scripts/{{BaseFile}}.just debug {{param}}

profile *param:
    just -f _scripts/{{BaseFile}}.just profile {{param}}

clippy *param:
    just -f _scripts/{{BaseFile}}.just clippy {{param}}

install-deps *param:
    just -f _scripts/{{BaseFile}}.just install-deps {{param}}
