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
function evaluate(source, filename, mocks = {}) {
  const compiled = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } }).outputText
  const loaded = { exports: {} }
  vm.runInThisContext('(function(require,module,exports){\n' + compiled + '\n})', { filename })((name) => Object.hasOwn(mocks, name) ? mocks[name] : require(name), loaded, loaded.exports)
  return loaded.exports
}
function goal(overrides = {}) {
  return { id: 'first', title: '整理资料', description: '长期研究任务', status: 'active', progress: '已有资料', next_action: '撰写报告',
    steps: [{ text: '整理', status: 'done' }], budget: { max_runs: 20, max_seconds_per_run: 600 }, used: { runs: 3, total_seconds: 5 },
    checkpoints: [{ at: 1, summary: '取得进展', outcome: 'ok' }], created_at: 1, updated_at: 1, auto_advance: false, auto_interval_secs: 300, auto_runs: 0, ...overrides }
}
function fixture(initial = [goal()]) {
  setActivePinia(createPinia())
  const calls = []
  let callback
  let cleaned = 0
  let failCommand = ''
  const snapshot = { goals: initial, running_id: null, automatic: false }
  const mocks = {
    '@tauri-apps/api/core': { async invoke(command, payload) {
      calls.push({ command, payload })
      if (command === failCommand) throw new Error('未授权，已停止')
      if (command === 'list_goals') return structuredClone(snapshot)
      if (command === 'create_goal') { const created = goal({ title: payload.title }); snapshot.goals.push(created); return created }
      if (command === 'goal_advance') return { goal: snapshot.goals[0], message: '已达到设定的推进次数上限' }
      if (command === 'set_goal_status') snapshot.goals.find(goal => goal.id === payload.id).status = payload.status
      if (command === 'update_goal_settings') Object.assign(snapshot.goals.find(goal => goal.id === payload.id), { auto_advance: payload.autoAdvance, auto_interval_secs: payload.autoIntervalSecs, budget: payload.budget })
      if (command === 'delete_goal') snapshot.goals = snapshot.goals.filter(goal => goal.id !== payload.id)
    } },
    '@tauri-apps/api/event': { async listen(name, handler) { calls.push({ command: 'listen', name }); callback = handler; return () => cleaned++ } },
  }
  const filename = fileURLToPath(new URL('src/stores/goalStore.ts', root))
  const store = evaluate(readFileSync(filename, 'utf8'), filename, mocks).useGoalStore()
  return { store, calls, snapshot, event: payload => callback({ payload }), cleaned: () => cleaned, fail: command => { failCommand = command } }
}
function panel(store) {
  const filename = fileURLToPath(new URL('src/components/GoalsPanel.vue', root))
  const { descriptor } = parse(readFileSync(filename, 'utf8'), { filename })
  const script = compileScript(descriptor, { id: 'goals-panel' })
  const component = evaluate(script.content, filename, { '../stores/goalStore': { useGoalStore: () => store } }).default
  const template = compileTemplate({ source: descriptor.template.content, filename, id: 'goals-panel', compilerOptions: { bindingMetadata: script.bindings } })
  assert.deepEqual(template.errors, [])
  component.render = evaluate(template.code, filename).render
  return component
}
function handlers(store) {
  const filename = fileURLToPath(new URL('src/components/GoalsPanel.vue', root))
  const source = readFileSync(filename, 'utf8').match(/<script setup lang="ts">([\s\S]*?)<\/script>/)[1]
    .replace("const emit = defineEmits<{ close: [] }>()", 'const emit = () => {}')
    + '\nexport { title, description, maxRuns, maxSeconds, create, remove, toggle, interval, budget };'
  return evaluate(source, filename, { '../stores/goalStore': { useGoalStore: () => store } , vue: { ...vue } })
}

test('subscribes before initial load, updates global running status, and cleans listener', async () => {
  const fixtureValue = fixture()
  await fixtureValue.store.init()
  assert.equal(fixtureValue.calls[0].command, 'listen')
  fixtureValue.event({ goals: [goal()], running_id: 'first', automatic: true })
  assert.equal(fixtureValue.store.runningTitle, '整理资料')
  assert.equal(fixtureValue.store.automatic, true)
  fixtureValue.store.dispose()
  assert.equal(fixtureValue.cleaned(), 1)
})

test('creates explicitly with the default budget and required Chinese notice', async () => {
  const fixtureValue = fixture([])
  await fixtureValue.store.create('研究报告', '需要长期整理', { max_runs: 20, max_seconds_per_run: 600 })
  assert.deepEqual(fixtureValue.calls[0], { command: 'create_goal', payload: { title: '研究报告', description: '需要长期整理', budget: { max_runs: 20, max_seconds_per_run: 600 } } })
  assert.equal(fixtureValue.store.goals[0].auto_advance, false)
  assert.equal(fixtureValue.store.feedback, '我建了一个目标：研究报告，你可以在「目标」面板里看到并让我继续')
})

test('manual advance displays Chinese stop explanation and reloads persisted state', async () => {
  const fixtureValue = fixture([goal({ status: 'paused' })])
  await fixtureValue.store.advance('first')
  assert.equal(fixtureValue.store.feedback, '已达到设定的推进次数上限')
  assert.equal(fixtureValue.store.goals[0].status, 'paused')
  assert.deepEqual(fixtureValue.calls.map(call => call.command), ['goal_advance', 'list_goals'])
})

test('pause, resume and cancel use explicit backend status transitions', async () => {
  const fixtureValue = fixture()
  for (const status of ['paused', 'active', 'cancelled']) await fixtureValue.store.setStatus('first', status)
  assert.deepEqual(fixtureValue.calls.filter(call => call.command === 'set_goal_status').map(call => call.payload.status), ['paused', 'active', 'cancelled'])
  assert.equal(fixtureValue.store.goals[0].status, 'cancelled')
})

test('auto settings send interval and budget without unrelated configuration writes', async () => {
  const fixtureValue = fixture()
  await fixtureValue.store.settings(goal(), true, 120, { max_runs: 30, max_seconds_per_run: 15 })
  assert.deepEqual(fixtureValue.calls[0], { command: 'update_goal_settings', payload: { id: 'first', autoAdvance: true, autoIntervalSecs: 120, budget: { max_runs: 30, max_seconds_per_run: 15 } } })
  assert.equal(fixtureValue.store.goals[0].auto_advance, true)
  assert.equal(fixtureValue.store.goals[0].auto_interval_secs, 120)
})

test('backend permission failure is visible and never optimistically enables auto mode', async () => {
  const fixtureValue = fixture()
  await fixtureValue.store.refresh()
  fixtureValue.fail('update_goal_settings')
  await fixtureValue.store.settings(goal(), true, 300)
  assert.match(fixtureValue.store.error, /未授权/)
  assert.equal(fixtureValue.store.goals[0].auto_advance, false)
})

test('delete command reloads the goal list', async () => {
  const fixtureValue = fixture()
  await fixtureValue.store.remove('first')
  assert.equal(fixtureValue.store.goals.length, 0)
  assert.deepEqual(fixtureValue.calls[0], { command: 'delete_goal', payload: { id: 'first' } })
})

test('new-session reminder includes active and blocked goals but does not advance them', async () => {
  const fixtureValue = fixture([goal(), goal({ id: 'blocked', status: 'blocked' }), goal({ id: 'done', status: 'done' }), goal({ id: 'cancelled', status: 'cancelled' })])
  await fixtureValue.store.remind()
  assert.equal(fixtureValue.store.attentionCount, 2)
  assert.match(fixtureValue.store.reminder, /系统消息：你有 2 个/)
  assert.match(fixtureValue.store.reminder, /由你决定/)
  assert.deepEqual(fixtureValue.calls.map(call => call.command), ['list_goals'])
  fixtureValue.snapshot.goals = []
  await fixtureValue.store.remind()
  assert.equal(fixtureValue.store.reminder, '')
})

test('panel renders all operations, blocked reason, step history, usage and billing warning', async () => {
  const fixtureValue = fixture([goal({ status: 'blocked', blocked_reason: '需要安装授权' })])
  await fixtureValue.store.refresh()
  const html = await renderToString(vue.createSSRApp(panel(fixtureValue.store)))
  for (const text of ['新建', '推进一次', '暂停', '继续', '取消', '删除', '需要你决定：需要安装授权', '3/20', '步骤与推进记录', '取得进展', '退出应用即停止', 'API 调用']) assert.ok(html.includes(text), text)
  assert.ok(!html.includes(' checked'))
})

test('panel displays visible automatic execution and automatic run count', async () => {
  const fixtureValue = fixture([goal({ auto_advance: true, auto_runs: 4 })])
  await fixtureValue.store.init()
  fixtureValue.event({ goals: fixtureValue.snapshot.goals, running_id: 'first', automatic: true })
  const html = await renderToString(vue.createSSRApp(panel(fixtureValue.store)))
  for (const text of ['正在自动推进', '已自动推进 4 次', '每 5 分钟', 'checked']) assert.ok(html.includes(text), text)
})

test('deletion requires confirmation and cancelled confirmation preserves goal', async () => {
  const fixtureValue = fixture()
  const controls = handlers(fixtureValue.store)
  const oldConfirm = globalThis.confirm
  try {
    globalThis.confirm = () => false
    await controls.remove(goal())
    assert.equal(fixtureValue.calls.length, 0)
    globalThis.confirm = message => { assert.match(message, /不可恢复/); return true }
    await controls.remove(goal())
    assert.equal(fixtureValue.calls[0].command, 'delete_goal')
  } finally { globalThis.confirm = oldConfirm }
})

test('default create controls and interval conversion match backend units', async () => {
  const fixtureValue = fixture([])
  const controls = handlers(fixtureValue.store)
  assert.equal(controls.maxRuns.value, 20)
  assert.equal(controls.maxSeconds.value, 600)
  controls.title.value = '研究报告'
  controls.description.value = '整理资料'
  await controls.create()
  controls.interval(goal(), { target: { value: '2' } })
  await new Promise(resolve => setImmediate(resolve))
  assert.equal(fixtureValue.calls.find(call => call.command === 'update_goal_settings').payload.autoIntervalSecs, 120)
})

test('failed automatic toggle rolls the visible checkbox back to persisted state', async () => {
  const fixtureValue = fixture()
  await fixtureValue.store.refresh()
  fixtureValue.fail('update_goal_settings')
  const controls = handlers(fixtureValue.store)
  const input = { checked: true }
  await controls.toggle(goal(), { target: input })
  assert.equal(input.checked, false)
  assert.match(fixtureValue.store.error, /未授权/)
})
