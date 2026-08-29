<!-- 《铃·记忆体》首次启动引导页（AI-7 任务 11 / 4.2）
     3 步：选择存储路径 → 选择模式 + 设置主密码 → 选择形象风格 -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useSettingStore } from '../stores/settingStore'

const setting = useSettingStore()

const step = ref(0)
const dataPath = ref('')
const modelMode = ref<'script' | 'api' | 'local'>('script')
const apiBaseUrl = ref('')
const apiKey = ref('')
const masterPwd = ref('')
const masterPwd2 = ref('')
const style = ref('daily')
const msg = ref('')
const busy = ref(false)

// 形象风格预设：影响主题 / 语言浓度 / 自称 / 称呼
const STYLES: Record<string, { label: string; emoji: string; theme: 'light' | 'dark'; mix: number; self: string; user: string; desc: string }> = {
  daily: { label: '日常', emoji: '🌸', theme: 'light', mix: 8, self: '铃', user: '主人', desc: '温柔陪伴，日常问候' },
  chuunibyou: { label: '中二', emoji: '⚔️', theme: 'dark', mix: 12, self: '本座·铃', user: '凡人', desc: '「吾乃月城铃华，被选中的主人啊！」' },
  healing: { label: '治愈', emoji: '🫂', theme: 'light', mix: 5, self: '铃', user: '亲爱的', desc: '软软糯糯，治愈人心' },
  lewd: { label: '涩涩', emoji: '🔞', theme: 'dark', mix: 15, self: '铃', user: '主人大人', desc: '调皮撒娇，偶尔撩拨（18+ 内容由你主导）' },
}

const stepTitles = ['选择存储路径', '选择模式 & 设置主密码', '选择形象风格']

onMounted(async () => {
  if (!setting.loaded) await setting.loadConfig()
  dataPath.value = setting.dataPath
  if (!dataPath.value) dataPath.value = ''
})

const canNext = computed(() => {
  if (step.value === 0) return dataPath.value.trim().length > 0
  if (step.value === 1) {
    // API 模式必须填地址；主密码若填需两次一致
    if (modelMode.value === 'api' && !apiBaseUrl.value.trim()) return false
    if (masterPwd.value && masterPwd.value !== masterPwd2.value) return false
    return true
  }
  return true
})

function next() {
  if (!canNext.value) return
  step.value++
}
function prev() {
  step.value = Math.max(0, step.value - 1)
}

async function finish() {
  busy.value = true
  msg.value = ''
  try {
    // 1) 设置主密码（可选，但推荐——用于加密 API Key）
    if (masterPwd.value) {
      if (masterPwd.value !== masterPwd2.value) throw new Error('两次输入的主密码不一致')
      await setting.setupMasterPassword(masterPwd.value)
    }
    // 2) 保存选择
    const updates: Record<string, unknown> = {
      data_path: dataPath.value.trim() || setting.dataPath,
      model_mode: modelMode.value,
      first_launch: false,
      theme: STYLES[style.value].theme,
      language_mix_rate: STYLES[style.value].mix,
      self_name: STYLES[style.value].self,
      user_name: STYLES[style.value].user,
    }
    if (modelMode.value === 'api') {
      updates.api_base_url = apiBaseUrl.value.trim() || null
      if (apiKey.value.trim() && setting.unlocked) updates.api_key = apiKey.value.trim()
    }
    await setting.update(updates)
    await setting.loadConfig()
    // 3) 通知 App 进入主界面
    window.dispatchEvent(new CustomEvent('onboarding-done'))
  } catch (e: unknown) {
    const m = e instanceof Error ? e.message : String(e)
    msg.value = `✗ ${m}`
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="onboard">
    <div class="onboard-card">
      <div class="logo">🐾 铃·记忆体</div>
      <div class="subtitle">你的桌面猫娘伴侣，初次见面～</div>

      <!-- 步骤指示器 -->
      <div class="steps">
        <div v-for="(t, i) in stepTitles" :key="i" class="step" :class="{ active: i === step, done: i < step }">
          <div class="dot">{{ i < step ? '✓' : i + 1 }}</div>
          <div class="stitle">{{ t }}</div>
        </div>
      </div>

      <!-- 步 1：存储路径 -->
      <div v-if="step === 0" class="step-body">
        <p class="hint">记忆与数据将保存在这个文件夹（默认：文档/铃记忆体）</p>
        <input v-model="dataPath" class="input full" placeholder="例如：C:\Users\你\Documents\铃记忆体" />
        <div v-if="dataPath" class="path-preview">📂 {{ dataPath }}</div>
      </div>

      <!-- 步 2：模式 + 主密码 -->
      <div v-else-if="step === 1" class="step-body">
        <p class="hint">选择初始运行模式</p>
        <div class="modes">
          <div class="mode" :class="{ sel: modelMode === 'script' }" @click="modelMode = 'script'">
            <div class="mode-icon">🧠</div>
            <div class="mode-name">脚本模式</div>
            <div class="mode-desc">无需配置，内置回复引擎，即开即用</div>
          </div>
          <div class="mode" :class="{ sel: modelMode === 'api' }" @click="modelMode = 'api'">
            <div class="mode-icon">☁️</div>
            <div class="mode-name">API 模式</div>
            <div class="mode-desc">接入远程大模型（需地址 + 密钥）</div>
          </div>
          <div class="mode" :class="{ sel: modelMode === 'local' }" @click="modelMode = 'local'">
            <div class="mode-icon">💻</div>
            <div class="mode-name">本地模式</div>
            <div class="mode-desc">调用本地模型（Ollama 等）</div>
          </div>
        </div>

        <div v-if="modelMode === 'api'" class="api-fields">
          <input v-model="apiBaseUrl" class="input full" placeholder="API 地址，如 https://api.example.com/v1" />
          <input v-model="apiKey" type="password" class="input full" placeholder="API 密钥（加密存储）" />
        </div>

        <p class="hint" style="margin-top: 14px">设置主密码（用于加密 API Key，重装系统后输入相同密码可恢复）</p>
        <div class="pwd-row">
          <input v-model="masterPwd" type="password" class="input" placeholder="主密码（可选）" />
          <input v-model="masterPwd2" type="password" class="input" placeholder="确认主密码" />
        </div>
        <div v-if="masterPwd && masterPwd !== masterPwd2" class="warn">⚠️ 两次密码不一致</div>
      </div>

      <!-- 步 3：形象 -->
      <div v-else class="step-body">
        <p class="hint">选择铃的初始形象风格</p>
        <div class="styles">
          <div v-for="(s, k) in STYLES" :key="k" class="style" :class="{ sel: style === k }" @click="style = k">
            <div class="style-emoji">{{ s.emoji }}</div>
            <div class="style-name">{{ s.label }}</div>
            <div class="style-desc">{{ s.desc }}</div>
          </div>
        </div>
      </div>

      <div v-if="msg" class="msg">{{ msg }}</div>

      <div class="nav">
        <button v-if="step > 0" class="btn ghost" @click="prev" :disabled="busy">上一步</button>
        <button v-if="step < 2" class="btn primary" @click="next" :disabled="!canNext || busy">下一步</button>
        <button v-else class="btn primary" @click="finish" :disabled="busy">
          {{ busy ? '启动中…' : '开始使用 ✨' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.onboard {
  height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #ffd3e0, #c9e4ff);
  font-family: system-ui, 'Microsoft YaHei', sans-serif;
}
.onboard-card {
  width: 520px;
  max-width: 92vw;
  background: rgba(255, 255, 255, 0.92);
  border-radius: 20px;
  padding: 28px 30px;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.18);
  color: #2b2323;
}
.logo { font-size: 24px; font-weight: 800; color: #d4637f; }
.subtitle { font-size: 13px; color: #8a8082; margin: 4px 0 18px; }
.steps { display: flex; gap: 8px; margin-bottom: 20px; }
.step { display: flex; align-items: center; gap: 6px; flex: 1; }
.dot {
  width: 22px; height: 22px; border-radius: 50%;
  display: flex; align-items: center; justify-content: center;
  background: #eee; color: #aaa; font-size: 12px; font-weight: 700;
}
.step.active .dot { background: #ff7a94; color: #fff; }
.step.done .dot { background: #7bc47f; color: #fff; }
.stitle { font-size: 12px; color: #8a8082; }
.step.active .stitle { color: #d4637f; font-weight: 600; }
.step-body { min-height: 220px; }
.hint { font-size: 13px; color: #8a8082; margin: 0 0 10px; }
.input {
  padding: 9px 12px; border-radius: 9px; border: 1px solid #ddd;
  font-size: 13px; width: 100%; box-sizing: border-box; margin-bottom: 8px;
}
.input.full { width: 100%; }
.pwd-row { display: flex; gap: 8px; }
.path-preview { font-size: 12px; color: #7bc47f; margin-top: 4px; }
.warn { color: #d9534f; font-size: 12px; margin-top: 4px; }
.modes { display: flex; gap: 10px; }
.mode {
  flex: 1; border: 2px solid #eee; border-radius: 12px; padding: 12px;
  cursor: pointer; transition: all 0.15s; text-align: center;
}
.mode.sel { border-color: #ff7a94; background: #fff0f5; }
.mode-icon { font-size: 24px; }
.mode-name { font-weight: 700; margin: 6px 0 4px; }
.mode-desc { font-size: 11px; color: #8a8082; }
.api-fields { margin-top: 12px; }
.styles { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
.style {
  border: 2px solid #eee; border-radius: 12px; padding: 14px; cursor: pointer; text-align: center;
}
.style.sel { border-color: #ff7a94; background: #fff0f5; }
.style-emoji { font-size: 26px; }
.style-name { font-weight: 700; margin: 4px 0; }
.style-desc { font-size: 11px; color: #8a8082; }
.msg { color: #d9534f; font-size: 13px; margin-top: 10px; }
.nav { display: flex; justify-content: space-between; margin-top: 20px; }
.btn {
  padding: 9px 20px; border-radius: 10px; border: none; cursor: pointer; font-size: 14px; font-weight: 600;
}
.btn.primary { background: #ff7a94; color: #fff; }
.btn.ghost { background: #eee; color: #555; }
.btn:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
