# ADR-010: Packaging Pipeline Hardening

**Status**: Proposed  
**Date**: 2026-08-28  
**Deciders**: ShareCLI maintainers  
**Addresses**: L108 macOS DMG (2/3), L109 Windows MSI (2/3), L110 Linux DEB (2/3), L111 Homebrew Bottle (1/3), C11 cluster, audit-v38-ext gaps

---

## Context

Packaging for all three platforms is partially implemented but not hardened:

| Package | Script | CI Gate | Status |
|---------|--------|---------|--------|
| macOS DMG | `build_dmg_layout.sh` | None | Layout only, notarize soft |
| Windows MSI | `build_msi_layout.sh` | None | Layout only |
| Linux DEB | `build_deb.sh` | None | No CI verification |
| Homebrew Bottle | `Formula/sharecli.rb` | None | SHA PLACEHOLDER |
| Homebrew TAP | `homebrew-omniroute/` | None | Not sharecli-specific |

The current `justfile` has `just package-macos`, `just package-windows`, `just package-linux` recipes but they only build layouts without verification.

## Decision

Implement a **complete packaging pipeline** with CI verification for all platforms.

### Phase 1: macOS DMG (Week 1)

1. Extend `build_dmg_layout.sh` to create a real DMG with `hdiutil`
2. Add `just package-dmg` recipe that:
   - Builds release binary
   - Creates DMG with proper icon, background, Applications symlink
   - Signs the DMG (when ADR-008 codesign is implemented)
   - Notarizes the DMG
3. Add CI step to `workflows/release.yml` that verifies DMG mounts correctly

```bash
# justfile
[private]
package-dmg:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release --target aarch64-apple-darwin
    mkdir -p dmg-staging/ShareCLI.app/Contents/MacOS
    cp target/aarch64-apple-darwin/release/sharecli dmg-staging/ShareCLI.app/Contents/MacOS/
    cp desktop/ShareCLITray/Info.plist dmg-staging/ShareCLI.app/Contents/
    hdiutil create -volname "ShareCLI" \
        -srcfolder dmg-staging \
        -ov -format UDZO \
        target/sharecli-$(just version)-aarch64.dmg
    echo "DMG created: target/sharecli-$(just version)-aarch64.dmg"
```

### Phase 2: Windows MSI (Week 2)

1. Extend `build_msi_layout.sh` to use `wix` or `msitools` for real MSI creation
2. Add `just package-msi` recipe
3. Add CI step that verifies MSI installs and uninstalls cleanly

```bash
# justfile
[private]
package-msi:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release --target x86_64-pc-windows-msvc
    # Use wix toolset to create MSI
    cargo install wix || true
    wix build -d "ShareCLIVersion=$(just version)" \
        -o target/sharecli-$(just version)-x64.msi \
        sharecli.wxs
    echo "MSI created: target/sharecli-$(just version)-x64.msi"
```

### Phase 3: Linux DEB (Week 2)

1. Extend `build_deb.sh` to create proper Debian package structure
2. Add `just package-deb` recipe
3. Add CI step that verifies DEB installs on Ubuntu

```bash
# justfile
[private]
package-deb:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release --target x86_64-unknown-linux-gnu
    DEB_DIR=target/deb/sharecli_$(just version)_amd64
    mkdir -p "$DEB_DIR/DEBIAN" "$DEB_DIR/usr/bin" "$DEB_DIR/usr/share/doc/sharecli"
    cp target/x86_64-unknown-linux-gnu/release/sharecli "$DEB_DIR/usr/bin/"
    cp LICENSE "$DEB_DIR/usr/share/doc/sharecli/"
    cat > "$DEB_DIR/DEBIAN/control" << EOF
    Package: sharecli
    Version: $(just version)
    Architecture: amd64
    Depends: libc6
    Description: OS-adjacent agent runtime
    EOF
    dpkg-deb --build "$DEB_DIR" target/sharecli_$(just version)_amd64.deb
    echo "DEB created: target/sharecli_$(just version)_amd64.deb"
```

### Phase 4: Homebrew Bottle (Week 3)

1. After ADR-008 codesign is implemented, run `brew bottle --root-url=<mirror> sharecli`
2. Replace `PLACEHOLDER` in `Formula/sharecli.rb` with real SHA256
3. Add `brew bottle` step to release workflow
4. Verify `brew install sharecli` works end-to-end

### Phase 5: CI verification (Week 3)

Add verification jobs to `workflows/release.yml`:

```yaml
jobs:
  verify-dmg:
    runs-on: macos-latest
    needs: [build-macos]
    steps:
      - uses: actions/download-artifact@v4
        with:
          name: sharecli-dmg
      - name: Verify DMG
        run: |
          hdiutil attach sharecli-*.dmg
          /Volumes/ShareCLI/ShareCLI.app/Contents/MacOS/sharecli --version
          hdiutil detach /Volumes/ShareCLI
  
  verify-msi:
    runs-on: windows-latest
    needs: [build-windows]
    steps:
      - uses: actions/download-artifact@v4
        with:
          name: sharecli-msi
      - name: Verify MSI
        run: |
          msiexec /i sharecli-*.msi /quiet /norestart
          & "C:\Program Files\ShareCLI\sharecli.exe" --version
          msiexec /x sharecli-*.msi /quiet /norestart
  
  verify-deb:
    runs-on: ubuntu-latest
    needs: [build-linux]
    steps:
      - uses: actions/download-artifact@v4
        with:
          name: sharecli-deb
      - name: Verify DEB
        run: |
          sudo dpkg -i sharecli_*.deb
          sharecli --version
          sudo dpkg -r sharecli
```

## Consequences

- **Positive**: L108/L109/L110 scores 2/3 -> 3/3, L111 score 1/3 -> 3/3
- **Positive**: Users get properly signed, installable packages for all platforms
- **Positive**: CI catches packaging regressions before release
- **Negative**: ~400 lines of packaging scripts + CI config
- **Negative**: macOS notarize requires Apple Developer account (ADR-008 dependency)
- **Risk**: MSI creation on Windows may need WiX toolset installation in CI

## Verification

- `just package-dmg` creates a mountable DMG
- `just package-msi` creates an installable MSI
- `just package-deb` creates a installable DEB
- `brew install sharecli` works with real bottle SHA
- CI verification jobs pass for all platforms
- `sharecli --version` works after each package install/uninstall cycle
