import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import vm from 'node:vm'
import test from 'node:test'
import ts from 'typescript'
import * as vue from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import { renderToString } from 'vue/server-renderer'

const require = createRequire(import.meta.url)
const { parse, compileScript, compileTemplate } = createRequire(require.resolve('vue/package.json'))('@vue/compiler-sfc')
const root = new URL('../', import.meta.url)
const read = path => readFileSync(new URL(path, root), 'utf8')
function evaluate(source, filename, mocks = {}) {
  const compiled = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } }).outputText
  const loaded = { exports: {} }
  vm.runInThisContext('(function(require,module,exports){\n' + compiled + '\n})', { filename })((name) => Object.hasOwn(mocks, name) ? mocks[name] : require(name), loaded, loaded.exports)
  return loaded.exports
}
function entry(overrides = {}) {
  return { id: 'audit-1', at: 100, kind: 'agent', title: '整理资料', request_id: '父请求', session_id: '会话', goal_id: null,
    tools: [{ name: 'file_write', intent: 'file.write', ok: true, brief: '写入资料.txt', at: 100 }], files_changed: ['资料.txt'], result: 'ok', summary: '本轮成功', duration_ms: 12, ...overrides }
}
function fixture(invoke) {
  setActivePinia(createPinia())
  const calls = []
  const filename = fileURLToPath(new URL('src/stores/auditStore.ts', root))
  const store = evaluate(read('src/stores/auditStore.ts'), filename, { '@tauri-apps/api/core': { async invoke(command, payload) {
    calls.push({ command, payload })
    if (invoke) return invoke(command, payload)
    if (command === 'list_audit') return [entry()]
    if (command === 'export_audit') return JSON.stringify([entry()])
  } } }).useAuditStore()
  return { store, calls }
}
function handlers(store) {
  const filename = fileURLToPath(new URL('src/components/AuditPanel.vue', root))
  const source = read('src/components/AuditPanel.vue').match(/<script setup lang="ts">([\s\S]*?)<\/script>/)[1]
    .replace('const emit = defineEmits<{ close: [] }>()', 'const emit = () => {}')
    + '\nexport { keyword, kind, result, range, query, expanded, refresh, clear, exportJson };'
  return evaluate(source, filename, { '../stores/auditStore': { useAuditStore: () => store }, vue: { ...vue, onMounted: () => {} } })
}
function panel(store) {
  const filename = fileURLToPath(new URL('src/components/AuditPanel.vue', root))
  const { descriptor } = parse(read('src/components/AuditPanel.vue'), { filename })
  const script = compileScript(descriptor, { id: 'audit-panel' })
  const component = evaluate(script.content, filename, { '../stores/auditStore': { useAuditStore: () => store } }).default
  const template = compileTemplate({ source: descriptor.template.content, filename, id: 'audit-panel', compilerOptions: { bindingMetadata: script.bindings } })
  assert.deepEqual(template.errors, [])
  component.render = evaluate(template.code, filename).render
  return component
}

test('loads audit records and forwards combined search filters', async () => {
  const { store, calls } = fixture()
  const query = { keyword: '资料', kind: 'agent', result: 'ok', since: 100, until: 200 }
  await store.load(query)
  assert.deepEqual(calls[0], { command: 'list_audit', payload: { query } })
  assert.equal(store.entries[0].title, '整理资料')
  assert.equal(store.loading, false)
})

test('stale search responses do not replace the newest result', async () => {
  const pending = []
  const { store } = fixture(() => new Promise(resolve => pending.push(resolve)))
  const first = store.load({ keyword: '旧' })
  const second = store.load({ keyword: '新' })
  pending[1]([entry({ title: '新结果' })]); await second
  pending[0]([entry({ title: '旧结果' })]); await first
  assert.equal(store.entries[0].title, '新结果')
})

test('read/export/clear failures display Chinese errors without destroying records', async () => {
  const { store } = fixture(() => { throw new Error('磁盘不可用') })
  store.entries = [entry()]
  await store.load()
  assert.match(store.error, /读取执行记录失败/)
  assert.equal(await store.exportJson(), null)
  assert.match(store.error, /导出执行记录失败/)
  assert.equal(await store.clear(true), false)
  assert.match(store.error, /清空执行记录失败/)
  assert.equal(store.entries.length, 1)
  assert.equal(store.loading, false)
})

test('clear refuses missing confirmation and invalidates in-flight loads', async () => {
  let complete
  const { store, calls } = fixture(command => command === 'list_audit' ? new Promise(resolve => { complete = resolve }) : undefined)
  assert.equal(await store.clear(false), false)
  assert.equal(calls.length, 0)
  const loading = store.load()
  assert.equal(await store.clear(true), true)
  complete([entry()]); await loading
  assert.deepEqual(store.entries, [])
  assert.deepEqual(calls[1], { command: 'clear_audit', payload: { confirmed: true } })
})

test('panel sends keyword, kind, result, today and seven-day queries', async () => {
  const { store, calls } = fixture()
  const actions = handlers(store)
  actions.keyword.value = '报告'; actions.kind.value = 'goal'; actions.result.value = 'blocked'
  actions.range.value = 'today'
  await actions.refresh()
  assert.equal(calls.at(-1).payload.query.since, new Date().setHours(0, 0, 0, 0) / 1000)
  assert.equal(calls.at(-1).payload.query.keyword, '报告')
  assert.equal(calls.at(-1).payload.query.kind, 'goal')
  assert.equal(calls.at(-1).payload.query.result, 'blocked')
  actions.range.value = 'week'
  const expected = Math.floor(Date.now() / 1000) - 7 * 86400
  assert.ok(Math.abs(actions.query.value.since - expected) <= 1)
  actions.range.value = 'all'; assert.equal(actions.query.value.since, undefined)
})

test('clearing requires an explicit click and Chinese confirmation', async () => {
  const { store, calls } = fixture()
  const actions = handlers(store)
  const previous = globalThis.confirm
  try {
    globalThis.confirm = message => { assert.match(message, /清空全部执行记录.*不可恢复/); return false }
    await actions.clear(); assert.equal(calls.length, 0)
    globalThis.confirm = () => true
    await actions.clear()
    assert.equal(calls[0].command, 'clear_audit')
    assert.equal(calls[1].command, 'list_audit')
  } finally { globalThis.confirm = previous }
})

test('export downloads JSON for the active query and revokes the object URL', async () => {
  const { store, calls } = fixture()
  const actions = handlers(store)
  actions.kind.value = 'agent'
  const previous = { document: globalThis.document, create: URL.createObjectURL, revoke: URL.revokeObjectURL, timeout: globalThis.setTimeout }
  const anchor = { click() { this.clicked = true } }
  let blob, revoked
  try {
    globalThis.document = { createElement: name => { assert.equal(name, 'a'); return anchor } }
    URL.createObjectURL = value => { blob = value; return 'blob:audit' }
    URL.revokeObjectURL = value => { revoked = value }
    globalThis.setTimeout = callback => callback()
    await actions.exportJson()
    assert.equal(anchor.download, '执行记录.json'); assert.equal(anchor.clicked, true)
    assert.equal(revoked, 'blob:audit'); assert.deepEqual(JSON.parse(await blob.text()), [entry()])
    assert.equal(calls[0].payload.query.kind, 'agent')
  } finally {
    globalThis.document = previous.document; URL.createObjectURL = previous.create
    URL.revokeObjectURL = previous.revoke; globalThis.setTimeout = previous.timeout
  }
})

test('panel renders every kind, result, counts, duration and safe escaped titles', async () => {
  const { store } = fixture()
  store.entries = ['chat', 'agent', 'sub_agent', 'goal', 'forge'].map((kind, index) => entry({ id: String(index), kind, result: ['ok', 'partial', 'failed', 'blocked', 'cancelled'][index], title: '<script>bad</script>' }))
  const html = await renderToString(vue.createSSRApp(panel(store)))
  for (const label of ['执行记录', '对话', '子Agent', '目标', '自造', '成功', '部分', '失败', '卡住', '已取消', '工具数 1', '改动文件数 1', '耗时 12 毫秒', '导出 JSON']) assert.ok(html.includes(label), label)
  assert.ok(html.includes('&lt;script&gt;')); assert.ok(!html.includes('<script>bad</script>'))
  assert.match(read('src/components/AuditPanel.vue'), /工具调用明细[\s\S]*tool\.intent[\s\S]*tool\.brief[\s\S]*改动的文件路径/)
})

test('all five real backend entry points are connected and commands are registered', () => {
  for (const [path, kind] of [['commands/send_message.rs', 'chat'], ['agent/loop_.rs', 'agent'], ['agent/sub_agents.rs', 'sub_agent'], ['agent/goals.rs', 'goal'], ['agent/forged_tools.rs', 'forge']]) {
    const source = read('src-tauri/src/' + path)
    assert.ok(source.includes('AuditRun::new("' + kind + '"'), kind)
    assert.ok(source.includes('audit.scope('), kind)
  }
  for (const command of ['list_audit', 'export_audit', 'clear_audit']) assert.ok(read('src-tauri/src/lib.rs').includes('commands::audit::' + command))
  assert.match(read('src/views/MainLayout.vue'), /<AuditPanel v-if="showAudit"/)
})
