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

class ImageReader {
  readAsDataURL() {
    this.result = 'data:image/jpeg;base64,AQID'
    this.onload()
  }
}

function load(relativePath, mocks = {}, exported = '', reader = ImageReader) {
  const filename = fileURLToPath(new URL(relativePath, root))
  const source = readFileSync(filename, 'utf8').replace('import.meta.env.VITE_USE_MOCK', "'0'")
  const script = relativePath.endsWith('.vue') ? source.match(/<script setup lang="ts">([\s\S]*?)<\/script>/)[1] : source
  const compiled = ts.transpileModule(script + exported, {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS },
  }).outputText
  const loaded = { exports: {} }
  const execute = vm.runInThisContext('(function(require, module, exports, FileReader, localStorage) {\n' + compiled + '\n})', { filename })
  execute((name) => Object.hasOwn(mocks, name) ? mocks[name] : require(name), loaded, loaded.exports, reader, { getItem: () => '0', setItem() {} })
  return loaded.exports
}

const files = load('src/utils/attachments.ts')
const textFile = (name = 'report.txt', data = '机密文本') => ({ name, size: Buffer.byteLength(data), text: async () => data })
const image = { kind: 'image', name: 'photo.jpg', mime: 'image/jpeg', size: 3, data: 'AQID' }
const text = { kind: 'text', name: 'report.txt', mime: 'text/plain', size: 12, data: '机密文本' }

test('all specified extensions and uppercase variants are supported', () => {
  for (const extension of ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp']) {
    assert.equal(files.fileSpec({ name: 'file.' + extension.toUpperCase(), size: 0 }).kind, 'image')
  }
  for (const extension of ['txt', 'md', 'json', 'csv', 'log', 'py', 'js', 'ts', 'rs', 'html', 'css', 'xml', 'yaml', 'toml', 'ini', 'sh']) {
    assert.equal(files.fileSpec({ name: 'file.' + extension, size: 0 }).kind, 'text')
    assert.ok(files.ATTACHMENT_ACCEPT.includes('.' + extension))
  }
  for (const name of ['report.pdf', 'document.docx', 'image.svg', 'noextension', 'txt', 'jpg']) {
    assert.throws(() => files.fileSpec({ name, size: 1 }), /暂不支持这种文件/)
  }
})

test('frontend enforces per-file limits and accepts exact boundaries', () => {
  assert.equal(files.fileSpec({ name: 'a.jpg', size: 5 * 1024 * 1024 }).kind, 'image')
  assert.equal(files.fileSpec({ name: 'a.txt', size: 1024 * 1024 }).kind, 'text')
  assert.throws(() => files.fileSpec({ name: 'a.jpg', size: 5 * 1024 * 1024 + 1 }), /图片附件不能超过 5MB/)
  assert.throws(() => files.fileSpec({ name: 'a.txt', size: 1024 * 1024 + 1 }), /文本附件不能超过 1MB/)
})

test('image base64 omits data prefix and text stays UTF-8', async () => {
  const picture = await files.readAttachment({ name: 'photo.JPG', size: 3 })
  assert.equal(picture.mime, 'image/jpeg')
  assert.equal(picture.data, 'AQID')
  assert.deepEqual(await files.readAttachment(textFile()), text)
  await assert.rejects(files.readAttachment({ ...textFile(), text: async () => '中'.repeat(400000) }), /文本附件不能超过 1MB/)
})

test('read failures produce Chinese messages rather than file contents', async () => {
  const failing = load('src/utils/attachments.ts', {}, '', class { readAsDataURL() { this.onerror() } })
  await assert.rejects(failing.readAttachment({ name: 'a.png', size: 1 }), /附件读取失败，请重新选择/)
  await assert.rejects(files.readAttachment({ ...textFile(), text: async () => { throw new Error('secret file contents') } }), /附件读取失败，请重新选择/)
})

function inputPanel(send = async () => true) {
  const calls = []
  const chat = { inputText: '', isLoading: false }
  const panel = load('src/components/ChatInput.vue', {
    vue: { ...vue, onMounted() {} },
    '@tauri-apps/api/event': { listen: async () => () => {} },
    '../stores/chatStore': { useChatStore: () => chat },
    '../stores/settingStore': { useSettingStore: () => ({ depth: 2 }) },
    '../utils/attachments': files,
    '../composables/useStreamRender': { useStreamRender: () => ({
      send: async (...args) => { calls.push(args); return send(...args) },
      sendAgent: async () => calls.push('agent'), cancelAgent() {},
    }) },
  }, '\nexport { attachments, attachmentError, readingFiles, agentMode, selectFiles, handleSend }')
  return { panel, chat, calls }
}

test('picker accepts valid files, reports unsupported ones, and allows removal', async () => {
  const { panel } = inputPanel()
  const target = { files: [textFile(), { name: 'a.pdf', size: 1 }], value: 'selected' }
  await panel.selectFiles({ target })
  assert.equal(panel.attachments.value.length, 1)
  assert.equal(panel.attachmentError.value, '暂不支持这种文件')
  assert.equal(target.value, '')
  assert.equal(panel.readingFiles.value, false)
  panel.attachments.value.splice(0, 1)
  assert.equal(panel.attachments.value.length, 0)
  assert.match(readFileSync(new URL('src/components/ChatInput.vue', root), 'utf8'), /@click="attachments.splice\(index, 1\)"/)
})

test('attachment-only send bypasses Agent and does not require AI routing preflight', async () => {
  const { panel, calls } = inputPanel()
  panel.agentMode.value = true
  panel.attachments.value = [image, text]
  await panel.handleSend()
  assert.deepEqual(calls, [['', 2, [image, text]]])
  assert.equal(panel.attachments.value.length, 0)
})

test('failed sends retain selected attachments; reading blocks sending', async () => {
  const { panel, calls, chat } = inputPanel(async () => false)
  panel.attachments.value = [text]
  chat.inputText = '你好'
  panel.readingFiles.value = true
  await panel.handleSend()
  assert.equal(calls.length, 0)
  panel.readingFiles.value = false
  await panel.handleSend()
  assert.equal(panel.attachments.value.length, 1)
  assert.equal(chat.inputText, '你好')
})

test('text-only input retains its original clearing behavior on listener failure', async () => {
  const { panel, chat } = inputPanel(async () => false)
  chat.inputText = '你好'
  await panel.handleSend()
  assert.equal(chat.inputText, '')
  assert.equal(panel.attachmentError.value, '')
})

test('IPC omits attachments for old calls and forwards them unchanged for uploads', async () => {
  const calls = []
  const api = load('src/utils/tauri.ts', {
    '@tauri-apps/api/core': { invoke: async (...args) => calls.push(args) },
    '@tauri-apps/api/event': {},
  })
  await api.sendMessage('你好', 2, 'session', 'request')
  assert.deepEqual(calls[0], ['send_message', { content: '你好', depth: 2, sessionId: 'session', requestId: 'request' }])
  await api.sendMessage('', 2, 'session', 'request', [image, text])
  assert.deepEqual(calls[1][1].attachments, [image, text])
})

async function rendererFixture(failSend = false, failedListener = null) {
  const callbacks = {}
  const sent = []
  const saved = []
  setActivePinia(createPinia())
  const session = (id, messages = []) => ({ meta: { id, updated_at: 'now' }, messages })
  const chatApi = {
    listSessions: async () => [], createSession: async () => session('session'),
    saveSession: async (id, messages) => { saved.push(JSON.stringify(messages)); return session(id, messages) },
  }
  const { useChatStore } = load('src/stores/chatStore.ts', { '../utils/tauri': chatApi })
  const chat = useChatStore()
  await chat.createSession()
  const api = { sendMessage: async (...args) => { sent.push(args); if (failSend) throw new Error('secret response') } }
  for (const name of ['onChatChunk', 'onChatEnd', 'onChatError', 'onChatUsage', 'onChatRoute']) {
    api[name] = async (callback) => {
      callbacks[name] = callback
      if (name === failedListener) throw new Error('listener unavailable')
      return () => {}
    }
  }
  const { useStreamRender } = load('src/composables/useStreamRender.ts', {
    vue: { ...vue, onMounted() {}, onUnmounted() {} },
    '../stores/chatStore': { useChatStore: () => chat },
    '../stores/settingStore': { useSettingStore: () => ({ aiToolbox: true }) },
    '../stores/desktopStore': { useDesktopStore: () => ({ executeToolboxItem: () => { throw new Error('must not intercept attachments') } }) },
    '../stores/quickCommandStore': { useQuickCommandStore: () => ({ commands: [], load: async () => { throw new Error('must not intercept attachments') } }) },
    '../stores/milestoneStore': { useMilestoneStore: () => ({ recordChat: async () => {}, record: async () => {} }) },
    '../utils/tauri': api,
  })
  return { renderer: useStreamRender(), callbacks, chat, sent, saved }
}

test('renderer sends data but persists only attachment names and kinds', async () => {
  const { renderer, callbacks, chat, sent, saved } = await rendererFixture()
  assert.equal(await renderer.send('截图', 2, [image, text]), true)
  assert.deepEqual(sent[0][4], [image, text])
  assert.deepEqual(chat.messages[0].attachments, [{ kind: 'image', name: 'photo.jpg' }, { kind: 'text', name: 'report.txt' }])
  callbacks.onChatEnd()
  await chat.saveCurrentSession()
  assert.ok(saved.length)
  assert.ok(saved.every((value) => !value.includes('AQID') && !value.includes('机密文本') && !value.includes('mime')))
})

test('failed attachment IPC does not silently generate a mock success', async () => {
  const { renderer, chat } = await rendererFixture(true)
  assert.equal(await renderer.send('你好', 2, [image]), false)
  assert.equal(chat.messages[1].content, '')
  assert.equal(chat.isLoading, false)
  assert.equal(chat.interruptedIds[chat.messages[1].id], true)
})

test('unavailable routing listener does not block attachment delivery', async () => {
  const { renderer, sent } = await rendererFixture(false, 'onChatRoute')
  assert.equal(await renderer.send('你好', 2, [image]), true)
  assert.deepEqual(sent[0][4], [image])
})

test('core stream listener failure still blocks sends safely', async () => {
  const { renderer, sent, chat } = await rendererFixture(false, 'onChatChunk')
  assert.equal(await renderer.send('你好', 2, [image]), false)
  assert.equal(sent.length, 0)
  assert.equal(chat.isLoading, false)
})

test('text-only sends retain the original requirement for route registration', async () => {
  const { renderer, sent, callbacks } = await rendererFixture(false, 'onChatRoute')
  assert.equal(await renderer.send('你好', 2, [image]), true)
  callbacks.onChatEnd()
  assert.equal(await renderer.send('你好', 2), false)
  assert.equal(sent.length, 1)
})
