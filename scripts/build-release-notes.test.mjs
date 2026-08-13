import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { buildBody } from './build-release-notes.mjs'

const changelog = `# Changelog

## [0.17.2] - 2026-08-13

### Fixed 修复

- point workflow

## [0.17.1] - 2026-06-12

- old
`

describe('buildBody', () => {
  it('renders the matching changelog section and both apps', () => {
    const body = buildBody('v0.17.2', changelog)
    assert.match(body, /^# ModbusSim v0\.17\.2\b/)
    assert.ok(body.includes('### Fixed 修复'))
    assert.ok(!body.includes('0.17.1'))
    assert.ok(body.includes('ModbusSlave_0.17.2_aarch64.dmg'))
    assert.ok(body.includes('ModbusMaster_0.17.2_amd64.AppImage'))
  })

  it('lists x64 and ARM64 Windows installers and portable executables', () => {
    const body = buildBody('v0.17.2', changelog)
    assert.ok(body.includes('ModbusSlave_0.17.2_x64-setup.exe'))
    assert.ok(body.includes('ModbusMaster_0.17.2_x64-portable.exe'))
    assert.ok(body.includes('ModbusSlave_0.17.2_arm64-setup.exe'))
    assert.ok(body.includes('ModbusMaster_0.17.2_arm64_en-US.msi'))
    assert.ok(body.includes('ModbusSlave_0.17.2_arm64-portable.exe'))
  })

  it('keeps mirror and macOS first-launch guidance', () => {
    const body = buildBody('v0.17.2', changelog)
    assert.ok(body.indexOf('ghfast.top/') < body.indexOf('## 下载 / Downloads'))
    assert.ok(body.includes('macOS 首次启动 / First launch on macOS'))
    assert.ok(body.includes('xattr -dr com.apple.quarantine'))
  })

  it('warns when the version section is missing', () => {
    assert.ok(buildBody('v9.9.9', changelog).includes('CHANGELOG.md 缺少 `9.9.9`'))
  })
})
