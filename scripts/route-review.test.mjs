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

function deferred() {
  let resolve
  let reject
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

async function makeChat(overrides = {}) {
  let sessionNumber = 0
  const session = (id, messages = []) => ({ meta: { id, updated_at: id }, messages })
  const api = {
    listSessions: async () => [],
    createSession: async () => session('session-' + ++sessionNumber),
    loadSession: async (id) => session(id),
    saveSession: async (id, messages) => session(id, messages),
    deleteSession: async () => {},
    ...overrides,
  }
  setActivePinia(createPinia())
  const { useChatStore } = loadTypeScript('src/stores/chatStore.ts', { '../utils/tauri': api })
  const chat = useChatStore()
  await chat.createSession()
  return chat
}

function route(chat, requestId = chat.streamingId, sessionId = chat.activeSessionId) {
  return { source: 'off', easy: true, needs_vision: false, model: 'cheap-model', request_id: requestId, session_id: sessionId }
}

function makeRenderer(chat, failedListener = null) {
  const registrations = []
  const callbacks = {}
  const mounted = []
  const unmounted = []
  const sent = []
  let cleanupCount = 0
  const api = {
    sendMessage: async (content, depth, sessionId, requestId) => {
      sent.push({ kind: 'chat', content, depth, sessionId, requestId })
      callbacks.onChatRoute(route(chat, requestId, sessionId))
    },
    agentRun: async (task, requestId) => { sent.push({ kind: 'agent', task, requestId }) },
    agentCancel: async () => {},
  }
  for (const name of ['onChatChunk', 'onChatEnd', 'onChatError', 'onChatUsage', 'onChatRoute']) {
    api[name] = (callback) => {
      callbacks[name] = callback
      const registration = deferred()
      registrations.push({ ...registration, name })
      return registration.promise
    }
  }
  const { useStreamRender } = loadTypeScript('src/composables/useStreamRender.ts', {
    vue: { ...vue, onMounted: (callback) => mounted.push(callback), onUnmounted: (callback) => unmounted.push(callback) },
    '../stores/chatStore': { useChatStore: () => chat },
    '../stores/settingStore': { useSettingStore: () => ({ aiToolbox: false }) },
    '../stores/desktopStore': { useDesktopStore: () => ({}) },
    '../stores/quickCommandStore': { useQuickCommandStore: () => ({ commands: [], load: async () => {} }) },
    '../stores/milestoneStore': { useMilestoneStore: () => ({ recordChat: async () => {}, record: async () => {} }) },
    '../utils/tauri': api,
  })
  const renderer = useStreamRender()
  const finishRegistration = (registration) => {
    if (registration.name === failedListener) registration.reject(new Error('test registration failure'))
    else registration.resolve(() => cleanupCount++)
  }
  return { renderer, mounted, unmounted, registrations, sent, finishRegistration, cleanupCount: () => cleanupCount }
}

test('G: beginning a new reply clears the previous route immediately', async () => {
  const chat = await makeChat()
  chat.beginStream('first')
  chat.setRoute(route(chat))
  assert.notEqual(chat.lastRoute, null)
  chat.beginStream('second')
  assert.equal(chat.lastRoute, null)
  chat.setRoute(route(chat, 'first'))
  assert.equal(chat.lastRoute, null)
  chat.setRoute(route(chat, 'second'))
  assert.equal(chat.lastRoute.request_id, 'second')
})

test('G: routes must match both current session and active request', async () => {
  const chat = await makeChat()
  chat.beginStream('current')
  chat.setRoute(route(chat, 'current', 'another-session'))
  chat.setRoute(route(chat, 'old-request'))
  assert.equal(chat.lastRoute, null)
  chat.setRoute(route(chat))
  const accepted = chat.lastRoute
  chat.finishStream('current')
  chat.setRoute({ ...route(chat, 'current'), model: 'late-model' })
  assert.deepEqual(chat.lastRoute, accepted)
})

test('G: switching clears route before saving and rejects delayed old-session events', async () => {
  const saving = deferred()
  const chat = await makeChat({ saveSession: (id) => saving.promise.then(() => ({ meta: { id, updated_at: id }, messages: [] })) })
  chat.beginStream('old-request')
  const oldRoute = route(chat)
  chat.setRoute(oldRoute)
  const switching = chat.switchSession('session-b')
  assert.equal(chat.lastRoute, null)
  chat.setRoute(oldRoute)
  assert.equal(chat.lastRoute, null)
  saving.resolve()
  await switching
  chat.setRoute(oldRoute)
  assert.equal(chat.lastRoute, null)
  assert.equal(chat.activeSessionId, 'session-b')
})

test('G: creating a session, clearing messages, and deleting the active session clear route ownership', async () => {
  const chat = await makeChat()
  for (const action of [() => chat.createSession(), () => chat.clearMessages(), () => chat.deleteSession(chat.activeSessionId)]) {
    chat.beginStream('request')
    const previousRoute = route(chat)
    chat.setRoute(previousRoute)
    await action()
    assert.equal(chat.lastRoute, null)
    chat.setRoute(previousRoute)
    assert.equal(chat.lastRoute, null)
  }
})

test('G: error completion rejects late routes without replacing the existing result', async () => {
  const chat = await makeChat()
  chat.beginStream('request')
  chat.setRoute(route(chat))
  const accepted = chat.lastRoute
  chat.errorStream('request')
  chat.setRoute({ ...route(chat, 'request'), model: 'late-model' })
  assert.deepEqual(chat.lastRoute, accepted)
})

test('H: chat IPC waits for all five listeners and receives an immediate route event', async () => {
  const chat = await makeChat()
  const fixture = makeRenderer(chat)
  fixture.mounted.forEach((callback) => callback())
  const sending = fixture.renderer.send('在吗')
  await new Promise(setImmediate)
  assert.equal(fixture.registrations.length, 5)
  assert.equal(fixture.sent.length, 0)
  fixture.registrations.slice(0, 4).forEach(fixture.finishRegistration)
  await new Promise(setImmediate)
  assert.equal(fixture.sent.length, 0)
  fixture.finishRegistration(fixture.registrations[4])
  await sending
  assert.equal(fixture.sent.length, 1)
  assert.equal(fixture.sent[0].sessionId, chat.activeSessionId)
  assert.equal(fixture.sent[0].requestId, chat.streamingId)
  assert.equal(chat.lastRoute.request_id, fixture.sent[0].requestId)
  assert.equal(chat.lastRoute.source, 'off')
})

test('H: sending before mount still registers listeners and waits for readiness', async () => {
  const chat = await makeChat()
  const fixture = makeRenderer(chat)
  const sending = fixture.renderer.send('你好')
  await new Promise(setImmediate)
  assert.equal(fixture.sent.length, 0)
  fixture.registrations.forEach(fixture.finishRegistration)
  await sending
  fixture.mounted.forEach((callback) => callback())
  assert.equal(fixture.registrations.length, 5)
  assert.equal(fixture.sent.length, 1)
})

test('H: Agent IPC also waits for listener readiness', async () => {
  const chat = await makeChat()
  const fixture = makeRenderer(chat)
  const sending = fixture.renderer.sendAgent('test task')
  await new Promise(setImmediate)
  assert.equal(fixture.sent.length, 0)
  fixture.registrations.forEach(fixture.finishRegistration)
  await sending
  assert.equal(fixture.sent[0].kind, 'agent')
})

test('H: failed registration cleans up partial listeners and never sends IPC', async () => {
  const chat = await makeChat()
  const fixture = makeRenderer(chat, 'onChatRoute')
  fixture.mounted.forEach((callback) => callback())
  const sending = fixture.renderer.send('你好')
  await new Promise(setImmediate)
  fixture.registrations.forEach(fixture.finishRegistration)
  await sending
  assert.equal(fixture.sent.length, 0)
  assert.equal(fixture.cleanupCount(), 4)
  assert.equal(chat.isLoading, false)
})

test('H: registrations that finish after idle unmount are cleaned up', async () => {
  const chat = await makeChat()
  const fixture = makeRenderer(chat)
  fixture.mounted.forEach((callback) => callback())
  fixture.unmounted.forEach((callback) => callback())
  fixture.registrations.forEach(fixture.finishRegistration)
  await new Promise(setImmediate)
  assert.equal(fixture.cleanupCount(), 5)
})

test('D: backend and settings page use identical vision hints', () => {
  const backend = readFileSync(new URL('src-tauri/src/engine/model_router.rs', root), 'utf8')
  const frontend = readFileSync(new URL('src/views/SettingView.vue', root), 'utf8')
  const rustHints = backend.match(/const VISION_HINTS: &\[&str\] = &\[([\s\S]*?)\];/)[1]
  const vueHints = frontend.match(/const VISION_MODELS = \[([^\]]+)\]/)[1]
  const hints = (source) => [...source.matchAll(/["']([^"']+)["']/g)].map((match) => match[1]).sort()
  assert.deepEqual(hints(rustHints), hints(vueHints))
})

test('G: IPC adapter forwards request and session IDs and receives the correlated route payload', async () => {
  const invocations = []
  let eventCallback
  const api = loadTypeScript('src/utils/tauri.ts', {
    '@tauri-apps/api/core': {
      invoke: async (command, payload) => { invocations.push({ command, payload }) },
      convertFileSrc: (path) => path,
    },
    '@tauri-apps/api/event': {
      listen: async (event, callback) => {
        assert.equal(event, 'chat_route')
        eventCallback = callback
        return () => {}
      },
    },
  })
  await api.sendMessage('在吗', 2, 'session-a', 'request-a')
  assert.deepEqual(invocations, [{ command: 'send_message', payload: {
    content: '在吗', depth: 2, sessionId: 'session-a', requestId: 'request-a',
  } }])
  let received
  await api.onChatRoute((payload) => { received = payload })
  const payload = { source: 'off', easy: true, needs_vision: false, model: 'cheap', session_id: 'session-a', request_id: 'request-a' }
  eventCallback({ payload })
  assert.deepEqual(received, payload)
})
