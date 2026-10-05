import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import vm from 'node:vm'
import test from 'node:test'
import ts from 'typescript'
import * as vue from 'vue'
import { createPinia, setActivePinia } from 'pinia'

const require = createRequire(import.meta.url)
const root = new URL('../', import.meta.url)

function load(relativePath, mocks, exported = '') {
  const filename = fileURLToPath(new URL(relativePath, root))
  const source = readFileSync(filename, 'utf8').replace('import.meta.env.VITE_USE_MOCK', "'0'")
  const script = relativePath.endsWith('.vue') ? source.match(/<script setup lang="ts">([\s\S]*?)<\/script>/)[1] : source
  const compiled = ts.transpileModule(script + exported, {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS },
  }).outputText
  const loaded = { exports: {} }
  const execute = vm.runInThisContext('(function(require, module, exports) {\n' + compiled + '\n})', { filename })
  execute((name) => Object.hasOwn(mocks, name) ? mocks[name] : name.endsWith('.vue') ? {} : require(name), loaded, loaded.exports)
  return loaded.exports
}

function settingsPage(overrides = {}, apiOverrides = {}) {
  const calls = []
  const store = {
    models: [{ id: 'main', name: 'main', roles: ['main'], enabled: true }],
    modelMode: 'api', apiBaseUrl: 'https://example.com', proxyEnabled: false, proxyUrl: null,
    async update(payload) {
      calls.push({ command: 'update', payload })
      store.proxyEnabled = payload.proxy_enabled
      store.proxyUrl = payload.proxy_url
    },
    ...overrides,
  }
  const page = load('src/views/SettingView.vue', {
    vue: { ...vue, onMounted() {} },
    '../stores/settingStore': { useSettingStore: () => store },
    '../stores/milestoneStore': { useMilestoneStore: () => ({}) },
    '../types': { MODEL_MODE_LABEL: {} },
    '../utils/tauri': {
      async networkDiagnostic() { calls.push({ command: 'diagnose' }); return '当前网络策略：直连' },
      async testApiConnection() { calls.push({ command: 'test' }); return { success: true, message: '连接成功' } },
      ...apiOverrides,
    },
  }, '\nexport { proxyEnabled, proxyUrl, networkMsg, diagnosingNetwork, savingNetwork, saveNetwork, diagnoseNetwork, testConnection, syncFromStore }')
  page.syncFromStore()
  return { page, store, calls }
}

test('old frontend config defaults to direct and settings persist through update', async () => {
  setActivePinia(createPinia())
  const calls = []
  const { useSettingStore } = load('src/stores/settingStore.ts', {
    '../utils/tauri': {
      async updateConfig(payload) { calls.push(payload); return { config: payload } },
    },
  })
  const store = useSettingStore()
  store.applyConfig({})
  assert.equal(store.proxyEnabled, false)
  assert.equal(store.proxyUrl, null)
  await store.update({ proxy_enabled: true, proxy_url: 'http://127.0.0.1:7890' })
  assert.deepEqual(calls, [{ proxy_enabled: true, proxy_url: 'http://127.0.0.1:7890' }])
  assert.equal(store.proxyEnabled, true)
  assert.equal(store.proxyUrl, 'http://127.0.0.1:7890')
  store.applyConfig({})
  assert.equal(store.proxyEnabled, false)
})

test('settings save only explicit proxy fields and trim address', async () => {
  const { page, calls } = settingsPage()
  page.proxyEnabled.value = true
  page.proxyUrl.value = ' http://127.0.0.1:7890 '
  assert.equal(await page.saveNetwork(), true)
  assert.deepEqual(calls, [{ command: 'update', payload: { proxy_enabled: true, proxy_url: 'http://127.0.0.1:7890' } }])
  assert.equal(page.savingNetwork.value, false)
})

test('disabling proxy keeps the saved address but never implicitly enables it', async () => {
  const { page, calls } = settingsPage({ proxyEnabled: true, proxyUrl: 'http://127.0.0.1:7890' })
  page.proxyEnabled.value = false
  await page.saveNetwork()
  assert.deepEqual(calls[0].payload, { proxy_enabled: false, proxy_url: 'http://127.0.0.1:7890' })
})

test('diagnostic uses the currently entered network policy and existing text feedback', async () => {
  const { page, calls } = settingsPage()
  await page.diagnoseNetwork()
  assert.deepEqual(calls.map((call) => call.command), ['update', 'diagnose'])
  assert.equal(page.networkMsg.value, '当前网络策略：直连')
  assert.equal(page.diagnosingNetwork.value, false)
})

test('failed settings save stops diagnostic without echoing sensitive errors', async () => {
  const { page, calls } = settingsPage({ async update() { throw new Error('api-secret') } })
  await page.diagnoseNetwork()
  assert.deepEqual(calls, [])
  assert.equal(page.networkMsg.value, '网络设置保存失败，请重试')
  assert.equal(page.diagnosingNetwork.value, false)
  assert.equal(page.savingNetwork.value, false)
})

test('diagnostic IPC errors receive Chinese feedback without leaking raw contents', async () => {
  const { page } = settingsPage({}, { async networkDiagnostic() { throw new Error('api-secret') } })
  await page.diagnoseNetwork()
  assert.equal(page.networkMsg.value, '网络诊断失败，请检查网络设置后重试')
  assert.equal(page.diagnosingNetwork.value, false)
})

test('connection test saves changed proxy settings first but leaves default behavior unchanged', async () => {
  const { page, calls } = settingsPage()
  await page.testConnection()
  assert.deepEqual(calls.map((call) => call.command), ['test'])
  calls.length = 0
  page.proxyEnabled.value = true
  page.proxyUrl.value = 'http://127.0.0.1:7890'
  await page.testConnection()
  assert.deepEqual(calls.map((call) => call.command), ['update', 'test'])
})

test('network diagnostic adapter sends no keys or user messages', async () => {
  const calls = []
  const api = load('src/utils/tauri.ts', {
    '@tauri-apps/api/core': { async invoke(...args) { calls.push(args); return '当前网络策略：直连' } },
    '@tauri-apps/api/event': { listen() {} },
  })
  assert.equal(await api.networkDiagnostic(), '当前网络策略：直连')
  assert.deepEqual(calls, [['network_diagnostic']])
})

test('proxy input is disabled when off and diagnostics preserve multiline text without HTML', () => {
  const source = readFileSync(new URL('src/views/SettingView.vue', root), 'utf8')
  assert.match(source, /id="network-proxy-url"[^>]*:disabled="!proxyEnabled"/)
  assert.match(source, /class="msg network-msg" role="status">{{ networkMsg }}/)
  assert.match(source, /\.network-msg[^}]*white-space: pre-wrap/)
  assert.doesNotMatch(source, /v-html="networkMsg"/)
})
