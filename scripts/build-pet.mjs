#!/usr/bin/env node
// 把桌宠（pet-native）编译出来并暂存到 pet-native/dist，供 Tauri 打进安装包。
//   pnpm build:pet
//
// 暂存后的结构（安装后在资源目录的 pet\ 下，就是桌宠 exe 的同目录）：
//   pet-native.exe
//   assets\avatar.png
//   live2d\PetL2D.dll  Live2DCubismCore.dll  FrameworkShaders\**  Resources\**
//
// 为什么要单独一个目录：live2d-render\build 里还混着 .ninja 文件、日志、测试程序，
// 整个目录塞进安装包会带上一堆垃圾。
import { execFileSync } from 'node:child_process'
import { cpSync, existsSync, mkdirSync, readdirSync, rmSync, statSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const petDir = join(root, 'pet-native')
const renderBuild = join(petDir, 'live2d-render', 'build')
const distDir = join(petDir, 'dist')

const fail = (msg) => {
  console.error(`\n[build:pet] ${msg}\n`)
  process.exit(1)
}

const fileCount = (dir) => {
  let files = 0
  let bytes = 0
  const walk = (d) => {
    for (const name of readdirSync(d)) {
      const p = join(d, name)
      const st = statSync(p)
      if (st.isDirectory()) walk(p)
      else {
        files += 1
        bytes += st.size
      }
    }
  }
  walk(dir)
  return { files, bytes }
}

// 1. 渲染 DLL 必须先有（改过 C++ 才需要重建，见 pet-native/README.md）
for (const dll of ['PetL2D.dll', 'Live2DCubismCore.dll']) {
  if (!existsSync(join(renderBuild, dll))) {
    fail(
      `没找到 ${dll}（${renderBuild}）。\n` +
        '先建渲染 DLL：\n' +
        "  . 'C:\\Tools\\devshell\\dev.ps1'\n" +
        `  cd "${join(petDir, 'live2d-render', 'build')}"\n` +
        '  cmake --build . --target PetL2D',
    )
  }
}

// 2. 编译桌宠本体
console.log('[build:pet] cargo build --release（pet-native）')
try {
  execFileSync('cargo', ['build', '--release', '--manifest-path', join(petDir, 'Cargo.toml')], {
    stdio: 'inherit',
  })
} catch {
  fail(
    'pet-native 编译失败。常见原因：\n' +
      '  · 桌宠正在运行，exe 被锁住（os error 5）→ 先右键退出桌宠或 Stop-Process -Name pet-native\n' +
      '  · 当前终端没有 MSVC 环境 → 先 . \'C:\\Tools\\devshell\\dev.ps1\'',
  )
}

const exePath = join(petDir, 'target', 'release', 'pet-native.exe')
if (!existsSync(exePath)) fail(`编译完了却没找到 ${exePath}`)

// 3. 清空重建暂存目录
rmSync(distDir, { recursive: true, force: true })
mkdirSync(join(distDir, 'live2d'), { recursive: true })
mkdirSync(join(distDir, 'assets'), { recursive: true })

cpSync(exePath, join(distDir, 'pet-native.exe'))
cpSync(join(petDir, 'assets', 'avatar.png'), join(distDir, 'assets', 'avatar.png'))

// 4. Live2D 运行时：DLL + 着色器缓存 + 模型
cpSync(join(renderBuild, 'PetL2D.dll'), join(distDir, 'live2d', 'PetL2D.dll'))
cpSync(join(renderBuild, 'Live2DCubismCore.dll'), join(distDir, 'live2d', 'Live2DCubismCore.dll'))
for (const sub of ['FrameworkShaders', 'Resources']) {
  const from = join(renderBuild, sub)
  if (!existsSync(from)) fail(`渲染运行时缺目录：${from}`)
  cpSync(from, join(distDir, 'live2d', sub), { recursive: true })
}

const { files, bytes } = fileCount(distDir)
console.log(
  `[build:pet] 好了 → ${distDir}（${files} 个文件，${(bytes / 1024 / 1024).toFixed(1)} MB）`,
)
