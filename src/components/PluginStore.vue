<!-- 《铃·记忆体》插件市场（基础版）
     真实市场服务接入前，这里只展示硬编码示例插件；安装按钮给出本地目录说明
     皮肤：全部走 --ui-* 令牌；图标走 TheIcon，不再用 emoji -->
<script setup lang="ts">
import { ref } from 'vue'
import { usePluginStore } from '../stores/pluginStore'
import { openUrl } from '../utils/tauri'
import { toastErr } from '../utils/toast'
import TheIcon from './TheIcon.vue'
import AppButton from './ui/AppButton.vue'
import AppStatusDot from './ui/AppStatusDot.vue'

const store = usePluginStore()
const installing = ref<string>('')
const msg = ref('')

// 硬编码示例市场（概念验证：真实市场服务后续接入）
const marketItems = [
  {
    id: 'file_search',
    name: '文件检索（基础版）',
    version: '0.1.0',
    author: '铃·记忆体 官方',
    description: '按文件名关键词递归搜索文件，内置示例插件',
    installed: true,
  },
  {
    id: 'clipboard_history',
    name: '剪贴板历史',
    version: '0.1.0',
    author: '示例开发者',
    description: '记录并检索剪贴板历史（接口预留，暂未实现）',
    installed: false,
  },
  {
    id: 'browser_control',
    name: '浏览器控制',
    version: '0.1.0',
    author: '示例开发者',
    description: '打开指定网页、控制浏览器（接口预留，暂未实现）',
    installed: false,
  },
]

const isInstalled = (id: string) => store.plugins.some((p) => p.id === id)

async function onInstall(item: (typeof marketItems)[number]) {
  if (isInstalled(item.id)) {
    msg.value = '该插件已安装，可在「插件」页管理～'
    return
  }
  // 概念验证：这些示例插件尚未有真实安装源，提示开发者放置目录
  msg.value = `「${item.name}」的安装源尚未发布。\n开发者可将插件目录放到 %APPDATA%/ling-memoria/plugins/ 下，重启应用即可加载。`
}

// 预留：真实市场接口
// const onInstallRemote = async () => { await store.install(gitUrl) }

// 打开外部链接（跳转 Hermes 官方技能目录）
function openExternal(url: string) {
  openUrl(url).catch((e) => toastErr(`打开链接失败：${e}`))
}
</script>

<template>
  <div class="store">
    <p v-if="msg" class="store-notice" role="status">{{ msg }}</p>

    <article v-for="item in marketItems" :key="item.id" class="store-item">
      <div class="store-info">
        <div class="store-head">
          <span class="store-icon"><TheIcon name="plugin" :size="15" /></span>
          <span class="store-name">{{ item.name }}</span>
          <AppStatusDot v-if="isInstalled(item.id)" state="ok" text="已安装" />
        </div>
        <div class="store-meta">版本 {{ item.version }} · {{ item.author }}</div>
        <div class="store-desc">{{ item.description }}</div>
      </div>
      <AppButton
        size="sm"
        :variant="isInstalled(item.id) ? 'text' : 'ghost'"
        :disabled="installing === item.id"
        @click="onInstall(item)"
      >
        {{ isInstalled(item.id) ? '已安装' : '安装' }}
      </AppButton>
    </article>

    <p class="store-tip">当前为示例列表：在线插件市场服务会在后续版本接入。</p>

    <div class="store-foot">
      <AppButton size="sm" @click="openExternal('https://hermes-agent.nousresearch.com/docs/reference/skills-catalog')">
        <TheIcon name="search" :size="14" />浏览 Hermes 技能目录
      </AppButton>
    </div>
  </div>
</template>

<style scoped>
/* 插件抽屉的「市场」页内容列（父级滚动区已有内边距） */
.store {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-3);
  min-width: 0;
  font-size: var(--ui-fs-13);
  color: var(--ui-text);
}

/* 安装提示：与卡片区分的浅色提示条 */
.store-notice {
  margin: 0;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  background: color-mix(in srgb, var(--ui-warn) 12%, transparent);
  border-left: 3px solid var(--ui-warn);
  border-radius: var(--ui-r-sm);
  color: var(--ui-text);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  white-space: pre-line;
  overflow-wrap: anywhere;
}

.store-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  min-width: 0;
  padding: var(--ui-sp-3);
  background: var(--ui-raised);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-md);
}

.store-info {
  flex: 1 1 200px;
  min-width: 0;
}
.store-head {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  min-width: 0;
}
.store-icon {
  display: inline-flex;
  align-items: center;
  color: var(--ui-accent);
  flex: none;
}
.store-name {
  min-width: 0;
  font-size: var(--ui-fs-14);
  font-weight: 600;
  color: var(--ui-text);
  overflow-wrap: anywhere;
}
.store-meta {
  margin-top: var(--ui-sp-1);
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}
.store-desc {
  margin-top: var(--ui-sp-1);
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}

.store-tip {
  margin: 0;
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
  text-align: center;
}

.store-foot {
  display: flex;
  justify-content: center;
}
</style>
