// 版本号一致性检查：一个版本号写在 5 个地方，靠人记必然漂，这里机器盯着。
//   package.json / src-tauri/tauri.conf.json / src-tauri/Cargo.toml
//   src-tauri/Cargo.lock（自己的包）/ README 版本徽章
// 另外核对最新 git tag 不能比文件里的版本新（打了 tag 却忘了改版本号）。
// 跑法：pnpm test:version
import assert from 'node:assert/strict'
import { execSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const root = new URL('../', import.meta.url)
const read = (p) => readFileSync(fileURLToPath(new URL(p, root)), 'utf8')

const pkg = JSON.parse(read('package.json'))
const tauri = JSON.parse(read('src-tauri/tauri.conf.json'))
const cargoToml = read('src-tauri/Cargo.toml')
const cargoLock = read('src-tauri/Cargo.lock')
const readme = read('README.md')

// Cargo.toml 里 [package] 段的 version（第一个顶格 version =）
const cargoVersion = /^version\s*=\s*"([^"]+)"/m.exec(cargoToml)?.[1]
// Cargo.lock 里自己那条：name = "mem" 紧跟 version
const lockVersion = /\[\[package\]\]\r?\nname = "mem"\r?\nversion = "([^"]+)"/.exec(cargoLock)?.[1]

test('package.json 与 tauri.conf.json 版本一致', () => {
  assert.ok(pkg.version, 'package.json 没有 version')
  assert.equal(tauri.version, pkg.version, `tauri.conf.json 是 ${tauri.version}，package.json 是 ${pkg.version}`)
})

test('Cargo.toml 与 Cargo.lock 版本跟上', () => {
  assert.ok(cargoVersion, 'Cargo.toml 没读到 version')
  assert.equal(cargoVersion, pkg.version, `Cargo.toml 是 ${cargoVersion}，package.json 是 ${pkg.version}`)
  assert.equal(lockVersion, pkg.version, `Cargo.lock 里是 ${lockVersion}，package.json 是 ${pkg.version}`)
})

test('README 版本徽章跟上', () => {
  // shields.io 把预发布的 "-" 写成 "--"：0.7.0-beta.9 => version-0.7.0--beta.9-<color>
  const expected = `badge/version-${pkg.version.replace(/-/g, '--')}-`
  assert.ok(readme.includes(expected), `README 里应有 ${expected}，实际徽章行：${(readme.match(/badge\/version-[^"]+/) || ['没找到'])[0]}`)
})

test('最新 git tag 不能比文件里的版本新', () => {
  let tag
  try {
    tag = execSync('git describe --tags --abbrev=0', { cwd: fileURLToPath(root), encoding: 'utf8' }).trim()
  } catch {
    return // 没有 git 或没有 tag，跳过
  }
  if (!tag) return
  const tagVersion = tag.replace(/^v/, '')
  // 只在「同一条 0.x 线且 tag 更新」时报警，跨分支 tag（v1.0-0.4.6-teens）不算
  const sameLine = (a, b) => a.split('.').slice(0, 2).join('.') === b.split('.').slice(0, 2).join('.')
  if (!sameLine(tagVersion, pkg.version)) return
  assert.ok(
    compare(tagVersion, pkg.version) <= 0,
    `最新 tag ${tag} 比文件版本 ${pkg.version} 新，说明打完 tag 忘了改版本号`,
  )
})

// 极简 semver 比较：核心数字 + 预发布标识，够这个检查用
function compare(a, b) {
  const split = (s) => {
    const clean = s.replace(/^v/, '').split('+')[0]
    const i = clean.indexOf('-')
    const core = i < 0 ? clean : clean.slice(0, i)
    const pre = i < 0 ? '' : clean.slice(i + 1)
    return { nums: core.split('.').map((x) => Number(x) || 0), pre: pre ? pre.split('.') : [] }
  }
  const x = split(a)
  const y = split(b)
  for (let i = 0; i < Math.max(x.nums.length, y.nums.length); i += 1) {
    const p = x.nums[i] ?? 0
    const q = y.nums[i] ?? 0
    if (p !== q) return p < q ? -1 : 1
  }
  if (!x.pre.length && !y.pre.length) return 0
  if (!x.pre.length) return 1
  if (!y.pre.length) return -1
  for (let i = 0; i < Math.max(x.pre.length, y.pre.length); i += 1) {
    const p = x.pre[i]
    const q = y.pre[i]
    if (p === undefined) return -1
    if (q === undefined) return 1
    const pn = Number(p)
    const qn = Number(q)
    if (Number.isNaN(pn) && Number.isNaN(qn)) {
      if (p !== q) return p < q ? -1 : 1
    } else if (Number.isNaN(pn)) {
      return 1
    } else if (Number.isNaN(qn)) {
      return -1
    } else if (pn !== qn) {
      return pn < qn ? -1 : 1
    }
  }
  return 0
}
