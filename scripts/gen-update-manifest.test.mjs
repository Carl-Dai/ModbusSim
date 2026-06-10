// 运行: node --test scripts/gen-update-manifest.test.mjs
import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { groupAssetsByRole, extractChangelogSection, buildManifest, MANIFEST_VARIANTS } from './gen-update-manifest.mjs'

// Filenames mirror what Tauri 2 + tauri-action upload to GitHub Releases.
// macOS .app.tar.gz has no version in the filename; Linux uses .AppImage
// directly (no .tar.gz wrapper); Windows uses .exe directly (no .nsis.zip).
const sample = [
  { name: 'ModbusSlave_aarch64.app.tar.gz', browser_download_url: 'u1' },
  { name: 'ModbusSlave_aarch64.app.tar.gz.sig', browser_download_url: 'u1s' },
  { name: 'ModbusSlave_0.16.0_x64-setup.exe', browser_download_url: 'u2' },
  { name: 'ModbusSlave_0.16.0_x64-setup.exe.sig', browser_download_url: 'u2s' },
  { name: 'ModbusSlave_0.16.0_arm64-setup.exe', browser_download_url: 'u2a' },
  { name: 'ModbusSlave_0.16.0_arm64-setup.exe.sig', browser_download_url: 'u2as' },
  { name: 'ModbusMaster_0.16.0_amd64.AppImage', browser_download_url: 'u3' },
  { name: 'ModbusMaster_0.16.0_amd64.AppImage.sig', browser_download_url: 'u3s' },
  // installers that should NOT match (.dmg, .msi, .deb, .rpm) — included to
  // verify the regex doesn't pull them in by accident
  { name: 'ModbusSlave_0.16.0_x64.dmg', browser_download_url: 'noise1' },
  { name: 'ModbusSlave_0.16.0_x64_en-US.msi', browser_download_url: 'noise2' },
  { name: 'ModbusMaster_0.16.0_amd64.deb', browser_download_url: 'noise3' },
  { name: 'ModbusMaster-0.16.0-1.x86_64.rpm', browser_download_url: 'noise4' },
]

describe('groupAssetsByRole', () => {
  it('separates slave and master assets', () => {
    const { slave, master } = groupAssetsByRole(sample)
    assert.equal(slave['darwin-aarch64'].url, 'u1')
    assert.equal(slave['darwin-aarch64'].sigUrl, 'u1s')
    assert.equal(slave['windows-x86_64'].url, 'u2')
    assert.equal(slave['windows-x86_64'].sigUrl, 'u2s')
    assert.equal(slave['windows-aarch64'].url, 'u2a')
    assert.equal(slave['windows-aarch64'].sigUrl, 'u2as')
    assert.equal(master['linux-x86_64'].url, 'u3')
    assert.equal(master['linux-x86_64'].sigUrl, 'u3s')
  })
  it('ignores non-updater installers (.dmg/.msi/.deb/.rpm)', () => {
    const { slave, master } = groupAssetsByRole(sample)
    const allUrls = Object.values(slave).concat(Object.values(master)).map((v) => v.url)
    assert.ok(!allUrls.includes('noise1'))
    assert.ok(!allUrls.includes('noise2'))
    assert.ok(!allUrls.includes('noise3'))
    assert.ok(!allUrls.includes('noise4'))
  })
})

describe('extractChangelogSection', () => {
  const md = `# Changelog\n\n## 1.0.9\n- foo\n- bar\n\n## 1.0.8\n- old\n`
  it('extracts the section for the given version', () => {
    assert.equal(extractChangelogSection(md, '1.0.9'), '- foo\n- bar')
  })
  it('returns empty string when version not found', () => {
    assert.equal(extractChangelogSection(md, '9.9.9'), '')
  })
  it('does not match a version that is a prefix of another', () => {
    const md2 = `## 1.0.10\n- new\n\n## 1.0.1\n- old\n`
    assert.equal(extractChangelogSection(md2, '1.0.1'), '- old')
    assert.equal(extractChangelogSection(md2, '1.0.10'), '- new')
  })
  it('handles the Keep-a-Changelog bracket style `## [1.2.3] - date`', () => {
    const md3 = `## [1.2.3] - 2026-04-28\n- new\n\n## [1.2.2] - 2026-04-27\n- old\n`
    assert.equal(extractChangelogSection(md3, '1.2.3'), '- new')
    assert.equal(extractChangelogSection(md3, '1.2.2'), '- old')
  })
})

describe('MANIFEST_VARIANTS', () => {
  it('declares 5 variants in proxy-first / github-last order', () => {
    assert.deepEqual(MANIFEST_VARIANTS, [
      { suffix: '-cn0', prefix: 'https://gh.daichangyu.com/' },
      { suffix: '-cn1', prefix: 'https://ghfast.top/' },
      { suffix: '-cn2', prefix: 'https://gh-proxy.com/' },
      { suffix: '-cn3', prefix: 'https://gh.idayer.com/' },
      { suffix: '',     prefix: null },
    ])
  })
})

describe('buildManifest', () => {
  const platforms = {
    'windows-x86_64': { signature: 'SIG', url: 'https://github.com/u/r/releases/download/v1/a.exe' },
    'darwin-aarch64': { signature: 'SIG2', url: 'https://github.com/u/r/releases/download/v1/b.tar.gz' },
  }
  const base = { version: '1.0.0', notes: 'n', pub_date: '2026-01-01T00:00:00Z', platforms }

  it('returns the original manifest unchanged when prefix is null', () => {
    assert.deepEqual(buildManifest(base, null), base)
  })

  it('prepends the prefix to every platform url, leaving signature untouched', () => {
    const got = buildManifest(base, 'https://ghfast.top/')
    assert.equal(got.platforms['windows-x86_64'].url, 'https://ghfast.top/https://github.com/u/r/releases/download/v1/a.exe')
    assert.equal(got.platforms['darwin-aarch64'].url, 'https://ghfast.top/https://github.com/u/r/releases/download/v1/b.tar.gz')
    assert.equal(got.platforms['windows-x86_64'].signature, 'SIG')
    assert.equal(got.platforms['darwin-aarch64'].signature, 'SIG2')
  })

  it('does not mutate the input manifest', () => {
    const snapshot = JSON.parse(JSON.stringify(base))
    buildManifest(base, 'https://ghfast.top/')
    assert.deepEqual(base, snapshot)
  })
})
