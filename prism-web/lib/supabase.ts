// Supabase Client Configuration
import { createClientComponentClient } from '@supabase/auth-helpers-nextjs'
import { createClient } from '@supabase/supabase-js'
import { Database } from './database.types'

// Client Component逕ｨ
export const createSupabaseClient = () => {
  return createClientComponentClient<Database>()
}

// Server Component/API Route逕ｨ
export const createServerSupabaseClient = () => {
  return createClient<Database>(
    process.env.NEXT_PUBLIC_SUPABASE_URL!,
    process.env.SUPABASE_SERVICE_ROLE_KEY!, // Server-side only
    {
      auth: {
        persistSession: false
      }
    }
  )
}

// Browser逕ｨ・育ｰ｡譏鍋沿・・export const supabase = createClient<Database>(
  process.env.NEXT_PUBLIC_SUPABASE_URL!,
  process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY!
)

