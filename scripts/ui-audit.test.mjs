// UI 规范门禁：把「设计规范」变成能 grep 出来的硬指标，别靠肉眼审。
//   pnpm test:ui
//
// 六个检查项：
//   1. 组件里不许出现 emoji 当图标/状态
//   2. 字面色值（#fff / rgb() / rgba()）只许出现在 tokens.css
//   3. --ui-* 令牌只许在 tokens.css 里定义
//   4. font-size 不许写死像素值（必须走 --ui-fs-*）
//   5. 面板组件不许自带 position: fixed（抽屉内嵌）
//   6. 页面里不许用 alert() / confirm()
//
// 两类文件不参与断言，但都会打印出来，避免「悄悄放行」：
//   · EXEMPT：结构性例外（独立窗口、必须固定色的画布等），永远不管
//   · LEGACY：还没迁移的旧文件，属于待办；迁移完就从这里删掉
import assert from 'node:assert/strict'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../', import.meta.url))
const srcDir = join(root, 'src')

/** 结构性例外：本目标明确不碰的窗口/固定色画布 */
const EXEMPT = new Map([
  ['src/components/FloatingBall.vue', '悬浮球是独立窗口，本目标不碰'],
  ['src/components/Bubble.vue', '气泡是独立窗口，本目标不碰'],
  ['src/components/PixelArtPanel.vue', '像素画布与调色板必须是固定色值'],
  ['src/views/RecoveryView.vue', '救援模式是独立窗口'],
  ['src/views/OnboardingView.vue', '首次启动引导页'],
])

/** 待迁移清单：还没换成新令牌的旧文件。迁移完请从这里删掉对应行。 */
const LEGACY = new Map([
  ['src/components/MonitorSettings.vue', '设置页子面板，等设置页批次收尾'],
  ['src/components/SyncPanel.vue', '设置页子面板，等设置页批次收尾'],
  ['src/components/QuickCommandPanel.vue', '快捷指令面板，表情图标要换成 TheIcon'],
  ['src/components/PluginStore.vue', '插件市场子页，跟随插件面板一起重做'],
  ['src/components/RegexTester.vue', '快捷指令里的正则测试器，跟随快捷指令面板一起重做'],
])

function collect(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name)
    const st = statSync(full)
    if (st.isDirectory()) {
      if (name === 'node_modules' || name === 'dist') continue
      collect(full, out)
    } else if (/\.(vue|ts)$/.test(name)) {
      out.push(full)
    }
  }
  return out
}

const files = collect(srcDir).map((full) => ({
  path: relative(root, full).replace(/\\/g, '/'),
  text: readFileSync(full, 'utf8'),
}))

/** 去掉注释（整块 + 行注释）后再匹配，避免把说明文字当成违规 */
function stripComments(text) {
  return text
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split(/\r?\n/)
    .filter((line) => !line.trim().startsWith('//'))
    .join('\n')
}

const EMOJI = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}\u{2B00}-\u{2BFF}\u{FE0F}]/u

/**
 * 逐行跑一条规则，返回 { strict, legacy, exempt } 三组命中。
 * strict 才断言；legacy 只统计；exempt 完全跳过。
 */
function scan(check) {
  const strict = []
  const legacy = []
  for (const f of files) {
    if (EXEMPT.has(f.path)) continue
    const bucket = LEGACY.has(f.path) ? legacy : strict
    const lines = stripComments(f.text).split(/\r?\n/)
    lines.forEach((line, i) => {
      if (check(line)) bucket.push(`${f.path}:${i + 1}  ${line.trim().slice(0, 78)}`)
    })
  }
  return { strict, legacy }
}

function failReport(hits, hint) {
  if (!hits.length) return ''
  return `\n${hint}\n` + hits.slice(0, 20).map((h) => `  · ${h}`).join('\n')
}

function logLegacy(name, hits) {
  if (!hits.length) return
  const byFile = new Map()
  for (const hit of hits) {
    const file = hit.split(':')[0]
    byFile.set(file, (byFile.get(file) ?? 0) + 1)
  }
  const parts = [...byFile.entries()].map(([f, n]) => `${f}×${n}`)
  console.log(`  ↳ [待迁移] ${name}：${hits.length} 处未清理 · ${parts.join('，')}`)
}

test('1. 组件里不许用 emoji 当图标或状态', () => {
  const { strict, legacy } = scan((line) => EMOJI.test(line))
  logLegacy('emoji', legacy)
  assert.equal(strict.length, 0, failReport(strict, '发现 emoji，换成 TheIcon / AppStatusDot / 纯文字：'))
})

test('2. 字面色值只许出现在 tokens.css', () => {
  const literal = /#[0-9a-fA-F]{3,8}\b|\brgba?\(/
  const { strict, legacy } = scan((line) => literal.test(line))
  logLegacy('字面色值', legacy)
  assert.equal(strict.length, 0, failReport(strict, '发现字面色值，改用 var(--ui-*)：'))
})

test('3. --ui-* 令牌只许在 tokens.css 定义', () => {
  const define = /--ui-[a-z0-9-]+\s*:/i
  const { strict, legacy } = scan((line) => define.test(line))
  logLegacy('令牌定义', legacy)
  assert.equal(strict.length, 0, failReport(strict, '令牌定义跑到组件里了，挪回 tokens.css：'))
})

test('4. font-size 不许写死像素', () => {
  const literal = /font-size\s*:\s*[0-9.]+px/
  const { strict, legacy } = scan((line) => literal.test(line))
  logLegacy('字号写死', legacy)
  assert.equal(strict.length, 0, failReport(strict, '字号写死了，改用 var(--ui-fs-*)：'))
})

test('5. 面板组件不许自带 position: fixed', () => {
  const panels = [
    'src/components/MemoryPanel.vue',
    'src/components/PluginManager.vue',
    'src/components/ToolboxPanel.vue',
    'src/components/GoalsPanel.vue',
    'src/components/AuditPanel.vue',
  ]
  const hits = []
  const legacy = []
  for (const path of panels) {
    const f = files.find((x) => x.path === path)
    if (!f) {
      hits.push(`${path}: 文件不存在（改名的话同步更新这个列表）`)
      continue
    }
    const bucket = LEGACY.has(path) ? legacy : hits
    f.text.split(/\r?\n/).forEach((line, i) => {
      if (/position\s*:\s*fixed/.test(line)) bucket.push(`${path}:${i + 1}  ${line.trim()}`)
    })
  }
  logLegacy('面板固定定位', legacy)
  assert.equal(hits.length, 0, failReport(hits, '面板还在用固定定位，改成普通流：'))
})

test('6. 页面里不许用 alert() / confirm()', () => {
  const call = /(^|[^.\w])(alert|confirm)\s*\(/
  const { strict, legacy } = scan((line) => call.test(line) && !line.includes('confirmDialog'))
  logLegacy('原生弹窗', legacy)
  assert.equal(strict.length, 0, failReport(strict, '还有原生弹窗，改用 toast() / confirmDialog()：'))
})
