<!-- 设置 · 形象
     VBIL 虚拟形象（既有组件整页嵌入） / 个性化 / 桌宠（悬浮球） / 桌面伙伴（原生桌宠） / 外观与主题组合 -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import AppButton from '../ui/AppButton.vue'
import AppCard from '../ui/AppCard.vue'
import AppField from '../ui/AppField.vue'
import AppInput from '../ui/AppInput.vue'
import AppStatusDot from '../ui/AppStatusDot.vue'
import AppSwitch from '../ui/AppSwitch.vue'
import AppTabs from '../ui/AppTabs.vue'
import VBILSettings from '../../views/VBILSettings.vue'
import { useMilestoneStore } from '../../stores/milestoneStore'
import { useSettingStore } from '../../stores/settingStore'
import { toastErr, toastOk } from '../../utils/toast'
import { nativePetRestart, nativePetStatus, nativePetSync, saveUiImage, type NativePetStatus } from '../../utils/tauri'
import { ensureSettingsLoaded } from './shared'

const setting = useSettingStore()
const milestone = useMilestoneStore()

// ==================== 个性化 ====================
const mixRate = ref(8)
const selfName = ref('铃')
const userName = ref('主人')

/** 形象人格：立即保存（原来在顶栏的下拉框，挪到这里） */
const PERSONA_OPTIONS = [
  { id: 'daily', label: '日常' },
  { id: 'chuunibyou', label: '中二' },
  { id: 'healing', label: '治愈' },
  { id: 'lewd', label: '涩涩' },
]
async function pickPersona(id: string) {
  await setting.update({ persona: id }).catch((e) => toastErr(`切换人格失败：${e}`))
}

async function savePersona() {
  await setting.update({
    language_mix_rate: mixRate.value,
    self_name: selfName.value,
    user_name: userName.value,
  })
  toastOk('个性化设置已保存')
}

// ==================== 桌宠（悬浮球） ====================
async function saveBallSettings() {
  await setting.update({
    floating_ball_mode: setting.floatingBallMode,
    floating_ball_size: setting.floatingBallSize,
    floating_ball_opacity: setting.floatingBallOpacity,
    floating_ball_breathing: setting.floatingBallBreathing,
    floating_ball_flash: setting.floatingBallFlash,
    floating_ball_chatter: setting.floatingBallChatter,
  })
  toastOk('悬浮球设置已保存')
}
/** 开关类改动：先写回 store 再整体保存（顺序固定，避免读到旧值） */
async function setBallFlag(
  key: 'floatingBallEnabled' | 'floatingBallBreathing' | 'floatingBallFlash' | 'floatingBallChatter',
  value: boolean,
) {
  setting[key] = value
  await saveBallSettings()
}
async function resetBallPosition() {
  await setting.update({ floating_ball_position: [0, 0] })
  toastOk('位置已重置（下次启动生效）')
}

const BALL_MODES = [
  { id: 'avatar', label: '头像（推荐）' },
  { id: 'simple', label: '纯文字' },
  { id: 'live2d', label: 'Live2D（实验）' },
]
function onBallMode(id: string) {
  setting.floatingBallMode = id as 'avatar' | 'simple' | 'live2d'
  void saveBallSettings()
}
const SIZE_ITEMS = [80, 100, 120, 160, 200].map((n) => ({ id: String(n), label: String(n) }))
function onBallSize(id: string) {
  setting.floatingBallSize = Number(id)
  void saveBallSettings()
}
const OPACITY_ITEMS = [1, 0.8, 0.6, 0.4].map((o) => ({ id: String(o), label: `${o * 100}%` }))
const opacityValue = computed(() => {
  const hit = OPACITY_ITEMS.find((i) => Math.abs(setting.floatingBallOpacity - Number(i.id)) < 0.01)
  return hit ? hit.id : String(setting.floatingBallOpacity)
})
function onBallOpacity(id: string) {
  setting.floatingBallOpacity = Number(id)
  void saveBallSettings()
}

// ==================== 桌面伙伴（原生桌宠） ====================
const nativePet = ref<NativePetStatus | null>(null)
const nativePetMsg = ref('')

async function refreshNativePet() {
  try {
    nativePet.value = await nativePetStatus()
  } catch {
    nativePet.value = null
  }
}
async function onNativePet(value: boolean) {
  setting.nativePetEnabled = value
  await saveNativePet()
}
async function saveNativePet() {
  await setting.update({ native_pet_enabled: setting.nativePetEnabled })
  try {
    nativePet.value = await nativePetSync()
    nativePetMsg.value = nativePet.value?.message ?? ''
  } catch (e) {
    nativePetMsg.value = `操作失败：${e}`
  }
}
async function restartNativePet() {
  try {
    nativePet.value = await nativePetRestart()
    nativePetMsg.value = nativePet.value?.message ?? ''
  } catch (e) {
    nativePetMsg.value = `重启失败：${e}`
  }
}

// ==================== 外观 ====================
type ThemeKey = 'light' | 'dark' | 'win10' | 'edge' | 'minimal' | 'ios-flat' | 'ios-glass'
const THEME_ITEMS = [
  { id: 'dark', label: '深色' },
  { id: 'light', label: '亮色' },
]
const STYLE_ITEMS = [
  { id: 'win10', label: 'Win10' },
  { id: 'edge', label: '微软浏览器' },
  { id: 'minimal', label: '极简文字' },
  { id: 'ios-flat', label: 'iOS 扁平' },
  { id: 'ios-glass', label: 'iOS 毛玻璃' },
]
const EMOJI_ITEMS = [
  { id: 'off', label: '关闭' },
  { id: 'partial', label: '局部' },
  { id: 'all', label: '全部' },
]

function onTheme(id: string) {
  void setting.setTheme(id as ThemeKey)
}
async function setEmojiMode(m: string) {
  const mode = m as 'off' | 'partial' | 'all'
  setting.emojiMode = mode
  await setting.update({ emoji_mode: mode })
  toastOk(`Emoji 显示已设为：${mode === 'off' ? '关闭' : mode === 'partial' ? '局部' : '全部'}`)
}

// 自定义颜色：后端存的是可空字符串，输入框要的是字符串，这里统一转一层
function colorModel(key: 'accentColor' | 'dangerColor' | 'bgColor' | 'bubbleUserColor' | 'bubbleSuzuColor') {
  return computed({
    get: () => setting[key] ?? '',
    set: (v: string) => {
      setting[key] = v
    },
  })
}
const accentColor = colorModel('accentColor')
const dangerColor = colorModel('dangerColor')
const bgColor = colorModel('bgColor')
const bubbleUserColor = colorModel('bubbleUserColor')
const bubbleSuzuColor = colorModel('bubbleSuzuColor')
const bgImage = computed({
  get: () => setting.bgImage ?? '',
  set: (v: string) => {
    setting.bgImage = v
  },
})
const avatarSuzu = computed({
  get: () => setting.avatarSuzu ?? '',
  set: (v: string) => {
    setting.avatarSuzu = v
  },
})
const avatarUser = computed({
  get: () => setting.avatarUser ?? '',
  set: (v: string) => {
    setting.avatarUser = v
  },
})

async function saveUiCustom() {
  await setting.update({
    accent_color: setting.accentColor || null,
    danger_color: setting.dangerColor || null,
    bg_color: setting.bgColor || null,
    bg_image: setting.bgImage || null,
    bubble_user_color: setting.bubbleUserColor || null,
    bubble_suzu_color: setting.bubbleSuzuColor || null,
    avatar_suzu: setting.avatarSuzu || null,
    avatar_user: setting.avatarUser || null,
    ui_radius: setting.uiRadius ?? null,
  })
  toastOk('外观自定义已保存')
  // P3：第一次自定义外观里程碑（幂等）
  milestone.record('first_custom', '第一次自定义外观').catch(() => {})
}

// —— 自定义主题组合 ——
const presetName = ref('')
async function savePreset() {
  const name = presetName.value.trim()
  if (!name) {
    toastErr('请先给这套风格起个名字')
    return
  }
  await setting.saveThemePreset(name)
  toastOk(`已保存主题组合「${name}」，可一键切换`)
  presetName.value = ''
}

// —— 软件内选择图片（背景图 / 头像）——
const bgFileInput = ref<HTMLInputElement | null>(null)
const avatarFileInput = ref<HTMLInputElement | null>(null)
const userAvatarFileInput = ref<HTMLInputElement | null>(null)
function fileToDataUrl(f: File): Promise<string> {
  return new Promise((res, rej) => {
    const r = new FileReader()
    r.onload = () => res(r.result as string)
    r.onerror = () => rej(new Error('读取图片失败'))
    r.readAsDataURL(f)
  })
}
function pickImage(kind: 'bg' | 'avatar' | 'user') {
  const el = kind === 'bg' ? bgFileInput.value : kind === 'avatar' ? avatarFileInput.value : userAvatarFileInput.value
  el?.click()
}
async function onBgFile(e: Event) {
  const input = e.target as HTMLInputElement
  const f = input.files?.[0]
  if (!f) return
  try {
    const path = await saveUiImage(await fileToDataUrl(f), 'bg')
    setting.bgImage = path
    toastOk('背景图已选择，点击「应用自定义外观」生效')
  } catch (err) {
    toastErr(`选择失败：${err}`)
  } finally {
    input.value = '' // 重置，允许下次选同一文件也触发
  }
}
async function onAvatarFile(e: Event) {
  const input = e.target as HTMLInputElement
  const f = input.files?.[0]
  if (!f) return
  try {
    const path = await saveUiImage(await fileToDataUrl(f), 'avatar')
    setting.avatarSuzu = path
    toastOk('头像已选择，点击「应用自定义外观」生效')
  } catch (err) {
    toastErr(`选择失败：${err}`)
  } finally {
    input.value = ''
  }
}
async function onUserAvatarFile(e: Event) {
  const input = e.target as HTMLInputElement
  const f = input.files?.[0]
  if (!f) return
  try {
    const path = await saveUiImage(await fileToDataUrl(f), 'user')
    setting.avatarUser = path
    toastOk('你的头像已选择，点击「应用自定义外观」生效')
  } catch (err) {
    toastErr(`选择失败：${err}`)
  } finally {
    input.value = ''
  }
}

onMounted(async () => {
  await ensureSettingsLoaded()
  mixRate.value = setting.languageMixRate
  selfName.value = setting.selfName
  userName.value = setting.userName
  await refreshNativePet()
})
</script>

<template>
  <div class="page">
    <!-- VBIL 虚拟形象（既有组件，样式留给它自己的批次） -->
    <VBILSettings />

    <!-- 个性化 -->
    <AppCard title="个性化">
      <AppField label="形象人格" hint="决定铃的说话风格，切换立即生效（API 模式下才用得到）。">
        <AppTabs :model-value="setting.persona" :items="PERSONA_OPTIONS" @update:model-value="pickPersona" />
      </AppField>
      <AppField label="日语修饰词浓度" hint="0-30；越高铃说话越带日语腔调，0 就是纯中文。">
        <input v-model.number="mixRate" class="range" type="range" min="0" max="30" />
        <p class="status-line">
          当前：{{ mixRate }}（{{ mixRate < 5 ? '低' : mixRate < 15 ? '中' : '高' }}）
        </p>
      </AppField>
      <div class="grid-2">
        <AppField label="自称">
          <AppInput v-model="selfName" placeholder="铃" />
        </AppField>
        <AppField label="对您的称呼">
          <AppInput v-model="userName" placeholder="主人" />
        </AppField>
      </div>
      <div class="row">
        <AppButton variant="primary" @click="savePersona">保存个性化</AppButton>
      </div>
    </AppCard>

    <!-- 桌宠（悬浮球） -->
    <AppCard title="桌宠（悬浮球）">
      <p class="muted">
        桌面上陪你工作的小铃：会自己动一动、偶尔说话，点一下有反应，摸头会长呼噜，拖动可以被拎起来。
      </p>
      <p class="muted">
        已知问题：在高缩放（如 150%）屏幕上，小铃的画面可能偏出窗口一角 —— 这是 Tauri 透明窗口 + WebView2
        的渲染问题，正在处理。
      </p>
      <AppSwitch
        :model-value="setting.floatingBallEnabled"
        label="显示桌宠"
        @update:model-value="(v) => setBallFlag('floatingBallEnabled', v)"
      />
      <template v-if="setting.floatingBallEnabled">
        <AppField
          label="形象"
          :hint="
            setting.floatingBallMode === 'live2d'
              ? 'Live2D 形象是实验功能：在 150% 等高缩放屏幕上，模型可能比窗口大、被裁掉一部分。显示不对就切回头像。'
              : ''
          "
        >
          <AppTabs :items="BALL_MODES" :model-value="setting.floatingBallMode" @update:model-value="onBallMode" />
        </AppField>
        <AppField label="大小（像素）">
          <AppTabs :items="SIZE_ITEMS" :model-value="String(setting.floatingBallSize)" @update:model-value="onBallSize" />
        </AppField>
        <AppField label="透明度">
          <AppTabs :items="OPACITY_ITEMS" :model-value="opacityValue" @update:model-value="onBallOpacity" />
        </AppField>
        <AppSwitch
          :model-value="setting.floatingBallBreathing"
          label="呼吸动画"
          @update:model-value="(v) => setBallFlag('floatingBallBreathing', v)"
        />
        <AppSwitch
          :model-value="setting.floatingBallFlash"
          label="消息闪烁"
          @update:model-value="(v) => setBallFlag('floatingBallFlash', v)"
        />
        <AppSwitch
          :model-value="setting.floatingBallChatter"
          label="偶尔自言自语"
          hint="关掉后只有动作，不冒气泡"
          @update:model-value="(v) => setBallFlag('floatingBallChatter', v)"
        />
        <div class="row">
          <AppButton @click="resetBallPosition">重置位置</AppButton>
        </div>
      </template>
    </AppCard>

    <!-- 桌面伙伴（原生桌宠） -->
    <AppCard title="桌面伙伴（原生桌宠）">
      <p class="muted">
        随安装包一起装到电脑上的独立小铃：不经过网页内核，透明背景、Live2D 动作，能拖动、摸头、说话，还能自己掉到屏幕底。
        在桌面双击「铃桌宠」也能单独启动，不会和这里重复开出两只。
      </p>
      <AppSwitch
        :model-value="setting.nativePetEnabled"
        label="随应用启动"
        @update:model-value="onNativePet"
      />
      <p v-if="nativePet && !nativePet.found" class="muted">
        没找到桌宠程序 pet\pet-native.exe：这份安装包里可能没带上它（开发版常见），用正式安装包装一次就有。
      </p>
      <template v-else>
        <div class="status-line">
          <span>状态</span>
          <AppStatusDot :state="nativePet?.running ? 'ok' : 'idle'" :text="nativePet?.running ? '运行中' : '未运行'" />
          <span v-if="nativePetMsg" class="muted">{{ nativePetMsg }}</span>
        </div>
        <div class="row">
          <AppButton :disabled="!setting.nativePetEnabled" @click="restartNativePet">重启桌宠</AppButton>
        </div>
      </template>
    </AppCard>

    <!-- 外观 -->
    <AppCard title="外观">
      <AppField label="主题">
        <AppTabs :items="THEME_ITEMS" :model-value="setting.theme" @update:model-value="onTheme" />
      </AppField>
      <AppField label="风格">
        <AppTabs :items="STYLE_ITEMS" :model-value="setting.theme" @update:model-value="onTheme" />
      </AppField>
      <AppField label="Emoji 显示" hint="控制铃说话里的 emoji 密度；纯文字界面不受影响。">
        <AppTabs :items="EMOJI_ITEMS" :model-value="setting.emojiMode" @update:model-value="setEmojiMode" />
      </AppField>

      <div class="divider" />
      <p class="block-title">自定义外观</p>

      <AppField label="主色" hint="设置主色后，铃的气泡 / 危险按钮自动跟随（可单独覆盖）。">
        <div class="row">
          <AppInput v-model="accentColor" placeholder="点右侧色块选择，或直接填色值" />
          <input v-model="accentColor" class="swatch" type="color" aria-label="选择主色" />
        </div>
      </AppField>
      <AppField label="危险色（删除/错误按钮）" hint="留空则跟随主色。">
        <div class="row">
          <AppInput v-model="dangerColor" placeholder="留空则跟随主色" />
          <input v-model="dangerColor" class="swatch" type="color" aria-label="选择危险色" />
        </div>
      </AppField>
      <AppField label="背景色">
        <div class="row">
          <AppInput v-model="bgColor" placeholder="点右侧色块选择" />
          <input v-model="bgColor" class="swatch" type="color" aria-label="选择背景色" />
        </div>
      </AppField>
      <AppField label="背景图（选填）">
        <div class="row">
          <AppInput v-model="bgImage" placeholder="图片路径，或点「选择图片」从本机挑" />
          <AppButton @click="pickImage('bg')">选择图片</AppButton>
        </div>
        <input ref="bgFileInput" class="hidden-file" type="file" accept="image/*" @change="onBgFile" />
      </AppField>
      <AppField label="铃的头像" hint="可以填 emoji / 文字，也可以选一张图片。">
        <div class="row">
          <AppInput v-model="avatarSuzu" placeholder="铃" />
          <AppButton @click="pickImage('avatar')">选头像图</AppButton>
        </div>
        <input ref="avatarFileInput" class="hidden-file" type="file" accept="image/*" @change="onAvatarFile" />
      </AppField>
      <AppField label="你的头像（选填）" hint="留空则不显示。">
        <div class="row">
          <AppInput v-model="avatarUser" placeholder="（留空则不显示）" />
          <AppButton @click="pickImage('user')">选头像图</AppButton>
        </div>
        <input ref="userAvatarFileInput" class="hidden-file" type="file" accept="image/*" @change="onUserAvatarFile" />
      </AppField>
      <AppField :label="`圆角：${setting.uiRadius ?? 12}px`">
        <input v-model.number="setting.uiRadius" class="range" type="range" min="0" max="24" />
      </AppField>
      <AppField label="你的气泡颜色">
        <div class="row">
          <AppInput v-model="bubbleUserColor" placeholder="点右侧色块选择" />
          <input v-model="bubbleUserColor" class="swatch" type="color" aria-label="选择你的气泡颜色" />
        </div>
      </AppField>
      <AppField label="铃的气泡颜色">
        <div class="row">
          <AppInput v-model="bubbleSuzuColor" placeholder="点右侧色块选择" />
          <input v-model="bubbleSuzuColor" class="swatch" type="color" aria-label="选择铃的气泡颜色" />
        </div>
      </AppField>
      <div class="row">
        <AppButton variant="primary" @click="saveUiCustom">应用自定义外观</AppButton>
      </div>

      <div class="divider" />
      <p class="block-title">主题组合</p>
      <AppField label="保存当前这套风格" hint="保存后一键切换你的专属风格，无需复制粘贴。">
        <div class="row">
          <AppInput v-model="presetName" placeholder="给这套风格起个名字" />
          <AppButton variant="primary" @click="savePreset">保存组合</AppButton>
        </div>
      </AppField>
      <div v-if="setting.uiThemes && setting.uiThemes.length" class="presets">
        <div v-for="t in setting.uiThemes" :key="t.name" class="preset">
          <span class="grow ellipsis">{{ t.name }}</span>
          <AppButton size="sm" @click="setting.switchThemePreset(t.name)">切换</AppButton>
          <AppButton size="sm" @click="setting.deleteThemePreset(t.name)">删除</AppButton>
        </div>
      </div>
    </AppCard>
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
.grid-2 {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--ui-sp-3);
}
.grow {
  flex: 1;
  min-width: 0;
}
.ellipsis {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.muted {
  margin: 0 0 var(--ui-sp-2);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-3);
  overflow-wrap: anywhere;
}
.status-line {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  margin: 0;
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-2);
}
.block-title {
  margin: var(--ui-sp-2) 0;
  font-size: var(--ui-fs-13);
  font-weight: 600;
  color: var(--ui-text);
}
.divider {
  height: 1px;
  margin: var(--ui-sp-3) 0;
  background: var(--ui-border);
}
.range {
  width: 100%;
  accent-color: var(--ui-accent);
}
.swatch {
  flex: none;
  width: 34px;
  height: var(--ui-ctl-h);
  padding: 0;
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-sm);
  background: transparent;
}
.hidden-file {
  display: none;
}
.presets {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
  max-height: 200px;
  overflow-y: auto;
}
.preset {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-1) var(--ui-sp-2);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  font-size: var(--ui-fs-12);
  color: var(--ui-text);
}
</style>
