import type { Attachment } from '../types'

const IMAGE_MIMES: Record<string, string> = {
  jpg: 'image/jpeg', jpeg: 'image/jpeg', png: 'image/png',
  gif: 'image/gif', webp: 'image/webp', bmp: 'image/bmp',
}
const TEXT_EXTENSIONS = new Set(['txt', 'md', 'json', 'csv', 'log', 'py', 'js', 'ts', 'rs', 'html', 'css', 'xml', 'yaml', 'toml', 'ini', 'sh'])
export const ATTACHMENT_ACCEPT = [...Object.keys(IMAGE_MIMES), ...TEXT_EXTENSIONS].map((extension) => '.' + extension).join(',')

export function fileSpec(file: Pick<File, 'name' | 'size'>): { kind: 'image' | 'text'; mime: string } {
  const extension = file.name.includes('.') ? file.name.split('.').pop()?.toLowerCase() ?? '' : ''
  const mime = IMAGE_MIMES[extension]
  if (!mime && !TEXT_EXTENSIONS.has(extension)) throw new Error('暂不支持这种文件')
  if (file.size > (mime ? 5 : 1) * 1024 * 1024) {
    throw new Error(mime ? '图片附件不能超过 5MB' : '文本附件不能超过 1MB')
  }
  return { kind: mime ? 'image' : 'text', mime: mime ?? 'text/plain' }
}

export async function readAttachment(file: File): Promise<Attachment> {
  const spec = fileSpec(file)
  let data: string
  if (spec.kind === 'image') {
    data = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader()
      reader.onerror = () => reject(new Error('附件读取失败，请重新选择'))
      reader.onabort = () => reject(new Error('附件读取已取消'))
      reader.onload = () => {
        const result = typeof reader.result === 'string' ? reader.result : ''
        const separator = result.indexOf(',')
        if (separator < 0) reject(new Error('附件读取失败，请重新选择'))
        else resolve(result.slice(separator + 1))
      }
      reader.readAsDataURL(file)
    })
  } else {
    try { data = await file.text() }
    catch { throw new Error('附件读取失败，请重新选择') }
    if (new TextEncoder().encode(data).length > 1024 * 1024) {
      throw new Error('文本附件不能超过 1MB')
    }
  }
  return { ...spec, name: file.name, data, size: file.size }
}
