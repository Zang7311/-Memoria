<!-- 设置 · 模型
     快速接入向导 / 选择运行方式 / 网络与代理 / 运行模式（模型槽位 · 密钥 · AI 路由 · 测试连接 · 回复速度）
     下半部分另有两个分页组件：LocalAiSettings（Ollama 与离线检索模型）、VaultSettings（主密码与加密） -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import AppButton from '../ui/AppButton.vue'
import AppCard from '../ui/AppCard.vue'
import AppField from '../ui/AppField.vue'
import AppInput from '../ui/AppInput.vue'
import AppSwitch from '../ui/AppSwitch.vue'
import TheIcon from '../TheIcon.vue'
import LocalAiSettings from './LocalAiSettings.vue'
import VaultSettings from './VaultSettings.vue'
import { useSettingStore } from '../../stores/settingStore'
import { toastErr, toastOk } from '../../utils/toast'
import { networkDiagnostic, testApiConnection } from '../../utils/tauri'
import { ensureSettingsLoaded } from './shared'

const setting = useSettingStore()

// ==================== 运行模式 / 模型槽位 ====================
const modelMode = ref<'script' | 'api' | 'local'>('script')
const apiBaseUrl = ref('')
interface ModelDraft {
  id: string
  name: string
  base_url: string
  roles: string[]
  enabled: boolean
  has_api_key?: boolean
  keyInput: string
}
const modelSlots = ref<ModelDraft[]>([])
const savingModels = ref(false)
const aiRouter = ref(false)
const apiKeyInput = ref('')
const depth = ref(2)

// 思考档位人话标签（P1：深度 1-4 → 快速/平衡/深入/认真思考）
const DEPTH_LABEL: Record<number, string> = {
  1: '快速',
  2: '平衡',
  3: '深入',
  4: '认真思考',
}
const ROLE_OPTIONS = [
  { id: 'main', label: '主力' },
  { id: 'cheap', label: '便宜' },
  { id: 'vision', label: '视觉' },
] as const

// 快速选择（P1：人话分层，一键填入推荐配置）
function pickQuick(kind: 'deepseek' | 'openai' | 'local' | 'script') {
  if (kind === 'deepseek') {
    modelMode.value = 'api'
    apiBaseUrl.value = 'https://api.deepseek.com'
    setMainModel('deepseek-chat')
  } else if (kind === 'openai') {
    modelMode.value = 'api'
    if (!apiBaseUrl.value) apiBaseUrl.value = 'https://api.openai.com/v1'
    if (!modelSlots.value.some((slot) => slot.roles.includes('main') && slot.name.trim())) setMainModel('gpt-4o-mini')
  } else if (kind === 'local') {
    modelMode.value = 'local'
  } else {
    modelMode.value = 'script'
  }
  // 选完运行方式后把标记位清掉，避免向导一直停在旧状态
  testOk.value = false
}

const quickCards = computed(() => [
  {
    id: 'deepseek' as const,
    icon: 'network',
    label: 'DeepSeek',
    tag: '推荐',
    desc: '云端智能，性价比高，中文好',
    active: modelMode.value === 'api' && apiBaseUrl.value.includes('deepseek'),
  },
  {
    id: 'local' as const,
    icon: 'monitor',
    label: '本地 AI',
    tag: '',
    desc: '模型跑在自己电脑，离线可用',
    active: modelMode.value === 'local',
  },
  {
    id: 'script' as const,
    icon: 'power',
    label: '离线模式',
    tag: '',
    desc: '零配置，内置回复库，即开即用',
    active: modelMode.value === 'script',
  },
])

// 常见 OpenAI 兼容模型预设（可下拉选择，也可手填自定义）
const MODEL_PRESETS = [
  'deepseek-chat',
  'deepseek-reasoner',
  'qwen-plus',
  'qwen-max',
  'qwen-turbo',
  'gpt-4o-mini',
  'gpt-4o',
  'gpt-3.5-turbo',
  'moonshot-v1-8k',
  'moonshot-v1-32k',
  'glm-4',
  'glm-4-plus',
  'kimi-latest',
]

function addModel() {
  modelSlots.value.push({ id: '', name: '', base_url: '', roles: [], enabled: true, keyInput: '' })
}
function setMainModel(name: string) {
  let main = modelSlots.value.find((slot) => slot.roles.includes('main'))
  if (!main) {
    addModel()
    main = modelSlots.value[modelSlots.value.length - 1]
    main.roles = ['main']
  }
  main.name = name
}
function toggleRole(slot: ModelDraft, role: string) {
  const at = slot.roles.indexOf(role)
  if (at >= 0) slot.roles.splice(at, 1)
  else slot.roles.push(role)
}

async function saveModel(): Promise<boolean> {
  if (modelMode.value === 'api' && !modelSlots.value.some((slot) => slot.enabled && slot.roles.includes('main'))) {
    toastErr('请至少启用一个勾选了「主力」的模型')
    return false
  }
  if (modelSlots.value.some((slot) => !slot.name.trim())) {
    toastErr('请填写每个模型的名称')
    return false
  }
  savingModels.value = true
  try {
    await setting.update({
      model_mode: modelMode.value,
      api_base_url: apiBaseUrl.value.trim() || null,
      models: modelSlots.value.map((slot) => ({
        id: slot.id,
        name: slot.name.trim(),
        base_url: slot.base_url.trim() || null,
        roles: [...slot.roles],
        enabled: slot.enabled,
      })),
      ai_router: aiRouter.value,
      depth: depth.value,
    })
    modelSlots.value.forEach((slot, index) => {
      slot.id = setting.models[index].id
      slot.has_api_key = setting.models[index].has_api_key
    })
    toastOk('模型设置已保存')
    return true
  } catch {
    toastErr('模型设置保存失败，请检查配置')
    return false
  } finally {
    savingModels.value = false
  }
}
async function saveSlotKey(index: number) {
  const slot = modelSlots.value[index]
  const plain = slot.keyInput
  if (!slot.id && !(await saveModel())) return
  savingModels.value = true
  try {
    await setting.saveSlotKey(slot.id, plain)
    slot.keyInput = ''
    slot.has_api_key = setting.models.find((saved) => saved.id === slot.id)?.has_api_key ?? false
    toastOk(plain ? '模型密钥已保存' : '模型密钥已清除，将沿用全局密钥')
  } catch {
    toastErr('模型密钥保存失败，请检查主密码是否已解锁')
  } finally {
    savingModels.value = false
  }
}
async function saveApiKey() {
  try {
    await setting.saveApiKey(apiKeyInput.value)
    apiKeyInput.value = ''
    toastOk(setting.hasMasterPassword ? '全局密钥已加密保存' : '全局密钥已保存（明文，建议设置主密码）')
  } catch {
    toastErr('全局密钥保存失败，请检查主密码是否已解锁')
  }
}

// ==================== 网络与代理 ====================
const proxyEnabled = ref(false)
const proxyUrl = ref('')
const networkMsg = ref('')
const savingNetwork = ref(false)
const diagnosingNetwork = ref(false)

async function saveNetwork(): Promise<boolean> {
  savingNetwork.value = true
  try {
    await setting.update({ proxy_enabled: proxyEnabled.value, proxy_url: proxyUrl.value.trim() || null })
    networkMsg.value = '网络设置已保存'
    return true
  } catch {
    networkMsg.value = '网络设置保存失败，请重试'
    return false
  } finally {
    savingNetwork.value = false
  }
}

async function diagnoseNetwork() {
  diagnosingNetwork.value = true
  try {
    if (!(await saveNetwork())) return
    networkMsg.value = '正在诊断网络…'
    networkMsg.value = await networkDiagnostic()
  } catch {
    networkMsg.value = '网络诊断失败，请检查网络设置后重试'
  } finally {
    diagnosingNetwork.value = false
  }
}

// ==================== 测试连接 ====================
const testMsg = ref('')
const testing = ref(false)
/** 第三步「测试连接」是否已完成（后端返回 success 才点亮） */
const testOk = ref(false)

async function testConnection() {
  const main = modelSlots.value.find((slot) => slot.enabled && slot.roles.includes('main'))
  const baseUrl = main?.base_url.trim() || apiBaseUrl.value.trim()
  if (!baseUrl) {
    testMsg.value = '请先填写模型地址或全局地址'
    return
  }
  testing.value = true
  testMsg.value = '正在测试连接…'
  try {
    const networkChanged =
      proxyEnabled.value !== (setting.proxyEnabled ?? false) ||
      (proxyUrl.value.trim() || null) !== (setting.proxyUrl ?? null)
    if (networkChanged && !(await saveNetwork())) {
      testMsg.value = '网络设置保存失败，请重试'
      return
    }
    const keyInput = main?.keyInput.trim() || (main?.has_api_key ? '' : apiKeyInput.value.trim())
    const res = await testApiConnection(baseUrl, keyInput, main?.id || undefined)
    testMsg.value = res.message
    testOk.value = res.success
  } catch {
    testMsg.value = '测试失败，请检查地址与主密码解锁状态'
    testOk.value = false
  } finally {
    testing.value = false
  }
}

function syncFromStore() {
  modelMode.value = setting.modelMode
  apiBaseUrl.value = setting.apiBaseUrl ?? ''
  proxyEnabled.value = setting.proxyEnabled ?? false
  proxyUrl.value = setting.proxyUrl ?? ''
  modelSlots.value = setting.models.map((slot) => ({
    id: slot.id,
    name: slot.name,
    base_url: slot.base_url ?? '',
    roles: [...slot.roles],
    enabled: slot.enabled,
    has_api_key: slot.has_api_key,
    keyInput: '',
  }))
  aiRouter.value = setting.aiRouter
  depth.value = setting.depth
}

onMounted(async () => {
  await ensureSettingsLoaded()
  syncFromStore()
})
</script>

<template>
  <div class="page">
    <!-- 快速接入向导（3 步） -->
    <AppCard title="快速接入向导">
      <div class="steps">
        <span class="step" :class="{ on: modelMode !== 'script' }">1 选运行模式</span>
        <span class="step-arrow">→</span>
        <span class="step" :class="{ on: modelMode === 'api' && !!apiBaseUrl }">2 填地址与模型</span>
        <span class="step-arrow">→</span>
        <span class="step" :class="{ on: testOk }">3 测试连接</span>
      </div>
      <p class="muted">
        {{
          modelMode === 'script'
            ? '当前：脚本模式（开箱即用，预设回复库，无需联网）'
            : modelMode === 'api'
              ? '当前：API 模式（接入 OpenAI 兼容大模型，如 DeepSeek / Qwen / GPT）'
              : '当前：本地模式（需自行安装 Ollama 并下载模型）'
        }}
      </p>
    </AppCard>

    <!-- 快速选择（P1：人话分层，点卡片自动填推荐配置） -->
    <AppCard title="选择铃的运行方式">
      <div class="qm-grid">
        <button
          v-for="card in quickCards"
          :key="card.id"
          type="button"
          class="qm"
          :class="{ on: card.active }"
          @click="pickQuick(card.id)"
        >
          <TheIcon :name="card.icon" :size="18" class="qm-icon" />
          <span class="qm-name">
            {{ card.label }}
            <span v-if="card.tag" class="qm-tag">{{ card.tag }}</span>
          </span>
          <span class="qm-desc">{{ card.desc }}</span>
        </button>
      </div>
      <p class="muted">点击卡片会自动切换模式并填入推荐配置；下方「运行模式」可手调高级参数。</p>
      <p class="muted">
        OpenAI 兼容：支持任意 OpenAI 兼容服务（中转站 / Qwen / 混元等）。在下方「运行模式」填 API 地址即可；DeepSeek
        卡片已填好官方示例，无需联网的海外 OpenAI 官方（需翻墙 + 付费 key）不做默认推荐。
      </p>
    </AppCard>

    <!-- 网络 -->
    <AppCard title="网络">
      <fieldset class="flat" :disabled="savingNetwork || diagnosingNetwork || testing">
        <AppSwitch
          v-model="proxyEnabled"
          label="使用代理"
          hint="默认关闭：国内 API 直连；要用国外 API 时才需要开。"
        />
        <AppField
          label="代理地址"
          hint="Clash / FlClash 之类的本地代理一般就是 http://127.0.0.1:7890。默认直连，完全忽略系统代理。"
        >
          <AppInput
            v-model="proxyUrl"
            :disabled="!proxyEnabled"
            placeholder="http://127.0.0.1:7890"
            autocomplete="off"
          />
        </AppField>
        <div class="row">
          <AppButton variant="primary" :disabled="savingNetwork" @click="saveNetwork">保存网络设置</AppButton>
          <AppButton :disabled="diagnosingNetwork" @click="diagnoseNetwork">网络诊断</AppButton>
        </div>
      </fieldset>
      <p v-if="networkMsg" class="note">{{ networkMsg }}</p>
    </AppCard>

    <!-- 运行模式（高级参数） -->
    <AppCard title="运行模式" hint="高级参数">
      <div class="row">
        <button
          type="button"
          class="chip lg"
          :class="{ on: modelMode === 'script' }"
          @click="modelMode = 'script'"
        >
          离线
        </button>
        <button type="button" class="chip lg" :class="{ on: modelMode === 'api' }" @click="modelMode = 'api'">
          云端
        </button>
        <button type="button" class="chip lg" :class="{ on: modelMode === 'local' }" @click="modelMode = 'local'">
          本地 AI
        </button>
      </div>

      <template v-if="modelMode === 'api'">
        <div class="block-head">
          <span class="block-title">模型列表</span>
          <AppButton size="sm" :disabled="savingModels" @click="addModel">添加模型</AppButton>
        </div>
        <p v-if="!modelSlots.length" class="muted">还没有模型，点「添加模型」加一个。</p>
        <fieldset v-for="(slot, index) in modelSlots" :key="slot.id || index" class="slot" :disabled="savingModels">
          <div class="block-head">
            <span class="block-title">模型 {{ index + 1 }}</span>
            <AppButton size="sm" :disabled="savingModels" @click="modelSlots.splice(index, 1)">删除</AppButton>
          </div>
          <AppField label="模型名">
            <AppInput v-model="slot.name" list="model-presets" placeholder="模型名（可从预设里选，也可手填）" />
          </AppField>
          <AppField label="用途">
            <div class="row wrap">
              <button
                v-for="role in ROLE_OPTIONS"
                :key="role.id"
                type="button"
                class="chip"
                :class="{ on: slot.roles.includes(role.id) }"
                :aria-pressed="slot.roles.includes(role.id)"
                @click="toggleRole(slot, role.id)"
              >
                {{ role.label }}
              </button>
              <span class="spacer" />
              <AppSwitch v-model="slot.enabled" label="启用" />
            </div>
          </AppField>
          <AppField label="地址" hint="留空 = 用下面的全局地址">
            <AppInput v-model="slot.base_url" placeholder="https://api.deepseek.com" />
          </AppField>
          <AppField
            label="密钥"
            :hint="slot.has_api_key ? '已保存独立密钥（不回显）；留空保存可清除' : '未保存独立密钥，沿用全局密钥'"
          >
            <div class="row">
              <AppInput
                v-model="slot.keyInput"
                type="password"
                autocomplete="new-password"
                placeholder="留空 = 沿用全局密钥"
              />
              <AppButton variant="primary" :disabled="savingModels" @click="saveSlotKey(index)">保存密钥</AppButton>
            </div>
          </AppField>
        </fieldset>
        <datalist id="model-presets">
          <option v-for="model in MODEL_PRESETS" :key="model" :value="model" />
        </datalist>

        <AppField label="全局地址" hint="OpenAI 兼容，带 / 不带 /v1 均可">
          <AppInput v-model="apiBaseUrl" placeholder="https://api.deepseek.com" />
        </AppField>
        <AppField
          label="全局密钥"
          :hint="`${setting.hasApiKey ? '已保存全局密钥' : '未保存全局密钥'}；主密码：${
            setting.hasMasterPassword
              ? setting.unlocked
                ? '已解锁'
                : '未解锁，请先解锁再保存密钥'
              : '未设置，密钥将明文存储'
          }`"
        >
          <div class="row">
            <AppInput
              v-model="apiKeyInput"
              type="password"
              autocomplete="new-password"
              placeholder="输入新密钥（不回显）"
            />
            <AppButton variant="primary" @click="saveApiKey">保存</AppButton>
          </div>
        </AppField>
        <AppSwitch
          v-model="aiRouter"
          label="用 AI 判断难度"
          hint="需要主力与便宜槽位的 id、模型名均不同且类型相同；便宜槽位轮询使用自己的地址和密钥。失败时回退本地判断。"
        />
        <div class="row">
          <AppButton :disabled="testing" @click="testConnection">{{ testing ? '测试中…' : '测试连接' }}</AppButton>
        </div>
        <p v-if="testMsg" class="note">{{ testMsg }}</p>
      </template>

      <template v-if="modelMode !== 'script'">
        <AppField label="回复速度" hint="只影响铃思考的深入程度，不改模型本身。">
          <input v-model.number="depth" class="range" type="range" min="1" max="4" step="1" />
          <div class="depth-labels">
            <span>快速</span><span>平衡</span><span>深入</span><span>认真思考</span>
          </div>
          <p class="status-line">当前：{{ DEPTH_LABEL[depth] || depth }}</p>
        </AppField>
        <div class="row">
          <AppButton variant="primary" :disabled="savingModels" @click="saveModel">保存模型设置</AppButton>
        </div>
      </template>
    </AppCard>

    <!-- 一键本地部署 AI + 离线语义检索模型 -->
    <LocalAiSettings />

    <!-- 主密码与加密 -->
    <VaultSettings />
  </div>
</template>

<style scoped>
.page {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-3);
  padding-bottom: var(--ui-sp-4);
}
.row {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  min-width: 0;
}
.row.wrap {
  flex-wrap: wrap;
}
.spacer {
  flex: 1;
}
.muted {
  margin: var(--ui-sp-2) 0 0;
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-3);
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
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.status-line {
  margin: 0;
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-2);
}
/* 用 fieldset 保留「保存中就整块禁用」的老行为，但不要浏览器默认边框 */
.flat,
.slot {
  min-width: 0;
  margin: 0;
  padding: 0;
  border: 0;
}
.slot {
  margin-top: var(--ui-sp-3);
  padding: var(--ui-sp-3);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-md);
  background: var(--ui-hover);
}
.flat:disabled,
.slot:disabled {
  opacity: 0.6;
}
/* —— 步骤条 —— */
.steps {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  margin-bottom: var(--ui-sp-2);
}
.step {
  padding: var(--ui-sp-1) var(--ui-sp-3);
  border-radius: var(--ui-r-pill);
  background: var(--ui-hover);
  color: var(--ui-text-3);
  font-size: var(--ui-fs-11);
}
.step.on {
  background: var(--ui-accent-soft);
  color: var(--ui-accent);
}
.step-arrow {
  color: var(--ui-text-3);
  font-size: var(--ui-fs-11);
}
/* —— 运行方式卡片 —— */
.qm-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--ui-sp-2);
}
.qm {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--ui-sp-1);
  padding: var(--ui-sp-3);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-md);
  background: var(--ui-raised);
  color: var(--ui-text);
  text-align: left;
  transition: border-color var(--ui-dur-1) var(--ui-ease), background var(--ui-dur-1) var(--ui-ease);
}
.qm:hover {
  border-color: var(--ui-border-strong);
  background: var(--ui-hover);
}
.qm.on {
  border-color: var(--ui-accent-line);
  background: var(--ui-accent-soft);
}
.qm-icon {
  color: var(--ui-accent);
}
.qm-name {
  display: inline-flex;
  align-items: center;
  gap: var(--ui-sp-1);
  font-size: var(--ui-fs-13);
  font-weight: 600;
}
.qm-tag {
  padding: 0 var(--ui-sp-1);
  border-radius: var(--ui-r-pill);
  background: var(--ui-accent);
  color: var(--ui-bg);
  font-size: var(--ui-fs-11);
  font-weight: 600;
  line-height: 16px;
}
.qm-desc {
  font-size: var(--ui-fs-11);
  line-height: var(--ui-line);
  color: var(--ui-text-3);
}
/* —— 块标题 —— */
.block-head {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  margin: var(--ui-sp-3) 0 var(--ui-sp-1);
}
.block-title {
  flex: 1;
  min-width: 0;
  font-size: var(--ui-fs-13);
  font-weight: 600;
  color: var(--ui-text);
}
/* —— 选择胶囊（单选/多选）—— */
.chip {
  height: 26px;
  padding: 0 var(--ui-sp-3);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-pill);
  background: transparent;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-12);
  transition: background var(--ui-dur-1) var(--ui-ease), color var(--ui-dur-1) var(--ui-ease),
    border-color var(--ui-dur-1) var(--ui-ease);
}
.chip:hover {
  background: var(--ui-hover);
  color: var(--ui-text);
}
.chip.on {
  border-color: var(--ui-accent-line);
  background: var(--ui-accent-soft);
  color: var(--ui-text);
}
.chip.lg {
  height: var(--ui-ctl-h);
  font-size: var(--ui-fs-13);
}
.range {
  width: 100%;
  accent-color: var(--ui-accent);
}
.depth-labels {
  display: flex;
  justify-content: space-between;
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
}
</style>
