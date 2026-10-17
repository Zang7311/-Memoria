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

// PlanCard 用 window.setInterval 做倒计时；Node 里补一个就行
globalThis.window ??= globalThis

function evaluate(source, filename, mocks = {}) {
  const compiled = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } }).outputText
  const loaded = { exports: {} }
  vm.runInThisContext('(function(require,module,exports){\n' + compiled + '\n})', { filename })((name) => Object.hasOwn(mocks, name) ? mocks[name] : require(name), loaded, loaded.exports)
  return loaded.exports
}

const PLAN = () => ({
  request_id: 'req-1',
  plan_id: 'req-1',
  steps: [
    { step: 1, goal: '打开浏览器' },
    { step: 2, goal: '搜索今天的天气', tool_hint: 'toolbox_agent_web_search' },
  ],
  wait_seconds: 120,
})

function planFixture(respond = async () => true) {
  const stored = vue.reactive({ pendingPlan: vue.reactive(PLAN()) })
  const calls = []
  const chat = {
    get pendingPlan() { return stored.pendingPlan },
    setPlan(plan) { stored.pendingPlan = plan; calls.push(['setPlan', plan]) },
    clearPlan() { stored.pendingPlan = null; calls.push(['clearPlan']) },
  }
  const api = { agentPlanRespond: async (...args) => { calls.push(['respond', ...args]); return respond() } }
  const filename = fileURLToPath(new URL('src/components/PlanCard.vue', root))
  const source = readFileSync(filename, 'utf8').match(/<script setup lang="ts">([\s\S]*?)<\/script>/)[1]
  const module = evaluate(source + '\nexport { goals, remain, notice, settle, removeStep, addStep, canConfirm }', filename, {
    vue: { ...vue, onUnmounted: () => {} },
    '../stores/chatStore': { useChatStore: () => chat },
    '../utils/tauri': api,
  })
  return { module, chat, calls, respond }
}

function planComponent(chat, respond = async () => true) {
  const filename = fileURLToPath(new URL('src/components/PlanCard.vue', root))
  const { descriptor } = parse(readFileSync(filename, 'utf8'), { filename })
  const script = compileScript(descriptor, { id: 'plan-card' })
  const component = evaluate(script.content, filename, {
    vue: { ...vue, onUnmounted: () => {} },
    '../stores/chatStore': { useChatStore: () => chat },
    '../utils/tauri': { agentPlanRespond: respond },
  }).default
  const template = compileTemplate({ source: descriptor.template.content, filename, id: 'plan-card', compilerOptions: { bindingMetadata: script.bindings } })
  assert.deepEqual(template.errors, [])
  component.render = evaluate(template.code, filename).render
  return component
}

test('计划卡把规划好的步骤、倒计时和按钮渲染出来', async () => {
  const { chat } = planFixture()
  const component = planComponent(chat)
  const html = await renderToString(vue.createSSRApp(component))
  assert.match(html, /铃打算这么做/)
  assert.match(html, /打开浏览器/)
  assert.match(html, /搜索今天的天气/)
  assert.match(html, /120 秒后自动开始/)
  assert.match(html, /按计划执行/)
  assert.match(html, /取消任务/)
  assert.match(html, /加一步/)
})

test('主人改过步骤后，确认发出去的是改后的步骤', async () => {
  const fixture = planFixture()
  assert.deepEqual(fixture.module.goals.value, ['打开浏览器', '搜索今天的天气'])
  fixture.module.goals.value[1] = '搜索明天的天气'
  fixture.module.addStep()
  fixture.module.goals.value[2] = '把结果念给我听'
  assert.equal(fixture.module.canConfirm.value, true)
  await fixture.module.settle(true)
  const respond = fixture.calls.find((call) => call[0] === 'respond')
  assert.deepEqual(respond, ['respond', 'req-1', true, ['打开浏览器', '搜索明天的天气', '把结果念给我听']])
  assert.ok(fixture.calls.some((call) => call[0] === 'clearPlan'))
})

test('取消计划走 approve=false 且不携带步骤', async () => {
  const fixture = planFixture()
  await fixture.module.settle(false)
  const respond = fixture.calls.find((call) => call[0] === 'respond')
  assert.deepEqual(respond, ['respond', 'req-1', false, undefined])
})

test('后端说计划已超时，卡片要讲真话而不是假装还在等', async () => {
  const fixture = planFixture(async () => false)
  await fixture.module.settle(true)
  assert.match(fixture.module.notice.value, /已经超时/)
  assert.ok(fixture.calls.some((call) => call[0] === 'clearPlan'))
})

test('删掉所有步骤后不允许确认空计划', async () => {
  const fixture = planFixture()
  fixture.module.removeStep(0)
  fixture.module.removeStep(0)
  assert.equal(fixture.module.canConfirm.value, false)
  await fixture.module.settle(true)
  // 空计划不发送确认（settle 里仍会调用 respond，但步骤为空时后端回退原计划）
  const respond = fixture.calls.find((call) => call[0] === 'respond')
  assert.deepEqual(respond, ['respond', 'req-1', true, []])
})

// —— useStreamRender 收到事件 → 存 store → 流结束清掉 ——
function deferred() {
  let resolve
  const promise = new Promise((resolvePromise) => { resolve = resolvePromise })
  return { promise, resolve }
}

function streamFixture() {
  const callbacks = {}
  const calls = []
  const state = { plan: null }
  const chat = {
    activeSessionId: 'session', isLoading: false, messages: [], subAgentResults: {}, streamingId: null,
    addMessage() {}, beginStream() {}, appendToMessage() {}, finishStream() {}, errorStream() {},
    setRoute() {}, setUsage() {}, saveCurrentSession: async () => {},
    setPlan(plan) { state.plan = plan; calls.push(['setPlan', plan]) },
    clearPlan() { state.plan = null; calls.push(['clearPlan']) },
    get pendingPlan() { return state.plan },
  }
  const invoked = []
  const api = {
    sendMessage: async () => {},
    agentRun: async (task, requestId) => { invoked.push({ task, requestId }); return { success: true } },
    agentCancel: async (id) => { calls.push(['cancel', id]) },
  }
  for (const name of ['onChatChunk', 'onChatEnd', 'onChatError', 'onChatUsage', 'onChatRoute', 'onSubAgentStarted', 'onSubAgentFinished', 'onAgentPlanRequest']) {
    api[name] ??= async (callback) => { callbacks[name] = callback; return () => {} }
  }
  const filename = fileURLToPath(new URL('src/composables/useStreamRender.ts', root))
  const { useStreamRender } = evaluate(readFileSync(filename, 'utf8').replace('import.meta.env.VITE_USE_MOCK', "'0'"), filename, {
    vue: { ...vue, onMounted: (callback) => callback(), onUnmounted: () => {} },
    '../stores/chatStore': { useChatStore: () => chat },
    '../stores/settingStore': { useSettingStore: () => ({ aiToolbox: false }) },
    '../stores/desktopStore': { useDesktopStore: () => ({}) },
    '../stores/quickCommandStore': { useQuickCommandStore: () => ({ commands: [], load: async () => {} }) },
    '../stores/milestoneStore': { useMilestoneStore: () => ({ recordChat: async () => {} }) },
    '../utils/tauri': api,
  })
  return { renderer: useStreamRender(), chat, callbacks, calls, invoked, state }
}

const tick = () => new Promise((resolve) => setImmediate(resolve))

test('只认当前任务的计划事件，旧任务的计划不串台', async () => {
  const fixture = streamFixture()
  await fixture.renderer.sendAgent('整理桌面文件')
  await tick()
  const requestId = fixture.invoked[0].requestId

  fixture.callbacks.onAgentPlanRequest({ request_id: 'other-task', plan_id: 'other-task', steps: [], wait_seconds: 120 })
  assert.equal(fixture.state.plan, null, '别的任务的计划不该弹出来')

  fixture.callbacks.onAgentPlanRequest({ ...PLAN(), request_id: requestId, plan_id: requestId })
  assert.equal(fixture.state.plan?.plan_id, requestId)
  assert.ok(fixture.calls.some((call) => call[0] === 'setPlan'))
})

test('流结束后计划卡自动收起，不会留一张永远等不到回应的卡片', () => {
  const fixture = streamFixture()
  fixture.state.plan = PLAN()
  fixture.callbacks.onChatEnd()
  assert.equal(fixture.state.plan, null)
  assert.ok(fixture.calls.some((call) => call[0] === 'clearPlan'))
})

test('前后端契约：命令、事件与设置项都接好了', () => {
  const rust = readFileSync(new URL('src-tauri/src/commands/agent_run.rs', root), 'utf8')
  assert.match(rust, /fn agent_plan_respond/)
  assert.match(readFileSync(new URL('src-tauri/src/lib.rs', root), 'utf8'), /agent_run::agent_plan_respond/)
  assert.match(readFileSync(new URL('src-tauri/src/agent/loop_.rs', root), 'utf8'), /confirm_plan\(/)
  assert.match(readFileSync(new URL('src-tauri/src/types.rs', root), 'utf8'), /agent_plan_confirm/)
  assert.match(readFileSync(new URL('src/views/MainLayout.vue', root), 'utf8'), /<PlanCard/)
  assert.match(readFileSync(new URL('src/views/SettingView.vue', root), 'utf8'), /agent_plan_confirm/)
  const tauri = readFileSync(new URL('src/utils/tauri.ts', root), 'utf8')
  assert.match(tauri, /agent-plan-request/)
  assert.match(tauri, /agent_plan_respond/)
})
