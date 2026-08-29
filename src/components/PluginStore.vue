<!-- 《铃·记忆体》插件市场概念验证（AI-5，可选模块）
     初期硬编码示例插件列表，点击"安装"调用后端安装命令（本地目录需先存在于磁盘） -->
<script setup lang="ts">
import { ref } from 'vue'
import { usePluginStore } from '../stores/pluginStore'

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
</script>

<template>
  <div class="store">
    <div v-if="msg" class="store-msg">{{ msg }}</div>
    <div v-for="item in marketItems" :key="item.id" class="store-card">
      <div class="store-info">
        <div class="store-name">
          {{ item.name }}
          <span v-if="isInstalled(item.id)" class="store-tag">已安装</span>
        </div>
        <div class="store-meta">v{{ item.version }} · {{ item.author }}</div>
        <div class="store-desc">{{ item.description }}</div>
      </div>
      <button
        class="primary-btn small"
        :disabled="installing === item.id"
        @click="onInstall(item)"
      >
        {{ isInstalled(item.id) ? '已安装' : '安装' }}
      </button>
    </div>
    <p class="store-tip">💡 概念验证：在线插件市场服务将在后续版本接入，当前列表为示例。</p>
  </div>
</template>

<style scoped>
.store {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.store-card {
  border: 1px solid var(--border, rgba(128, 128, 128, 0.25));
  border-radius: 10px;
  padding: 10px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  background: var(--bg-main, rgba(255, 255, 255, 0.5));
}
.store-name {
  font-weight: 600;
  font-size: 13px;
  display: flex;
  align-items: center;
  gap: 6px;
}
.store-tag {
  font-size: 10px;
  background: #d1fae5;
  color: #047857;
  padding: 1px 6px;
  border-radius: 8px;
  font-weight: 400;
}
.store-meta {
  font-size: 11px;
  opacity: 0.6;
  margin-top: 2px;
}
.store-desc {
  font-size: 12px;
  opacity: 0.8;
  margin-top: 4px;
}
.store-msg {
  background: rgba(255, 193, 7, 0.15);
  color: #b45309;
  border-radius: 6px;
  padding: 8px 10px;
  font-size: 12px;
  white-space: pre-line;
}
.store-tip {
  font-size: 11px;
  opacity: 0.6;
  text-align: center;
}
</style>
