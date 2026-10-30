<!-- 设置 · 模型页下部 · 本地 AI
     一键本地部署 AI（Ollama：检测 / 拉取模型 / 存储路径） + 离线语义检索模型（bge：检测 / 检索 / 一键安装） -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import AppButton from '../ui/AppButton.vue'
import AppCard from '../ui/AppCard.vue'
import AppField from '../ui/AppField.vue'
import AppInput from '../ui/AppInput.vue'
import AppStatusDot from '../ui/AppStatusDot.vue'
import { confirmDialog } from '../../utils/dialog'
import { toastErr, toastOk } from '../../utils/toast'
import {
  checkVectorModelStatus,
  detectGpuVram,
  detectOllama,
  installModel,
  openUrl,
  pullModel,
  scanModelFiles,
  setOllamaModelsPath,
  type ModelCandidate,
} from '../../utils/tauri'
import { ensureSettingsLoaded } from './shared'

// ==================== Ollama（一键本地部署） ====================
const ollama = ref<{ installed: boolean; models: string[] }>({ installed: false, models: [] })
const ollamaChecked = ref(false)
const pullModelName = ref('qwen2.5:3b')
const localAiMsg = ref('')
const gpuVram = ref<{ name: string; vram_mb: number }[]>([])
const modelsPath = ref('')
const pathMsg = ref('')

const vramText = computed(() =>
  gpuVram.value.map((g) => `${g.name}（${(g.vram_mb / 1024).toFixed(1)}GB）`).join('、'),
)

async function detectLocalAI() {
  try {
    ollama.value = await detectOllama()
  } catch {
    ollama.value = { installed: false, models: [] }
  }
  ollamaChecked.value = true
  await detectVramOnly()
}
async function detectVramOnly() {
  try {
    gpuVram.value = await detectGpuVram()
  } catch {
    gpuVram.value = []
  }
}

// 部署前检查显存，不合适先提醒（确认继续才拉，拒绝就停在原地）
const vramForceOk = ref(false)
async function checkVram(): Promise<boolean> {
  try {
    const gpus = await detectGpuVram()
    gpuVram.value = gpus
    if (gpus.length === 0) return true
    const maxVram = Math.max(...gpus.map((g) => g.vram_mb))
    if (maxVram < 4000 && !vramForceOk.value) {
      const go = await confirmDialog(
        `你的显卡显存只有 ${(maxVram / 1024).toFixed(1)}GB，跑本地 AI 会比较吃力。建议改用轻量模型（qwen2.5:3b 或更小），或切换 API 模式。还要继续拉取吗？`,
        { title: '显存不足提醒', confirmText: '继续拉取', cancelText: '改用 API 模式' },
      )
      if (!go) return false
      vramForceOk.value = true
    }
    return true
  } catch {
    return true
  }
}

async function doPullModel() {
  if (!pullModelName.value.trim()) return
  if (!(await checkVram())) return
  localAiMsg.value = `正在拉取 ${pullModelName.value.trim()}…（首次可能需几分钟，取决于网速）`
  try {
    const r = await pullModel(pullModelName.value.trim())
    localAiMsg.value = r
    await detectLocalAI()
  } catch (e) {
    localAiMsg.value = `拉取失败：${e}`
  }
}
async function doSaveModelsPath() {
  if (!modelsPath.value.trim()) return
  try {
    pathMsg.value = await setOllamaModelsPath(modelsPath.value.trim())
  } catch (e) {
    pathMsg.value = `保存失败：${e}`
  }
}
function openOllamaSite() {
  openUrl('https://ollama.com/download').catch((e) => toastErr(`打开链接失败：${e}`))
}

// ==================== 离线语义检索模型（bge，方案3） ====================
const vectorModelStatus = ref<{ available: boolean; message: string } | null>(null)
const vectorCandidates = ref<ModelCandidate[]>([])
const vectorScanning = ref(false)
const vectorInstalling = ref(false)
const vectorMsg = ref('')

async function checkVectorModel() {
  try {
    vectorModelStatus.value = await checkVectorModelStatus()
  } catch (e) {
    vectorModelStatus.value = null
    vectorMsg.value = `检测失败：${e}`
  }
}
async function scanVectorModels() {
  vectorScanning.value = true
  vectorMsg.value = ''
  try {
    vectorCandidates.value = await scanModelFiles()
    if (!vectorCandidates.value.length) {
      vectorMsg.value = '未在常见位置找到模型，请手动放入 ~/.铃记忆体/models/'
    }
  } catch (e) {
    vectorMsg.value = `扫描失败：${e}`
  } finally {
    vectorScanning.value = false
  }
}
async function doInstallVectorModel(path: string) {
  vectorInstalling.value = true
  vectorMsg.value = ''
  try {
    await installModel(path)
    await checkVectorModel()
    toastOk('模型安装成功，方案3（语义检索）已可用')
  } catch (e) {
    vectorMsg.value = `安装失败：${e}`
  } finally {
    vectorInstalling.value = false
  }
}

onMounted(async () => {
  await ensureSettingsLoaded()
  await detectLocalAI()
  await checkVectorModel()
})
</script>

<template>
  <AppCard title="一键本地部署 AI（Ollama）">
    <div class="status-line">
      <AppStatusDot
        :state="!ollamaChecked ? 'idle' : ollama.installed ? 'ok' : 'err'"
        :text="
          !ollamaChecked ? '检测中…' : ollama.installed ? `Ollama 已安装（${ollama.models.length} 个模型）` : '未检测到 Ollama，需先安装'
        "
      />
      <span class="spacer" />
      <AppButton v-if="ollamaChecked" size="sm" @click="detectLocalAI">重新检测</AppButton>
    </div>

    <template v-if="ollamaChecked && ollama.installed">
      <p v-if="ollama.models.length" class="muted">已装模型：{{ ollama.models.join('、') }}</p>
      <p v-else class="muted">还没有模型，拉一个开始用：</p>
      <p v-if="gpuVram.length" class="muted">显卡：{{ vramText }}</p>
      <AppField label="要拉取的模型" hint="推荐 qwen2.5:3b（轻量）或 qwen2.5:7b（均衡）。">
        <div class="row">
          <AppInput v-model="pullModelName" placeholder="qwen2.5:3b" />
          <AppButton variant="primary" @click="doPullModel">一键拉取模型</AppButton>
        </div>
      </AppField>
      <p v-if="localAiMsg" class="note">{{ localAiMsg }}</p>
      <AppField label="模型存储路径" hint="可选，例如 D:\ollama-models；保存后写入 OLLAMA_MODELS 用户环境变量。">
        <div class="row">
          <AppInput v-model="modelsPath" placeholder="模型存储路径（可选，如 D:\ollama-models）" />
          <AppButton @click="doSaveModelsPath">保存路径</AppButton>
        </div>
      </AppField>
      <p v-if="pathMsg" class="note">{{ pathMsg }}</p>
    </template>

    <template v-else-if="ollamaChecked && !ollama.installed">
      <div class="row">
        <AppButton variant="primary" @click="openOllamaSite">打开 Ollama 官网下载</AppButton>
      </div>
      <p class="muted">
        安装 Ollama 后回到本页点「重新检测」，再一键拉取模型。推荐 qwen2.5:3b（轻量）或 qwen2.5:7b（均衡）。
      </p>
    </template>
  </AppCard>

  <AppCard title="离线语义检索模型（bge）" hint="约 91MB">
    <p class="muted">
      离线模式下让「铃」更聪明地选回复（语义匹配，比关键词更懂你）。模型约 91MB，完整版内置；轻量版可一键检索安装。
    </p>
    <div v-if="vectorModelStatus" class="status-line">
      <AppStatusDot :state="vectorModelStatus.available ? 'ok' : 'warn'" :text="vectorModelStatus.message" />
    </div>
    <div class="row">
      <AppButton :disabled="vectorScanning" @click="scanVectorModels">
        {{ vectorScanning ? '检索中…' : '检索模型' }}
      </AppButton>
    </div>
    <div v-if="vectorCandidates.length" class="candidates">
      <div v-for="c in vectorCandidates" :key="c.path" class="candidate">
        <span class="grow ellipsis">{{ c.filename }}（{{ c.size_mb.toFixed(1) }}MB）</span>
        <AppButton v-if="c.exists_in_target" size="sm" disabled>已安装</AppButton>
        <AppButton v-else size="sm" variant="primary" :disabled="vectorInstalling" @click="doInstallVectorModel(c.path)">
          {{ vectorInstalling ? '安装中…' : '一键安装' }}
        </AppButton>
      </div>
    </div>
    <p v-if="vectorMsg" class="note">{{ vectorMsg }}</p>
  </AppCard>
</template>

<style scoped>
.row {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  min-width: 0;
}
.grow {
  flex: 1;
  min-width: 0;
}
.spacer {
  flex: 1;
}
.ellipsis {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.muted {
  margin: var(--ui-sp-2) 0 0;
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-3);
  overflow-wrap: anywhere;
}
.note {
  margin: var(--ui-sp-2) 0 0;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-left: 2px solid var(--ui-accent-line);
  border-radius: var(--ui-r-sm);
  background: var(--ui-hover);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-2);
  overflow-wrap: anywhere;
}
.status-line {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  margin-bottom: var(--ui-sp-2);
  min-width: 0;
}
.candidates {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
  margin-top: var(--ui-sp-2);
  padding: var(--ui-sp-2);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-md);
  background: var(--ui-hover);
}
.candidate {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
}
</style>
