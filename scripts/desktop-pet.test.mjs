import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import vm from 'node:vm'
import test from 'node:test'
import ts from 'typescript'

const require = createRequire(import.meta.url)
const root = new URL('../', import.meta.url)

function evaluate(source, filename, mocks = {}) {
  const compiled = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } }).outputText
  const loaded = { exports: {} }
  vm.runInThisContext('(function(require,module,exports){\n' + compiled + '\n})', { filename })((name) => Object.hasOwn(mocks, name) ? mocks[name] : require(name), loaded, loaded.exports)
  return loaded.exports
}

const pet = evaluate(readFileSync(fileURLToPath(new URL('src/utils/pet.ts', root)), 'utf8'), 'pet.ts')

test('每类台词池都有内容且取到的是池里的句子', () => {
  for (const kind of Object.keys(pet.PET_LINES)) {
    const line = pet.pickPetLine(kind)
    assert.equal(typeof line, 'string')
    assert.ok(line.length > 0, `${kind} 台词池不应为空`)
    assert.ok(pet.PET_LINES[kind].includes(line), `${kind} 应返回池里的句子`)
  }
})

test('random 返回 1 时取最后一句，不能越界（Math.random 理论到不了 1，但注入的随机数可能是 1）', () => {
  assert.equal(pet.pickPetLine('poke', () => 1), pet.PET_LINES.poke[pet.PET_LINES.poke.length - 1])
  assert.equal(pet.pickPetLine('poke', () => 0), pet.PET_LINES.poke[0])
})

test('待机小动作与自言自语的节奏都在约定区间内', () => {
  assert.equal(pet.nextIdleDelay(() => 0), 18000)
  assert.equal(pet.nextIdleDelay(() => 0.999999), 44999)
  assert.equal(pet.nextChatterDelay(() => 0), 45000)
  assert.equal(pet.nextChatterDelay(() => 0.999999), 119999)
  // 任何一次取值都不能越界
  for (let i = 0; i < 50; i += 1) {
    const idle = pet.nextIdleDelay()
    assert.ok(idle >= 18000 && idle < 45000, `待机间隔越界：${idle}`)
    const chatter = pet.nextChatterDelay()
    assert.ok(chatter >= 45000 && chatter < 120000, `自言自语间隔越界：${chatter}`)
  }
})

test('掉落目标贴屏幕底，不会算出负数', () => {
  assert.equal(pet.fallTargetY(1080, 464), 612)
  assert.equal(pet.fallTargetY(1080, 464, 0), 616)
  assert.equal(pet.fallTargetY(400, 464), 0, '窗口比屏幕还高时不能落到负坐标')
  assert.equal(pet.SLEEP_AFTER_MS, 5 * 60 * 1000)
})

test('桌宠契约：气泡、摸头、拖动掉落、打瞌睡、自言自语都接在外壳里', () => {
  const ball = readFileSync(new URL('src/components/FloatingBall.vue', root), 'utf8')
  for (const token of ['speech', 'playPat', 'playPoke', 'dropAfterDrag', 'maybeSleep', 'maybeChatter', 'pickPetLine', 'zzz']) {
    assert.match(ball, new RegExp(token), `桌宠外壳缺少 ${token}`)
  }
  // 音量开关：自言自语受设置控制，关掉后不再冒气泡
  assert.match(ball, /floatingBallChatter/)
})

test('前后端设置与默认形象一致：桌宠默认头像，可关闭自言自语', () => {
  const defaults = readFileSync(new URL('src-tauri/src/config/defaults.rs', root), 'utf8')
  // Live2D 在高缩放屏幕上会溢出窗口（实机踩到），默认形象用稳定的头像
  assert.match(defaults, /floating_ball_mode: "avatar"/)
  assert.match(defaults, /floating_ball_chatter: true/)
  assert.match(readFileSync(new URL('src-tauri/src/types.rs', root), 'utf8'), /pub floating_ball_chatter: bool/)
  assert.match(readFileSync(new URL('src/stores/settingStore.ts', root), 'utf8'), /floatingBallChatter/)
  assert.match(readFileSync(new URL('src/views/SettingView.vue', root), 'utf8'), /floating_ball_chatter/)
  assert.match(readFileSync(new URL('src/views/SettingView.vue', root), 'utf8'), /桌宠（悬浮球）/)
})
