//! Represents the Tailwind CSS [default color palette][palette].
//!
//! [palette]: https://tailwindcss.com/docs/customizing-colors#default-color-palette
//!
//! There are 22 palettes. Each palette has 11 colors, with variants from 50 to 950. Black and White
//! are also included for completeness and to avoid being affected by any terminal theme that might
//! be in use.
//!
//! <style>
//! .color { display: flex; align-items: center; }
//! .color > div { width: 2rem; height: 2rem; }
//! .color > div.name { width: 150px; !important; }
//! </style>
//! <div style="overflow-x: auto">
//! <div style="display: flex; flex-direction:column; text-align: left">
//! <div class="color" style="font-size:0.8em">
//!     <div class="name"></div>
//!     <div>C50</div> <div>C100</div> <div>C200</div> <div>C300</div> <div>C400</div>
//!     <div>C500</div> <div>C600</div> <div>C700</div> <div>C800</div> <div>C900</div>
//!     <div>C950</div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`SLATE`]</div>
//!     <div style="background-color: #f8fafc"></div> <div style="background-color: #f1f5f9"></div>
//!     <div style="background-color: #e2e8f0"></div> <div style="background-color: #cbd5e1"></div>
//!     <div style="background-color: #94a3b8"></div> <div style="background-color: #64748b"></div>
//!     <div style="background-color: #475569"></div> <div style="background-color: #334155"></div>
//!     <div style="background-color: #1e293b"></div> <div style="background-color: #0f172a"></div>
//!     <div style="background-color: #020617"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`GRAY`]</div>
//!     <div style="background-color: #f9fafb"></div> <div style="background-color: #f3f4f6"></div>
//!     <div style="background-color: #e5e7eb"></div> <div style="background-color: #d1d5db"></div>
//!     <div style="background-color: #9ca3af"></div> <div style="background-color: #6b7280"></div>
//!     <div style="background-color: #4b5563"></div> <div style="background-color: #374151"></div>
//!     <div style="background-color: #1f2937"></div> <div style="background-color: #111827"></div>
//!     <div style="background-color: #0a0a0a"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`ZINC`]</div>
//!     <div style="background-color: #fafafa"></div> <div style="background-color: #f5f5f5"></div>
//!     <div style="background-color: #e5e5e5"></div> <div style="background-color: #d4d4d4"></div>
//!     <div style="background-color: #a1a1aa"></div> <div style="background-color: #71717a"></div>
//!     <div style="background-color: #52525b"></div> <div style="background-color: #404040"></div>
//!     <div style="background-color: #262626"></div> <div style="background-color: #171717"></div>
//!     <div style="background-color: #0a0a0a"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`NEUTRAL`]</div>
//!     <div style="background-color: #fafafa"></div> <div style="background-color: #f5f5f5"></div>
//!     <div style="background-color: #e5e5e5"></div> <div style="background-color: #d4d4d4"></div>
//!     <div style="background-color: #a3a3a3"></div> <div style="background-color: #737373"></div>
//!     <div style="background-color: #525252"></div> <div style="background-color: #404040"></div>
//!     <div style="background-color: #262626"></div> <div style="background-color: #171717"></div>
//!     <div style="background-color: #0a0a0a"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`STONE`]</div>
//!     <div style="background-color: #fafaf9"></div> <div style="background-color: #f5f5f4"></div>
//!     <div style="background-color: #e7e5e4"></div> <div style="background-color: #d6d3d1"></div>
//!     <div style="background-color: #a8a29e"></div> <div style="background-color: #78716c"></div>
//!     <div style="background-color: #57534e"></div> <div style="background-color: #44403c"></div>
//!     <div style="background-color: #292524"></div> <div style="background-color: #1c1917"></div>
//!     <div style="background-color: #0c0a09"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`RED`]</div>
//!     <div style="background-color: #fef2f2"></div> <div style="background-color: #fee2e2"></div>
//!     <div style="background-color: #fecaca"></div> <div style="background-color: #fca5a5"></div>
//!     <div style="background-color: #f87171"></div> <div style="background-color: #ef4444"></div>
//!     <div style="background-color: #dc2626"></div> <div style="background-color: #b91c1c"></div>
//!     <div style="background-color: #991b1b"></div> <div style="background-color: #7f1d1d"></div>
//!     <div style="background-color: #450a0a"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`ORANGE`]</div>
//!     <div style="background-color: #fff7ed"></div> <div style="background-color: #ffedd5"></div>
//!     <div style="background-color: #fed7aa"></div> <div style="background-color: #fdba74"></div>
//!     <div style="background-color: #fb923c"></div> <div style="background-color: #f97316"></div>
//!     <div style="background-color: #ea580c"></div> <div style="background-color: #c2410c"></div>
//!     <div style="background-color: #9a3412"></div> <div style="background-color: #7c2d12"></div>
//!     <div style="background-color: #431407"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`AMBER`]</div>
//!     <div style="background-color: #fffbeb"></div> <div style="background-color: #fef3c7"></div>
//!     <div style="background-color: #fde68a"></div> <div style="background-color: #fcd34d"></div>
//!     <div style="background-color: #fbbf24"></div> <div style="background-color: #f59e0b"></div>
//!     <div style="background-color: #d97706"></div> <div style="background-color: #b45309"></div>
//!     <div style="background-color: #92400e"></div> <div style="background-color: #78350f"></div>
//!     <div style="background-color: #451a03"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`YELLOW`]</div>
//!     <div style="background-color: #fefce8"></div> <div style="background-color: #fef9c3"></div>
//!     <div style="background-color: #fef08a"></div> <div style="background-color: #fde047"></div>
//!     <div style="background-color: #facc15"></div> <div style="background-color: #eab308"></div>
//!     <div style="background-color: #ca8a04"></div> <div style="background-color: #a16207"></div>
//!     <div style="background-color: #854d0e"></div> <div style="background-color: #713f12"></div>
//!     <div style="background-color: #422006"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`LIME`]</div>
//!     <div style="background-color: #f7fee7"></div> <div style="background-color: #ecfccb"></div>
//!     <div style="background-color: #d9f99d"></div> <div style="background-color: #bef264"></div>
//!     <div style="background-color: #a3e635"></div> <div style="background-color: #84cc16"></div>
//!     <div style="background-color: #65a30d"></div> <div style="background-color: #4d7c0f"></div>
//!     <div style="background-color: #3f6212"></div> <div style="background-color: #365314"></div>
//!     <div style="background-color: #1a2e05"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`GREEN`]</div>
//!     <div style="background-color: #f0fdf4"></div> <div style="background-color: #dcfce7"></div>
//!     <div style="background-color: #bbf7d0"></div> <div style="background-color: #86efac"></div>
//!     <div style="background-color: #4ade80"></div> <div style="background-color: #22c55e"></div>
//!     <div style="background-color: #16a34a"></div> <div style="background-color: #15803d"></div>
//!     <div style="background-color: #166534"></div> <div style="background-color: #14532d"></div>
//!     <div style="background-color: #052e16"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`EMERALD`]</div>
//!     <div style="background-color: #ecfdf5"></div> <div style="background-color: #d1fae5"></div>
//!     <div style="background-color: #a7f3d0"></div> <div style="background-color: #6ee7b7"></div>
//!     <div style="background-color: #34d399"></div> <div style="background-color: #10b981"></div>
//!     <div style="background-color: #059669"></div> <div style="background-color: #047857"></div>
//!     <div style="background-color: #065f46"></div> <div style="background-color: #064e3b"></div>
//!     <div style="background-color: #022c22"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`TEAL`]</div>
//!     <div style="background-color: #f0fdfa"></div> <div style="background-color: #ccfbf1"></div>
//!     <div style="background-color: #99f6e4"></div> <div style="background-color: #5eead4"></div>
//!     <div style="background-color: #2dd4bf"></div> <div style="background-color: #14b8a6"></div>
//!     <div style="background-color: #0d9488"></div> <div style="background-color: #0f766e"></div>
//!     <div style="background-color: #115e59"></div> <div style="background-color: #134e4a"></div>
//!     <div style="background-color: #042f2e"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`CYAN`]</div>
//!     <div style="background-color: #ecfeff"></div> <div style="background-color: #cffafe"></div>
//!     <div style="background-color: #a5f3fc"></div> <div style="background-color: #67e8f9"></div>
//!     <div style="background-color: #22d3ee"></div> <div style="background-color: #06b6d4"></div>
//!     <div style="background-color: #0891b2"></div> <div style="background-color: #0e7490"></div>
//!     <div style="background-color: #155e75"></div> <div style="background-color: #164e63"></div>
//!     <div style="background-color: #083344"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`SKY`]</div>
//!     <div style="background-color: #f0f9ff"></div> <div style="background-color: #e0f2fe"></div>
//!     <div style="background-color: #bae6fd"></div> <div style="background-color: #7dd3fc"></div>
//!     <div style="background-color: #38bdf8"></div> <div style="background-color: #0ea5e9"></div>
//!     <div style="background-color: #0284c7"></div> <div style="background-color: #0369a1"></div>
//!     <div style="background-color: #075985"></div> <div style="background-color: #0c4a6e"></div>
//!     <div style="background-color: #082f49"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`BLUE`]</div>
//!     <div style="background-color: #eff6ff"></div> <div style="background-color: #dbeafe"></div>
//!     <div style="background-color: #bfdbfe"></div> <div style="background-color: #93c5fd"></div>
//!     <div style="background-color: #60a5fa"></div> <div style="background-color: #3b82f6"></div>
//!     <div style="background-color: #2563eb"></div> <div style="background-color: #1d4ed8"></div>
//!     <div style="background-color: #1e40af"></div> <div style="background-color: #1e3a8a"></div>
//!     <div style="background-color: #172554"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`INDIGO`]</div>
//!     <div style="background-color: #eef2ff"></div> <div style="background-color: #e0e7ff"></div>
//!     <div style="background-color: #c7d2fe"></div> <div style="background-color: #a5b4fc"></div>
//!     <div style="background-color: #818cf8"></div> <div style="background-color: #6366f1"></div>
//!     <div style="background-color: #4f46e5"></div> <div style="background-color: #4338ca"></div>
//!     <div style="background-color: #3730a3"></div> <div style="background-color: #312e81"></div>
//!     <div style="background-color: #1e1b4b"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`VIOLET`]</div>
//!     <div style="background-color: #f5f3ff"></div> <div style="background-color: #ede9fe"></div>
//!     <div style="background-color: #ddd6fe"></div> <div style="background-color: #c4b5fd"></div>
//!     <div style="background-color: #a78bfa"></div> <div style="background-color: #8b5cf6"></div>
//!     <div style="background-color: #7c3aed"></div> <div style="background-color: #6d28d9"></div>
//!     <div style="background-color: #5b21b6"></div> <div style="background-color: #4c1d95"></div>
//!     <div style="background-color: #2e1065"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`PURPLE`]</div>
//!     <div style="background-color: #faf5ff"></div> <div style="background-color: #f3e8ff"></div>
//!     <div style="background-color: #e9d5ff"></div> <div style="background-color: #d8b4fe"></div>
//!     <div style="background-color: #c084fc"></div> <div style="background-color: #a855f7"></div>
//!     <div style="background-color: #9333ea"></div> <div style="background-color: #7e22ce"></div>
//!     <div style="background-color: #6b21a8"></div> <div style="background-color: #581c87"></div>
//!     <div style="background-color: #4c136e"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`FUCHSIA`]</div>
//!     <div style="background-color: #fdf4ff"></div> <div style="background-color: #fae8ff"></div>
//!     <div style="background-color: #f5d0fe"></div> <div style="background-color: #f0abfc"></div>
//!     <div style="background-color: #e879f9"></div> <div style="background-color: #d946ef"></div>
//!     <div style="background-color: #c026d3"></div> <div style="background-color: #a21caf"></div>
//!     <div style="background-color: #86198f"></div> <div style="background-color: #701a75"></div>
//!     <div style="background-color: #4e145b"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`PINK`]</div>
//!     <div style="background-color: #fdf2f8"></div> <div style="background-color: #fce7f3"></div>
//!     <div style="background-color: #fbcfe8"></div> <div style="background-color: #f9a8d4"></div>
//!     <div style="background-color: #f472b6"></div> <div style="background-color: #ec4899"></div>
//!     <div style="background-color: #db2777"></div> <div style="background-color: #be185d"></div>
//!     <div style="background-color: #9d174d"></div> <div style="background-color: #831843"></div>
//!     <div style="background-color: #5f0b37"></div>
//! </div>
//! <div class="color">
//!    <div class="name">
//!
//! [`BLACK`]</div>
//!     <div style="background-color: #000000; width:22rem"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`WHITE`]</div>
//!     <div style="background-color: #ffffff; width:22rem"></div>
//! </div>
//! </div>
//! </div>
//!
//! # Example
//!
//! ```rust
//! use ratatui_core::style::Color;
//! use ratatui_core::style::palette::tailwind::{BLUE, RED};
//!
//! assert_eq!(RED.c500, Color::Rgb(239, 68, 68));
//! assert_eq!(BLUE.c500, Color::Rgb(59, 130, 246));
//! ```

use crate::style::Color;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Palette {
    pub c50: Color,
    pub c100: Color,
    pub c200: Color,
    pub c300: Color,
    pub c400: Color,
    pub c500: Color,
    pub c600: Color,
    pub c700: Color,
    pub c800: Color,
    pub c900: Color,
    pub c950: Color,
}

#[rustfmt::skip]
/// <style>.palette div{width:22rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #000000"></div></div>
pub const BLACK: Color = Color::from_u32(0x000000);

#[rustfmt::skip]
/// <style>.palette div{width:22rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #ffffff"></div></div>
pub const WHITE: Color = Color::from_u32(0xffffff);

#[rustfmt::skip]
/// <style>.palette div{width:2rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #f8fafc"></div><div style="background-color: #f1f5f9"></div><div style="background-color: #e2e8f0"></div><div style="background-color: #cbd5e1"></div><div style="background-color: #94a3b8"></div><div style="background-color: #64748b"></div><div style="background-color: #475569"></div><div style="background-color: #334155"></div><div style="background-color: #1e293b"></div><div style="background-color: #0f172a"></div><div style="background-color: #020617"></div></div>
pub const SLATE: Palette = Palette {
    c50: Color::from_u32(0xf8fafc),
    c100: Color::from_u32(0xf1f5f9),
    c200: Color::from_u32(0xe2e8f0),
    c300: Color::from_u32(0xcbd5e1),
    c400: Color::from_u32(0x94a3b8),
    c500: Color::from_u32(0x64748b),
    c600: Color::from_u32(0x475569),
    c700: Color::from_u32(0x334155),
    c800: Color::from_u32(0x1e293b),
    c900: Color::from_u32(0x0f172a),
    c950: Color::from_u32(0x020617),
};

#[rustfmt::skip]
/// <style>.palette div{width:2rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #f9fafb"></div><div style="background-color: #f3f4f6"></div><div style="background-color: #e5e7eb"></div><div style="background-color: #d1d5db"></div><div style="background-color: #9ca3af"></div><div style="background-color: #6b7280"></div><div style="background-color: #4b5563"></div><div style="background-color: #374151"></div><div style="background-color: #1f2937"></div><div style="background-color: #111827"></div><div style="background-color: #030712"></div></div>
pub const GRAY: Palette = Palette {
    c50: Color::from_u32(0xf9fafb),
    c100: Color::from_u32(0xf3f4f6),
    c200: Color::from_u32(0xe5e7eb),
    c300: Color::from_u32(0xd1d5db),
    c400: Color::from_u32(0x9ca3af),
    c500: Color::from_u32(0x6b7280),
    c600: Color::from_u32(0x4b5563),
    c700: Color::from_u32(0x374151),
    c800: Color::from_u32(0x1f2937),
    c900: Color::from_u32(0x111827),
    c950: Color::from_u32(0x030712),
};

#[rustfmt::skip]
/// <style>.palette div{width:2rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #fafafa"></div><div style="background-color: #f5f5f5"></div><div style="background-color: #e5e5e5"></div><div style="background-color: #d4d4d4"></div><div style="background-color: #a1a1aa"></div><div style="background-color: #71717a"></div><div style="background-color: #52525b"></div><div style="background-color: #404040"></div><div style="background-color: #262626"></div><div style="background-color: #171717"></div><div style="background-color: #09090b"></div></div>
pub const ZINC: Palette = Palette {
    c50: Color::from_u32(0xfafafa),
    c100: Color::from_u32(0xf4f4f5),
    c200: Color::from_u32(0xe4e4e7),
    c300: Color::from_u32(0xd4d4d8),
    c400: Color::from_u32(0xa1a1aa),
    c500: Color::from_u32(0x71717a),
    c600: Color::from_u32(0x52525b),
    c700: Color::from_u32(0x3f3f46),
    c800: Color::from_u32(0x27272a),
    c900: Color::from_u32(0x18181b),
    c950: Color::from_u32(0x09090b),
};

#[rustfmt::skip]
/// <style>.palette div{width:2rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #fafafa"></div><div style="background-color: #f5f5f5"></div><div style="background-color: #e5e5e5"></div><div style="background-color: #d4d4d4"></div><div style="background-color: #a3a3a3"></div><div style="background-color: #737373"></div><div style="background-color: #525252"></div><div style="background-color: #404040"></div><div style="background-color: #262626"></div><div style="background-color: #171717"></div><div style="background-color: #0a0a0a"></div></div>
pub const NEUTRAL: Palette = Palette {
    c50: Color::from_u32(0xfafafa),
    c100: Color::from_u32(0xf5f5f5),
    c200: Color::from_u32(0xe5e5e5),
    c300: Color::from_u32(0xd4d4d4),
    c400: Color::from_u32(0xa3a3a3),
    c500: Color::from_u32(0x737373),
    c600: Color::from_u32(0x525252),
    c700: Color::from_u32(0x404040),
    c800: Color::from_u32(0x262626),
    c900: Color::from_u32(0x171717),
    c950: Color::from_u32(0x0a0a0a),
};

#[rustfmt::skip]
/// <style>.palette div{width:2rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #fafaf9"></div><div style="background-color: #f5f5f4"></div><div style="background-color: #e7e5e4"></div><div style="background-color: #d6d3d1"></div><div style="background-color: #a8a29e"></div><div style="background-color: #78716c"></div><div style="background-color: #57534e"></div><div style="background-color: #44403c"></div><div style="background-color: #292524"></div><div style="background-color: #1c1917"></div><div style="background-color: #0c0a09"></div></div>
pub const STONE: Palette = Palette {
    c50: Color::from_u32(0xfafaf9),
    c100: Color::from_u32(0xf5f5f4),
    c200: Color::from_u32(0xe7e5e4),
    c300: Color::from_u32(0xd6d3d1),
    c400: Color::from_u32(0xa8a29e),
    c500: Color::from_u32(0x78716c),
    c600: Color::from_u32(0x57534e),
    c700: Color::from_u32(0x44403c),
    c800: Color::from_u32(0x292524),
    c900: Color::from_u32(0x1c1917),
    c950: Color::from_u32(0x0c0a09),
};

#[rustfmt::skip]
/// <style>.palette div{width:2rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #fef2f2"></div><div style="background-color: #fee2e2"></div><div style="background-color: #fecaca"></div><div style="background-color: #fca5a5"></div><div style="background-color: #f87171"></div><div style="background-color: #ef4444"></div><div style="background-color: #dc2626"></div><div style="background-color: #b91c1c"></div><div style="background-color: #991b1b"></div><div style="background-color: #7f1d1d"></div><div style="background-color: #450a0a"></div></div>
pub const RED: Palette = Palette {
    c50: Color::from_u32(0xfef2f2),
    c100: Color::from_u32(0xfee2e2),
    c200: Color::from_u32(0xfecaca),
    c300: Color::from_u32(0xfca5a5),
    c400: Color::from_u32(0xf87171),
    c500: Color::from_u32(0xef4444),
    c600: Color::from_u32(0xdc2626),
    c700: Color::from_u32(0xb91c1c),
    c800: Color::from_u32(0x991b1b),
    c900: Color::from_u32(0x7f1d1d),
    c950: Color::from_u32(0x450a0a),
};

#[rustfmt::skip]
/// <style>.palette div{width:2rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #fff7ed"></div><div style="background-color: #ffedd5"></div><div style="background-color: #fed7aa"></div><div style="background-color: #fdba74"></div><div style="background-color: #fb923c"></div><div style="background-color: #f97316"></div><div style="background-color: #ea580c"></div><div style="background-color: #c2410c"></div><div style="background-color: #9a3412"></div><div style="background-color: #7c2d12"></div><div style="background-color: #431407"></div></div>
pub const ORANGE: Palette = Palette {
    c50: Color::from_u32(0xfff7ed),
    c100: Color::from_u32(0xffedd5),
    c200: Color::from_u32(0xfed7aa),
    c300: Color::from_u32(0xfdba74),
    c400: Color::from_u32(0xfb923c),
    c500: Color::from_u32(0xf97316),
    c600: Color::from_u32(0xea580c),
    c700: Color::from_u32(0xc2410c),
    c800: Color::from_u32(0x9a3412),
    c900: Color::from_u32(0x7c2d12),
    c950: Color::from_u32(0x431407),
};

#[rustfmt::skip]
/// <style>.palette div{width:2rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #fffbeb"></div><div style="background-color: #fef3c7"></div><div style="background-color: #fde68a"></div><div style="background-color: #fcd34d"></div><div style="background-color: #fbbf24"></div><div style="background-color: #f59e0b"></div><div style="background-color: #d97706"></div><div style="background-color: #b45309"></div><div style="background-color: #92400e"></div><div style="background-color: #78350f"></div><div style="background-color: #451a03"></div></div>
pub const AMBER: Palette = Palette {
    c50: Color::from_u32(0xfffbeb),
    c100: Color::from_u32(0xfef3c7),
    c200: Color::from_u32(0xfde68a),
    c300: Color::from_u32(0xfcd34d),
    c400: Color::from_u32(0xfbbf24),
    c500: Color::from_u32(0xf59e0b),
    c600: Color::from_u32(0xd97706),
    c700: Color::from_u32(0xb45309),
    c800: Color::from_u32(0x92400e),
    c900: Color::from_u32(0x78350f),
    c950: Color::from_u32(0x451a03),
};

#[rustfmt::skip]
/// <style>.palette div{width:2rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #fefce8"></div><div style="background-color: #fef9c3"></div><div style="background-color: #fef08a"></div><div style="background-color: #fde047"></div><div style="background-color: #facc15"></div><div style="background-color: #eab308"></div><div style="background-color: #ca8a04"></div><div style="background-color: #a16207"></div><div style="background-color: #854d0e"></div><div style="background-color: #713f12"></div><div style="background-color: #422006"></div></div>
pub const YELLOW: Palette = Palette {
    c50: Color::from_u32(0xfefce8),
    c100: Color::from_u32(0xfef9c3),
    c200: Color::from_u32(0xfef08a),
    c300: Color::from_u32(0xfde047),
    c400: Color::from_u32(0xfacc15),
    c500: Color::from_u32(0xeab308),
    c600: Color::from_u32(0xca8a04),
    c700: Color::from_u32(0xa16207),
    c800: Color::from_u32(0x854d0e),
    c900: Color::from_u32(0x713f12),
    c950: Color::from_u32(0x422006),
};

#[rustfmt::skip]
/// <style>.palette div{width:2rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #f7fee7"></div><div style="background-color: #ecfccb"></div><div style="background-color: #d9f99d"></div><div style="background-color: #bef264"></div><div style="background-color: #a3e635"></div><div style="background-color: #84cc16"></div><div style="background-color: #65a30d"></div><div style="background-color: #4d7c0f"></div><div style="background-color: #3f6212"></div><div style="background-color: #365314"></div><div style="background-color: #1a2e05"></div></div>
pub const LIME: Palette = Palette {
    c50: Color::from_u32(0xf7fee7),
    c100: Color::from_u32(0xecfccb),
    c200: Color::from_u32(0xd9f99d),
    c300: Color::from_u32(0xbef264),
    c400: Color::from_u32(0xa3e635),
    c500: Color::from_u32(0x84cc16),
    c600: Color::from_u32(0x65a30d),
    c700: Color::from_u32(0x4d7c0f),
    c800: Color::from_u32(0x3f6212),
    c900: Color::from_u32(0x365314),
    c950: Color::from_u32(0x1a2e05),
};

#[rustfmt::skip]
/// <style>.palette div{width:2rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #f0fdf4"></div><div style="background-color: #dcfce7"></div><div style="background-color: #bbf7d0"></div><div style="background-color: #86efac"></div><div style="background-color: #4ade80"></div><div style="background-color: #22c55e"></div><div style="background-color: #16a34a"></div><div style="background-color: #15803d"></div><div style="background-color: #166534"></div><div style="background-color: #14532d"></div><div style="background-color: #052e16"></div></div>
pub const GREEN: Palette = Palette {
    c50: Color::from_u32(0xf0fdf4),
    c100: Color::from_u32(0xdcfce7),
    c200: Color::from_u32(0xbbf7d0),
    c300: Color::from_u32(0x86efac),
    c400: Color::from_u32(0x4ade80),
    c500: Color::from_u32(0x22c55e),
    c600: Color::from_u32(0x16a34a),
    c700: Color::from_u32(0x15803d),
    c800: Color::from_u32(0x166534),
    c900: Color::from_u32(0x14532d),
    c950: Color::from_u32(0x052e16),
};

#[rustfmt::skip]
/// <style>.palette div{width:2rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #f0fdfa"></div><div style="background-color: #ccfbf1"></div><div style="background-color: #99f6e4"></div><div style="background-color: #5eead4"></div><div style="background-color: #2dd4bf"></div><div style="background-color: #14b8a6"></div><div style="background-color: #0d9488"></div><div style="background-color: #0f766e"></div><div style="background-color: #115e59"></div><div style="background-color: #134e4a"></div><div style="background-color: #042f2e"></div></div>
pub const EMERALD: Palette = Palette {
    c50: Color::from_u32(0xecfdf5),
    c100: Color::from_u32(0xd1fae5),
    c200: Color::from_u32(0xa7f3d0),
    c300: Color::from_u32(0x6ee7b7),
    c400: Color::from_u32(0x34d399),
    c500: Color::from_u32(0x10b981),
    c600: Color::from_u32(0x059669),
    c700: Color::from_u32(0x047857),
    c800: Color::from_u32(0x065f46),
    c900: Color::from_u32(0x064e3b),
    c950: Color::from_u32(0x022c22),
};

#[rustfmt::skip]
/// <style>.palette div{width:2rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #f5fdf4"></div><div style="background-color: #e7f9e7"></div><div style="background-color: #c6f6d5"></div><div style="background-color: #9ae6b4"></div><div style="background-color: #68d391"></div><div style="background-color: #48bb78"></div><div style="background-color: #38a169"></div><div style="background-color: #2f855a"></div><div style="background-color: #276749"></div><div style="background-color: #22543d"></div><div style="background-color: #0d3321"></div></div>
pub const TEAL: Palette = Palette {
    c50: Color::from_u32(0xf0fdfa),
    c100: Color::from_u32(0xccfbf1),
    c200: Color::from_u32(0x99f6e4),
    c300: Color::from_u32(0x5eead4),
    c400: Color::from_u32(0x2dd4bf),
    c500: Color::from_u32(0x14b8a6),
    c600: Color::from_u32(0x0d9488),
    c700: Color::from_u32(0x0f766e),
    c800: Color::from_u32(0x115e59),
    c900: Color::from_u32(0x134e4a),
    c950: Color::from_u32(0x042f2e),
};

#[rustfmt::skip]
/// <style>.palette div{width:2rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #ecfeff"></div><div style="background-color: #cffafe"></div><div style="background-color: #a5f3fc"></div><div style="background-color: #67e8f9"></div><div style="background-color: #22d3ee"></div><div style="background-color: #06b6d4"></div><div style="background-color: #0891b2"></div><div style="background-color: #0e7490"></div><div style="background-color: #155e75"></div><div style="background-color: #164e63"></div><div style="background-color: #083344"></div></div>
pub const CYAN: Palette = Palette {
    c50: Color::from_u32(0xecfeff),
    c100: Color::from_u32(0xcffafe),
    c200: Color::from_u32(0xa5f3fc),
    c300: Color::from_u32(0x67e8f9),
    c400: Color::from_u32(0x22d3ee),
    c500: Color::from_u32(0x06b6d4),
    c600: Color::from_u32(0x0891b2),
    c700: Color::from_u32(0x0e7490),
    c800: Color::from_u32(0x155e75),
    c900: Color::from_u32(0x164e63),
    c950: Color::from_u32(0x083344),
};

#[rustfmt::skip]
/// <style>.palette div{width:22rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #f0f9ff"></div><div style="background-color: #e0f2fe"></div><div style="background-color: #bae6fd"></div><div style="background-color: #7dd3fc"></div><div style="background-color: #38bdf8"></div><div style="background-color: #0ea5e9"></div><div style="background-color: #0284c7"></div><div style="background-color: #0369a1"></div><div style="background-color: #075985"></div><div style="background-color: #0c4a6e"></div><div style="background-color: #082f49"></div></div>
pub const SKY: Palette = Palette {
    c50: Color::from_u32(0xf0f9ff),
    c100: Color::from_u32(0xe0f2fe),
    c200: Color::from_u32(0xbae6fd),
    c300: Color::from_u32(0x7dd3fc),
    c400: Color::from_u32(0x38bdf8),
    c500: Color::from_u32(0x0ea5e9),
    c600: Color::from_u32(0x0284c7),
    c700: Color::from_u32(0x0369a1),
    c800: Color::from_u32(0x075985),
    c900: Color::from_u32(0x0c4a6e),
    c950: Color::from_u32(0x082f49),
};

#[rustfmt::skip]
/// <style>.palette div{width:22rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #eff6ff"></div><div style="background-color: #dbeafe"></div><div style="background-color: #bfdbfe"></div><div style="background-color: #93c5fd"></div><div style="background-color: #60a5fa"></div><div style="background-color: #3b82f6"></div><div style="background-color: #2563eb"></div><div style="background-color: #1d4ed8"></div><div style="background-color: #1e40af"></div><div style="background-color: #1e3a8a"></div><div style="background-color: #172554"></div></div>
pub const BLUE: Palette = Palette {
    c50: Color::from_u32(0xeff6ff),
    c100: Color::from_u32(0xdbeafe),
    c200: Color::from_u32(0xbfdbfe),
    c300: Color::from_u32(0x93c5fd),
    c400: Color::from_u32(0x60a5fa),
    c500: Color::from_u32(0x3b82f6),
    c600: Color::from_u32(0x2563eb),
    c700: Color::from_u32(0x1d4ed8),
    c800: Color::from_u32(0x1e40af),
    c900: Color::from_u32(0x1e3a8a),
    c950: Color::from_u32(0x172554),
};

#[rustfmt::skip]
/// <style>.palette div{width:22rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #eef2ff"></div><div style="background-color: #e0e7ff"></div><div style="background-color: #c7d2fe"></div><div style="background-color: #a5b4fc"></div><div style="background-color: #818cf8"></div><div style="background-color: #6366f1"></div><div style="background-color: #4f46e5"></div><div style="background-color: #4338ca"></div><div style="background-color: #3730a3"></div><div style="background-color: #312e81"></div><div style="background-color: #1e1b4b"></div></div>
pub const INDIGO: Palette = Palette {
    c50: Color::from_u32(0xeef2ff),
    c100: Color::from_u32(0xe0e7ff),
    c200: Color::from_u32(0xc7d2fe),
    c300: Color::from_u32(0xa5b4fc),
    c400: Color::from_u32(0x818cf8),
    c500: Color::from_u32(0x6366f1),
    c600: Color::from_u32(0x4f46e5),
    c700: Color::from_u32(0x4338ca),
    c800: Color::from_u32(0x3730a3),
    c900: Color::from_u32(0x312e81),
    c950: Color::from_u32(0x1e1b4b),
};

#[rustfmt::skip]
/// <style>.palette div{width:22rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #f5f3ff"></div><div style="background-color: #ede9fe"></div><div style="background-color: #ddd6fe"></div><div style="background-color: #c4b5fd"></div><div style="background-color: #a78bfa"></div><div style="background-color: #8b5cf6"></div><div style="background-color: #7c3aed"></div><div style="background-color: #6d28d9"></div><div style="background-color: #5b21b6"></div><div style="background-color: #4c1d95"></div><div style="background-color: #2e1065"></div></div>
pub const VIOLET: Palette = Palette {
    c50: Color::from_u32(0xf5f3ff),
    c100: Color::from_u32(0xede9fe),
    c200: Color::from_u32(0xddd6fe),
    c300: Color::from_u32(0xc4b5fd),
    c400: Color::from_u32(0xa78bfa),
    c500: Color::from_u32(0x8b5cf6),
    c600: Color::from_u32(0x7c3aed),
    c700: Color::from_u32(0x6d28d9),
    c800: Color::from_u32(0x5b21b6),
    c900: Color::from_u32(0x4c1d95),
    c950: Color::from_u32(0x2e1065),
};

#[rustfmt::skip]
/// <style>.palette div{width:22rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #faf5ff"></div><div style="background-color: #f3e8ff"></div><div style="background-color: #e9d5ff"></div><div style="background-color: #d8b4fe"></div><div style="background-color: #c084fc"></div><div style="background-color: #a855f7"></div><div style="background-color: #9333ea"></div><div style="background-color: #7e22ce"></div><div style="background-color: #6b21a8"></div><div style="background-color: #581c87"></div><div style="background-color: #3b0764"></div></div>
pub const PURPLE: Palette = Palette {
    c50: Color::from_u32(0xfaf5ff),
    c100: Color::from_u32(0xf3e8ff),
    c200: Color::from_u32(0xe9d5ff),
    c300: Color::from_u32(0xd8b4fe),
    c400: Color::from_u32(0xc084fc),
    c500: Color::from_u32(0xa855f7),
    c600: Color::from_u32(0x9333ea),
    c700: Color::from_u32(0x7e22ce),
    c800: Color::from_u32(0x6b21a8),
    c900: Color::from_u32(0x581c87),
    c950: Color::from_u32(0x3b0764),
};

#[rustfmt::skip]
/// <style>.palette div{width:22rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #fdf4ff"></div><div style="background-color: #fae8ff"></div><div style="background-color: #f5d0fe"></div><div style="background-color: #f0abfc"></div><div style="background-color: #e879f9"></div><div style="background-color: #d946ef"></div><div style="background-color: #c026d3"></div><div style="background-color: #a21caf"></div><div style="background-color: #86198f"></div><div style="background-color: #701a75"></div><div style="background-color: #4a044e"></div></div>
pub const FUCHSIA: Palette = Palette {
    c50: Color::from_u32(0xfdf4ff),
    c100: Color::from_u32(0xfae8ff),
    c200: Color::from_u32(0xf5d0fe),
    c300: Color::from_u32(0xf0abfc),
    c400: Color::from_u32(0xe879f9),
    c500: Color::from_u32(0xd946ef),
    c600: Color::from_u32(0xc026d3),
    c700: Color::from_u32(0xa21caf),
    c800: Color::from_u32(0x86198f),
    c900: Color::from_u32(0x701a75),
    c950: Color::from_u32(0x4a044e),
};

#[rustfmt::skip]
/// <style>.palette div{width:22rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #fdf2f8"></div><div style="background-color: #fce7f3"></div><div style="background-color: #fbcfe8"></div><div style="background-color: #f9a8d4"></div><div style="background-color: #f472b6"></div><div style="background-color: #ec4899"></div><div style="background-color: #db2777"></div><div style="background-color: #be185d"></div><div style="background-color: #9d174d"></div><div style="background-color: #831843"></div><div style="background-color: #500724"></div></div>
pub const PINK: Palette = Palette {
    c50: Color::from_u32(0xfdf2f8),
    c100: Color::from_u32(0xfce7f3),
    c200: Color::from_u32(0xfbcfe8),
    c300: Color::from_u32(0xf9a8d4),
    c400: Color::from_u32(0xf472b6),
    c500: Color::from_u32(0xec4899),
    c600: Color::from_u32(0xdb2777),
    c700: Color::from_u32(0xbe185d),
    c800: Color::from_u32(0x9d174d),
    c900: Color::from_u32(0x831843),
    c950: Color::from_u32(0x500724),
};

#[rustfmt::skip]
/// <style>.palette div{width:22rem;height:2rem}</style><div class="palette" style="display:flex;flex-direction:row"><div style="background-color: #fff1f2"></div><div style="background-color: #ffe4e6"></div><div style="background-color: #fecdd3"></div><div style="background-color: #fda4af"></div><div style="background-color: #fb7185"></div><div style="background-color: #f43f5e"></div><div style="background-color: #e11d48"></div><div style="background-color: #be123c"></div><div style="background-color: #9f1239"></div><div style="background-color: #881337"></div><div style="background-color: #4c0519"></div></div>
pub const ROSE: Palette = Palette {
    c50: Color::from_u32(0xfff1f2),
    c100: Color::from_u32(0xffe4e6),
    c200: Color::from_u32(0xfecdd3),
    c300: Color::from_u32(0xfda4af),
    c400: Color::from_u32(0xfb7185),
    c500: Color::from_u32(0xf43f5e),
    c600: Color::from_u32(0xe11d48),
    c700: Color::from_u32(0xbe123c),
    c800: Color::from_u32(0x9f1239),
    c900: Color::from_u32(0x881337),
    c950: Color::from_u32(0x4c0519),
};
