# ADR-008: Code Signing Hardening

**Status**: Proposed  
**Date**: 2026-08-28  
**Deciders**: ShareCLI maintainers  
**Addresses**: L112 Code Signing (score 2/3), C11 cluster, audit-v38-ext gap

---

## Context

Code signing for macOS is currently a **soft gate** (`workflows/codesign-soft.yml`). This means:

- `codesign` and `notarize` steps run but failures do not block the release
- The Homebrew bottle formula has a `PLACEHOLDER` SHA
- Windows MSI code signing is not implemented
- Container images are signed via Cosign (L82 is complete), but binary distribution is not

Current state:

| Artifact | Signing | Gate | Status |
|----------|---------|------|--------|
| macOS binary | codesign + notarize | SOFT | Not enforced |
| Windows binary | None | N/A | Not implemented |
| Linux binary | None | N/A | Not implemented |
| Container image | Cosign | HARD | Complete |
| Homebrew bottle | SHA placeholder | N/A | Blocked on signing |

## Decision

Promote code signing to a **hard gate** for macOS, and add Windows signing support.

### Phase 1: macOS hardening (Week 1)

1. Add Apple Developer ID certificate to GitHub Actions secrets (`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PWD`, `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD`)
2. Convert `codesign-soft.yml` to `codesign.yml` (hard gate)
3. Add `notarytool submit` + `notarytool wait` to release workflow
4. Verify notarization ticket is stapled: `stapler staple ShareCLI.dmg`
5. **Gate condition**: Release workflow fails if codesign or notarize fails

### Phase 2: Windows signing (Week 2)

1. Add Azure Key Vault or DigiCert signing certificate to secrets
2. Create `workflows/codesign-windows.yml` using `azure-sign` or `signtool.exe`
3. Sign the MSI and exe artifacts
4. Add signing verification step post-build

### Phase 3: Homebrew bottle (Week 2)

1. After macOS signing is verified, run `brew bottle --root-url=<mirror> sharecli`
2. Replace `PLACEHOLDER` in `Formula/sharecli.rb` with real SHA256
3. Add `brew bottle` step to release workflow
4. **Gate condition**: Release workflow fails if bottle SHA is still placeholder

### Signing workflow structure

```yaml
# workflows/codesign.yml (replaces codesign-soft.yml)
name: Code Sign (Hard Gate)
on:
  release:
    types: [published]
  workflow_call:
    secrets:
      APPLE_CERTIFICATE:
        required: true
      APPLE_CERTIFICATE_PWD:
        required: true

jobs:
  sign-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - name: Import certificate
        run: |
          echo "$APPLE_CERTIFICATE" | base64 -d > cert.p12
          security import cert.p12 -k ~/Library/Keychains/build.keychain
      - name: Build release binary
        run: cargo build --release --target aarch64-apple-darwin
      - name: Codesign
        run: |
          codesign --force --sign "Developer ID Application" \
            --options runtime \
            --timestamp \
            target/aarch64-apple-darwin/release/sharecli
      - name: Notarize
        run: |
          ditto -c -k --sequesterRsrc target/aarch64-apple-darwin/release/sharecli sharecli.zip
          notarytool submit sharecli.zip \
            --apple-id "$APPLE_ID" \
            --team-id "$APPLE_TEAM_ID" \
            --password "$APPLE_APP_PASSWORD" \
            --wait
      - name: Verify
        run: codesign --verify --verbose=2 target/aarch64-apple-darwin/release/sharecli
```

## Consequences

- **Positive**: L112 score 2/3 -> 3/3, overall scorecard lifts ~0.5%
- **Positive**: Users can verify binary authenticity
- **Positive**: Homebrew bottle SHA becomes real, unblocking L111
- **Negative**: Requires Apple Developer Program membership ($99/year)
- **Negative**: Windows signing requires certificate purchase
- **Risk**: Certificate expiry/renewal needs annual maintenance

## Verification

- `codesign --verify --verbose=2 sharecli` passes
- `spctl --assess --type exec sharecli` returns accepted
- `notarytool info <submission-id>` shows `Accepted`
- Homebrew bottle SHA matches actual bottle
- `brew install --head` works end-to-end
