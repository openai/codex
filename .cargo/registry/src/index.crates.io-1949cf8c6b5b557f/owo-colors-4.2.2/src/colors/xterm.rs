macro_rules! xterm_colors {
    ($(
        $xterm_num:literal $name:ident ($r:literal, $g:literal, $b:literal)
    )*) => {

        pub(crate) mod dynamic {
            use core::fmt;

            #[allow(unused_imports)]
            use crate::OwoColorize;

            /// Available Xterm colors for use with [`OwoColorize::color`](OwoColorize::color)
            /// or [`OwoColorize::on_color`](OwoColorize::on_color)
            #[derive(Copy, Clone, Debug, PartialEq, Eq)]
            pub enum XtermColors {
                $(
                    #[allow(missing_docs)]
                    $name,
                )*
            }

            impl crate::private::Sealed for XtermColors {}

            impl crate::DynColor for XtermColors {
                fn fmt_ansi_fg(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    let color = match self {
                        $(
                            XtermColors::$name => concat!("\x1b[38;5;", stringify!($xterm_num), "m"),
                        )*
                    };

                    f.write_str(color)
                }

                fn fmt_ansi_bg(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    let color = match self {
                        $(
                            XtermColors::$name => concat!("\x1b[48;5;", stringify!($xterm_num), "m"),
                        )*
                    };

                    f.write_str(color)
                }

                fn fmt_raw_ansi_fg(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    let color = match self {
                        $(
                            XtermColors::$name => concat!("38;5;", stringify!($xterm_num)),
                        )*
                    };

                    f.write_str(color)
                }

                fn fmt_raw_ansi_bg(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    let color = match self {
                        $(
                            XtermColors::$name => concat!("48;5;", stringify!($xterm_num)),
                        )*
                    };

                    f.write_str(color)
                }

                #[doc(hidden)]
                fn get_dyncolors_fg(&self) -> crate::DynColors {
                    crate::DynColors::Xterm(*self)
                }

                #[doc(hidden)]
                fn get_dyncolors_bg(&self) -> crate::DynColors {
                    crate::DynColors::Xterm(*self)
                }
            }

            impl From<u8> for XtermColors {
                fn from(x: u8) -> Self {
                    match x {
                        $(
                            $xterm_num => XtermColors::$name,
                        )*
                    }
                }
            }

            impl From<XtermColors> for u8 {
                fn from(color: XtermColors) -> Self {
                    match color {
                        $(
                            XtermColors::$name => $xterm_num,
                        )*
                    }
                }
            }
        }

        $(
            #[allow(missing_docs)]
            pub struct $name;

            impl crate::private::Sealed for $name {}

            impl crate::Color for $name {
                const ANSI_FG: &'static str = concat!("\x1b[38;5;", stringify!($xterm_num), "m");
                const ANSI_BG: &'static str = concat!("\x1b[48;5;", stringify!($xterm_num), "m");

                const RAW_ANSI_BG: &'static str = concat!("48;5;", stringify!($xterm_num));
                const RAW_ANSI_FG: &'static str = concat!("38;5;", stringify!($xterm_num));

                #[doc(hidden)]
                type DynEquivalent = dynamic::XtermColors;

                #[doc(hidden)]
                const DYN_EQUIVALENT: Self::DynEquivalent = dynamic::XtermColors::$name;

                #[doc(hidden)]
                const DYN_COLORS_EQUIVALENT: crate::DynColors = crate::DynColors::Xterm(Self::DYN_EQUIVALENT);
            }
        )*
    };
}

xterm_colors! {
    0 UserBlack              (0,0,0)
    1 UserRed                (128,0,0)
    2 UserGreen              (0,128,0)
    3 UserYellow             (128,128,0)
    4 UserBlue               (0,0,128)
    5 UserMagenta            (128,0,128)
    6 UserCyan               (0,128,128)
    7 UserWhite              (192,192,192)
    8 UserBrightBlack        (128,128,128)
    9 UserBrightRed          (255,0,0)
   10 UserBrightGreen        (0,255,0)
   11 UserBrightYellow       (255,255,0)
   12 UserBrightBlue         (0,0,255)
   13 UserBrightMagenta      (255,0,255)
   14 UserBrightCyan         (0,255,255)
   15 UserBrightWhite        (255,255,255)
   16 Black                  (0,0,0)
   17 StratosBlue            (0,0,95)
   18 NavyBlue               (0,0,135)
   19 MidnightBlue           (0,0,175)
   20 DarkBlue               (0,0,215)
   21 Blue                   (0,0,255)
   22 CamaroneGreen          (0,95,0)
   23 BlueStone              (0,95,95)
   24 OrientBlue             (0,95,135)
   25 EndeavourBlue          (0,95,175)
   26 ScienceBlue            (0,95,215)
   27 BlueRibbon             (0,95,255)
   28 JapaneseLaurel         (0,135,0)
   29 DeepSeaGreen           (0,135,95)
   30 Teal                   (0,135,135)
   31 DeepCerulean           (0,135,175)
   32 LochmaraBlue           (0,135,215)
   33 AzureRadiance          (0,135,255)
   34 LightJapaneseLaurel    (0,175,0)
   35 Jade                   (0,175,95)
   36 PersianGreen           (0,175,135)
   37 BondiBlue              (0,175,175)
   38 Cerulean               (0,175,215)
   39 LightAzureRadiance     (0,175,255)
   40 DarkGreen              (0,215,0)
   41 Malachite              (0,215,95)
   42 CaribbeanGreen         (0,215,135)
   43 LightCaribbeanGreen    (0,215,175)
   44 RobinEggBlue           (0,215,215)
   45 Aqua                   (0,215,255)
   46 Green                  (0,255,0)
   47 DarkSpringGreen        (0,255,95)
   48 SpringGreen            (0,255,135)
   49 LightSpringGreen       (0,255,175)
   50 BrightTurquoise        (0,255,215)
   51 Cyan                   (0,255,255)
   52 Rosewood               (95,0,0)
   53 PompadourMagenta       (95,0,95)
   54 PigmentIndigo          (95,0,135)
   55 DarkPurple             (95,0,175)
   56 ElectricIndigo         (95,0,215)
   57 ElectricPurple         (95,0,255)
   58 VerdunGreen            (95,95,0)
   59 ScorpionOlive          (95,95,95)
   60 Lilac                  (95,95,135)
   61 ScampiIndigo           (95,95,175)
   62 Indigo                 (95,95,215)
   63 DarkCornflowerBlue     (95,95,255)
   64 DarkLimeade            (95,135,0)
   65 GladeGreen             (95,135,95)
   66 JuniperGreen           (95,135,135)
   67 HippieBlue             (95,135,175)
   68 HavelockBlue           (95,135,215)
   69 CornflowerBlue         (95,135,255)
   70 Limeade                (95,175,0)
   71 FernGreen              (95,175,95)
   72 SilverTree             (95,175,135)
   73 Tradewind              (95,175,175)
   74 ShakespeareBlue        (95,175,215)
   75 DarkMalibuBlue         (95,175,255)
   76 DarkBrightGreen        (95,215,0)
   77 DarkPastelGreen        (95,215,95)
   78 PastelGreen            (95,215,135)
   79 DownyTeal              (95,215,175)
   80 Viking                 (95,215,215)
   81 MalibuBlue             (95,215,255)
   82 BrightGreen            (95,255,0)
   83 DarkScreaminGreen      (95,255,95)
   84 ScreaminGreen          (95,255,135)
   85 DarkAquamarine         (95,255,175)
   86 Aquamarine             (95,255,215)
   87 LightAquamarine        (95,255,255)
   88 Maroon                 (135,0,0)
   89 DarkFreshEggplant      (135,0,95)
   90 LightFreshEggplant     (135,0,135)
   91 Purple                 (135,0,175)
   92 ElectricViolet         (135,0,215)
   93 LightElectricViolet    (135,0,255)
   94 Brown                  (135,95,0)
   95 CopperRose             (135,95,95)
   96 StrikemasterPurple     (135,95,135)
   97 DelugePurple           (135,95,175)
   98 DarkMediumPurple       (135,95,215)
   99 DarkHeliotropePurple   (135,95,255)
  100 Olive                  (135,135,0)
  101 ClayCreekOlive         (135,135,95)
  102 DarkGray               (135,135,135)
  103 WildBlueYonder         (135,135,175)
  104 ChetwodeBlue           (135,135,215)
  105 SlateBlue              (135,135,255)
  106 LightLimeade           (135,175,0)
  107 ChelseaCucumber        (135,175,95)
  108 BayLeaf                (135,175,135)
  109 GulfStream             (135,175,175)
  110 PoloBlue               (135,175,215)
  111 LightMalibuBlue        (135,175,255)
  112 Pistachio              (135,215,0)
  113 LightPastelGreen       (135,215,95)
  114 DarkFeijoaGreen        (135,215,135)
  115 VistaBlue              (135,215,175)
  116 Bermuda                (135,215,215)
  117 DarkAnakiwaBlue        (135,215,255)
  118 ChartreuseGreen        (135,255,0)
  119 LightScreaminGreen     (135,255,95)
  120 DarkMintGreen          (135,255,135)
  121 MintGreen              (135,255,175)
  122 LighterAquamarine      (135,255,215)
  123 AnakiwaBlue            (135,255,255)
  124 BrightRed              (175,0,0)
  125 DarkFlirt              (175,0,95)
  126 Flirt                  (175,0,135)
  127 LightFlirt             (175,0,175)
  128 DarkViolet             (175,0,215)
  129 BrightElectricViolet   (175,0,255)
  130 RoseofSharonOrange     (175,95,0)
  131 MatrixPink             (175,95,95)
  132 TapestryPink           (175,95,135)
  133 FuchsiaPink            (175,95,175)
  134 MediumPurple           (175,95,215)
  135 Heliotrope             (175,95,255)
  136 PirateGold             (175,135,0)
  137 MuesliOrange           (175,135,95)
  138 PharlapPink            (175,135,135)
  139 Bouquet                (175,135,175)
  140 Lavender               (175,135,215)
  141 LightHeliotrope        (175,135,255)
  142 BuddhaGold             (175,175,0)
  143 OliveGreen             (175,175,95)
  144 HillaryOlive           (175,175,135)
  145 SilverChalice          (175,175,175)
  146 WistfulLilac           (175,175,215)
  147 MelroseLilac           (175,175,255)
  148 RioGrandeGreen         (175,215,0)
  149 ConiferGreen           (175,215,95)
  150 Feijoa                 (175,215,135)
  151 PixieGreen             (175,215,175)
  152 JungleMist             (175,215,215)
  153 LightAnakiwaBlue       (175,215,255)
  154 Lime                   (175,255,0)
  155 GreenYellow            (175,255,95)
  156 LightMintGreen         (175,255,135)
  157 Celadon                (175,255,175)
  158 AeroBlue               (175,255,215)
  159 FrenchPassLightBlue    (175,255,255)
  160 GuardsmanRed           (215,0,0)
  161 RazzmatazzCerise       (215,0,95)
  162 MediumVioletRed        (215,0,135)
  163 HollywoodCerise        (215,0,175)
  164 DarkPurplePizzazz      (215,0,215)
  165 BrighterElectricViolet (215,0,255)
  166 TennOrange             (215,95,0)
  167 RomanOrange            (215,95,95)
  168 CranberryPink          (215,95,135)
  169 HopbushPink            (215,95,175)
  170 Orchid                 (215,95,215)
  171 LighterHeliotrope      (215,95,255)
  172 MangoTango             (215,135,0)
  173 Copperfield            (215,135,95)
  174 SeaPink                (215,135,135)
  175 CanCanPink             (215,135,175)
  176 LightOrchid            (215,135,215)
  177 BrightHeliotrope       (215,135,255)
  178 DarkCorn               (215,175,0)
  179 DarkTachaOrange        (215,175,95)
  180 TanBeige               (215,175,135)
  181 ClamShell              (215,175,175)
  182 ThistlePink            (215,175,215)
  183 Mauve                  (215,175,255)
  184 Corn                   (215,215,0)
  185 TachaOrange            (215,215,95)
  186 DecoOrange             (215,215,135)
  187 PaleGoldenrod          (215,215,175)
  188 AltoBeige              (215,215,215)
  189 FogPink                (215,215,255)
  190 ChartreuseYellow       (215,255,0)
  191 Canary                 (215,255,95)
  192 Honeysuckle            (215,255,135)
  193 ReefPaleYellow         (215,255,175)
  194 SnowyMint              (215,255,215)
  195 OysterBay              (215,255,255)
  196 Red                    (255,0,0)
  197 DarkRose               (255,0,95)
  198 Rose                   (255,0,135)
  199 LightHollywoodCerise   (255,0,175)
  200 PurplePizzazz          (255,0,215)
  201 Fuchsia                (255,0,255)
  202 BlazeOrange            (255,95,0)
  203 BittersweetOrange      (255,95,95)
  204 WildWatermelon         (255,95,135)
  205 DarkHotPink            (255,95,175)
  206 HotPink                (255,95,215)
  207 PinkFlamingo           (255,95,255)
  208 FlushOrange            (255,135,0)
  209 Salmon                 (255,135,95)
  210 VividTangerine         (255,135,135)
  211 PinkSalmon             (255,135,175)
  212 DarkLavenderRose       (255,135,215)
  213 BlushPink              (255,135,255)
  214 YellowSea              (255,175,0)
  215 TexasRose              (255,175,95)
  216 Tacao                  (255,175,135)
  217 Sundown                (255,175,175)
  218 CottonCandy            (255,175,215)
  219 LavenderRose           (255,175,255)
  220 Gold                   (255,215,0)
  221 Dandelion              (255,215,95)
  222 GrandisCaramel         (255,215,135)
  223 Caramel                (255,215,175)
  224 CosmosSalmon           (255,215,215)
  225 PinkLace               (255,215,255)
  226 Yellow                 (255,255,0)
  227 LaserLemon             (255,255,95)
  228 DollyYellow            (255,255,135)
  229 PortafinoYellow        (255,255,175)
  230 Cumulus                (255,255,215)
  231 White                  (255,255,255)
  232 DarkCodGray            (8,8,8)
  233 CodGray                (18,18,18)
  234 LightCodGray           (28,28,28)
  235 DarkMineShaft          (38,38,38)
  236 MineShaft              (48,48,48)
  237 LightMineShaft         (58,58,58)
  238 DarkTundora            (68,68,68)
  239 Tundora                (78,78,78)
  240 ScorpionGray           (88,88,88)
  241 DarkDoveGray           (98,98,98)
  242 DoveGray               (108,108,108)
  243 Boulder                (118,118,118)
  244 Gray                   (128,128,128)
  245 LightGray              (138,138,138)
  246 DustyGray              (148,148,148)
  247 NobelGray              (158,158,158)
  248 DarkSilverChalice      (168,168,168)
  249 LightSilverChalice     (178,178,178)
  250 DarkSilver             (188,188,188)
  251 Silver                 (198,198,198)
  252 DarkAlto               (208,208,208)
  253 Alto                   (218,218,218)
  254 Mercury                (228,228,228)
  255 GalleryGray            (238,238,238)
}
