# Release Packaging: Shell Completions

## Generated Files

After building `roost`, completion scripts are generated in:
```
target/<profile>/build/roost-<hash>/out/
├── roost.bash      # Bash completions
├── _roost          # Zsh completions
├── roost.fish      # Fish completions
└── _roost.ps1      # PowerShell completions
```

These files are produced by `build.rs` using `clap_complete` and reflect the exact CLI structure of the built binary.

## Release Checklist

### 1. Build Release Binary
```bash
cargo build --release
```

### 2. Collect Completion Files
```bash
# Find the latest build output directory
OUT_DIR=$(find target/release/build -name 'roost-*' -type d | head -1)/out

# Copy to a staging directory for packaging
mkdir -p completions
"$OUT_DIR"/roost.bash completions/
"$OUT_DIR"/_roost completions/
"$OUT_DIR"/roost.fish completions/
"$OUT_DIR"/_roost.ps1 completions/
```

### 3. Package for Distribution

Include the `completions/` directory alongside the binary in your release tarball/zip.

Example tarball structure:
```
roost-0.2.0/
├── roost                    # Binary
└── completions/
    ├── roost.bash
    ├── _roost
    ├── roost.fish
    └── _roost.ps1
```

### 4. Installation Instructions for Users

After extracting the release archive, users should copy completions to their shell's directory:

**Bash:**
```bash
mkdir -p ~/.bash_completion.d
cp completions/roost.bash ~/.bash_completion.d/
echo 'source ~/.bash_completion.d/roost.bash' >> ~/.bashrc
```

**Zsh:**
```bash
mkdir -p ~/.zsh/completions
cp completions/_roost ~/.zsh/completions/
echo 'fpath+=(~/.zsh/completions)' >> ~/.zshrc
```

**Fish:**
```bash
mkdir -p ~/.config/fish/completions
cp completions/roost.fish ~/.config/fish/completions/
```

**PowerShell:**
```powershell
$completionsDir = "$PROFILE\..\Completions"
New-Item -ItemType Directory -Force -Path $completionsDir
Copy-Item completions\_roost.ps1 $completionsDir\
```

## Package Manager Integration

### Homebrew Formula
Homebrew has a `bash_completion`, `zsh_completion`, and `fish_completion` DSL:
```ruby
def install
  bin.install "roost"
  bash_completion.install "completions/roost.bash" => "roost"
  zsh_completion.install "completions/_roost" => "_roost"
  fish_completion.install "completions/roost.fish"
end
```

### Debian/Ubuntu (.deb)
Install to:
- `/usr/share/bash-completion/completions/roost`
- `/usr/share/zsh/site-functions/_roost`
- `/usr/share/fish/vendor_completions.d/roost.fish`

### Arch Linux (PKGBUILD)
Install to:
- `/usr/share/bash-completion/completions/roost`
- `/usr/share/zsh/site-functions/_roost`
- `/usr/share/fish/vendor_completions.d/roost.fish`

## Automation Script

A helper script `scripts/package-release.sh` can automate the collection and packaging:

```bash
#!/bin/bash
set -euo pipefail

VERSION=$(grep '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
PROFILE="release"
TARGET="target/${PROFILE}"
OUT_DIR=$(find "${TARGET}/build" -name 'roost-*' -type d | head -1)/out

# Build
cargo build --"${PROFILE}"

# Stage
mkdir -p "dist/roost-${VERSION}/completions"
cp "${TARGET}/roost" "dist/roost-${VERSION}/"
cp "${OUT_DIR}"/roost.bash "dist/roost-${VERSION}/completions/"
cp "${OUT_DIR}"/_roost "dist/roost-${VERSION}/completions/"
cp "${OUT_DIR}"/roost.fish "dist/roost-${VERSION}/completions/"
cp "${OUT_DIR}"/_roost.ps1 "dist/roost-${VERSION}/completions/"

# Package
cd dist
tar czf "roost-${VERSION}.tar.gz" "roost-${VERSION}"
echo "Created dist/roost-${VERSION}.tar.gz"
```
