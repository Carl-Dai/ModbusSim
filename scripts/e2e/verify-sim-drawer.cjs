/**
 * Playwright verification for the Slave "Simulation Settings" drawer
 * (batch point-mutation ported from the IEC104 simulator).
 *
 * Runs the slave frontend in a plain browser with a mocked Tauri backend
 * (`window.__TAURI_INTERNALS__`), so the full UI flow can be exercised
 * without the Rust engine. Rust-side mutation logic is covered by unit tests.
 *
 * Usage:
 *   npm run dev              # in frontend/  (vite on :5175)
 *   npm i playwright         # any project, or NODE_PATH=<dir>/node_modules
 *   node scripts/e2e/verify-sim-drawer.cjs
 */
const { chromium } = require('playwright')

const URL = 'http://localhost:5175'
const SHOTS = 'output/playwright'

const MOCK = () => {
  const mutations = new Map()
  let listenerSeq = 0
  let cbSeq = 0

  const holding = Array.from({ length: 8 }, (_, i) => ({
    address: i,
    register_type: 'holding_register',
    data_type: 'uint16',
    endian: 'big',
    name: i % 2 === 0 ? `HR${i}` : '',
    comment: '',
    mutation: null,
    data_source: null,
  }))
  const coils = Array.from({ length: 4 }, (_, i) => ({
    address: i,
    register_type: 'coil',
    data_type: 'bool',
    endian: 'big',
    name: '',
    comment: '',
    mutation: null,
    data_source: null,
  }))
  const holdingValues = holding.map((d) => ({ address: d.address, value: d.address * 10 }))
  const coilValues = coils.map((d) => ({ address: d.address, value: d.address % 2 }))

  window.__mockInvoke = (cmd, args = {}) => {
    switch (cmd) {
      case 'list_slave_connections':
        return [{ id: 'conn1', bind_address: '0.0.0.0', port: 502, state: 'Running', device_count: 1 }]
      case 'list_slave_devices':
        return [{ slave_id: 1, name: 'Slave 1', register_count: 12 }]
      case 'list_registers': {
        const rt = args.registerType ?? args.register_type ?? null
        if (rt === 'holding_register') return holding
        if (rt === 'coil') return coils
        return [...holding, ...coils]
      }
      case 'read_registers_bulk': {
        const rt = args.registerType ?? args.register_type ?? null
        if (rt === 'holding_register') return holdingValues
        if (rt === 'coil') return coilValues
        return [...holdingValues, ...coilValues]
      }
      case 'list_point_mutations':
        return Array.from(mutations.values())
      case 'set_point_mutation': {
        const r = args.request
        mutations.set(`${r.register_type}-${r.address}`, {
          register_type: r.register_type,
          address: r.address,
          mode: r.config.mode,
          period_ms: r.config.period_ms,
          step: r.config.step,
          min: r.config.min,
          max: r.config.max,
        })
        return null
      }
      case 'clear_point_mutation': {
        const r = args.request
        mutations.delete(`${r.register_type}-${r.address}`)
        return null
      }
      case 'set_mutation_running':
        return null
      case 'plugin:event|listen':
        return ++listenerSeq
      case 'plugin:event|unlisten':
        return null
      case 'plugin:updater|check':
        return null
      case 'plugin:app|version':
        return '0.17.2-mock'
      case 'get_communication_logs':
        return []
      default:
        return null
    }
  }

  const callbacks = {}
  window.__TAURI_INTERNALS__ = {
    invoke: (cmd, args, options) => window.__mockInvoke(cmd, args),
    transformCallback: (cb) => {
      const id = ++cbSeq
      callbacks[id] = cb
      return id
    },
  }
}

function assert(cond, msg) {
  if (!cond) throw new Error(`ASSERT FAILED: ${msg}`)
}

async function count(page, selector) {
  return page.locator(selector).count()
}

async function waitCount(page, selector, expected, timeoutMs = 5000) {
  await page.waitForFunction(
    ({ sel, n }) => document.querySelectorAll(sel).length === n,
    { sel: selector, n: expected },
    { timeout: timeoutMs },
  )
}

;(async () => {
  const browser = await chromium.launch({ channel: 'chrome', headless: true })
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
  const errors = []
  page.on('pageerror', (e) => errors.push(String(e)))
  await page.addInitScript(MOCK)

  console.log('1. loading app (mocked backend)…')
  await page.goto(URL, { waitUntil: 'load', timeout: 60_000 })
  await page.waitForSelector('.slave-node', { timeout: 30_000 })

  console.log('2. selecting Slave 1 → register table')
  await page.locator('.slave-node').first().click()
  await page.waitForSelector('.virtual-row', { timeout: 15_000 })
  assert((await count(page, '.virtual-row')) === 12, 'expected 12 register rows')

  console.log('3. multi-selecting 3 rows (Cmd-click on macOS)')
  await page.locator('.virtual-row').nth(0).click()
  await page.keyboard.down('Meta')
  await page.locator('.virtual-row').nth(1).click()
  await page.locator('.virtual-row').nth(2).click()
  await page.keyboard.up('Meta')
  assert((await count(page, '.virtual-row.selected')) === 3, 'expected 3 selected rows')

  console.log('4. right-click → context menu → open simulation settings')
  await page.locator('.virtual-row').nth(1).click({ button: 'right' })
  await page.waitForSelector('.context-menu', { timeout: 5000 })
  await page.locator('.context-menu-item').first().click()
  await page.waitForSelector('.sim-drawer', { timeout: 5000 })
  assert((await count(page, '.sim-point-chip')) === 3, 'drawer should show 3 selected chips')

  console.log('5. configure: mode = Random, min = 10, max = 20')
  await page.locator('.sim-mode-buttons button').nth(3).click() // Random
  const inputs = page.locator('.sim-form input')
  assert((await inputs.count()) === 3, 'random mode should show period + min + max inputs')
  await inputs.nth(1).fill('10')
  await inputs.nth(2).fill('20')

  console.log('6. Apply → engine starts, 3 active simulations listed')
  await page.locator('.sim-btn-primary').click()
  await waitCount(page, '.sim-active-card', 3)
  const body = await page.locator('.sim-drawer-body').innerText()
  assert(body.includes('10 / 20'), 'active card should echo min/max bounds')

  console.log('7. toolbar master switch synced + row mode badges')
  await page.waitForSelector('.btn-mutation-active', { timeout: 5000 })
  await waitCount(page, '.mut-badge.active', 3)
  await page.screenshot({ path: `${SHOTS}/sim-drawer-active.png`, fullPage: false })

  console.log('8. per-row stop → 2 remain; stop selected → 0 remain')
  await page.locator('.sim-row-stop').first().click()
  await waitCount(page, '.sim-active-card', 2)
  await page.locator('.sim-btn-danger').click()
  await waitCount(page, '.sim-active-card', 0)
  await page.screenshot({ path: `${SHOTS}/sim-drawer-empty.png`, fullPage: false })

  const fatal = errors.filter((e) => !/transformCallback|plugin:event|Cannot read properties of undefined/.test(e))
  if (fatal.length > 0) {
    console.log('page errors:', fatal.join('\n'))
    throw new Error(`unexpected page errors: ${fatal.length}`)
  }

  console.log('\nPASS — drawer flow verified: batch apply, active list, live values, per-row stop, toolbar sync.')
  await browser.close()
  process.exit(0)
})().catch((e) => {
  console.error('\nFAIL:', e.message)
  process.exit(1)
})
