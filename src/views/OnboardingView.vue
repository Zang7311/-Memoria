<!-- 《铃·记忆体》首次启动引导页（AI-7 任务 11 / 4.2）
     3 步：选择存储路径 → 选择模式 + 设置主密码 → 选择形象风格 -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useSettingStore } from '../stores/settingStore'
import TheIcon from '../components/TheIcon.vue'

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
// 折叠展开的模式（点击下划线人话展开技术说明）
const modeExpanded = ref<string | null>(null)
// 点人话展开技术说明（不切换模式）
function toggleModeExpand(key: string) {
  modeExpanded.value = modeExpanded.value === key ? null : key
}

// 形象风格预设：影响主题 / 语言浓度 / 自称 / 称呼
const STYLES: Record<string, { label: string; icon: string; theme: 'light' | 'dark'; mix: number; self: string; user: string; desc: string }> = {
  daily: { label: '日常', icon: 'chat', theme: 'light', mix: 8, self: '铃', user: '主人', desc: '温柔陪伴，日常问候' },
  chuunibyou: { label: '中二', icon: 'command', theme: 'dark', mix: 12, self: '本座·铃', user: '凡人', desc: '「吾乃月城铃华，被选中的主人啊！」' },
  healing: { label: '治愈', icon: 'sponsor', theme: 'light', mix: 5, self: '铃', user: '亲爱的', desc: '软软糯糯，治愈人心' },
  lewd: { label: '涩涩', icon: 'lock', theme: 'dark', mix: 15, self: '铃', user: '主人大人', desc: '调皮撒娇，偶尔撩拨（18+ 内容由你主导）' },
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
      persona: style.value,
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
    msg.value = String(m)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="onboard">
    <div class="onboard-card">
      <div class="logo">铃·记忆体</div>
      <div class="slogan">我，与你交谈，为你存忆</div>
      <div class="subtitle">你的桌面猫娘伴侣，初次见面～</div>

      <!-- 步骤指示器 -->
      <div class="steps">
        <div v-for="(t, i) in stepTitles" :key="i" class="step" :class="{ active: i === step, done: i < step }">
          <div class="dot">{{ i < step ? '' : i + 1 }}</div>
          <div class="stitle">{{ t }}</div>
        </div>
      </div>

      <!-- 步 1：存储路径 -->
      <div v-if="step === 0" class="step-body">
        <p class="hint">记忆与数据将保存在这个文件夹（默认：文档/铃记忆体）</p>
        <input v-model="dataPath" class="input full" placeholder="例如：C:\Users\你\Documents\铃记忆体" />
        <div v-if="dataPath" class="path-preview"><TheIcon name="file" :size="12" /> {{ dataPath }}</div>
      </div>

      <!-- 步 2：模式 + 主密码 -->
      <div v-else-if="step === 1" class="step-body">
        <p class="hint">选择铃的运行方式（之后随时可在设置里更改）</p>
        <div class="modes">
          <div class="mode" :class="{ sel: modelMode === 'api' }" @click="modelMode = 'api'">
            <div class="mode-icon"><TheIcon name="network" :size="24" /></div>
            <div class="mode-name">云端AI<span class="mode-tag">推荐</span></div>
            <div class="mode-tech">API 模式 · 需地址 + 密钥 · 联网</div>
            <div class="mode-human" :class="{ open: modeExpanded === 'api' }" @click.stop="toggleModeExpand('api')">
              <span v-if="modeExpanded !== 'api'" class="human-entry">人话 ▾</span>
              <span v-else>像请了一位云端大脑，回复更聪明～</span>
            </div>
          </div>
          <div class="mode" :class="{ sel: modelMode === 'local' }" @click="modelMode = 'local'">
            <div class="mode-icon"><TheIcon name="monitor" :size="24" /></div>
            <div class="mode-name">本地AI<span class="mode-tag">高级</span></div>
            <div class="mode-tech">Ollama 本地模型 · 需先安装 · 离线可用</div>
            <div class="mode-human" :class="{ open: modeExpanded === 'local' }" @click.stop="toggleModeExpand('local')">
              <span v-if="modeExpanded !== 'local'" class="human-entry">人话 ▾</span>
              <span v-else>模型跑在你自己电脑上，不吃网络～</span>
            </div>
          </div>
          <div class="mode" :class="{ sel: modelMode === 'script' }" @click="modelMode = 'script'">
            <div class="mode-icon"><TheIcon name="chat" :size="24" /></div>
            <div class="mode-name">离线模式</div>
            <div class="mode-tech">内置回复引擎 · 无需配置 · 基础聊天</div>
            <div class="mode-human" :class="{ open: modeExpanded === 'script' }" @click.stop="toggleModeExpand('script')">
              <span v-if="modeExpanded !== 'script'" class="human-entry">人话 ▾</span>
              <span v-else>不用联网不用配置，即开即用～</span>
            </div>
          </div>
        </div>
        <div class="mode-legend">
          点卡片上的 <span class="mode-legend-hint">下划线人话</span> 可展开人话解释
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
        <div v-if="masterPwd && masterPwd !== masterPwd2" class="warn">两次密码不一致</div>
      </div>

      <!-- 步 3：形象 -->
      <div v-else class="step-body">
        <p class="hint">选择铃的初始形象风格</p>
        <div class="styles">
          <div v-for="(s, k) in STYLES" :key="k" class="style" :class="{ sel: style === k }" @click="style = k">
            <div class="style-icon"><TheIcon :name="s.icon" :size="26" /></div>
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
          {{ busy ? '启动中…' : '开始使用' }}
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
  padding: var(--ui-sp-4);
  background: linear-gradient(135deg, var(--ui-accent-soft), var(--ui-panel));
  color: var(--ui-text);
}
.onboard-card {
  width: min(520px, 100%);
  max-height: 92vh;
  overflow-y: auto;
  padding: var(--ui-sp-5) var(--ui-sp-6);
  background: var(--ui-raised);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-lg);
  box-shadow: var(--ui-shadow);
  color: var(--ui-text);
}
.logo { font-size: var(--ui-fs-20); font-weight: 800; color: var(--ui-accent); }
.slogan { margin-top: var(--ui-sp-1); font-size: var(--ui-fs-13); color: var(--ui-text-2); letter-spacing: 1px; }
.subtitle { font-size: var(--ui-fs-13); color: var(--ui-text-2); margin: var(--ui-sp-1) 0 var(--ui-sp-4); }
.steps { display: flex; gap: var(--ui-sp-2); margin-bottom: var(--ui-sp-5); flex-wrap: wrap; }
.step { display: flex; align-items: center; gap: var(--ui-sp-1); flex: 1; min-width: 0; }
.dot {
  width: 22px; height: 22px; flex: none; border-radius: var(--ui-r-pill);
  display: flex; align-items: center; justify-content: center;
  background: var(--ui-active); color: var(--ui-text-3);
  font-size: var(--ui-fs-12); font-weight: 700;
}
.step.active .dot { background: var(--ui-accent); color: var(--ui-on-accent); }
.step.done .dot { background: var(--ui-ok); color: var(--ui-on-accent); }
.stitle { font-size: var(--ui-fs-12); color: var(--ui-text-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.step.active .stitle { color: var(--ui-accent); font-weight: 600; }
.step-body { min-height: 220px; }
.hint { font-size: var(--ui-fs-13); color: var(--ui-text-2); margin: 0 0 var(--ui-sp-2); line-height: var(--ui-line); }
.input {
  width: 100%; height: var(--ui-ctl-h); padding: 0 var(--ui-sp-3); margin-bottom: var(--ui-sp-2);
  border: 1px solid var(--ui-border-strong); border-radius: var(--ui-r-sm);
  background: var(--ui-raised); color: var(--ui-text); font-size: var(--ui-fs-13);
}
.input:focus { border-color: var(--ui-accent); outline: none; }
.pwd-row { display: flex; gap: var(--ui-sp-2); flex-wrap: wrap; }
.path-preview { font-size: var(--ui-fs-12); color: var(--ui-ok); margin-top: var(--ui-sp-1); overflow-wrap: anywhere; }
.warn { color: var(--ui-err); font-size: var(--ui-fs-12); margin-top: var(--ui-sp-1); }
.modes { display: grid; grid-template-columns: repeat(auto-fit, minmax(170px, 1fr)); gap: var(--ui-sp-2); }
.mode {
  padding: var(--ui-sp-3);
  border: 2px solid var(--ui-border); border-radius: var(--ui-r-md);
  background: var(--ui-panel); cursor: pointer; text-align: center;
  transition: border-color var(--ui-dur-1) var(--ui-ease), background var(--ui-dur-1) var(--ui-ease);
}
.mode.sel { border-color: var(--ui-accent); background: var(--ui-accent-soft); }
.mode-icon { color: var(--ui-text-2); display: flex; justify-content: center; }
.mode-name { font-weight: 700; margin: var(--ui-sp-1) 0; color: var(--ui-text); }
.mode-tag {
  display: inline-block; margin-left: var(--ui-sp-1); padding: 0 var(--ui-sp-1);
  border-radius: var(--ui-r-pill); font-size: var(--ui-fs-11); font-weight: 600;
  background: var(--ui-accent); color: var(--ui-on-accent); vertical-align: 2px;
}
.mode-tech { margin: 2px auto var(--ui-sp-1); font-size: var(--ui-fs-11); color: var(--ui-text-3); line-height: var(--ui-line); max-width: 90%; }
.mode-human { font-size: var(--ui-fs-11); color: var(--ui-text-3); cursor: help; display: block; }
.human-entry { text-decoration: underline dotted; text-underline-offset: 3px; opacity: 0.9; }
.mode-human.open { color: var(--ui-accent); }
.mode-legend { font-size: var(--ui-fs-11); color: var(--ui-text-3); margin-top: var(--ui-sp-1); text-align: center; }
.mode-legend-hint { text-decoration: underline dotted; text-underline-offset: 2px; }
.api-fields { margin-top: var(--ui-sp-3); }
.styles { display: grid; grid-template-columns: 1fr 1fr; gap: var(--ui-sp-2); }
.style {
  padding: var(--ui-sp-3); border: 2px solid var(--ui-border); border-radius: var(--ui-r-md);
  background: var(--ui-panel); cursor: pointer; text-align: center;
  transition: border-color var(--ui-dur-1) var(--ui-ease), background var(--ui-dur-1) var(--ui-ease);
}
.style.sel { border-color: var(--ui-accent); background: var(--ui-accent-soft); }
.style-icon { color: var(--ui-accent); display: flex; justify-content: center; }
.style-name { font-weight: 700; margin: var(--ui-sp-1) 0; color: var(--ui-text); }
.style-desc { font-size: var(--ui-fs-11); color: var(--ui-text-3); line-height: var(--ui-line); }
.msg { color: var(--ui-err); font-size: var(--ui-fs-13); margin-top: var(--ui-sp-2); overflow-wrap: anywhere; }
.nav { display: flex; justify-content: space-between; gap: var(--ui-sp-2); margin-top: var(--ui-sp-5); flex-wrap: wrap; }
.btn {
  height: var(--ui-ctl-h); padding: 0 var(--ui-sp-4);
  border: 1px solid var(--ui-border); border-radius: var(--ui-r-sm);
  cursor: pointer; font-size: var(--ui-fs-14); font-weight: 600;
  background: var(--ui-panel); color: var(--ui-text);
}
.btn.primary { background: var(--ui-accent); border-color: var(--ui-accent); color: var(--ui-on-accent); }
.btn.ghost { background: var(--ui-hover); color: var(--ui-text); }
.btn:disabled { opacity: 0.5; cursor: not-allowed; }
</style>