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

function makeSettings(overrides = {}) {
  const calls = []
  let nextId = 0
  const store = {
    models: [{ id: 'main-id', name: 'main-text', roles: ['main'], enabled: true }],
    modelMode: 'api', apiBaseUrl: 'https://global.example', aiRouter: false,
    async update(payload) {
      calls.push({ command: 'update', payload })
      if (payload.models) store.models = payload.models.map((slot) => ({ ...slot, id: slot.id || 'generated-' + ++nextId }))
    },
    async saveSlotKey(id, plain) {
      calls.push({ command: 'key', id, plain })
      store.models.find((slot) => slot.id === id).has_api_key = !!plain
    },
    async saveApiKey(plain) { calls.push({ command: 'global-key', plain }) },
    ...overrides,
  }
  const page = load('src/views/SettingView.vue', {
    vue: { ...vue, onMounted() {} },
    '../stores/settingStore': { useSettingStore: () => store },
    '../stores/milestoneStore': { useMilestoneStore: () => ({}) },
    '../types': { MODEL_MODE_LABEL: {} },
    '../utils/tauri': { async testApiConnection(base, key, slotId) {
      calls.push({ command: 'test', base, key, slotId })
      return { success: true, message: '连接成功' }
    } },
  }, '\nexport { modelSlots, modelMode, apiBaseUrl, apiKeyInput, generalMsg, testMsg, addModel, saveModel, saveSlotKey, saveApiKey, testConnection, syncFromStore, pickQuick }')
  page.syncFromStore()
  return { page, store, calls }
}

test('adding and deleting model drafts never changes stored models', () => {
  const { page, store } = makeSettings()
  page.addModel()
  assert.equal(page.modelSlots.value.length, 2)
  assert.deepEqual(page.modelSlots.value[1].roles, [])
  assert.equal(page.modelSlots.value[1].id, '')
  assert.equal(page.modelSlots.value[1].enabled, true)
  page.modelSlots.value.splice(0, 1)
  assert.equal(store.models.length, 1)
  assert.equal(store.models[0].name, 'main-text')
})

test('saving requires an enabled main model and populated names', async () => {
  const { page, calls } = makeSettings()
  page.modelSlots.value[0].roles = ['cheap']
  assert.equal(await page.saveModel(), false)
  assert.match(page.generalMsg.value, /主力/)
  page.modelSlots.value[0].roles = ['main']
  page.modelSlots.value[0].enabled = false
  assert.equal(await page.saveModel(), false)
  page.modelSlots.value[0].enabled = true
  page.modelSlots.value[0].name = ' '
  assert.equal(await page.saveModel(), false)
  assert.equal(calls.length, 0)
})

test('metadata saves include all model properties but never key drafts', async () => {
  const { page, calls } = makeSettings()
  page.modelSlots.value[0].keyInput = 'never-send-in-model-json'
  page.modelSlots.value[0].base_url = ' https://own.example '
  page.modelSlots.value[0].roles = ['main', 'vision']
  assert.equal(await page.saveModel(), true)
  assert.deepEqual(calls[0].payload.models, [{ id: 'main-id', name: 'main-text', base_url: 'https://own.example', roles: ['main', 'vision'], enabled: true }])
  assert.equal(JSON.stringify(calls[0]).includes('never-send-in-model-json'), false)
  assert.equal(page.modelSlots.value[0].keyInput, 'never-send-in-model-json')
})

test('new slot keys first persist metadata, then use the assigned ID, and clear the input', async () => {
  const { page, calls } = makeSettings()
  page.addModel()
  const slot = page.modelSlots.value[1]
  slot.name = 'cheap-text'
  slot.roles = ['cheap']
  slot.keyInput = 'slot-secret'
  await page.saveSlotKey(1)
  assert.deepEqual(calls.map((call) => call.command), ['update', 'key'])
  assert.equal(JSON.stringify(calls[0]).includes('slot-secret'), false)
  assert.equal(calls[1].id, slot.id)
  assert.equal(calls[1].plain, 'slot-secret')
  assert.equal(slot.keyInput, '')
  assert.equal(slot.has_api_key, true)
})

test('failed key saves keep the draft and never echo backend secrets', async () => {
  const { page } = makeSettings({ async saveSlotKey() { throw new Error('slot-secret') } })
  page.modelSlots.value[0].keyInput = 'slot-secret'
  await page.saveSlotKey(0)
  assert.equal(page.modelSlots.value[0].keyInput, 'slot-secret')
  assert.equal(page.generalMsg.value.includes('slot-secret'), false)
})

test('global keys are saved separately and inputs clear after success', async () => {
  const { page, calls } = makeSettings()
  page.apiKeyInput.value = 'global-secret'
  await page.saveApiKey()
  assert.deepEqual(calls, [{ command: 'global-key', plain: 'global-secret' }])
  assert.equal(page.apiKeyInput.value, '')
})

test('connection test uses the main slot address and its saved key ID', async () => {
  const { page, calls } = makeSettings()
  page.modelSlots.value[0].base_url = 'https://own.example'
  page.modelSlots.value[0].has_api_key = true
  page.apiKeyInput.value = 'different-global-secret'
  await page.testConnection()
  assert.deepEqual(calls, [{ command: 'test', base: 'https://own.example', key: '', slotId: 'main-id' }])
  assert.equal(page.testMsg.value, '连接成功')
})

test('setting store reloads slot flags after separate key IPC without retaining key text', async () => {
  const calls = []
  const cfg = { models: [{ id: 'main', name: 'selected-text', enabled: true, roles: ['main'], has_api_key: true }] }
  setActivePinia(createPinia())
  const { useSettingStore } = load('src/stores/settingStore.ts', {
    '../utils/tauri': {
      async saveModelSlotKey(slotId, plain) { calls.push({ slotId, plain }) },
      async getConfig() { return { config: cfg } },
      async masterPasswordStatus() { return { has_master_password: false, unlocked: false } },
    },
  })
  const store = useSettingStore()
  await store.saveSlotKey('main', 'transient-secret')
  assert.deepEqual(calls, [{ slotId: 'main', plain: 'transient-secret' }])
  assert.equal(store.apiModel, 'selected-text')
  assert.equal(store.models[0].has_api_key, true)
  assert.equal(JSON.stringify(store.$state).includes('transient-secret'), false)
})

test('IPC adapter sends slot IDs and keys only to their separate command', async () => {
  const calls = []
  const api = load('src/utils/tauri.ts', {
    '@tauri-apps/api/core': { invoke: async (command, payload) => calls.push({ command, payload }) },
    '@tauri-apps/api/event': {},
  })
  await api.saveModelSlotKey('slot', 'secret')
  assert.deepEqual(calls, [{ command: 'save_model_slot_key', payload: { slotId: 'slot', plain: 'secret' } }])
})
