<!-- 《铃·记忆体》气泡弹窗（AI-6 任务 6）
     独立半透明气泡窗口：监听 monitor-trigger 事件 → 显示铃的回复
     显示 3 秒后自动隐藏；点击 → 恢复主窗口并聚焦 -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { currentMonitor, getCurrentWindow, LogicalPosition, primaryMonitor } from '@tauri-apps/api/window'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { assetUrl, isImagePath, onMonitorTrigger } from '../utils/tauri'
import { useSettingStore } from '../stores/settingStore'

const win = getCurrentWindow()
const setting = useSettingStore()
// 铃的头像：图片路径则显示图片
const avatarImg = computed(() => (isImagePath(setting.avatarSuzu) ? assetUrl(setting.avatarSuzu!) : null))

const visible = ref(false)
const message = ref('')
const appName = ref('')
const windowTitle = ref('')

let hideTimer: number | undefined
let unlisten: (() => void) | undefined

// 显示时长（毫秒），可在设置扩展；默认 3 秒
const DURATION = 3000

onMounted(async () => {
  // 定位到屏幕右下角（monitor.size 是物理像素，除以 scaleFactor 换算逻辑坐标）
  try {
    const monitor = (await currentMonitor()) || (await primaryMonitor())
    if (monitor) {
      const scale = monitor.scaleFactor || 1
      const lw = Math.round(monitor.size.width / scale)
      const lh = Math.round(monitor.size.height / scale)
      const w = 380
      const h = 120
      await win.setPosition(
        new LogicalPosition(Math.max(0, lw - w - 24), Math.max(0, lh - h - 24)),
      )
    }
  } catch { /* 忽略 */ }

  unlisten = await onMonitorTrigger((payload) => {
    message.value = payload.reply
    appName.value = payload.app_name
    windowTitle.value = payload.window_title
    visible.value = true
    if (hideTimer) clearTimeout(hideTimer)
    hideTimer = window.setTimeout(() => {
      visible.value = false
      win.hide()
    }, DURATION)
    win.show().catch(() => {})
  })
})

onUnmounted(() => {
  if (hideTimer) clearTimeout(hideTimer)
  unlisten?.()
})

// 点击气泡 → 恢复主窗口并跳转到对话
async function onClickBubble() {
  try {
    const main = await WebviewWindow.getByLabel('main')
    await main?.show()
    await main?.setFocus()
    win.hide()
  } catch { /* 忽略 */ }
}
</script>

<template>
  <div class="bubble-shell" @click="onClickBubble">
    <transition name="fade">
      <div v-if="visible" class="bubble-card">
        <div class="bubble-head">
          <span class="avatar">
            <img v-if="avatarImg" :src="avatarImg" class="avatar-img" />
            <template v-else>{{ setting.avatarSuzu || '铃' }}</template>
          </span>
          <span class="from">铃</span>
          <span class="ctx">{{ appName }}<template v-if="windowTitle"> · {{ windowTitle }}</template></span>
        </div>
        <div class="bubble-text">{{ message }}</div>
        <div class="bubble-hint">点击回到主窗口</div>
      </div>
    </transition>
  </div>
</template>

<style scoped>
.bubble-shell {
  width: 100vw;
  height: 100vh;
  display: flex;
  align-items: flex-end;
  justify-content: flex-end;
  user-select: none;
  -webkit-user-select: none;
}
.bubble-card {
  width: 364px;
  border-radius: 16px;
  background: linear-gradient(135deg, rgba(255, 211, 224, 0.95), rgba(201, 228, 255, 0.95));
  color: #4a3641;
  padding: 12px 14px;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
  border: 1.5px solid rgba(255, 255, 255, 0.7);
  cursor: pointer;
}
.bubble-head {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 6px;
}
.avatar {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.75);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  overflow: hidden;
  flex-shrink: 0;
}
.avatar-img { width: 100%; height: 100%; object-fit: cover; }
.from {
  font-weight: 600;
  font-size: 13px;
}
.ctx {
  font-size: 11px;
  opacity: 0.65;
  margin-left: auto;
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.bubble-text {
  font-size: 13px;
  line-height: 1.5;
  word-break: break-all;
  max-height: 56px;
  overflow: hidden;
}
.bubble-hint {
  margin-top: 6px;
  text-align: right;
  font-size: 10px;
  opacity: 0.55;
}
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.25s ease, transform 0.25s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
  transform: translateY(10px);
}
</style>

<!-- 气泡窗口必须全透明 -->
<style>
html,
body,
#app,
.app-root {
  background: transparent !important;
}
</style>
