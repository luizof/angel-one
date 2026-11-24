// Cliente HTTP do Angel One: cabeçalhos de aparelho/tripulante, upload e jobs de IA.

function makeId(): string {
  try {
    return crypto.randomUUID()
  } catch {
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
      const r = (Math.random() * 16) | 0
      return (c === 'x' ? r : (r & 0x3) | 0x8).toString(16)
    })
  }
}

export const deviceId: string = (() => {
  const k = 'ao.device'
  let v = localStorage.getItem(k)
  if (!v) {
    v = makeId()
    localStorage.setItem(k, v)
  }
  return v
})()

let crewId: number | null = null
export function setCrewHeader(id: number | null) {
  crewId = id
}

export class ApiError extends Error {
  status: number
  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

function headers(json = true): Record<string, string> {
  const h: Record<string, string> = { 'X-Device-Id': deviceId }
  if (crewId != null) h['X-Crew-Id'] = String(crewId)
  if (json) h['Content-Type'] = 'application/json'
  return h
}

async function handle<T>(res: Response): Promise<T> {
  if (res.status === 204) return undefined as T
  const text = await res.text()
  let data: any = null
  try {
    data = text ? JSON.parse(text) : null
  } catch {
    data = null
  }
  if (!res.ok) {
    const msg = data?.error || (res.status >= 500 ? 'O servidor não respondeu direito. Tente de novo.' : 'Algo deu errado.')
    throw new ApiError(res.status, msg)
  }
  return data as T
}

export async function api<T = any>(path: string, opts: { method?: string; body?: unknown } = {}): Promise<T> {
  let res: Response
  try {
    res = await fetch(path, {
      method: opts.method ?? (opts.body !== undefined ? 'POST' : 'GET'),
      headers: headers(opts.body !== undefined),
      body: opts.body !== undefined ? JSON.stringify(opts.body) : undefined,
    })
  } catch {
    throw new ApiError(0, 'Sem conexão com o servidor.')
  }
  return handle<T>(res)
}

/** Reduz a foto no aparelho (corrige orientação, converte HEIC→JPEG) antes de enviar. */
export async function prepareImage(file: File, max = 2048): Promise<Blob> {
  let bmp: ImageBitmap
  try {
    bmp = await createImageBitmap(file, { imageOrientation: 'from-image' } as ImageBitmapOptions)
  } catch {
    const url = URL.createObjectURL(file)
    const img = new Image()
    img.src = url
    await img.decode()
    bmp = await createImageBitmap(img)
    URL.revokeObjectURL(url)
  }
  const scale = Math.min(1, max / Math.max(bmp.width, bmp.height))
  const w = Math.round(bmp.width * scale)
  const h = Math.round(bmp.height * scale)
  const canvas = document.createElement('canvas')
  canvas.width = w
  canvas.height = h
  const ctx = canvas.getContext('2d')!
  ctx.drawImage(bmp, 0, 0, w, h)
  bmp.close?.()
  return await new Promise<Blob>((resolve, reject) =>
    canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('Falha ao processar a foto'))), 'image/jpeg', 0.88),
  )
}

export async function upload(file: File): Promise<{ url: string; width: number; height: number }> {
  const blob = await prepareImage(file)
  const fd = new FormData()
  fd.append('file', blob, 'foto.jpg')
  let res: Response
  try {
    res = await fetch('/api/upload', { method: 'POST', headers: headers(false), body: fd })
  } catch {
    throw new ApiError(0, 'Sem conexão para enviar a foto.')
  }
  return handle(res)
}

export interface Job<T> {
  id: string
  status: 'pending' | 'done' | 'error'
  result: T | null
  error: string | null
}

/** Dispara um job de IA e espera o resultado (polling). */
export async function runJob<T = any>(path: string, body: unknown, timeoutMs = 240_000): Promise<T> {
  const { job_id } = await api<{ job_id: string }>(path, { body })
  const started = Date.now()
  let wait = 1200
  while (Date.now() - started < timeoutMs) {
    await new Promise((r) => setTimeout(r, wait))
    wait = Math.min(2500, wait + 200)
    let job: Job<T>
    try {
      job = await api<Job<T>>(`/api/ai/jobs/${job_id}`)
    } catch (e) {
      if (e instanceof ApiError && e.status === 0) continue
      throw e
    }
    if (job.status === 'done') return job.result as T
    if (job.status === 'error') throw new ApiError(500, job.error || 'A IA não conseguiu concluir.')
  }
  throw new ApiError(504, 'A IA demorou demais. Tente de novo.')
}
