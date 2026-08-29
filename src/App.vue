<!-- 《铃·记忆体》根组件：按窗口 label 分发渲染
     main → 主布局 / floating-ball → 悬浮球 / bubble → 气泡弹窗 -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import MainLayout from './views/MainLayout.vue'
import FloatingBall from './components/FloatingBall.vue'
import Bubble from './components/Bubble.vue'
import OnboardingView from './views/OnboardingView.vue'
import { useSettingStore } from './stores/settingStore'
import { getCurrentWindow } from '@tauri-apps/api/window'

const setting = useSettingStore()
const theme = computed(() => setting.theme || 'dark')

// 当前窗口 label（Tauri 环境；非 Tauri 环境默认主窗口）
const winLabel = ref('main')
// 首次启动引导状态（仅主窗口）
const firstLaunch = ref(false)
const onboarded = ref(false)

onMounted(async () => {
  try {
    winLabel.value = getCurrentWindow().label
  } catch {
    winLabel.value = 'main'
  }
  // 主窗口：加载配置，若是首次启动则显示引导页
  if (winLabel.value === 'main') {
    try {
      await setting.loadConfig()
      firstLaunch.value = setting.firstLaunch && !onboarded.value
    } catch {
      firstLaunch.value = false
    }
  }
  // 引导完成后：进入主界面
  window.addEventListener('onboarding-done', () => {
    onboarded.value = true
    firstLaunch.value = false
    setting.firstLaunch = false
  })
})
</script>

<template>
  <div class="app-root" :class="theme">
    <!-- 首次启动引导（仅主窗口） -->
    <OnboardingView v-if="winLabel === 'main' && firstLaunch && !onboarded" />
    <MainLayout v-else-if="winLabel === 'main'" />
    <FloatingBall v-else-if="winLabel === 'floating-ball'" />
    <Bubble v-else-if="winLabel === 'bubble'" />
  </div>
</template>

<style scoped>
.app-root {
  height: 100vh;
  width: 100vw;
  background: var(--bg-main, #f6f6f6);
}
</style>

<!-- 全局主题变量定义（亮/暗两套） -->
<style>
:root,
.app-root.light {
  --bg-main: #f7f2f4;
  --bg-bar: rgba(255, 255, 255, 0.75);
  --text-main: #2b2323;
  --text-secondary: #8a8082;
  --text-user: #ffffff;
  --text-suzu: #5b3a63;
  --bubble-user-bg: #2d2d2d;
  --bubble-suzu-bg: linear-gradient(135deg, #ffe4e1, #fff0f5);
  --border: rgba(128, 128, 128, 0.25);
  --input-bg: #ffffff;
  --accent: #ff8fa3;
  --danger: #d9534f;
}
.app-root.dark {
  --bg-main: #1d1b1f;
  --bg-bar: rgba(34, 32, 36, 0.85);
  --text-main: #eee6e7;
  --text-secondary: #9a9294;
  --text-user: #ffffff;
  --text-suzu: #f3d9d9;
  --bubble-user-bg: #3a3438;
  --bubble-suzu-bg: linear-gradient(135deg, #4a3641, #3a2a36);
  --border: rgba(255, 255, 255, 0.12);
  --input-bg: #2a272b;
  --accent: #ff7a94;
  --danger: #ff6b6b;
}
/* ================= 五套 UI 风格主题（大版本方向） ================= */
/* ① Win10：亚克力深灰 + 微软蓝 */
.app-root.win10 {
  --bg-main: #202225;
  --bg-bar: rgba(32, 34, 37, 0.92);
  --text-main: #f0f0f0;
  --text-secondary: #a0a0a0;
  --text-user: #ffffff;
  --text-suzu: #d8d8d8;
  --bubble-user-bg: #2f3136;
  --bubble-suzu-bg: #3a3d42;
  --border: rgba(255, 255, 255, 0.12);
  --input-bg: #2b2d30;
  --accent: #00a4ef;
  --danger: #e81123;
}
/* ② 微软浏览器(Edge)：深蓝灰 + 简洁浏览器感 */
.app-root.edge {
  --bg-main: #232627;
  --bg-bar: rgba(35, 38, 39, 0.9);
  --text-main: #e8eaed;
  --text-secondary: #9aa0a6;
  --text-user: #ffffff;
  --text-suzu: #d5dbe0;
  --bubble-user-bg: #2c2f31;
  --bubble-suzu-bg: #3a3f42;
  --border: rgba(255, 255, 255, 0.13);
  --input-bg: #2a2d2f;
  --accent: #4d8bf5;
  --danger: #f28b82;
}
/* ③ 极简文字风：纯黑白 + 高对比 */
.app-root.minimal {
  --bg-main: #0d0d0d;
  --bg-bar: rgba(13, 13, 13, 0.96);
  --text-main: #d4d4d4;
  --text-secondary: #666666;
  --text-user: #000000;
  --text-suzu: #cccccc;
  --bubble-user-bg: #ffffff;
  --bubble-suzu-bg: #1a1a1a;
  --border: rgba(255, 255, 255, 0.22);
  --input-bg: #161616;
  --accent: #888888;
  --danger: #ff4444;
}
/* ④ iOS 毛玻璃之前(扁平化)：纯色块 + 系统蓝 */
.app-root.ios-flat {
  --bg-main: #1c1c1e;
  --bg-bar: rgba(44, 44, 46, 0.95);
  --text-main: #ffffff;
  --text-secondary: #98989d;
  --text-user: #ffffff;
  --text-suzu: #c8c8cc;
  --bubble-user-bg: #0a84ff;
  --bubble-suzu-bg: #2c2c2e;
  --border: rgba(255, 255, 255, 0.15);
  --input-bg: #2c2c2e;
  --accent: #0a84ff;
  --danger: #ff453a;
}
/* ⑤ iOS 毛玻璃之后(现代)：半透明 + 粉红 + 柔和渐变 */
.app-root.ios-glass {
  --bg-main: #1b1b1f;
  --bg-bar: rgba(44, 44, 46, 0.55);
  --text-main: #ffffff;
  --text-secondary: #a0a0a8;
  --text-user: #ffffff;
  --text-suzu: #f0d8e0;
  --bubble-user-bg: #ff2d55;
  --bubble-suzu-bg: linear-gradient(135deg, #4a3a4a, #3a2a3e);
  --border: rgba(255, 255, 255, 0.2);
  --input-bg: rgba(44, 44, 46, 0.6);
  --accent: #ff2d55;
  --danger: #ff453a;
}
/* ================= 五套主题 · 风格深化层（质感/圆角/控件/字体/滚动条） ================= */
/* 字体基调 */
.app-root.minimal { font-family: 'Cascadia Mono', 'Sarasa Mono SC', Consolas, 'Courier New', monospace; }
.app-root.ios-glass, .app-root.ios-flat { font-family: -apple-system, 'SF Pro Rounded', 'PingFang SC', 'Microsoft YaHei', system-ui, sans-serif; }

/* —— 滚动条（每套质感不同）—— */
.app-root ::-webkit-scrollbar { width: 8px; height: 8px; }
.app-root ::-webkit-scrollbar-thumb { border-radius: 4px; }
.app-root.win10 ::-webkit-scrollbar-thumb { background: rgba(128, 128, 128, 0.55); border-radius: 0; }
.app-root.edge ::-webkit-scrollbar-thumb { background: rgba(128, 128, 128, 0.45); }
.app-root.minimal ::-webkit-scrollbar { width: 6px; }
.app-root.minimal ::-webkit-scrollbar-thumb { background: #444; border-radius: 0; }
.app-root.ios-flat ::-webkit-scrollbar-thumb { background: rgba(120, 120, 128, 0.5); }
.app-root.ios-glass ::-webkit-scrollbar-thumb { background: rgba(255, 45, 85, 0.45); border-radius: 6px; }

/* ① Win10：亚克力 + 小圆角 + 扁平方块控件 */
.app-root.win10 .card, .app-root.win10 .settings-panel, .app-root.win10 .session-tab { border-radius: 4px !important; }
.app-root.win10 .btn { border-radius: 2px !important; }
.app-root.win10 .bubble-user, .app-root.win10 .bubble-suzu { border-radius: 6px !important; }
.app-root.win10 .bubble-suzu { border-bottom-left-radius: 2px !important; }
.app-root.win10 .bubble-user { border-bottom-right-radius: 2px !important; }
.app-root.win10 .card { background: rgba(32, 34, 37, 0.72); backdrop-filter: blur(10px); box-shadow: none; border: 1px solid rgba(255, 255, 255, 0.1); }
.app-root.win10 textarea { border-radius: 2px !important; }

/* ② Edge 浏览器：圆角卡片 + 轻阴影 + 浏览器标签感 */
.app-root.edge .card, .app-root.edge .settings-panel { border-radius: 10px !important; }
.app-root.edge .btn { border-radius: 8px !important; }
.app-root.edge .bubble-user, .app-root.edge .bubble-suzu { border-radius: 12px !important; }
.app-root.edge .card { background: rgba(35, 38, 39, 0.82); box-shadow: 0 4px 18px rgba(0, 0, 0, 0.25); }
.app-root.edge .session-tab { border-radius: 8px; }

/* ③ 极简文字：无圆角 + 无阴影 + 纯黑白 + 细边框 */
.app-root.minimal .card, .app-root.minimal .btn, .app-root.minimal .bubble-user,
.app-root.minimal .bubble-suzu, .app-root.minimal .settings-panel,
.app-root.minimal .session-tab, .app-root.minimal textarea { border-radius: 0 !important; }
.app-root.minimal .card, .app-root.minimal .settings-panel { background: #111 !important; box-shadow: none !important; border: 1px solid #333; }
.app-root.minimal .btn { background: #1a1a1a; border: 1px solid #333; box-shadow: none; }
.app-root.minimal .bubble-suzu, .app-root.minimal .bubble-user { box-shadow: none !important; }
.app-root.minimal textarea { background: #111; border: 1px solid #333; }

/* ④ iOS 扁平：纯色块 + 中圆角 + 无阴影 */
.app-root.ios-flat .card, .app-root.ios-flat .settings-panel { border-radius: 10px !important; background: #2c2c2e; box-shadow: none; border: none; }
.app-root.ios-flat .btn { border-radius: 8px !important; }
.app-root.ios-flat .bubble-user, .app-root.ios-flat .bubble-suzu { border-radius: 14px !important; box-shadow: none; }
.app-root.ios-flat .session-tab { border-radius: 8px; }

/* ⑤ iOS 毛玻璃：半透明 + 大圆角 + 模糊 + 轻阴影 */
.app-root.ios-glass .card, .app-root.ios-glass .settings-panel {
  border-radius: 16px !important;
  background: rgba(44, 44, 46, 0.4) !important;
  backdrop-filter: blur(20px) saturate(140%);
  -webkit-backdrop-filter: blur(20px) saturate(140%);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.25);
  border: 1px solid rgba(255, 255, 255, 0.15);
}
.app-root.ios-glass .btn { border-radius: 12px !important; }
.app-root.ios-glass .bubble-user, .app-root.ios-glass .bubble-suzu { border-radius: 18px !important; }
.app-root.ios-glass .bubble-suzu { border-bottom-left-radius: 6px !important; background: linear-gradient(135deg, rgba(74, 58, 74, 0.6), rgba(58, 42, 62, 0.6)) !important; backdrop-filter: blur(10px); }
.app-root.ios-glass .bubble-user { border-bottom-right-radius: 6px !important; }
.app-root.ios-glass textarea { border-radius: 12px !important; background: rgba(44, 44, 46, 0.5); }
.app-root.ios-glass .session-tab { border-radius: 12px; }
/* 毛玻璃需多彩背景才能体现模糊 → 主界面加柔和渐变 */
.app-root.ios-glass { background: linear-gradient(160deg, #2e2e40 0%, #23233a 45%, #3a2440 100%); }
.app-root.ios-glass .main-layout { background: transparent !important; }

html,
body {
  margin: 0;
  padding: 0;
  height: 100%;
  font-family: system-ui, 'Microsoft YaHei', -apple-system, sans-serif;
}
#app {
  height: 100%;
}
</style>
