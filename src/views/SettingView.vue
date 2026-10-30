<!-- 《铃·记忆体》设置页外框
     左侧分区导航 + 右侧内容。只保留「标签切换」和跨分页共享状态（配置文件加载、工具箱面板开关），
     各分区的内容都在 src/components/settings/ 下，一页一个组件，样式统一走 design tokens。 -->
<script setup lang="ts">
import { computed, onMounted, provide, ref, type Component } from 'vue'
import TheIcon from '../components/TheIcon.vue'
import ToolboxPanel from '../components/ToolboxPanel.vue'
import DiagnosticView from '../components/DiagnosticView.vue'
import MonitorSettings from '../components/MonitorSettings.vue'
import PluginManager from '../components/PluginManager.vue'
import QuickCommandPanel from '../components/QuickCommandPanel.vue'
import SyncPanel from '../components/SyncPanel.vue'
import GeneralSettings from '../components/settings/GeneralSettings.vue'
import ModelSettings from '../components/settings/ModelSettings.vue'
import MemorySettings from '../components/settings/MemorySettings.vue'
import PersonaSettings from '../components/settings/PersonaSettings.vue'
import SponsorSettings from '../components/settings/SponsorSettings.vue'
import { SETTINGS_TOOLBOX } from '../components/settings/shared'
import { useMilestoneStore } from '../stores/milestoneStore'
import { useSettingStore } from '../stores/settingStore'

interface NavItem {
  key: string
  label: string
  icon: string
  page: Component
}

// 分区顺序沿用旧版标签顺序；「监测 / 插件 / 指令 / 同步 / 诊断」是既有组件的整页嵌入，直接引用不动它们
const NAV: NavItem[] = [
  { key: 'general', label: '通用', icon: 'settings', page: GeneralSettings },
  { key: 'model', label: '模型', icon: 'command', page: ModelSettings },
  { key: 'memory', label: '记忆', icon: 'memory', page: MemorySettings },
  { key: 'monitor', label: '监测', icon: 'monitor', page: MonitorSettings },
  { key: 'style', label: '形象', icon: 'customize', page: PersonaSettings },
  { key: 'plugin', label: '插件', icon: 'plugin', page: PluginManager },
  { key: 'quick', label: '指令', icon: 'command', page: QuickCommandPanel },
  { key: 'sync', label: '同步', icon: 'sync', page: SyncPanel },
  { key: 'sponsor', label: '赞助', icon: 'sponsor', page: SponsorSettings },
  { key: 'diag', label: '诊断', icon: 'diagnostic', page: DiagnosticView },
]

const active = ref('general')
const current = computed(() => NAV.find((item) => item.key === active.value) ?? NAV[0])

// 工具箱面板挂在外框上：切分区时不跟着「通用」卸载，行为与旧版一致
const toolboxOpen = ref(false)
provide(SETTINGS_TOOLBOX, {
  open: toolboxOpen,
  toggle: () => {
    toolboxOpen.value = !toolboxOpen.value
  },
})

const setting = useSettingStore()
const milestone = useMilestoneStore()

onMounted(async () => {
  if (!setting.loaded) await setting.loadConfig()
  // P3：确保陪伴记录已加载（防 App.vue 未加载完成）
  if (!milestone.loaded) await milestone.load()
})
</script>

<template>
  <div class="settings-view">
    <nav class="side" role="tablist" aria-label="设置分区">
      <button
        v-for="item in NAV"
        :key="item.key"
        type="button"
        role="tab"
        class="side-btn"
        :class="{ on: active === item.key }"
        :aria-selected="active === item.key"
        @click="active = item.key"
      >
        <TheIcon :name="item.icon" :size="15" />
        <span class="side-label">{{ item.label }}</span>
      </button>
    </nav>

    <div class="main">
      <header class="head">
        <h3 class="head-title">设置</h3>
        <span class="head-sub">{{ current.label }}</span>
      </header>
      <div class="content">
        <component :is="current.page" :key="current.key" />
      </div>
    </div>

    <ToolboxPanel v-if="toolboxOpen" @close="toolboxOpen = false" />
  </div>
</template>

<style scoped>
.settings-view {
  display: flex;
  height: 100%;
  min-height: 0;
  background: var(--ui-bg);
  color: var(--ui-text);
}

/* ---------- 左侧分区导航 ---------- */
.side {
  flex: none;
  width: 168px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--ui-sp-3) var(--ui-sp-2);
  border-right: 1px solid var(--ui-border);
  overflow-y: auto;
}
.side-btn {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  width: 100%;
  height: 34px;
  padding: 0 var(--ui-sp-2);
  border: 0;
  border-radius: var(--ui-r-sm);
  background: transparent;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-13);
  line-height: 1;
  text-align: left;
  transition: background var(--ui-dur-1) var(--ui-ease), color var(--ui-dur-1) var(--ui-ease);
}
.side-btn:hover {
  background: var(--ui-hover);
  color: var(--ui-text);
}
.side-btn.on {
  background: var(--ui-active);
  color: var(--ui-text);
}
.side-btn.on::before {
  content: '';
  position: absolute;
  left: 0;
  width: 2px;
  height: 16px;
  border-radius: var(--ui-r-pill);
  background: var(--ui-accent);
}
.side-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ---------- 右侧内容 ---------- */
.main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}
.head {
  flex: none;
  display: flex;
  align-items: baseline;
  gap: var(--ui-sp-2);
  height: var(--ui-head-h);
  padding: 0 var(--ui-sp-5);
  padding-right: calc(var(--ui-sp-6) + var(--ui-sp-5));
  border-bottom: 1px solid var(--ui-border);
}
.head-title {
  margin: 0;
  font-size: var(--ui-fs-16);
  font-weight: 600;
  color: var(--ui-text);
}
.head-sub {
  font-size: var(--ui-fs-12);
  color: var(--ui-text-3);
}
.content {
  flex: 1;
  min-height: 0;
  padding: var(--ui-sp-4) var(--ui-sp-5);
  overflow-y: auto;
  overflow-x: hidden;
}
</style>
