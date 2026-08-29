<!-- 《铃·记忆体》悬浮球（AI-6 任务 2 / 4.1）
     独立透明小窗：拖拽移动 / 双击恢复主窗口 / 右键菜单（显示主窗口、暂停监测、退出）
     空闲呼吸动画；收到监测消息时闪烁 -->
<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import {
  currentMonitor,
  getCurrentWindow,
  LogicalPosition,
  primaryMonitor,
} from '@tauri-apps/api/window'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { getMonitorRules, onMonitorTrigger, toggleMonitoring } from '../utils/tauri'

const win = getCurrentWindow()

// —— 拖拽状态 ——
const dragging = ref(false)
let startMouse = { x: 0, y: 0 }
let startPos = { x: 0, y: 0 }

// —— 右键菜单 ——
const menuVisible = ref(false)
const menuPos = ref({ x: 0, y: 0 })

// —— 消息闪烁 ——
const hasMessage = ref(false)
let flashTimer: number | undefined
let unlistenTrigger: (() => void) | undefined

// 位置持久化（localStorage，下次启动恢复）
const POS_KEY = 'floating-ball-pos'
onMounted(async () => {
  try {
    const saved = localStorage.getItem(POS_KEY)
    if (saved) {
      const { x, y } = JSON.parse(saved)
      // 位置越界保护：防止旧坐标/多显示器切换导致窗口飞出屏幕
      const monitor = (await currentMonitor()) || (await primaryMonitor())
      if (monitor) {
        const scale = monitor.scaleFactor || 1
        const lw = Math.round(monitor.size.width / scale)
        const lh = Math.round(monitor.size.height / scale)
        const cx = Math.min(Math.max(0, x), Math.max(0, lw - 90))
        const cy = Math.min(Math.max(0, y), Math.max(0, lh - 90))
        await win.setPosition(new LogicalPosition(cx, cy))
      }
    } else {
      // 默认右上角：monitor.size 是物理像素，必须除以 scaleFactor 换算逻辑坐标
      const monitor = (await currentMonitor()) || (await primaryMonitor())
      if (monitor) {
        const scale = monitor.scaleFactor || 1
        const lw = Math.round(monitor.size.width / scale)
        const x = Math.max(0, lw - 110)
        await win.setPosition(new LogicalPosition(x, 60))
      }
    }
  } catch { /* 非 Tauri 环境忽略 */ }

  // 监听监测触发 → 闪烁提示
  unlistenTrigger = await onMonitorTrigger(() => {
    hasMessage.value = true
    if (flashTimer) clearTimeout(flashTimer)
    flashTimer = window.setTimeout(() => (hasMessage.value = false), 3000)
  })
})

onUnmounted(() => {
  if (flashTimer) clearTimeout(flashTimer)
  unlistenTrigger?.()
})

// —— 拖拽：mousedown 记录起点，mousemove 更新窗口位置 ——
async function onMouseDown(e: MouseEvent) {
  if (e.button !== 0) return
  dragging.value = true
  startMouse = { x: e.screenX, y: e.screenY }
  try {
    const pos = await win.outerPosition()
    startPos = { x: pos.x, y: pos.y }
  } catch {
    startPos = { x: 0, y: 0 }
  }
}

async function onMouseMove(e: MouseEvent) {
  if (!dragging.value) return
  const nx = startPos.x + (e.screenX - startMouse.x)
  const ny = startPos.y + (e.screenY - startMouse.y)
  try {
    await win.setPosition(new LogicalPosition(nx, ny))
  } catch { /* 忽略 */ }
}

function onMouseUp() {
  if (!dragging.value) return
  dragging.value = false
  // 持久化当前位置（带越界保护，避免存到屏幕外坐标）
  win.outerPosition().then((p) => {
    try {
      localStorage.setItem(POS_KEY, JSON.stringify({ x: p.x, y: p.y }))
    } catch { /* 忽略 */ }
  })
}

// —— 双击 → 恢复主窗口 ——
async function onDoubleClick() {
  try {
    const main = await WebviewWindow.getByLabel('main')
    await main?.show()
    await main?.setFocus()
  } catch { /* 忽略 */ }
}

// —— 右键菜单 ——
function onContextMenu(e: MouseEvent) {
  e.preventDefault()
  menuPos.value = { x: e.clientX, y: e.clientY }
  menuVisible.value = true
}

function hideMenu() {
  menuVisible.value = false
}

async function menuShowMain() {
  hideMenu()
  await onDoubleClick()
}

async function menuToggleMonitor() {
  hideMenu()
  // 通过 IPC 切换监测状态
  const res = await getMonitorRules()
  await toggleMonitoring(!res.enabled)
}

function menuExit() {
  hideMenu()
  // 退出应用：关闭主窗口（Tauri 默认全部窗口关闭后进程退出）
  WebviewWindow.getByLabel('main').then((m) => m?.close())
}
</script>

<template>
  <div
    class="ball-shell"
    @mousedown="onMouseDown"
    @mousemove="onMouseMove"
    @mouseup="onMouseUp"
    @mouseleave="onMouseUp"
    @dblclick="onDoubleClick"
    @contextmenu="onContextMenu"
  >
    <!-- 猫娘圆球：呼吸动画 / 消息闪烁 -->
    <div class="ball" :class="{ breathing: !dragging && !hasMessage, flashing: hasMessage, dragging }">
      <span class="ball-face">🐾</span>
    </div>

    <!-- 右键菜单 -->
    <div
      v-if="menuVisible"
      class="ball-menu"
      :style="{ left: menuPos.x + 'px', top: menuPos.y + 'px' }"
      @click.stop
    >
      <div class="menu-item" @click="menuShowMain">显示主窗口</div>
      <div class="menu-item" @click="menuToggleMonitor">暂停/开启监测</div>
      <div class="menu-item danger" @click="menuExit">退出</div>
    </div>

    <!-- 点击空白处关闭菜单 -->
    <div v-if="menuVisible" class="menu-mask" @click="hideMenu" @contextmenu.prevent="hideMenu" />
  </div>
</template>

<style scoped>
.ball-shell {
  width: 100vw;
  height: 100vh;
  overflow: hidden;
  position: relative;
  user-select: none;
  -webkit-user-select: none;
}
.ball {
  width: 72px;
  height: 72px;
  border-radius: 50%;
  margin: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #ffd3e0, #c9e4ff);
  border: 2px solid rgba(255, 255, 255, 0.7);
  box-shadow: 0 4px 16px rgba(255, 138, 171, 0.45);
  cursor: grab;
  transition: transform 0.15s ease, box-shadow 0.15s ease;
}
.ball.dragging {
  cursor: grabbing;
  transform: scale(1.1);
  box-shadow: 0 8px 24px rgba(255, 138, 171, 0.65);
}
/* 空闲呼吸动画 */
.ball.breathing {
  animation: breathe 3s ease-in-out infinite;
}
@keyframes breathe {
  0%, 100% { transform: scale(1); box-shadow: 0 4px 16px rgba(255, 138, 171, 0.45); }
  50% { transform: scale(1.06); box-shadow: 0 6px 22px rgba(255, 138, 171, 0.65); }
}
/* 消息闪烁 */
.ball.flashing {
  animation: flash 0.5s ease-in-out 6;
  border-color: #ff7a94;
}
@keyframes flash {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.35; transform: scale(1.15); }
}
.ball-face {
  font-size: 30px;
}
.ball-menu {
  position: fixed;
  z-index: 999;
  min-width: 130px;
  background: rgba(30, 28, 32, 0.95);
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 10px;
  padding: 6px;
  backdrop-filter: blur(8px);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
}
.menu-item {
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 13px;
  color: #eee6e7;
  cursor: pointer;
}
.menu-item:hover {
  background: rgba(255, 138, 171, 0.25);
}
.menu-item.danger {
  color: #ff7a94;
}
.menu-mask {
  position: fixed;
  inset: 0;
  z-index: 998;
}
</style>

<!-- 悬浮球窗口必须全透明 -->
<style>
html,
body,
#app,
.app-root {
  background: transparent !important;
}
</style>
