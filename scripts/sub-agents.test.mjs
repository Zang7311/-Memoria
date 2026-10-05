import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import vm from 'node:vm'
import test from 'node:test'
import ts from 'typescript'
import * as vue from 'vue'
import { renderToString } from 'vue/server-renderer'
import { createPinia, setActivePinia } from 'pinia'

const require = createRequire(import.meta.url)
const root = new URL('../', import.meta.url)

function loadTypeScript(relativePath, mocks) {
  const filename = fileURLToPath(new URL(relativePath, root))
  const source = readFileSync(filename, 'utf8').replace('import.meta.env.VITE_USE_MOCK', "'0'")
  const compiled = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS },
  }).outputText
  const loaded = { exports: {} }
  const execute = vm.runInThisContext('(function(require, module, exports) {\n' + compiled + '\n})', { filename })
  execute((name) => Object.hasOwn(mocks, name) ? mocks[name] : require(name), loaded, loaded.exports)
  return loaded.exports
}

const storeModule = loadTypeScript('src/stores/chatStore.ts', { '../utils/tauri': {} })
const event = (overrides = {}) => ({ request_id: 'parent', sub_id: 'child', goal: '分析文件', status: '处理中', summary: '', ...overrides })

function deferred() {
  let resolve
  let reject
  const promise = new Promise((resolvePromise, rejectPromise) => { resolve = resolvePromise; reject = rejectPromise })
  return { promise, resolve, reject }
}

function makeRenderer(overrides = {}) {
  const callbacks = {}
  const cleaned = []
  const invoked = []
  const chat = {
    activeSessionId: 'session', isLoading: false, messages: [], subAgentResults: {}, streamingId: null,
    addMessage(message) { this.messages.push(message) },
    beginStream(id) { this.streamingId = id; this.isLoading = true },
    appendToMessage(id, chunk) { this.messages.find((message) => message.id === id).content += chunk },
    finishStream() { this.isLoading = false; this.streamingId = null },
    errorStream() { this.isLoading = false; this.streamingId = null },
    saveCurrentSession: async () => {}, setRoute() {}, setUsage() {},
    updateSubAgent(id, payload) { this.subAgentResults[id] = storeModule.mergeSubAgentEvent(this.subAgentResults[id] ?? [], payload) },
  }
  const api = {
    agentRun: async (task, requestId) => { invoked.push({ task, requestId }) },
    agentCancel: async () => {},
    ...overrides,
  }
  for (const name of ['onChatChunk', 'onChatEnd', 'onChatError', 'onChatUsage', 'onChatRoute', 'onSubAgentStarted', 'onSubAgentFinished']) {
    api[name] ??= async (callback) => { callbacks[name] = callback; return () => cleaned.push(name) }
  }
  const { useStreamRender } = loadTypeScript('src/composables/useStreamRender.ts', {
    vue: { ...vue, onMounted() {}, onUnmounted() {} },
    '../stores/chatStore': { useChatStore: () => chat },
    '../stores/settingStore': { useSettingStore: () => ({ aiToolbox: false }) },
    '../stores/desktopStore': { useDesktopStore: () => ({}) },
    '../stores/quickCommandStore': { useQuickCommandStore: () => ({ commands: [], load: async () => {} }) },
    '../stores/milestoneStore': { useMilestoneStore: () => ({ recordChat: async () => {} }) },
    '../utils/tauri': api,
  })
  return { renderer: useStreamRender(), chat, api, callbacks, cleaned, invoked }
}

const tick = () => new Promise((resolve) => setImmediate(resolve))

test('results merge by child ID without regressing completed cards', () => {
  const started = storeModule.mergeSubAgentEvent([], event())
  const finished = storeModule.mergeSubAgentEvent(started, event({ status: '完成', summary: '分析结果' }))
  assert.equal(finished.length, 1)
  assert.equal(finished[0].summary, '分析结果')
  assert.equal(started[0].status, '处理中')
  assert.deepEqual(storeModule.mergeSubAgentEvent(finished, event()), finished)
  assert.equal(storeModule.mergeSubAgentEvent(finished, event({ sub_id: 'second' })).length, 2)
})

test('card text caps count Unicode characters rather than UTF-16 units', () => {
  const result = storeModule.mergeSubAgentEvent([], event({ goal: '猫'.repeat(81), summary: '𠮷'.repeat(801) }))[0]
  assert.equal(Array.from(result.goal).length, 80)
  assert.equal(Array.from(result.summary).length, 800)
})

test('store attaches results to their own message, not another conversation', () => {
  setActivePinia(createPinia())
  const chat = storeModule.useChatStore()
  chat.updateSubAgent('first-message', event())
  chat.updateSubAgent('second-message', event({ sub_id: 'second' }))
  chat.updateSubAgent('first-message', event({ status: '完成' }))
  assert.equal(chat.subAgentResults['first-message'][0].status, '完成')
  assert.equal(chat.subAgentResults['second-message'][0].status, '处理中')
})

test('Agent invocation waits for both child-event registrations', async () => {
  const started = deferred()
  const finished = deferred()
  const fixture = makeRenderer({ onSubAgentStarted: () => started.promise, onSubAgentFinished: () => finished.promise })
  const sending = fixture.renderer.sendAgent('分析任务')
  await tick()
  assert.equal(fixture.invoked.length, 0)
  started.resolve(() => {})
  await tick()
  assert.equal(fixture.invoked.length, 0)
  finished.resolve(() => {})
  await sending
  assert.equal(fixture.invoked.length, 1)
})

test('partial child-listener failure cleans up and never starts the Agent', async () => {
  let cleaned = 0
  const fixture = makeRenderer({
    onSubAgentStarted: async () => () => { cleaned++ },
    onSubAgentFinished: async () => { throw new Error('注册失败') },
  })
  const originalWarn = console.warn
  console.warn = () => {}
  try { await fixture.renderer.sendAgent('分析任务') } finally { console.warn = originalWarn }
  assert.equal(cleaned, 1)
  assert.equal(fixture.invoked.length, 0)
  assert.equal(fixture.chat.isLoading, false)
  assert.match(fixture.chat.messages[1].content, /子 Agent 事件监听器注册失败/)
})

test('stale child events never attach to another request', async () => {
  const running = deferred()
  const fixture = makeRenderer({ agentRun: () => running.promise })
  const sending = fixture.renderer.sendAgent('分析任务')
  await tick()
  const requestId = fixture.chat.messages[1].id
  fixture.callbacks.onSubAgentStarted(event({ request_id: 'another-request' }))
  assert.deepEqual(fixture.chat.subAgentResults, {})
  running.resolve()
  await sending
  assert.equal(fixture.cleaned.filter((name) => name.startsWith('onSubAgent')).length, 2)
  assert.equal(fixture.chat.subAgentResults[requestId], undefined)
})

test('delegated results remain visible until parent completion and cancellation stays available', async () => {
  const running = deferred()
  let parentId
  let cancelledId
  const fixture = makeRenderer({
    agentRun: (_, requestId) => { parentId = requestId; return running.promise },
    agentCancel: async (requestId) => { cancelledId = requestId },
  })
  const sending = fixture.renderer.sendAgent('分析任务')
  await tick()
  const assistantId = fixture.chat.messages[1].id
  fixture.callbacks.onSubAgentStarted(event({ request_id: parentId }))
  fixture.callbacks.onChatChunk('派出子任务')
  fixture.callbacks.onChatEnd()
  assert.equal(fixture.chat.isLoading, true)
  await fixture.renderer.cancelAgent()
  assert.equal(cancelledId, parentId)
  fixture.callbacks.onSubAgentFinished(event({ request_id: parentId, status: '完成', summary: '<script>仅为文本</script>' }))
  fixture.callbacks.onChatChunk('最终汇总')
  fixture.callbacks.onChatEnd()
  assert.equal(fixture.chat.isLoading, true)
  running.resolve()
  await sending
  assert.equal(fixture.chat.isLoading, false)
  assert.equal(fixture.chat.subAgentResults[assistantId][0].status, '完成')
  assert.equal(fixture.chat.subAgentResults[assistantId][0].summary, '<script>仅为文本</script>')
  assert.equal(fixture.cleaned.filter((name) => name.startsWith('onSubAgent')).length, 2)
})

test('ordinary Agent streams still finish on the existing chat_end event', async () => {
  const running = deferred()
  const fixture = makeRenderer({ agentRun: () => running.promise })
  const sending = fixture.renderer.sendAgent('普通任务')
  await tick()
  fixture.callbacks.onChatChunk('普通结果')
  fixture.callbacks.onChatEnd()
  assert.equal(fixture.chat.isLoading, false)
  assert.equal(fixture.chat.messages[1].content, '普通结果')
  running.resolve()
  await sending
})

test('native details cards are collapsed by default and escape executable-looking text', async () => {
  const source = readFileSync(new URL('src/components/ChatBubble.vue', root), 'utf8')
  const fragment = source.match(/<details v-for="result in chat\.subAgentResults\[message\.id\]"[\s\S]*?<\/details>/)[0]
  const app = vue.createSSRApp({
    setup: () => ({ chat: { subAgentResults: { message: [event({ goal: '<b>目标</b>', status: '完成', summary: '<script>危险代码</script>' })] } }, message: { id: 'message' } }),
    render: vue.compile(fragment),
  })
  const html = await renderToString(app)
  assert.match(html, /<details/)
  assert.match(html, /<summary>/)
  assert.doesNotMatch(html, /<details[^>]*\sopen(?:\s|>|=)/)
  assert.match(html, /&lt;script&gt;危险代码&lt;\/script&gt;/)
  assert.doesNotMatch(html, /<script>/)
  assert.doesNotMatch(fragment, /v-html/)
})
