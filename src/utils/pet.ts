// 桌宠（悬浮球）行为内核：台词池与随机节奏。
// 抽成纯函数，组件只负责窗口与动画，行为规则在这里单测。

/** 各类台词池（铃的人设：猫娘、叫主人、少量日语腔） */
export const PET_LINES = {
  poke: ['欸？', '在呢在呢～', '别戳啦，痒…', '有什么事吗？', '主人叫我？'],
  pat: ['唔…好舒服', '再摸一下嘛', '呼噜呼噜…', '今天也很温柔呢', '嘿嘿…'],
  wake: ['唔…醒了', '主人回来啦？', '刚刚打了个盹…'],
  sleep: ['有点困了…', '先眯一会儿…', 'zzz…'],
  drag: ['哇啊——', '放我下来啦～', '要掉下去了！', '轻一点呀…'],
  drop: ['安全着陆…', '哎哟…', '站稳啦'],
  chatter: [
    '今天过得怎么样？',
    '要不要休息一下呀',
    '记得喝水哦',
    '我在等你说话呢',
    '外面天气好吗？',
    '主人～在忙什么？',
  ],
} as const

export type PetLineKind = keyof typeof PET_LINES

/** 从对应池里随机取一句；random 可注入便于测试 */
export function pickPetLine(kind: PetLineKind, random: () => number = Math.random): string {
  const pool = PET_LINES[kind]
  if (!pool.length) return ''
  const index = Math.min(pool.length - 1, Math.floor(random() * pool.length))
  return pool[index]
}

/** 待机小动作间隔：18~45 秒 */
export function nextIdleDelay(random: () => number = Math.random): number {
  return 18_000 + Math.floor(random() * 27_000)
}

/** 自言自语间隔：45~120 秒 */
export function nextChatterDelay(random: () => number = Math.random): number {
  return 45_000 + Math.floor(random() * 75_000)
}

/** 多久没互动就趴下打瞌睡：5 分钟 */
export const SLEEP_AFTER_MS = 5 * 60_000

/** 台词气泡显示时长 */
export const BUBBLE_MS = 4200

/** 掉落目标 y：窗口底边贴屏幕底（留一点缝） */
export function fallTargetY(screenHeight: number, winHeight: number, gap = 4): number {
  return Math.max(0, screenHeight - winHeight - gap)
}
