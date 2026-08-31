<!-- 离线增强方案面板：折叠/展开，三档检索模式切换 -->
<script setup lang="ts">
import { ref, watch } from 'vue'
import {
  getSearchMode,
  setSearchMode,
  checkVectorModelStatus,
  type SearchMode,
  type VectorModelStatus,
} from '../utils/tauri'

const expanded = ref(false)
const currentMode = ref<SearchMode>('bigram')
const saving = ref(false)

// 方案3 相关状态
const memoryWarning = ref<string | null>(null)
const modelStatus = ref<VectorModelStatus | null>(null)
const checkingModel = ref(false)

async function toggle() {
  expanded.value = !expanded.value
  if (expanded.value && currentMode.value === 'bigram') {
    currentMode.value = await getSearchMode()
  }
}

async function selectMode(mode: SearchMode) {
  if (mode === currentMode.value) return
  if (mode === 'vector') {
    await handleVectorCheck()
  } else {
    memoryWarning.value = null
    modelStatus.value = null
  }
  saving.value = true
  try {
    await setSearchMode(mode)
    currentMode.value = mode
  } finally {
    saving.value = false
  }
}

async function handleVectorCheck() {
  // 内存检测：优先 navigator.deviceMemory（GB，可能不精确），最低 1
  const ram = (navigator as Navigator & { deviceMemory?: number }).deviceMemory ?? 0
  if (ram > 0 && ram < 4) {
    memoryWarning.value = `你的电脑配置较低（约 ${ram}GB 内存），开启后可能卡顿，建议 8GB 以上内存使用`
  } else if (ram >= 4 && ram < 8) {
    memoryWarning.value = `检测到约 ${ram}GB 内存，可运行但可能卡顿`
  } else if (ram >= 8) {
    memoryWarning.value = `检测到约 ${ram}GB 内存，可流畅运行`
  } else {
    // 无法检测
    memoryWarning.value = null
  }

  // 向量模型文件检测
  checkingModel.value = true
  try {
    modelStatus.value = await checkVectorModelStatus()
  } finally {
    checkingModel.value = false
  }
}

// 打开时若当前已是 vector 模式，也执行一次检测
watch(expanded, async (v) => {
  if (v) {
    currentMode.value = await getSearchMode()
    if (currentMode.value === 'vector') {
      await handleVectorCheck()
    }
  }
})

// 内存警告样式：红色=低配，橙=中等，绿=充足
function memWarningClass(msg: string | null) {
  if (!msg) return ''
  if (msg.includes('可流畅')) return 'warn-ok'
  if (msg.includes('可能卡顿') && msg.includes('建议')) return 'warn-bad'
  return 'warn-mid'
}
</script>

<template>
  <div class="smp-wrap">
    <button class="smp-toggle" :class="{ active: expanded }" @click="toggle">
      离线增强方案{{ expanded ? ' ▲' : ' ▼' }}
    </button>

    <div v-if="expanded" class="smp-panel">
      <p class="smp-desc">选择离线记忆检索引擎，切换立即生效。</p>

      <div class="smp-modes">
        <!-- 方案1 -->
        <button
          class="smp-mode-btn"
          :class="{ sel: currentMode === 'bigram' }"
          :disabled="saving"
          @click="selectMode('bigram')"
        >
          <span class="smp-mode-name">方案1</span>
          <span class="smp-mode-label">字符 bigram</span>
          <span class="smp-mode-sub">零依赖，默认</span>
        </button>

        <!-- 方案2 -->
        <button
          class="smp-mode-btn"
          :class="{ sel: currentMode === 'bm25' }"
          :disabled="saving"
          @click="selectMode('bm25')"
        >
          <span class="smp-mode-name">方案2</span>
          <span class="smp-mode-label">jieba + BM25</span>
          <span class="smp-mode-sub">分词更精准</span>
        </button>

        <!-- 方案3 -->
        <button
          class="smp-mode-btn"
          :class="{ sel: currentMode === 'vector' }"
          :disabled="saving"
          @click="selectMode('vector')"
        >
          <span class="smp-mode-name">方案3</span>
          <span class="smp-mode-label">93MB 向量模型</span>
          <span class="smp-mode-sub">语义增强</span>
        </button>
      </div>

      <!-- 方案3 附加信息 -->
      <div v-if="currentMode === 'vector'" class="smp-vector-info">
        <p class="smp-req">至少需要 4GB 内存才能开启 93MB 模型，否则卡顿；8GB 可较流畅运行。</p>

        <p v-if="memoryWarning" class="smp-mem-warn" :class="memWarningClass(memoryWarning)">
          {{ memoryWarning }}
        </p>

        <p v-if="checkingModel" class="smp-checking">检测模型文件中…</p>
        <template v-else-if="modelStatus">
          <p v-if="modelStatus.available" class="smp-model-ok">已检测到模型文件。</p>
          <p v-else class="smp-model-missing">
            未检测到模型文件，请放入 ~/.铃记忆体/models/ 目录（支持 embedding.bin / model.onnx / embedding.gguf / model.safetensors）。
          </p>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.smp-wrap {
  margin-top: 6px;
}

.smp-toggle {
  background: none;
  border: 1px solid var(--border, #ccc);
  border-radius: 6px;
  padding: 2px 10px;
  font-size: var(--fs-11, 11px);
  color: var(--text-secondary, #888);
  cursor: pointer;
  line-height: 1.6;
  transition: background 0.15s, color 0.15s;
}
.smp-toggle:hover,
.smp-toggle.active {
  background: var(--input-bg, #f4f4f4);
  color: var(--text-main, #333);
}

.smp-panel {
  margin-top: 6px;
  padding: 10px 12px;
  border: 1px solid var(--border, #e0e0e0);
  border-radius: 8px;
  background: var(--input-bg, #fafafa);
  font-size: var(--fs-12, 12px);
  color: var(--text-main, #333);
  max-width: 340px;
}

.smp-desc {
  margin: 0 0 8px;
  color: var(--text-secondary, #888);
  font-size: var(--fs-11, 11px);
}

.smp-modes {
  display: flex;
  gap: 6px;
}

.smp-mode-btn {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: 6px 4px;
  border: 1px solid var(--border, #ddd);
  border-radius: 6px;
  background: var(--bg-main, #fff);
  cursor: pointer;
  font-size: var(--fs-11, 11px);
  color: var(--text-main, #333);
  transition: border-color 0.15s, background 0.15s;
  line-height: 1.4;
}
.smp-mode-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.smp-mode-btn:hover:not(:disabled) {
  border-color: var(--accent, #a78bba);
  background: var(--input-bg, #f4f4f4);
}
.smp-mode-btn.sel {
  border-color: var(--accent, #a78bba);
  background: color-mix(in srgb, var(--accent, #a78bba) 12%, transparent);
  color: var(--accent, #6b3f8a);
  font-weight: 600;
}

.smp-mode-name {
  font-size: var(--fs-10, 10px);
  opacity: 0.6;
}
.smp-mode-label {
  font-size: var(--fs-12, 12px);
  font-weight: 500;
}
.smp-mode-sub {
  font-size: var(--fs-10, 10px);
  opacity: 0.65;
}

.smp-vector-info {
  margin-top: 8px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.smp-req {
  margin: 0;
  font-size: var(--fs-11, 11px);
  color: var(--danger, #d9534f);
  line-height: 1.5;
}

.smp-mem-warn {
  margin: 0;
  font-size: var(--fs-11, 11px);
  line-height: 1.5;
}
.warn-bad { color: var(--danger, #d9534f); }
.warn-mid { color: var(--warning, #e6a817); }
.warn-ok  { color: var(--success, #4caf50); }

.smp-checking {
  margin: 0;
  font-size: var(--fs-11, 11px);
  color: var(--text-secondary, #888);
}
.smp-model-ok {
  margin: 0;
  font-size: var(--fs-11, 11px);
  color: var(--success, #4caf50);
}
.smp-model-missing {
  margin: 0;
  font-size: var(--fs-11, 11px);
  color: var(--danger, #d9534f);
  line-height: 1.5;
}
</style>
