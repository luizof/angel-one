import type { Item, Tag } from './types'
import { fmt2 } from './util'

const PLURAL: Record<string, string> = {
  garrafa: 'garrafas', lata: 'latas', 'long neck': 'long necks', unidade: 'unidades', un: 'un', pacote: 'pacotes',
  caixa: 'caixas', saco: 'sacos', pote: 'potes', kg: 'kg', g: 'g', L: 'L', ml: 'ml', fardo: 'fardos', galão: 'galões',
  dúzia: 'dúzias', vidro: 'vidros', rolo: 'rolos', bandeja: 'bandejas',
}
export const COUNT_UNITS = ['garrafa', 'lata', 'long neck', 'unidade', 'pacote', 'caixa', 'fardo', 'saco', 'pote', 'vidro', 'galão', 'rolo', 'bandeja', 'dúzia', 'kg', 'g', 'L']

export function unitLabel(unit: string, n: number) {
  if (Math.abs(n) === 1 || (Math.abs(n) < 1 && n !== 0)) return unit
  return PLURAL[unit] ?? (unit.endsWith('s') ? unit : unit + 's')
}

export function qtyLabel(i: Item, n = i.quantity) {
  return `${fmt2(n)} ${unitLabel(i.count_unit || 'un', n)}`
}

export function contentLabel(i: Item) {
  if (!i.content_amount || !i.content_unit) return ''
  return `${fmt2(i.content_amount)} ${i.content_unit}`
}

export function tagTotalLabel(t: Tag, n = t.total) {
  return `${fmt2(n)} ${t.unit}`
}

export function itemLow(i: Item) {
  return i.min_qty != null && i.quantity < i.min_qty
}

export function itemMissing(i: Item) {
  return i.min_qty != null ? Math.max(0, i.min_qty - i.quantity) : 0
}

export function tagMissing(t: Tag) {
  return t.min_qty != null ? Math.max(0, t.min_qty - t.total) : 0
}

/** Nível 0..1 para desenhar (itens fracionados mostram a garrafa aberta). */
export function fillLevel(i: Item) {
  if (i.min_qty) return Math.min(1, i.quantity / Math.max(i.min_qty * 2, 1))
  return Math.min(1, i.quantity / 4)
}
