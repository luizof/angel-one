export type Band = 'manha' | 'tarde' | 'final_de_tarde' | 'noite'
export type Dir = 'N' | 'NE' | 'L' | 'SE' | 'S' | 'SO' | 'O' | 'NO'
export const DIRS: Dir[] = ['N', 'NE', 'L', 'SE', 'S', 'SO', 'O', 'NO']

export interface Crew {
  id: number
  name: string
  emoji: string
  created_at?: string
}

export interface Settings {
  text_model: string
  image_model: string
  edit_model: string
  cruise_speed_kn: number
  moving_threshold_kn: number
  weather_lat: number
  weather_lon: number
  openrouter_key_set: boolean
  vapid_public_key: string
}

export interface AiModel {
  id: string
  name: string
  provider: string
  image_input?: boolean
}

export interface Destination {
  id: number
  name: string
  lat: number
  lon: number
  photo: string | null
  photo_original: string | null
  good_winds: Dir[]
  votes: number
  my_vote: boolean
  created_by?: string | null
  created_at?: string
}

export interface Route {
  destination_id: number
  distance_m: number
  straight_m: number
  via_water: boolean
  path: [number, number][]
}

export interface WeatherHour {
  time: string
  wind_kn: number | null
  gust_kn: number | null
  wind_dir: number | null
  wave_m: number | null
  wave_dir: number | null
  swell_m: number | null
  swell_dir: number | null
}

export interface Weather {
  updated_at: string
  timezone: string
  hours: WeatherHour[]
}

export interface BoatStatus {
  lat: number | null
  lon: number | null
  speed_kn: number | null
  moving: boolean
  heading: number | null
  updated_at: string | null
  stale: boolean
}

export type ContentUnit = 'ml' | 'L' | 'g' | 'kg' | 'un'
export type TagUnit = 'L' | 'ml' | 'kg' | 'g' | 'un'

export interface Item {
  id: number
  name: string
  photo: string | null
  photo_original: string | null
  tag_ids: number[]
  count_unit: string
  content_amount: number | null
  content_unit: ContentUnit | null
  quantity: number
  min_qty: number | null
  fractional: boolean
  updated_at?: string
  updated_by?: string | null
}

export interface Tag {
  id: number
  name: string
  emoji: string | null
  unit: TagUnit
  min_qty: number | null
  total: number
  below_min: boolean
  item_ids: number[]
}

export interface Inventory {
  items: Item[]
  tags: Tag[]
}

export interface Movement {
  at: string
  by: string | null
  reason: string
  before: number
  after: number
}

export type Category = 'drink' | 'petisco' | 'bebida'

export interface MenuItem {
  id: number
  name: string
  category: Category
  description: string
  recipe: string
  photo: string | null
  available: boolean
  sort: number
  created_at?: string
}

export type OrderStatus = 'recebido' | 'preparando' | 'pronto' | 'entregue' | 'cancelado'

export interface Order {
  id: number
  menu_item_id: number
  item_name: string
  item_photo: string | null
  category: Category
  recipe: string
  for_name: string
  notes: string
  status: OrderStatus
  handled_by: string | null
  created_at: string
  updated_at: string
}
