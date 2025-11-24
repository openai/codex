//! PSP C type definitions
//!
//! These type declarations are not enough, as they must be ultimately resolved
//! by the linker. Crates that use these definitions must, somewhere in the
//! crate graph, include a stub provider crate such as the `psp` crate.

use crate::prelude::*;

pub type intmax_t = i64;
pub type uintmax_t = u64;

pub type size_t = usize;
pub type ptrdiff_t = isize;
pub type intptr_t = isize;
pub type uintptr_t = usize;
pub type ssize_t = isize;

pub type SceKernelVTimerHandler = unsafe extern "C" fn(
    uid: SceUid,
    arg1: *mut SceKernelSysClock,
    arg2: *mut SceKernelSysClock,
    arg3: *mut c_void,
) -> u32;

pub type SceKernelVTimerHandlerWide =
    unsafe extern "C" fn(uid: SceUid, arg1: i64, arg2: i64, arg3: *mut c_void) -> u32;

pub type SceKernelThreadEventHandler =
    unsafe extern "C" fn(mask: i32, thid: SceUid, common: *mut c_void) -> i32;

pub type SceKernelAlarmHandler = unsafe extern "C" fn(common: *mut c_void) -> u32;

pub type SceKernelCallbackFunction =
    unsafe extern "C" fn(arg1: i32, arg2: i32, arg: *mut c_void) -> i32;

pub type SceKernelThreadEntry = unsafe extern "C" fn(args: usize, argp: *mut c_void) -> i32;

pub type PowerCallback = extern "C" fn(unknown: i32, power_info: i32);

pub type IoPermissions = i32;

pub type UmdCallback = fn(unknown: i32, event: i32) -> i32;

pub type SceMpegRingbufferCb =
    Option<unsafe extern "C" fn(data: *mut c_void, num_packets: i32, param: *mut c_void) -> i32>;

pub type GuCallback = Option<extern "C" fn(id: i32, arg: *mut c_void)>;
pub type GuSwapBuffersCallback =
    Option<extern "C" fn(display: *mut *mut c_void, render: *mut *mut c_void)>;

pub type SceNetAdhocctlHandler =
    Option<unsafe extern "C" fn(flag: i32, error: i32, unknown: *mut c_void)>;

pub type AdhocMatchingCallback = Option<
    unsafe extern "C" fn(
        matching_id: i32,
        event: i32,
        mac: *mut u8,
        opt_len: i32,
        opt_data: *mut c_void,
    ),
>;

pub type SceNetApctlHandler = Option<
    unsafe extern "C" fn(oldState: i32, newState: i32, event: i32, error: i32, pArg: *mut c_void),
>;

pub type HttpMallocFunction = Option<unsafe extern "C" fn(size: usize) -> *mut c_void>;
pub type HttpReallocFunction =
    Option<unsafe extern "C" fn(p: *mut c_void, size: usize) -> *mut c_void>;
pub type HttpFreeFunction = Option<unsafe extern "C" fn(p: *mut c_void)>;
pub type HttpPasswordCB = Option<
    unsafe extern "C" fn(
        request: i32,
        auth_type: HttpAuthType,
        realm: *const u8,
        username: *mut u8,
        password: *mut u8,
        need_entity: i32,
        entity_body: *mut *mut u8,
        entity_size: *mut usize,
        save: *mut i32,
    ) -> i32,
>;

pub type socklen_t = u32;

e! {
    #[repr(u32)]
    pub enum AudioFormat {
        Stereo = 0,
        Mono = 0x10,
    }

    #[repr(u32)]
    pub enum DisplayMode {
        Lcd = 0,
    }

    #[repr(u32)]
    pub enum DisplayPixelFormat {
        Psm5650 = 0,
        Psm5551 = 1,
        Psm4444 = 2,
        Psm8888 = 3,
    }

    #[repr(u32)]
    pub enum DisplaySetBufSync {
        Immediate = 0,
        NextFrame = 1,
    }

    #[repr(i32)]
    pub enum AudioOutputFrequency {
        Khz48 = 48000,
        Khz44_1 = 44100,
        Khz32 = 32000,
        Khz24 = 24000,
        Khz22_05 = 22050,
        Khz16 = 16000,
        Khz12 = 12000,
        Khz11_025 = 11025,
        Khz8 = 8000,
    }

    #[repr(i32)]
    pub enum AudioInputFrequency {
        Khz44_1 = 44100,
        Khz22_05 = 22050,
        Khz11_025 = 11025,
    }

    #[repr(u32)]
    pub enum CtrlMode {
        Digital = 0,
        Analog,
    }

    #[repr(i32)]
    pub enum GeMatrixType {
        Bone0 = 0,
        Bone1,
        Bone2,
        Bone3,
        Bone4,
        Bone5,
        Bone6,
        Bone7,
        World,
        View,
        Projection,
        TexGen,
    }

    #[repr(i32)]
    pub enum GeListState {
        Done = 0,
        Queued,
        DrawingDone,
        StallReached,
        CancelDone,
    }

    #[repr(u8)]
    pub enum GeCommand {
        Nop = 0,
        Vaddr = 0x1,
        Iaddr = 0x2,
        Prim = 0x4,
        Bezier = 0x5,
        Spline = 0x6,
        BoundingBox = 0x7,
        Jump = 0x8,
        BJump = 0x9,
        Call = 0xa,
        Ret = 0xb,
        End = 0xc,
        Signal = 0xe,
        Finish = 0xf,
        Base = 0x10,
        VertexType = 0x12,
        OffsetAddr = 0x13,
        Origin = 0x14,
        Region1 = 0x15,
        Region2 = 0x16,
        LightingEnable = 0x17,
        LightEnable0 = 0x18,
        LightEnable1 = 0x19,
        LightEnable2 = 0x1a,
        LightEnable3 = 0x1b,
        DepthClampEnable = 0x1c,
        CullFaceEnable = 0x1d,
        TextureMapEnable = 0x1e,
        FogEnable = 0x1f,
        DitherEnable = 0x20,
        AlphaBlendEnable = 0x21,
        AlphaTestEnable = 0x22,
        ZTestEnable = 0x23,
        StencilTestEnable = 0x24,
        AntiAliasEnable = 0x25,
        PatchCullEnable = 0x26,
        ColorTestEnable = 0x27,
        LogicOpEnable = 0x28,
        BoneMatrixNumber = 0x2a,
        BoneMatrixData = 0x2b,
        MorphWeight0 = 0x2c,
        MorphWeight1 = 0x2d,
        MorphWeight2 = 0x2e,
        MorphWeight3 = 0x2f,
        MorphWeight4 = 0x30,
        MorphWeight5 = 0x31,
        MorphWeight6 = 0x32,
        MorphWeight7 = 0x33,
        PatchDivision = 0x36,
        PatchPrimitive = 0x37,
        PatchFacing = 0x38,
        WorldMatrixNumber = 0x3a,
        WorldMatrixData = 0x3b,
        ViewMatrixNumber = 0x3c,
        ViewMatrixData = 0x3d,
        ProjMatrixNumber = 0x3e,
        ProjMatrixData = 0x3f,
        TGenMatrixNumber = 0x40,
        TGenMatrixData = 0x41,
        ViewportXScale = 0x42,
        ViewportYScale = 0x43,
        ViewportZScale = 0x44,
        ViewportXCenter = 0x45,
        ViewportYCenter = 0x46,
        ViewportZCenter = 0x47,
        TexScaleU = 0x48,
        TexScaleV = 0x49,
        TexOffsetU = 0x4a,
        TexOffsetV = 0x4b,
        OffsetX = 0x4c,
        OffsetY = 0x4d,
        ShadeMode = 0x50,
        ReverseNormal = 0x51,
        MaterialUpdate = 0x53,
        MaterialEmissive = 0x54,
        MaterialAmbient = 0x55,
        MaterialDiffuse = 0x56,
        MaterialSpecular = 0x57,
        MaterialAlpha = 0x58,
        MaterialSpecularCoef = 0x5b,
        AmbientColor = 0x5c,
        AmbientAlpha = 0x5d,
        LightMode = 0x5e,
        LightType0 = 0x5f,
        LightType1 = 0x60,
        LightType2 = 0x61,
        LightType3 = 0x62,
        Light0X = 0x63,
        Light0Y,
        Light0Z,
        Light1X,
        Light1Y,
        Light1Z,
        Light2X,
        Light2Y,
        Light2Z,
        Light3X,
        Light3Y,
        Light3Z,
        Light0DirectionX = 0x6f,
        Light0DirectionY,
        Light0DirectionZ,
        Light1DirectionX,
        Light1DirectionY,
        Light1DirectionZ,
        Light2DirectionX,
        Light2DirectionY,
        Light2DirectionZ,
        Light3DirectionX,
        Light3DirectionY,
        Light3DirectionZ,
        Light0ConstantAtten = 0x7b,
        Light0LinearAtten,
        Light0QuadtraticAtten,
        Light1ConstantAtten,
        Light1LinearAtten,
        Light1QuadtraticAtten,
        Light2ConstantAtten,
        Light2LinearAtten,
        Light2QuadtraticAtten,
        Light3ConstantAtten,
        Light3LinearAtten,
        Light3QuadtraticAtten,
        Light0ExponentAtten = 0x87,
        Light1ExponentAtten,
        Light2ExponentAtten,
        Light3ExponentAtten,
        Light0CutoffAtten = 0x8b,
        Light1CutoffAtten,
        Light2CutoffAtten,
        Light3CutoffAtten,
        Light0Ambient = 0x8f,
        Light0Diffuse,
        Light0Specular,
        Light1Ambient,
        Light1Diffuse,
        Light1Specular,
        Light2Ambient,
        Light2Diffuse,
        Light2Specular,
        Light3Ambient,
        Light3Diffuse,
        Light3Specular,
        Cull = 0x9b,
        FrameBufPtr = 0x9c,
        FrameBufWidth = 0x9d,
        ZBufPtr = 0x9e,
        ZBufWidth = 0x9f,
        TexAddr0 = 0xa0,
        TexAddr1,
        TexAddr2,
        TexAddr3,
        TexAddr4,
        TexAddr5,
        TexAddr6,
        TexAddr7,
        TexBufWidth0 = 0xa8,
        TexBufWidth1,
        TexBufWidth2,
        TexBufWidth3,
        TexBufWidth4,
        TexBufWidth5,
        TexBufWidth6,
        TexBufWidth7,
        ClutAddr = 0xb0,
        ClutAddrUpper = 0xb1,
        TransferSrc,
        TransferSrcW,
        TransferDst,
        TransferDstW,
        TexSize0 = 0xb8,
        TexSize1,
        TexSize2,
        TexSize3,
        TexSize4,
        TexSize5,
        TexSize6,
        TexSize7,
        TexMapMode = 0xc0,
        TexShadeLs = 0xc1,
        TexMode = 0xc2,
        TexFormat = 0xc3,
        LoadClut = 0xc4,
        ClutFormat = 0xc5,
        TexFilter = 0xc6,
        TexWrap = 0xc7,
        TexLevel = 0xc8,
        TexFunc = 0xc9,
        TexEnvColor = 0xca,
        TexFlush = 0xcb,
        TexSync = 0xcc,
        Fog1 = 0xcd,
        Fog2 = 0xce,
        FogColor = 0xcf,
        TexLodSlope = 0xd0,
        FramebufPixFormat = 0xd2,
        ClearMode = 0xd3,
        Scissor1 = 0xd4,
        Scissor2 = 0xd5,
        MinZ = 0xd6,
        MaxZ = 0xd7,
        ColorTest = 0xd8,
        ColorRef = 0xd9,
        ColorTestmask = 0xda,
        AlphaTest = 0xdb,
        StencilTest = 0xdc,
        StencilOp = 0xdd,
        ZTest = 0xde,
        BlendMode = 0xdf,
        BlendFixedA = 0xe0,
        BlendFixedB = 0xe1,
        Dith0 = 0xe2,
        Dith1,
        Dith2,
        Dith3,
        LogicOp = 0xe6,
        ZWriteDisable = 0xe7,
        MaskRgb = 0xe8,
        MaskAlpha = 0xe9,
        TransferStart = 0xea,
        TransferSrcPos = 0xeb,
        TransferDstPos = 0xec,
        TransferSize = 0xee,
        Vscx = 0xf0,
        Vscy = 0xf1,
        Vscz = 0xf2,
        Vtcs = 0xf3,
        Vtct = 0xf4,
        Vtcq = 0xf5,
        Vcv = 0xf6,
        Vap = 0xf7,
        Vfc = 0xf8,
        Vscv = 0xf9,

        Unknown03 = 0x03,
        Unknown0D = 0x0d,
        Unknown11 = 0x11,
        Unknown29 = 0x29,
        Unknown34 = 0x34,
        Unknown35 = 0x35,
        Unknown39 = 0x39,
        Unknown4E = 0x4e,
        Unknown4F = 0x4f,
        Unknown52 = 0x52,
        Unknown59 = 0x59,
        Unknown5A = 0x5a,
        UnknownB6 = 0xb6,
        UnknownB7 = 0xb7,
        UnknownD1 = 0xd1,
        UnknownED = 0xed,
        UnknownEF = 0xef,
        UnknownFA = 0xfa,
        UnknownFB = 0xfb,
        UnknownFC = 0xfc,
        UnknownFD = 0xfd,
        UnknownFE = 0xfe,
        NopFF = 0xff,
    }

    #[repr(i32)]
    pub enum SceSysMemPartitionId {
        SceKernelUnknownPartition = 0,
        SceKernelPrimaryKernelPartition = 1,
        SceKernelPrimaryUserPartition = 2,
        SceKernelOtherKernelPartition1 = 3,
        SceKernelOtherKernelPartition2 = 4,
        SceKernelVshellPARTITION = 5,
        SceKernelScUserPartition = 6,
        SceKernelMeUserPartition = 7,
        SceKernelExtendedScKernelPartition = 8,
        SceKernelExtendedSc2KernelPartition = 9,
        SceKernelExtendedMeKernelPartition = 10,
        SceKernelVshellKernelPartition = 11,
        SceKernelExtendedKernelPartition = 12,
    }

    #[repr(i32)]
    pub enum SceSysMemBlockTypes {
        Low = 0,
        High,
        Addr,
    }

    #[repr(u32)]
    pub enum Interrupt {
        Gpio = 4,
        Ata = 5,
        Umd = 6,
        Mscm0 = 7,
        Wlan = 8,
        Audio = 10,
        I2c = 12,
        Sircs = 14,
        Systimer0 = 15,
        Systimer1 = 16,
        Systimer2 = 17,
        Systimer3 = 18,
        Thread0 = 19,
        Nand = 20,
        Dmacplus = 21,
        Dma0 = 22,
        Dma1 = 23,
        Memlmd = 24,
        Ge = 25,
        Vblank = 30,
        Mecodec = 31,
        Hpremote = 36,
        Mscm1 = 60,
        Mscm2 = 61,
        Thread1 = 65,
        Interrupt = 66,
    }

    #[repr(u32)]
    pub enum SubInterrupt {
        Gpio = Interrupt::Gpio as u32,
        Ata = Interrupt::Ata as u32,
        Umd = Interrupt::Umd as u32,
        Dmacplus = Interrupt::Dmacplus as u32,
        Ge = Interrupt::Ge as u32,
        Display = Interrupt::Vblank as u32,
    }

    #[repr(u32)]
    pub enum SceKernelIdListType {
        Thread = 1,
        Semaphore = 2,
        EventFlag = 3,
        Mbox = 4,
        Vpl = 5,
        Fpl = 6,
        Mpipe = 7,
        Callback = 8,
        ThreadEventHandler = 9,
        Alarm = 10,
        VTimer = 11,
        SleepThread = 64,
        DelayThread = 65,
        SuspendThread = 66,
        DormantThread = 67,
    }

    #[repr(i32)]
    pub enum UsbCamResolution {
        Px160_120 = 0,
        Px176_144 = 1,
        Px320_240 = 2,
        Px352_288 = 3,
        Px640_480 = 4,
        Px1024_768 = 5,
        Px1280_960 = 6,
        Px480_272 = 7,
        Px360_272 = 8,
    }

    #[repr(i32)]
    pub enum UsbCamResolutionEx {
        Px160_120 = 0,
        Px176_144 = 1,
        Px320_240 = 2,
        Px352_288 = 3,
        Px360_272 = 4,
        Px480_272 = 5,
        Px640_480 = 6,
        Px1024_768 = 7,
        Px1280_960 = 8,
    }

    #[repr(i32)]
    pub enum UsbCamDelay {
        NoDelay = 0,
        Delay10Sec = 1,
        Delay20Sec = 2,
        Delay30Sec = 3,
    }

    #[repr(i32)]
    pub enum UsbCamFrameRate {
        Fps3_75 = 0,
        Fps5 = 1,
        Fps7_5 = 2,
        Fps10 = 3,
        Fps15 = 4,
        Fps20 = 5,
        Fps30 = 6,
        Fps60 = 7,
    }

    #[repr(i32)]
    pub enum UsbCamWb {
        Auto = 0,
        Daylight = 1,
        Fluorescent = 2,
        Incadescent = 3,
    }

    #[repr(i32)]
    pub enum UsbCamEffectMode {
        Normal = 0,
        Negative = 1,
        Blackwhite = 2,
        Sepia = 3,
        Blue = 4,
        Red = 5,
        Green = 6,
    }

    #[repr(i32)]
    pub enum UsbCamEvLevel {
        Pos2_0 = 0,
        Pos1_7 = 1,
        Pos1_5 = 2,
        Pos1_3 = 3,
        Pos1_0 = 4,
        Pos0_7 = 5,
        Pos0_5 = 6,
        Pos0_3 = 7,
        Zero = 8,
        Neg0_3,
        Neg0_5,
        Neg0_7,
        Neg1_0,
        Neg1_3,
        Neg1_5,
        Neg1_7,
        Neg2_0,
    }

    #[repr(i32)]
    pub enum RtcCheckValidError {
        InvalidYear = -1,
        InvalidMonth = -2,
        InvalidDay = -3,
        InvalidHour = -4,
        InvalidMinutes = -5,
        InvalidSeconds = -6,
        InvalidMicroseconds = -7,
    }

    #[repr(u32)]
    pub enum PowerTick {
        All = 0,
        Suspend = 1,
        Display = 6,
    }

    #[repr(u32)]
    pub enum IoAssignPerms {
        RdWr = 0,
        RdOnly = 1,
    }

    #[repr(u32)]
    pub enum IoWhence {
        Set = 0,
        Cur = 1,
        End = 2,
    }

    #[repr(u32)]
    pub enum UmdType {
        Game = 0x10,
        Video = 0x20,
        Audio = 0x40,
    }

    #[repr(u32)]
    pub enum GuPrimitive {
        Points = 0,
        Lines = 1,
        LineStrip = 2,
        Triangles = 3,
        TriangleStrip = 4,
        TriangleFan = 5,
        Sprites = 6,
    }

    #[repr(u32)]
    pub enum PatchPrimitive {
        Points = 0,
        LineStrip = 2,
        TriangleStrip = 4,
    }

    #[repr(u32)]
    pub enum GuState {
        AlphaTest = 0,
        DepthTest = 1,
        ScissorTest = 2,
        StencilTest = 3,
        Blend = 4,
        CullFace = 5,
        Dither = 6,
        Fog = 7,
        ClipPlanes = 8,
        Texture2D = 9,
        Lighting = 10,
        Light0 = 11,
        Light1 = 12,
        Light2 = 13,
        Light3 = 14,
        LineSmooth = 15,
        PatchCullFace = 16,
        ColorTest = 17,
        ColorLogicOp = 18,
        FaceNormalReverse = 19,
        PatchFace = 20,
        Fragment2X = 21,
    }

    #[repr(u32)]
    pub enum MatrixMode {
        Projection = 0,
        View = 1,
        Model = 2,
        Texture = 3,
    }

    #[repr(u32)]
    pub enum TexturePixelFormat {
        Psm5650 = 0,
        Psm5551 = 1,
        Psm4444 = 2,
        Psm8888 = 3,
        PsmT4 = 4,
        PsmT8 = 5,
        PsmT16 = 6,
        PsmT32 = 7,
        PsmDxt1 = 8,
        PsmDxt3 = 9,
        PsmDxt5 = 10,
    }

    #[repr(u32)]
    pub enum SplineMode {
        FillFill = 0,
        OpenFill = 1,
        FillOpen = 2,
        OpenOpen = 3,
    }

    #[repr(u32)]
    pub enum ShadingModel {
        Flat = 0,
        Smooth = 1,
    }

    #[repr(u32)]
    pub enum LogicalOperation {
        Clear = 0,
        And = 1,
        AndReverse = 2,
        Copy = 3,
        AndInverted = 4,
        Noop = 5,
        Xor = 6,
        Or = 7,
        Nor = 8,
        Equiv = 9,
        Inverted = 10,
        OrReverse = 11,
        CopyInverted = 12,
        OrInverted = 13,
        Nand = 14,
        Set = 15,
    }

    #[repr(u32)]
    pub enum TextureFilter {
        Nearest = 0,
        Linear = 1,
        NearestMipmapNearest = 4,
        LinearMipmapNearest = 5,
        NearestMipmapLinear = 6,
        LinearMipmapLinear = 7,
    }

    #[repr(u32)]
    pub enum TextureMapMode {
        TextureCoords = 0,
        TextureMatrix = 1,
        EnvironmentMap = 2,
    }

    #[repr(u32)]
    pub enum TextureLevelMode {
        Auto = 0,
        Const = 1,
        Slope = 2,
    }

    #[repr(u32)]
    pub enum TextureProjectionMapMode {
        Position = 0,
        Uv = 1,
        NormalizedNormal = 2,
        Normal = 3,
    }

    #[repr(u32)]
    pub enum GuTexWrapMode {
        Repeat = 0,
        Clamp = 1,
    }

    #[repr(u32)]
    pub enum FrontFaceDirection {
        Clockwise = 0,
        CounterClockwise = 1,
    }

    #[repr(u32)]
    pub enum AlphaFunc {
        Never = 0,
        Always,
        Equal,
        NotEqual,
        Less,
        LessOrEqual,
        Greater,
        GreaterOrEqual,
    }

    #[repr(u32)]
    pub enum StencilFunc {
        Never = 0,
        Always,
        Equal,
        NotEqual,
        Less,
        LessOrEqual,
        Greater,
        GreaterOrEqual,
    }

    #[repr(u32)]
    pub enum ColorFunc {
        Never = 0,
        Always,
        Equal,
        NotEqual,
    }

    #[repr(u32)]
    pub enum DepthFunc {
        Never = 0,
        Always,
        Equal,
        NotEqual,
        Less,
        LessOrEqual,
        Greater,
        GreaterOrEqual,
    }

    #[repr(u32)]
    pub enum TextureEffect {
        Modulate = 0,
        Decal = 1,
        Blend = 2,
        Replace = 3,
        Add = 4,
    }

    #[repr(u32)]
    pub enum TextureColorComponent {
        Rgb = 0,
        Rgba = 1,
    }

    #[repr(u32)]
    pub enum MipmapLevel {
        None = 0,
        Level1,
        Level2,
        Level3,
        Level4,
        Level5,
        Level6,
        Level7,
    }

    #[repr(u32)]
    pub enum BlendOp {
        Add = 0,
        Subtract = 1,
        ReverseSubtract = 2,
        Min = 3,
        Max = 4,
        Abs = 5,
    }

    #[repr(u32)]
    pub enum BlendSrc {
        SrcColor = 0,
        OneMinusSrcColor = 1,
        SrcAlpha = 2,
        OneMinusSrcAlpha = 3,
        Fix = 10,
    }

    #[repr(u32)]
    pub enum BlendDst {
        DstColor = 0,
        OneMinusDstColor = 1,
        DstAlpha = 4,
        OneMinusDstAlpha = 5,
        Fix = 10,
    }

    #[repr(u32)]
    pub enum StencilOperation {
        Keep = 0,
        Zero = 1,
        Replace = 2,
        Invert = 3,
        Incr = 4,
        Decr = 5,
    }

    #[repr(u32)]
    pub enum LightMode {
        SingleColor = 0,
        SeparateSpecularColor = 1,
    }

    #[repr(u32)]
    pub enum LightType {
        Directional = 0,
        Pointlight = 1,
        Spotlight = 2,
    }

    #[repr(u32)]
    pub enum GuContextType {
        Direct = 0,
        Call = 1,
        Send = 2,
    }

    #[repr(u32)]
    pub enum GuQueueMode {
        Tail = 0,
        Head = 1,
    }

    #[repr(u32)]
    pub enum GuSyncMode {
        Finish = 0,
        Signal = 1,
        Done = 2,
        List = 3,
        Send = 4,
    }

    #[repr(u32)]
    pub enum GuSyncBehavior {
        Wait = 0,
        NoWait = 1,
    }

    #[repr(u32)]
    pub enum GuCallbackId {
        Signal = 1,
        Finish = 4,
    }

    #[repr(u32)]
    pub enum SignalBehavior {
        Suspend = 1,
        Continue = 2,
    }

    #[repr(u32)]
    pub enum ClutPixelFormat {
        Psm5650 = 0,
        Psm5551 = 1,
        Psm4444 = 2,
        Psm8888 = 3,
    }

    #[repr(C)]
    pub enum KeyType {
        Directory = 1,
        Integer = 2,
        String = 3,
        Bytes = 4,
    }

    #[repr(u32)]
    pub enum UtilityMsgDialogMode {
        Error,
        Text,
    }

    #[repr(u32)]
    pub enum UtilityMsgDialogPressed {
        Unknown1,
        Yes,
        No,
        Back,
    }

    #[repr(u32)]
    pub enum UtilityDialogButtonAccept {
        Circle,
        Cross,
    }

    #[repr(u32)]
    pub enum SceUtilityOskInputLanguage {
        Default,
        Japanese,
        English,
        French,
        Spanish,
        German,
        Italian,
        Dutch,
        Portugese,
        Russian,
        Korean,
    }

    #[repr(u32)]
    pub enum SceUtilityOskInputType {
        All,
        LatinDigit,
        LatinSymbol,
        LatinLowercase = 4,
        LatinUppercase = 8,
        JapaneseDigit = 0x100,
        JapaneseSymbol = 0x200,
        JapaneseLowercase = 0x400,
        JapaneseUppercase = 0x800,
        JapaneseHiragana = 0x1000,
        JapaneseHalfWidthKatakana = 0x2000,
        JapaneseKatakana = 0x4000,
        JapaneseKanji = 0x8000,
        RussianLowercase = 0x10000,
        RussianUppercase = 0x20000,
        Korean = 0x40000,
        Url = 0x80000,
    }

    #[repr(u32)]
    pub enum SceUtilityOskState {
        None,
        Initializing,
        Initialized,
        Visible,
        Quit,
        Finished,
    }

    #[repr(u32)]
    pub enum SceUtilityOskResult {
        Unchanged,
        Cancelled,
        Changed,
    }

    #[repr(u32)]
    pub enum SystemParamLanguage {
        Japanese,
        English,
        French,
        Spanish,
        German,
        Italian,
        Dutch,
        Portugese,
        Russian,
        Korean,
        ChineseTraditional,
        ChineseSimplified,
    }

    #[repr(u32)]
    pub enum SystemParamId {
        StringNickname = 1,
        AdhocChannel,
        WlanPowerSave,
        DateFormat,
        TimeFormat,
        Timezone,
        DaylightSavings,
        Language,
        Unknown,
    }

    #[repr(u32)]
    pub enum SystemParamAdhocChannel {
        ChannelAutomatic = 0,
        Channel1 = 1,
        Channel6 = 6,
        Channel11 = 11,
    }

    #[repr(u32)]
    pub enum SystemParamWlanPowerSaveState {
        Off,
        On,
    }

    #[repr(u32)]
    pub enum SystemParamDateFormat {
        YYYYMMDD,
        MMDDYYYY,
        DDMMYYYY,
    }

    #[repr(u32)]
    pub enum SystemParamTimeFormat {
        Hour24,
        Hour12,
    }

    #[repr(u32)]
    pub enum SystemParamDaylightSavings {
        Std,
        Dst,
    }

    #[repr(u32)]
    pub enum AvModule {
        AvCodec,
        SasCore,
        Atrac3Plus,
        MpegBase,
        Mp3,
        Vaudio,
        Aac,
        G729,
    }

    #[repr(u32)]
    pub enum Module {
        NetCommon = 0x100,
        NetAdhoc,
        NetInet,
        NetParseUri,
        NetHttp,
        NetSsl,

        UsbPspCm = 0x200,
        UsbMic,
        UsbCam,
        UsbGps,

        AvCodec = 0x300,
        AvSascore,
        AvAtrac3Plus,
        AvMpegBase,
        AvMp3,
        AvVaudio,
        AvAac,
        AvG729,

        NpCommon = 0x400,
        NpService,
        NpMatching2,
        NpDrm = 0x500,

        Irda = 0x600,
    }

    #[repr(u32)]
    pub enum NetModule {
        NetCommon = 1,
        NetAdhoc,
        NetInet,
        NetParseUri,
        NetHttp,
        NetSsl,
    }

    #[repr(u32)]
    pub enum UsbModule {
        UsbPspCm = 1,
        UsbAcc,
        UsbMic,
        UsbCam,
        UsbGps,
    }

    #[repr(u32)]
    pub enum NetParam {
        Name,
        Ssid,
        Secure,
        WepKey,
        IsStaticIp,
        Ip,
        NetMask,
        Route,
        ManualDns,
        PrimaryDns,
        SecondaryDns,
        ProxyUser,
        ProxyPass,
        UseProxy,
        ProxyServer,
        ProxyPort,
        Unknown1,
        Unknown2,
    }

    #[repr(u32)]
    pub enum UtilityNetconfAction {
        ConnectAP,
        DisplayStatus,
        ConnectAdhoc,
    }

    #[repr(u32)]
    pub enum UtilitySavedataMode {
        AutoLoad,
        AutoSave,
        Load,
        Save,
        ListLoad,
        ListSave,
        ListDelete,
        Delete,
    }

    #[repr(u32)]
    pub enum UtilitySavedataFocus {
        Unknown1,
        FirstList,
        LastList,
        Latest,
        Oldest,
        Unknown2,
        Unknown3,
        FirstEmpty,
        LastEmpty,
    }

    #[repr(u32)]
    pub enum UtilityGameSharingMode {
        Single = 1,
        Multiple,
    }

    #[repr(u32)]
    pub enum UtilityGameSharingDataType {
        File = 1,
        Memory,
    }

    #[repr(u32)]
    pub enum UtilityHtmlViewerInterfaceMode {
        Full,
        Limited,
        None,
    }

    #[repr(u32)]
    pub enum UtilityHtmlViewerCookieMode {
        Disabled = 0,
        Enabled,
        Confirm,
        Default,
    }

    #[repr(u32)]
    pub enum UtilityHtmlViewerTextSize {
        Large,
        Normal,
        Small,
    }

    #[repr(u32)]
    pub enum UtilityHtmlViewerDisplayMode {
        Normal,
        Fit,
        SmartFit,
    }

    #[repr(u32)]
    pub enum UtilityHtmlViewerConnectMode {
        Last,
        ManualOnce,
        ManualAll,
    }

    #[repr(u32)]
    pub enum UtilityHtmlViewerDisconnectMode {
        Enable,
        Disable,
        Confirm,
    }

    #[repr(u32)]
    pub enum ScePspnetAdhocPtpState {
        Closed,
        Listen,
        SynSent,
        SynReceived,
        Established,
    }

    #[repr(u32)]
    pub enum AdhocMatchingMode {
        Host = 1,
        Client,
        Ptp,
    }

    #[repr(u32)]
    pub enum ApctlState {
        Disconnected,
        Scanning,
        Joining,
        GettingIp,
        GotIp,
        EapAuth,
        KeyExchange,
    }

    #[repr(u32)]
    pub enum ApctlEvent {
        ConnectRequest,
        ScanRequest,
        ScanComplete,
        Established,
        GetIp,
        DisconnectRequest,
        Error,
        Info,
        EapAuth,
        KeyExchange,
        Reconnect,
    }

    #[repr(u32)]
    pub enum ApctlInfo {
        ProfileName,
        Bssid,
        Ssid,
        SsidLength,
        SecurityType,
        Strength,
        Channel,
        PowerSave,
        Ip,
        SubnetMask,
        Gateway,
        PrimaryDns,
        SecondaryDns,
        UseProxy,
        ProxyUrl,
        ProxyPort,
        EapType,
        StartBrowser,
        Wifisp,
    }

    #[repr(u32)]
    pub enum ApctlInfoSecurityType {
        None,
        Wep,
        Wpa,
    }

    #[repr(u32)]
    pub enum HttpMethod {
        Get,
        Post,
        Head,
    }

    #[repr(u32)]
    pub enum HttpAuthType {
        Basic,
        Digest,
    }
}

s_paren! {
    #[repr(transparent)]
    pub struct SceUid(pub i32);

    #[repr(transparent)]
    #[allow(dead_code)]
    pub struct SceMpeg(*mut *mut c_void);

    #[repr(transparent)]
    #[allow(dead_code)]
    pub struct SceMpegStream(*mut c_void);

    #[repr(transparent)]
    pub struct Mp3Handle(pub i32);

    #[repr(transparent)]
    #[allow(dead_code)]
    pub struct RegHandle(u32);
}

s! {
    pub struct sockaddr {
        pub sa_len: u8,
        pub sa_family: u8,
        pub sa_data: [u8; 14],
    }

    pub struct in_addr {
        pub s_addr: u32,
    }

    pub struct AudioInputParams {
        pub unknown1: i32,
        pub gain: i32,
        pub unknown2: i32,
        pub unknown3: i32,
        pub unknown4: i32,
        pub unknown5: i32,
    }

    pub struct Atrac3BufferInfo {
        pub puc_write_position_first_buf: *mut u8,
        pub ui_writable_byte_first_buf: u32,
        pub ui_min_write_byte_first_buf: u32,
        pub ui_read_position_first_buf: u32,
        pub puc_write_position_second_buf: *mut u8,
        pub ui_writable_byte_second_buf: u32,
        pub ui_min_write_byte_second_buf: u32,
        pub ui_read_position_second_buf: u32,
    }

    pub struct SceCtrlData {
        pub timestamp: u32,
        pub buttons: i32,
        pub lx: u8,
        pub ly: u8,
        pub rsrv: [u8; 6],
    }

    pub struct SceCtrlLatch {
        pub ui_make: u32,
        pub ui_break: u32,
        pub ui_press: u32,
        pub ui_release: u32,
    }

    pub struct GeStack {
        pub stack: [u32; 8],
    }

    // FIXME(1.0): This should not implement `PartialEq`
    #[allow(unpredictable_function_pointer_comparisons)]
    pub struct GeCallbackData {
        pub signal_func: Option<extern "C" fn(id: i32, arg: *mut c_void)>,
        pub signal_arg: *mut c_void,
        pub finish_func: Option<extern "C" fn(id: i32, arg: *mut c_void)>,
        pub finish_arg: *mut c_void,
    }

    pub struct GeListArgs {
        pub size: u32,
        pub context: *mut GeContext,
        pub num_stacks: u32,
        pub stacks: *mut GeStack,
    }

    pub struct GeBreakParam {
        pub buf: [u32; 4],
    }

    pub struct SceKernelLoadExecParam {
        pub size: usize,
        pub args: usize,
        pub argp: *mut c_void,
        pub key: *const u8,
    }

    pub struct timeval {
        pub tv_sec: i32,
        pub tv_usec: i32,
    }

    pub struct timezone {
        pub tz_minutes_west: i32,
        pub tz_dst_time: i32,
    }

    pub struct IntrHandlerOptionParam {
        size: i32,
        entry: u32,
        common: u32,
        gp: u32,
        intr_code: u16,
        sub_count: u16,
        intr_level: u16,
        enabled: u16,
        calls: u32,
        field_1c: u32,
        total_clock_lo: u32,
        total_clock_hi: u32,
        min_clock_lo: u32,
        min_clock_hi: u32,
        max_clock_lo: u32,
        max_clock_hi: u32,
    }

    pub struct SceKernelLMOption {
        pub size: usize,
        pub m_pid_text: SceUid,
        pub m_pid_data: SceUid,
        pub flags: u32,
        pub position: u8,
        pub access: u8,
        pub c_reserved: [u8; 2usize],
    }

    pub struct SceKernelSMOption {
        pub size: usize,
        pub m_pid_stack: SceUid,
        pub stack_size: usize,
        pub priority: i32,
        pub attribute: u32,
    }

    pub struct SceKernelModuleInfo {
        pub size: usize,
        pub n_segment: u8,
        pub reserved: [u8; 3usize],
        pub segment_addr: [i32; 4usize],
        pub segment_size: [i32; 4usize],
        pub entry_addr: u32,
        pub gp_value: u32,
        pub text_addr: u32,
        pub text_size: u32,
        pub data_size: u32,
        pub bss_size: u32,
        pub attribute: u16,
        pub version: [u8; 2usize],
        pub name: [u8; 28usize],
    }

    pub struct DebugProfilerRegs {
        pub enable: u32,
        pub systemck: u32,
        pub cpuck: u32,
        pub internal: u32,
        pub memory: u32,
        pub copz: u32,
        pub vfpu: u32,
        pub sleep: u32,
        pub bus_access: u32,
        pub uncached_load: u32,
        pub uncached_store: u32,
        pub cached_load: u32,
        pub cached_store: u32,
        pub i_miss: u32,
        pub d_miss: u32,
        pub d_writeback: u32,
        pub cop0_inst: u32,
        pub fpu_inst: u32,
        pub vfpu_inst: u32,
        pub local_bus: u32,
    }

    pub struct SceKernelSysClock {
        pub low: u32,
        pub hi: u32,
    }

    pub struct SceKernelThreadOptParam {
        pub size: usize,
        pub stack_mpid: SceUid,
    }

    // FIXME(1.0): This should not implement `PartialEq`
    #[allow(unpredictable_function_pointer_comparisons)]
    pub struct SceKernelThreadInfo {
        pub size: usize,
        pub name: [u8; 32],
        pub attr: u32,
        pub status: i32,
        pub entry: SceKernelThreadEntry,
        pub stack: *mut c_void,
        pub stack_size: i32,
        pub gp_reg: *mut c_void,
        pub init_priority: i32,
        pub current_priority: i32,
        pub wait_type: i32,
        pub wait_id: SceUid,
        pub wakeup_count: i32,
        pub exit_status: i32,
        pub run_clocks: SceKernelSysClock,
        pub intr_preempt_count: u32,
        pub thread_preempt_count: u32,
        pub release_count: u32,
    }

    pub struct SceKernelThreadRunStatus {
        pub size: usize,
        pub status: i32,
        pub current_priority: i32,
        pub wait_type: i32,
        pub wait_id: i32,
        pub wakeup_count: i32,
        pub run_clocks: SceKernelSysClock,
        pub intr_preempt_count: u32,
        pub thread_preempt_count: u32,
        pub release_count: u32,
    }

    pub struct SceKernelSemaOptParam {
        pub size: usize,
    }

    pub struct SceKernelSemaInfo {
        pub size: usize,
        pub name: [u8; 32],
        pub attr: u32,
        pub init_count: i32,
        pub current_count: i32,
        pub max_count: i32,
        pub num_wait_threads: i32,
    }

    pub struct SceKernelEventFlagInfo {
        pub size: usize,
        pub name: [u8; 32],
        pub attr: u32,
        pub init_pattern: u32,
        pub current_pattern: u32,
        pub num_wait_threads: i32,
    }

    pub struct SceKernelEventFlagOptParam {
        pub size: usize,
    }

    pub struct SceKernelMbxOptParam {
        pub size: usize,
    }

    pub struct SceKernelMbxInfo {
        pub size: usize,
        pub name: [u8; 32usize],
        pub attr: u32,
        pub num_wait_threads: i32,
        pub num_messages: i32,
        pub first_message: *mut c_void,
    }

    // FIXME(1.0): This should not implement `PartialEq`
    #[allow(unpredictable_function_pointer_comparisons)]
    pub struct SceKernelVTimerInfo {
        pub size: usize,
        pub name: [u8; 32],
        pub active: i32,
        pub base: SceKernelSysClock,
        pub current: SceKernelSysClock,
        pub schedule: SceKernelSysClock,
        pub handler: SceKernelVTimerHandler,
        pub common: *mut c_void,
    }

    // FIXME(1.0): This should not implement `PartialEq`
    #[allow(unpredictable_function_pointer_comparisons)]
    pub struct SceKernelThreadEventHandlerInfo {
        pub size: usize,
        pub name: [u8; 32],
        pub thread_id: SceUid,
        pub mask: i32,
        pub handler: SceKernelThreadEventHandler,
        pub common: *mut c_void,
    }

    // FIXME(1.0): This should not implement `PartialEq`
    #[allow(unpredictable_function_pointer_comparisons)]
    pub struct SceKernelAlarmInfo {
        pub size: usize,
        pub schedule: SceKernelSysClock,
        pub handler: SceKernelAlarmHandler,
        pub common: *mut c_void,
    }

    pub struct SceKernelSystemStatus {
        pub size: usize,
        pub status: u32,
        pub idle_clocks: SceKernelSysClock,
        pub comes_out_of_idle_count: u32,
        pub thread_switch_count: u32,
        pub vfpu_switch_count: u32,
    }

    pub struct SceKernelMppInfo {
        pub size: usize,
        pub name: [u8; 32],
        pub attr: u32,
        pub buf_size: i32,
        pub free_size: i32,
        pub num_send_wait_threads: i32,
        pub num_receive_wait_threads: i32,
    }

    pub struct SceKernelVplOptParam {
        pub size: usize,
    }

    pub struct SceKernelVplInfo {
        pub size: usize,
        pub name: [u8; 32],
        pub attr: u32,
        pub pool_size: i32,
        pub free_size: i32,
        pub num_wait_threads: i32,
    }

    pub struct SceKernelFplOptParam {
        pub size: usize,
    }

    pub struct SceKernelFplInfo {
        pub size: usize,
        pub name: [u8; 32usize],
        pub attr: u32,
        pub block_size: i32,
        pub num_blocks: i32,
        pub free_blocks: i32,
        pub num_wait_threads: i32,
    }

    pub struct SceKernelVTimerOptParam {
        pub size: usize,
    }

    // FIXME(1.0): This should not implement `PartialEq`
    #[allow(unpredictable_function_pointer_comparisons)]
    pub struct SceKernelCallbackInfo {
        pub size: usize,
        pub name: [u8; 32usize],
        pub thread_id: SceUid,
        pub callback: SceKernelCallbackFunction,
        pub common: *mut c_void,
        pub notify_count: i32,
        pub notify_arg: i32,
    }

    pub struct UsbCamSetupStillParam {
        pub size: i32,
        pub resolution: UsbCamResolution,
        pub jpeg_size: i32,
        pub reverse_flags: i32,
        pub delay: UsbCamDelay,
        pub comp_level: i32,
    }

    pub struct UsbCamSetupStillExParam {
        pub size: i32,
        pub unk: u32,
        pub resolution: UsbCamResolutionEx,
        pub jpeg_size: i32,
        pub comp_level: i32,
        pub unk2: u32,
        pub unk3: u32,
        pub flip: i32,
        pub mirror: i32,
        pub delay: UsbCamDelay,
        pub unk4: [u32; 5usize],
    }

    pub struct UsbCamSetupVideoParam {
        pub size: i32,
        pub resolution: UsbCamResolution,
        pub framerate: UsbCamFrameRate,
        pub white_balance: UsbCamWb,
        pub saturation: i32,
        pub brightness: i32,
        pub contrast: i32,
        pub sharpness: i32,
        pub effect_mode: UsbCamEffectMode,
        pub frame_size: i32,
        pub unk: u32,
        pub evl_evel: UsbCamEvLevel,
    }

    pub struct UsbCamSetupVideoExParam {
        pub size: i32,
        pub unk: u32,
        pub resolution: UsbCamResolutionEx,
        pub framerate: UsbCamFrameRate,
        pub unk2: u32,
        pub unk3: u32,
        pub white_balance: UsbCamWb,
        pub saturation: i32,
        pub brightness: i32,
        pub contrast: i32,
        pub sharpness: i32,
        pub unk4: u32,
        pub unk5: u32,
        pub unk6: [u32; 3usize],
        pub effect_mode: UsbCamEffectMode,
        pub unk7: u32,
        pub unk8: u32,
        pub unk9: u32,
        pub unk10: u32,
        pub unk11: u32,
        pub frame_size: i32,
        pub unk12: u32,
        pub ev_level: UsbCamEvLevel,
    }

    pub struct ScePspDateTime {
        pub year: u16,
        pub month: u16,
        pub day: u16,
        pub hour: u16,
        pub minutes: u16,
        pub seconds: u16,
        pub microseconds: u32,
    }

    pub struct SceIoStat {
        pub st_mode: i32,
        pub st_attr: i32,
        pub st_size: i64,
        pub st_ctime: ScePspDateTime,
        pub st_atime: ScePspDateTime,
        pub st_mtime: ScePspDateTime,
        pub st_private: [u32; 6usize],
    }

    pub struct UmdInfo {
        pub size: u32,
        pub type_: UmdType,
    }

    // FIXME(1.0): This should not implement `PartialEq`
    #[allow(unpredictable_function_pointer_comparisons)]
    pub struct SceMpegRingbuffer {
        pub packets: i32,
        pub unk0: u32,
        pub unk1: u32,
        pub unk2: u32,
        pub unk3: u32,
        pub data: *mut c_void,
        pub callback: SceMpegRingbufferCb,
        pub cb_param: *mut c_void,
        pub unk4: u32,
        pub unk5: u32,
        pub sce_mpeg: *mut c_void,
    }

    pub struct SceMpegAu {
        pub pts_msb: u32,
        pub pts: u32,
        pub dts_msb: u32,
        pub dts: u32,
        pub es_buffer: u32,
        pub au_size: u32,
    }

    pub struct SceMpegAvcMode {
        pub unk0: i32,
        pub pixel_format: super::DisplayPixelFormat,
    }

    #[repr(align(64))]
    pub struct SceMpegLLI {
        pub src: *mut c_void,
        pub dst: *mut c_void,
        pub next: *mut c_void,
        pub size: i32,
    }

    #[repr(align(64))]
    pub struct SceMpegYCrCbBuffer {
        pub frame_buffer_height16: i32,
        pub frame_buffer_width16: i32,
        pub unknown: i32,
        pub unknown2: i32,
        pub y_buffer: *mut c_void,
        pub y_buffer2: *mut c_void,
        pub cr_buffer: *mut c_void,
        pub cb_buffer: *mut c_void,
        pub cr_buffer2: *mut c_void,
        pub cb_buffer2: *mut c_void,

        pub frame_height: i32,
        pub frame_width: i32,
        pub frame_buffer_width: i32,
        pub unknown3: [i32; 11usize],
    }

    pub struct ScePspSRect {
        pub x: i16,
        pub y: i16,
        pub w: i16,
        pub h: i16,
    }

    pub struct ScePspIRect {
        pub x: i32,
        pub y: i32,
        pub w: i32,
        pub h: i32,
    }

    pub struct ScePspL64Rect {
        pub x: u64,
        pub y: u64,
        pub w: u64,
        pub h: u64,
    }

    pub struct ScePspSVector2 {
        pub x: i16,
        pub y: i16,
    }

    pub struct ScePspIVector2 {
        pub x: i32,
        pub y: i32,
    }

    pub struct ScePspL64Vector2 {
        pub x: u64,
        pub y: u64,
    }

    pub struct ScePspSVector3 {
        pub x: i16,
        pub y: i16,
        pub z: i16,
    }

    pub struct ScePspIVector3 {
        pub x: i32,
        pub y: i32,
        pub z: i32,
    }

    pub struct ScePspL64Vector3 {
        pub x: u64,
        pub y: u64,
        pub z: u64,
    }

    pub struct ScePspSVector4 {
        pub x: i16,
        pub y: i16,
        pub z: i16,
        pub w: i16,
    }

    pub struct ScePspIVector4 {
        pub x: i32,
        pub y: i32,
        pub z: i32,
        pub w: i32,
    }

    pub struct ScePspL64Vector4 {
        pub x: u64,
        pub y: u64,
        pub z: u64,
        pub w: u64,
    }

    pub struct ScePspIMatrix2 {
        pub x: ScePspIVector2,
        pub y: ScePspIVector2,
    }

    pub struct ScePspIMatrix3 {
        pub x: ScePspIVector3,
        pub y: ScePspIVector3,
        pub z: ScePspIVector3,
    }

    #[repr(align(16))]
    pub struct ScePspIMatrix4 {
        pub x: ScePspIVector4,
        pub y: ScePspIVector4,
        pub z: ScePspIVector4,
        pub w: ScePspIVector4,
    }

    pub struct ScePspIMatrix4Unaligned {
        pub x: ScePspIVector4,
        pub y: ScePspIVector4,
        pub z: ScePspIVector4,
        pub w: ScePspIVector4,
    }

    pub struct SceMp3InitArg {
        pub mp3_stream_start: u32,
        pub unk1: u32,
        pub mp3_stream_end: u32,
        pub unk2: u32,
        pub mp3_buf: *mut c_void,
        pub mp3_buf_size: i32,
        pub pcm_buf: *mut c_void,
        pub pcm_buf_size: i32,
    }

    pub struct OpenPSID {
        pub data: [u8; 16usize],
    }

    pub struct UtilityDialogCommon {
        pub size: u32,
        pub language: SystemParamLanguage,
        pub button_accept: UtilityDialogButtonAccept,
        pub graphics_thread: i32,
        pub access_thread: i32,
        pub font_thread: i32,
        pub sound_thread: i32,
        pub result: i32,
        pub reserved: [i32; 4usize],
    }

    pub struct UtilityNetconfAdhoc {
        pub name: [u8; 8usize],
        pub timeout: u32,
    }

    pub struct UtilityNetconfData {
        pub base: UtilityDialogCommon,
        pub action: UtilityNetconfAction,
        pub adhocparam: *mut UtilityNetconfAdhoc,
        pub hotspot: i32,
        pub hotspot_connected: i32,
        pub wifisp: i32,
    }

    pub struct UtilitySavedataFileData {
        pub buf: *mut c_void,
        pub buf_size: usize,
        pub size: usize,
        pub unknown: i32,
    }

    pub struct UtilitySavedataListSaveNewData {
        pub icon0: UtilitySavedataFileData,
        pub title: *mut u8,
    }

    pub struct UtilityGameSharingParams {
        pub base: UtilityDialogCommon,
        pub unknown1: i32,
        pub unknown2: i32,
        pub name: [u8; 8usize],
        pub unknown3: i32,
        pub unknown4: i32,
        pub unknown5: i32,
        pub result: i32,
        pub filepath: *mut u8,
        pub mode: UtilityGameSharingMode,
        pub datatype: UtilityGameSharingDataType,
        pub data: *mut c_void,
        pub datasize: u32,
    }

    pub struct UtilityHtmlViewerParam {
        pub base: UtilityDialogCommon,
        pub memaddr: *mut c_void,
        pub memsize: u32,
        pub unknown1: i32,
        pub unknown2: i32,
        pub initialurl: *mut u8,
        pub numtabs: u32,
        pub interfacemode: UtilityHtmlViewerInterfaceMode,
        pub options: i32,
        pub dldirname: *mut u8,
        pub dlfilename: *mut u8,
        pub uldirname: *mut u8,
        pub ulfilename: *mut u8,
        pub cookiemode: UtilityHtmlViewerCookieMode,
        pub unknown3: u32,
        pub homeurl: *mut u8,
        pub textsize: UtilityHtmlViewerTextSize,
        pub displaymode: UtilityHtmlViewerDisplayMode,
        pub connectmode: UtilityHtmlViewerConnectMode,
        pub disconnectmode: UtilityHtmlViewerDisconnectMode,
        pub memused: u32,
        pub unknown4: [i32; 10usize],
    }

    pub struct SceUtilityOskData {
        pub unk_00: i32,
        pub unk_04: i32,
        pub language: SceUtilityOskInputLanguage,
        pub unk_12: i32,
        pub inputtype: SceUtilityOskInputType,
        pub lines: i32,
        pub unk_24: i32,
        pub desc: *mut u16,
        pub intext: *mut u16,
        pub outtextlength: i32,
        pub outtext: *mut u16,
        pub result: SceUtilityOskResult,
        pub outtextlimit: i32,
    }

    pub struct SceUtilityOskParams {
        pub base: UtilityDialogCommon,
        pub datacount: i32,
        pub data: *mut SceUtilityOskData,
        pub state: SceUtilityOskState,
        pub unk_60: i32,
    }

    pub struct SceNetMallocStat {
        pub pool: i32,
        pub maximum: i32,
        pub free: i32,
    }

    pub struct SceNetAdhocctlAdhocId {
        pub unknown: i32,
        pub adhoc_id: [u8; 9usize],
        pub unk: [u8; 3usize],
    }

    pub struct SceNetAdhocctlScanInfo {
        pub next: *mut SceNetAdhocctlScanInfo,
        pub channel: i32,
        pub name: [u8; 8usize],
        pub bssid: [u8; 6usize],
        pub unknown: [u8; 2usize],
        pub unknown2: i32,
    }

    pub struct SceNetAdhocctlGameModeInfo {
        pub count: i32,
        pub macs: [[u8; 6usize]; 16usize],
    }

    pub struct SceNetAdhocPtpStat {
        pub next: *mut SceNetAdhocPtpStat,
        pub ptp_id: i32,
        pub mac: [u8; 6usize],
        pub peermac: [u8; 6usize],
        pub port: u16,
        pub peerport: u16,
        pub sent_data: u32,
        pub rcvd_data: u32,
        pub state: ScePspnetAdhocPtpState,
    }

    pub struct SceNetAdhocPdpStat {
        pub next: *mut SceNetAdhocPdpStat,
        pub pdp_id: i32,
        pub mac: [u8; 6usize],
        pub port: u16,
        pub rcvd_data: u32,
    }

    pub struct AdhocPoolStat {
        pub size: i32,
        pub maxsize: i32,
        pub freesize: i32,
    }
}

s_no_extra_traits! {
    pub struct GeContext {
        pub context: [u32; 512],
    }

    pub struct SceKernelUtilsSha1Context {
        pub h: [u32; 5usize],
        pub us_remains: u16,
        pub us_computed: u16,
        pub ull_total_len: u64,
        pub buf: [u8; 64usize],
    }

    pub struct SceKernelUtilsMt19937Context {
        pub count: u32,
        pub state: [u32; 624usize],
    }

    pub struct SceKernelUtilsMd5Context {
        pub h: [u32; 4usize],
        pub pad: u32,
        pub us_remains: u16,
        pub us_computed: u16,
        pub ull_total_len: u64,
        pub buf: [u8; 64usize],
    }

    pub struct SceIoDirent {
        pub d_stat: SceIoStat,
        pub d_name: [u8; 256usize],
        pub d_private: *mut c_void,
        pub dummy: i32,
    }

    pub struct ScePspFRect {
        pub x: f32,
        pub y: f32,
        pub w: f32,
        pub h: f32,
    }

    #[repr(align(16))]
    pub struct ScePspFVector3 {
        pub x: f32,
        pub y: f32,
        pub z: f32,
    }

    #[repr(align(16))]
    pub struct ScePspFVector4 {
        pub x: f32,
        pub y: f32,
        pub z: f32,
        pub w: f32,
    }

    pub struct ScePspFVector4Unaligned {
        pub x: f32,
        pub y: f32,
        pub z: f32,
        pub w: f32,
    }

    pub struct ScePspFVector2 {
        pub x: f32,
        pub y: f32,
    }

    pub struct ScePspFMatrix2 {
        pub x: ScePspFVector2,
        pub y: ScePspFVector2,
    }

    pub struct ScePspFMatrix3 {
        pub x: ScePspFVector3,
        pub y: ScePspFVector3,
        pub z: ScePspFVector3,
    }

    #[repr(align(16))]
    pub struct ScePspFMatrix4 {
        pub x: ScePspFVector4,
        pub y: ScePspFVector4,
        pub z: ScePspFVector4,
        pub w: ScePspFVector4,
    }

    pub struct ScePspFMatrix4Unaligned {
        pub x: ScePspFVector4,
        pub y: ScePspFVector4,
        pub z: ScePspFVector4,
        pub w: ScePspFVector4,
    }

    pub union ScePspVector3 {
        pub fv: ScePspFVector3,
        pub iv: ScePspIVector3,
        pub f: [f32; 3usize],
        pub i: [i32; 3usize],
    }

    pub union ScePspVector4 {
        pub fv: ScePspFVector4,
        pub iv: ScePspIVector4,
        pub qw: u128,
        pub f: [f32; 4usize],
        pub i: [i32; 4usize],
    }

    pub union ScePspMatrix2 {
        pub fm: ScePspFMatrix2,
        pub im: ScePspIMatrix2,
        pub fv: [ScePspFVector2; 2usize],
        pub iv: [ScePspIVector2; 2usize],
        pub v: [ScePspVector2; 2usize],
        pub f: [[f32; 2usize]; 2usize],
        pub i: [[i32; 2usize]; 2usize],
    }

    pub union ScePspMatrix3 {
        pub fm: ScePspFMatrix3,
        pub im: ScePspIMatrix3,
        pub fv: [ScePspFVector3; 3usize],
        pub iv: [ScePspIVector3; 3usize],
        pub v: [ScePspVector3; 3usize],
        pub f: [[f32; 3usize]; 3usize],
        pub i: [[i32; 3usize]; 3usize],
    }

    pub union ScePspVector2 {
        pub fv: ScePspFVector2,
        pub iv: ScePspIVector2,
        pub f: [f32; 2usize],
        pub i: [i32; 2usize],
    }

    pub union ScePspMatrix4 {
        pub fm: ScePspFMatrix4,
        pub im: ScePspIMatrix4,
        pub fv: [ScePspFVector4; 4usize],
        pub iv: [ScePspIVector4; 4usize],
        pub v: [ScePspVector4; 4usize],
        pub f: [[f32; 4usize]; 4usize],
        pub i: [[i32; 4usize]; 4usize],
    }

    pub struct Key {
        pub key_type: KeyType,
        pub name: [u8; 256usize],
        pub name_len: u32,
        pub unk2: u32,
        pub unk3: u32,
    }

    pub struct UtilityMsgDialogParams {
        pub base: UtilityDialogCommon,
        pub unknown: i32,
        pub mode: UtilityMsgDialogMode,
        pub error_value: u32,
        pub message: [u8; 512usize],
        pub options: i32,
        pub button_pressed: UtilityMsgDialogPressed,
    }

    pub union UtilityNetData {
        pub as_uint: u32,
        pub as_string: [u8; 128usize],
    }

    pub struct UtilitySavedataSFOParam {
        pub title: [u8; 128usize],
        pub savedata_title: [u8; 128usize],
        pub detail: [u8; 1024usize],
        pub parental_level: u8,
        pub unknown: [u8; 3usize],
    }

    pub struct SceUtilitySavedataParam {
        pub base: UtilityDialogCommon,
        pub mode: UtilitySavedataMode,
        pub unknown1: i32,
        pub overwrite: i32,
        pub game_name: [u8; 13usize],
        pub reserved: [u8; 3usize],
        pub save_name: [u8; 20usize],
        pub save_name_list: *mut [u8; 20usize],
        pub file_name: [u8; 13usize],
        pub reserved1: [u8; 3usize],
        pub data_buf: *mut c_void,
        pub data_buf_size: usize,
        pub data_size: usize,
        pub sfo_param: UtilitySavedataSFOParam,
        pub icon0_file_data: UtilitySavedataFileData,
        pub icon1_file_data: UtilitySavedataFileData,
        pub pic1_file_data: UtilitySavedataFileData,
        pub snd0_file_data: UtilitySavedataFileData,
        pub new_data: *mut UtilitySavedataListSaveNewData,
        pub focus: UtilitySavedataFocus,
        pub unknown2: [i32; 4usize],
        pub key: [u8; 16],
        pub unknown3: [u8; 20],
    }

    pub struct SceNetAdhocctlPeerInfo {
        pub next: *mut SceNetAdhocctlPeerInfo,
        pub nickname: [u8; 128usize],
        pub mac: [u8; 6usize],
        pub unknown: [u8; 6usize],
        pub timestamp: u32,
    }

    pub struct SceNetAdhocctlParams {
        pub channel: i32,
        pub name: [u8; 8usize],
        pub bssid: [u8; 6usize],
        pub nickname: [u8; 128usize],
    }

    pub union SceNetApctlInfo {
        pub name: [u8; 64usize],
        pub bssid: [u8; 6usize],
        pub ssid: [u8; 32usize],
        pub ssid_length: u32,
        pub security_type: u32,
        pub strength: u8,
        pub channel: u8,
        pub power_save: u8,
        pub ip: [u8; 16usize],
        pub sub_net_mask: [u8; 16usize],
        pub gateway: [u8; 16usize],
        pub primary_dns: [u8; 16usize],
        pub secondary_dns: [u8; 16usize],
        pub use_proxy: u32,
        pub proxy_url: [u8; 128usize],
        pub proxy_port: u16,
        pub eap_type: u32,
        pub start_browser: u32,
        pub wifisp: u32,
    }
}

pub const INT_MIN: c_int = -2147483648;
pub const INT_MAX: c_int = 2147483647;

pub const AUDIO_VOLUME_MAX: u32 = 0x8000;
pub const AUDIO_CHANNEL_MAX: u32 = 8;
pub const AUDIO_NEXT_CHANNEL: i32 = -1;
pub const AUDIO_SAMPLE_MIN: u32 = 64;
pub const AUDIO_SAMPLE_MAX: u32 = 65472;

pub const PSP_CTRL_SELECT: i32 = 0x000001;
pub const PSP_CTRL_START: i32 = 0x000008;
pub const PSP_CTRL_UP: i32 = 0x000010;
pub const PSP_CTRL_RIGHT: i32 = 0x000020;
pub const PSP_CTRL_DOWN: i32 = 0x000040;
pub const PSP_CTRL_LEFT: i32 = 0x000080;
pub const PSP_CTRL_LTRIGGER: i32 = 0x000100;
pub const PSP_CTRL_RTRIGGER: i32 = 0x000200;
pub const PSP_CTRL_TRIANGLE: i32 = 0x001000;
pub const PSP_CTRL_CIRCLE: i32 = 0x002000;
pub const PSP_CTRL_CROSS: i32 = 0x004000;
pub const PSP_CTRL_SQUARE: i32 = 0x008000;
pub const PSP_CTRL_HOME: i32 = 0x010000;
pub const PSP_CTRL_HOLD: i32 = 0x020000;
pub const PSP_CTRL_NOTE: i32 = 0x800000;
pub const PSP_CTRL_SCREEN: i32 = 0x400000;
pub const PSP_CTRL_VOLUP: i32 = 0x100000;
pub const PSP_CTRL_VOLDOWN: i32 = 0x200000;
pub const PSP_CTRL_WLAN_UP: i32 = 0x040000;
pub const PSP_CTRL_REMOTE: i32 = 0x080000;
pub const PSP_CTRL_DISC: i32 = 0x1000000;
pub const PSP_CTRL_MS: i32 = 0x2000000;

pub const USB_CAM_PID: i32 = 0x282;
pub const USB_BUS_DRIVER_NAME: &str = "USBBusDriver";
pub const USB_CAM_DRIVER_NAME: &str = "USBCamDriver";
pub const USB_CAM_MIC_DRIVER_NAME: &str = "USBCamMicDriver";
pub const USB_STOR_DRIVER_NAME: &str = "USBStor_Driver";

pub const ACTIVATED: i32 = 0x200;
pub const CONNECTED: i32 = 0x020;
pub const ESTABLISHED: i32 = 0x002;

pub const USB_CAM_FLIP: i32 = 1;
pub const USB_CAM_MIRROR: i32 = 0x100;

pub const THREAD_ATTR_VFPU: i32 = 0x00004000;
pub const THREAD_ATTR_USER: i32 = 0x80000000;
pub const THREAD_ATTR_USBWLAN: i32 = 0xa0000000;
pub const THREAD_ATTR_VSH: i32 = 0xc0000000;
pub const THREAD_ATTR_SCRATCH_SRAM: i32 = 0x00008000;
pub const THREAD_ATTR_NO_FILLSTACK: i32 = 0x00100000;
pub const THREAD_ATTR_CLEAR_STACK: i32 = 0x00200000;

pub const EVENT_WAIT_MULTIPLE: i32 = 0x200;

pub const EVENT_WAIT_AND: i32 = 0;
pub const EVENT_WAIT_OR: i32 = 1;
pub const EVENT_WAIT_CLEAR: i32 = 0x20;

pub const POWER_INFO_POWER_SWITCH: i32 = 0x80000000;
pub const POWER_INFO_HOLD_SWITCH: i32 = 0x40000000;
pub const POWER_INFO_STANDBY: i32 = 0x00080000;
pub const POWER_INFO_RESUME_COMPLETE: i32 = 0x00040000;
pub const POWER_INFO_RESUMING: i32 = 0x00020000;
pub const POWER_INFO_SUSPENDING: i32 = 0x00010000;
pub const POWER_INFO_AC_POWER: i32 = 0x00001000;
pub const POWER_INFO_BATTERY_LOW: i32 = 0x00000100;
pub const POWER_INFO_BATTERY_EXIST: i32 = 0x00000080;
pub const POWER_INFO_BATTERY_POWER: i32 = 0x0000007;

pub const FIO_S_IFLNK: i32 = 0x4000;
pub const FIO_S_IFDIR: i32 = 0x1000;
pub const FIO_S_IFREG: i32 = 0x2000;
pub const FIO_S_ISUID: i32 = 0x0800;
pub const FIO_S_ISGID: i32 = 0x0400;
pub const FIO_S_ISVTX: i32 = 0x0200;
pub const FIO_S_IRUSR: i32 = 0x0100;
pub const FIO_S_IWUSR: i32 = 0x0080;
pub const FIO_S_IXUSR: i32 = 0x0040;
pub const FIO_S_IRGRP: i32 = 0x0020;
pub const FIO_S_IWGRP: i32 = 0x0010;
pub const FIO_S_IXGRP: i32 = 0x0008;
pub const FIO_S_IROTH: i32 = 0x0004;
pub const FIO_S_IWOTH: i32 = 0x0002;
pub const FIO_S_IXOTH: i32 = 0x0001;

pub const FIO_SO_IFLNK: i32 = 0x0008;
pub const FIO_SO_IFDIR: i32 = 0x0010;
pub const FIO_SO_IFREG: i32 = 0x0020;
pub const FIO_SO_IROTH: i32 = 0x0004;
pub const FIO_SO_IWOTH: i32 = 0x0002;
pub const FIO_SO_IXOTH: i32 = 0x0001;

pub const PSP_O_RD_ONLY: i32 = 0x0001;
pub const PSP_O_WR_ONLY: i32 = 0x0002;
pub const PSP_O_RD_WR: i32 = 0x0003;
pub const PSP_O_NBLOCK: i32 = 0x0004;
pub const PSP_O_DIR: i32 = 0x0008;
pub const PSP_O_APPEND: i32 = 0x0100;
pub const PSP_O_CREAT: i32 = 0x0200;
pub const PSP_O_TRUNC: i32 = 0x0400;
pub const PSP_O_EXCL: i32 = 0x0800;
pub const PSP_O_NO_WAIT: i32 = 0x8000;

pub const UMD_NOT_PRESENT: i32 = 0x01;
pub const UMD_PRESENT: i32 = 0x02;
pub const UMD_CHANGED: i32 = 0x04;
pub const UMD_INITING: i32 = 0x08;
pub const UMD_INITED: i32 = 0x10;
pub const UMD_READY: i32 = 0x20;

pub const PLAY_PAUSE: i32 = 0x1;
pub const FORWARD: i32 = 0x4;
pub const BACK: i32 = 0x8;
pub const VOL_UP: i32 = 0x10;
pub const VOL_DOWN: i32 = 0x20;
pub const HOLD: i32 = 0x80;

pub const GU_PI: f32 = 3.141593;

pub const GU_TEXTURE_8BIT: i32 = 1;
pub const GU_TEXTURE_16BIT: i32 = 2;
pub const GU_TEXTURE_32BITF: i32 = 3;
pub const GU_COLOR_5650: i32 = 4 << 2;
pub const GU_COLOR_5551: i32 = 5 << 2;
pub const GU_COLOR_4444: i32 = 6 << 2;
pub const GU_COLOR_8888: i32 = 7 << 2;
pub const GU_NORMAL_8BIT: i32 = 1 << 5;
pub const GU_NORMAL_16BIT: i32 = 2 << 5;
pub const GU_NORMAL_32BITF: i32 = 3 << 5;
pub const GU_VERTEX_8BIT: i32 = 1 << 7;
pub const GU_VERTEX_16BIT: i32 = 2 << 7;
pub const GU_VERTEX_32BITF: i32 = 3 << 7;
pub const GU_WEIGHT_8BIT: i32 = 1 << 9;
pub const GU_WEIGHT_16BIT: i32 = 2 << 9;
pub const GU_WEIGHT_32BITF: i32 = 3 << 9;
pub const GU_INDEX_8BIT: i32 = 1 << 11;
pub const GU_INDEX_16BIT: i32 = 2 << 11;
pub const GU_WEIGHTS1: i32 = (((1 - 1) & 7) << 14) as i32;
pub const GU_WEIGHTS2: i32 = (((2 - 1) & 7) << 14) as i32;
pub const GU_WEIGHTS3: i32 = (((3 - 1) & 7) << 14) as i32;
pub const GU_WEIGHTS4: i32 = (((4 - 1) & 7) << 14) as i32;
pub const GU_WEIGHTS5: i32 = (((5 - 1) & 7) << 14) as i32;
pub const GU_WEIGHTS6: i32 = (((6 - 1) & 7) << 14) as i32;
pub const GU_WEIGHTS7: i32 = (((7 - 1) & 7) << 14) as i32;
pub const GU_WEIGHTS8: i32 = (((8 - 1) & 7) << 14) as i32;
pub const GU_VERTICES1: i32 = (((1 - 1) & 7) << 18) as i32;
pub const GU_VERTICES2: i32 = (((2 - 1) & 7) << 18) as i32;
pub const GU_VERTICES3: i32 = (((3 - 1) & 7) << 18) as i32;
pub const GU_VERTICES4: i32 = (((4 - 1) & 7) << 18) as i32;
pub const GU_VERTICES5: i32 = (((5 - 1) & 7) << 18) as i32;
pub const GU_VERTICES6: i32 = (((6 - 1) & 7) << 18) as i32;
pub const GU_VERTICES7: i32 = (((7 - 1) & 7) << 18) as i32;
pub const GU_VERTICES8: i32 = (((8 - 1) & 7) << 18) as i32;
pub const GU_TRANSFORM_2D: i32 = 1 << 23;
pub const GU_TRANSFORM_3D: i32 = 0;

pub const GU_COLOR_BUFFER_BIT: i32 = 1;
pub const GU_STENCIL_BUFFER_BIT: i32 = 2;
pub const GU_DEPTH_BUFFER_BIT: i32 = 4;
pub const GU_FAST_CLEAR_BIT: i32 = 16;

pub const GU_AMBIENT: i32 = 1;
pub const GU_DIFFUSE: i32 = 2;
pub const GU_SPECULAR: i32 = 4;
pub const GU_UNKNOWN_LIGHT_COMPONENT: i32 = 8;

pub const SYSTEM_REGISTRY: [u8; 7] = *b"/system";
pub const REG_KEYNAME_SIZE: u32 = 27;

pub const UTILITY_MSGDIALOG_ERROR: i32 = 0;
pub const UTILITY_MSGDIALOG_TEXT: i32 = 1;
pub const UTILITY_MSGDIALOG_YES_NO_BUTTONS: i32 = 0x10;
pub const UTILITY_MSGDIALOG_DEFAULT_NO: i32 = 0x100;

pub const UTILITY_HTMLVIEWER_OPEN_SCE_START_PAGE: i32 = 0x000001;
pub const UTILITY_HTMLVIEWER_DISABLE_STARTUP_LIMITS: i32 = 0x000002;
pub const UTILITY_HTMLVIEWER_DISABLE_EXIT_DIALOG: i32 = 0x000004;
pub const UTILITY_HTMLVIEWER_DISABLE_CURSOR: i32 = 0x000008;
pub const UTILITY_HTMLVIEWER_DISABLE_DOWNLOAD_COMPLETE_DIALOG: i32 = 0x000010;
pub const UTILITY_HTMLVIEWER_DISABLE_DOWNLOAD_START_DIALOG: i32 = 0x000020;
pub const UTILITY_HTMLVIEWER_DISABLE_DOWNLOAD_DESTINATION_DIALOG: i32 = 0x000040;
pub const UTILITY_HTMLVIEWER_LOCK_DOWNLOAD_DESTINATION_DIALOG: i32 = 0x000080;
pub const UTILITY_HTMLVIEWER_DISABLE_TAB_DISPLAY: i32 = 0x000100;
pub const UTILITY_HTMLVIEWER_ENABLE_ANALOG_HOLD: i32 = 0x000200;
pub const UTILITY_HTMLVIEWER_ENABLE_FLASH: i32 = 0x000400;
pub const UTILITY_HTMLVIEWER_DISABLE_LRTRIGGER: i32 = 0x000800;

extern "C" {
    pub fn sceAudioChReserve(channel: i32, sample_count: i32, format: AudioFormat) -> i32;
    pub fn sceAudioChRelease(channel: i32) -> i32;
    pub fn sceAudioOutput(channel: i32, vol: i32, buf: *mut c_void) -> i32;
    pub fn sceAudioOutputBlocking(channel: i32, vol: i32, buf: *mut c_void) -> i32;
    pub fn sceAudioOutputPanned(
        channel: i32,
        left_vol: i32,
        right_vol: i32,
        buf: *mut c_void,
    ) -> i32;
    pub fn sceAudioOutputPannedBlocking(
        channel: i32,
        left_vol: i32,
        right_vol: i32,
        buf: *mut c_void,
    ) -> i32;
    pub fn sceAudioGetChannelRestLen(channel: i32) -> i32;
    pub fn sceAudioGetChannelRestLength(channel: i32) -> i32;
    pub fn sceAudioSetChannelDataLen(channel: i32, sample_count: i32) -> i32;
    pub fn sceAudioChangeChannelConfig(channel: i32, format: AudioFormat) -> i32;
    pub fn sceAudioChangeChannelVolume(channel: i32, left_vol: i32, right_vol: i32) -> i32;
    pub fn sceAudioOutput2Reserve(sample_count: i32) -> i32;
    pub fn sceAudioOutput2Release() -> i32;
    pub fn sceAudioOutput2ChangeLength(sample_count: i32) -> i32;
    pub fn sceAudioOutput2OutputBlocking(vol: i32, buf: *mut c_void) -> i32;
    pub fn sceAudioOutput2GetRestSample() -> i32;
    pub fn sceAudioSRCChReserve(
        sample_count: i32,
        freq: AudioOutputFrequency,
        channels: i32,
    ) -> i32;
    pub fn sceAudioSRCChRelease() -> i32;
    pub fn sceAudioSRCOutputBlocking(vol: i32, buf: *mut c_void) -> i32;
    pub fn sceAudioInputInit(unknown1: i32, gain: i32, unknown2: i32) -> i32;
    pub fn sceAudioInputInitEx(params: *mut AudioInputParams) -> i32;
    pub fn sceAudioInputBlocking(sample_count: i32, freq: AudioInputFrequency, buf: *mut c_void);
    pub fn sceAudioInput(sample_count: i32, freq: AudioInputFrequency, buf: *mut c_void);
    pub fn sceAudioGetInputLength() -> i32;
    pub fn sceAudioWaitInputEnd() -> i32;
    pub fn sceAudioPollInputEnd() -> i32;

    pub fn sceAtracGetAtracID(ui_codec_type: u32) -> i32;
    pub fn sceAtracSetDataAndGetID(buf: *mut c_void, bufsize: usize) -> i32;
    pub fn sceAtracDecodeData(
        atrac_id: i32,
        out_samples: *mut u16,
        out_n: *mut i32,
        out_end: *mut i32,
        out_remain_frame: *mut i32,
    ) -> i32;
    pub fn sceAtracGetRemainFrame(atrac_id: i32, out_remain_frame: *mut i32) -> i32;
    pub fn sceAtracGetStreamDataInfo(
        atrac_id: i32,
        write_pointer: *mut *mut u8,
        available_bytes: *mut u32,
        read_offset: *mut u32,
    ) -> i32;
    pub fn sceAtracAddStreamData(atrac_id: i32, bytes_to_add: u32) -> i32;
    pub fn sceAtracGetBitrate(atrac_id: i32, out_bitrate: *mut i32) -> i32;
    pub fn sceAtracSetLoopNum(atrac_id: i32, nloops: i32) -> i32;
    pub fn sceAtracReleaseAtracID(atrac_id: i32) -> i32;
    pub fn sceAtracGetNextSample(atrac_id: i32, out_n: *mut i32) -> i32;
    pub fn sceAtracGetMaxSample(atrac_id: i32, out_max: *mut i32) -> i32;
    pub fn sceAtracGetBufferInfoForReseting(
        atrac_id: i32,
        ui_sample: u32,
        pbuffer_info: *mut Atrac3BufferInfo,
    ) -> i32;
    pub fn sceAtracGetChannel(atrac_id: i32, pui_channel: *mut u32) -> i32;
    pub fn sceAtracGetInternalErrorInfo(atrac_id: i32, pi_result: *mut i32) -> i32;
    pub fn sceAtracGetLoopStatus(
        atrac_id: i32,
        pi_loop_num: *mut i32,
        pui_loop_status: *mut u32,
    ) -> i32;
    pub fn sceAtracGetNextDecodePosition(atrac_id: i32, pui_sample_position: *mut u32) -> i32;
    pub fn sceAtracGetSecondBufferInfo(
        atrac_id: i32,
        pui_position: *mut u32,
        pui_data_byte: *mut u32,
    ) -> i32;
    pub fn sceAtracGetSoundSample(
        atrac_id: i32,
        pi_end_sample: *mut i32,
        pi_loop_start_sample: *mut i32,
        pi_loop_end_sample: *mut i32,
    ) -> i32;
    pub fn sceAtracResetPlayPosition(
        atrac_id: i32,
        ui_sample: u32,
        ui_write_byte_first_buf: u32,
        ui_write_byte_second_buf: u32,
    ) -> i32;
    pub fn sceAtracSetData(atrac_id: i32, puc_buffer_addr: *mut u8, ui_buffer_byte: u32) -> i32;
    pub fn sceAtracSetHalfwayBuffer(
        atrac_id: i32,
        puc_buffer_addr: *mut u8,
        ui_read_byte: u32,
        ui_buffer_byte: u32,
    ) -> i32;
    pub fn sceAtracSetHalfwayBufferAndGetID(
        puc_buffer_addr: *mut u8,
        ui_read_byte: u32,
        ui_buffer_byte: u32,
    ) -> i32;
    pub fn sceAtracSetSecondBuffer(
        atrac_id: i32,
        puc_second_buffer_addr: *mut u8,
        ui_second_buffer_byte: u32,
    ) -> i32;

    pub fn sceCtrlSetSamplingCycle(cycle: i32) -> i32;
    pub fn sceCtrlGetSamplingCycle(pcycle: *mut i32) -> i32;
    pub fn sceCtrlSetSamplingMode(mode: CtrlMode) -> i32;
    pub fn sceCtrlGetSamplingMode(pmode: *mut i32) -> i32;
    pub fn sceCtrlPeekBufferPositive(pad_data: *mut SceCtrlData, count: i32) -> i32;
    pub fn sceCtrlPeekBufferNegative(pad_data: *mut SceCtrlData, count: i32) -> i32;
    pub fn sceCtrlReadBufferPositive(pad_data: *mut SceCtrlData, count: i32) -> i32;
    pub fn sceCtrlReadBufferNegative(pad_data: *mut SceCtrlData, count: i32) -> i32;
    pub fn sceCtrlPeekLatch(latch_data: *mut SceCtrlLatch) -> i32;
    pub fn sceCtrlReadLatch(latch_data: *mut SceCtrlLatch) -> i32;
    pub fn sceCtrlSetIdleCancelThreshold(idlereset: i32, idleback: i32) -> i32;
    pub fn sceCtrlGetIdleCancelThreshold(idlereset: *mut i32, idleback: *mut i32) -> i32;

    pub fn sceDisplaySetMode(mode: DisplayMode, width: usize, height: usize) -> u32;
    pub fn sceDisplayGetMode(pmode: *mut i32, pwidth: *mut i32, pheight: *mut i32) -> i32;
    pub fn sceDisplaySetFrameBuf(
        top_addr: *const u8,
        buffer_width: usize,
        pixel_format: DisplayPixelFormat,
        sync: DisplaySetBufSync,
    ) -> u32;
    pub fn sceDisplayGetFrameBuf(
        top_addr: *mut *mut c_void,
        buffer_width: *mut usize,
        pixel_format: *mut DisplayPixelFormat,
        sync: DisplaySetBufSync,
    ) -> i32;
    pub fn sceDisplayGetVcount() -> u32;
    pub fn sceDisplayWaitVblank() -> i32;
    pub fn sceDisplayWaitVblankCB() -> i32;
    pub fn sceDisplayWaitVblankStart() -> i32;
    pub fn sceDisplayWaitVblankStartCB() -> i32;
    pub fn sceDisplayGetAccumulatedHcount() -> i32;
    pub fn sceDisplayGetCurrentHcount() -> i32;
    pub fn sceDisplayGetFramePerSec() -> f32;
    pub fn sceDisplayIsForeground() -> i32;
    pub fn sceDisplayIsVblank() -> i32;

    pub fn sceGeEdramGetSize() -> u32;
    pub fn sceGeEdramGetAddr() -> *mut u8;
    pub fn sceGeEdramSetAddrTranslation(width: i32) -> i32;
    pub fn sceGeGetCmd(cmd: i32) -> u32;
    pub fn sceGeGetMtx(type_: GeMatrixType, matrix: *mut c_void) -> i32;
    pub fn sceGeGetStack(stack_id: i32, stack: *mut GeStack) -> i32;
    pub fn sceGeSaveContext(context: *mut GeContext) -> i32;
    pub fn sceGeRestoreContext(context: *const GeContext) -> i32;
    pub fn sceGeListEnQueue(
        list: *const c_void,
        stall: *mut c_void,
        cbid: i32,
        arg: *mut GeListArgs,
    ) -> i32;
    pub fn sceGeListEnQueueHead(
        list: *const c_void,
        stall: *mut c_void,
        cbid: i32,
        arg: *mut GeListArgs,
    ) -> i32;
    pub fn sceGeListDeQueue(qid: i32) -> i32;
    pub fn sceGeListUpdateStallAddr(qid: i32, stall: *mut c_void) -> i32;
    pub fn sceGeListSync(qid: i32, sync_type: i32) -> GeListState;
    pub fn sceGeDrawSync(sync_type: i32) -> GeListState;
    pub fn sceGeBreak(mode: i32, p_param: *mut GeBreakParam) -> i32;
    pub fn sceGeContinue() -> i32;
    pub fn sceGeSetCallback(cb: *mut GeCallbackData) -> i32;
    pub fn sceGeUnsetCallback(cbid: i32) -> i32;

    pub fn sceKernelExitGame();
    pub fn sceKernelRegisterExitCallback(id: SceUid) -> i32;
    pub fn sceKernelLoadExec(file: *const u8, param: *mut SceKernelLoadExecParam) -> i32;

    pub fn sceKernelAllocPartitionMemory(
        partition: SceSysMemPartitionId,
        name: *const u8,
        type_: SceSysMemBlockTypes,
        size: u32,
        addr: *mut c_void,
    ) -> SceUid;
    pub fn sceKernelGetBlockHeadAddr(blockid: SceUid) -> *mut c_void;
    pub fn sceKernelFreePartitionMemory(blockid: SceUid) -> i32;
    pub fn sceKernelTotalFreeMemSize() -> usize;
    pub fn sceKernelMaxFreeMemSize() -> usize;
    pub fn sceKernelDevkitVersion() -> u32;
    pub fn sceKernelSetCompiledSdkVersion(version: u32) -> i32;
    pub fn sceKernelGetCompiledSdkVersion() -> u32;

    pub fn sceKernelLibcTime(t: *mut i32) -> i32;
    pub fn sceKernelLibcClock() -> u32;
    pub fn sceKernelLibcGettimeofday(tp: *mut timeval, tzp: *mut timezone) -> i32;
    pub fn sceKernelDcacheWritebackAll();
    pub fn sceKernelDcacheWritebackInvalidateAll();
    pub fn sceKernelDcacheWritebackRange(p: *const c_void, size: u32);
    pub fn sceKernelDcacheWritebackInvalidateRange(p: *const c_void, size: u32);
    pub fn sceKernelDcacheInvalidateRange(p: *const c_void, size: u32);
    pub fn sceKernelIcacheInvalidateAll();
    pub fn sceKernelIcacheInvalidateRange(p: *const c_void, size: u32);
    pub fn sceKernelUtilsMt19937Init(ctx: *mut SceKernelUtilsMt19937Context, seed: u32) -> i32;
    pub fn sceKernelUtilsMt19937UInt(ctx: *mut SceKernelUtilsMt19937Context) -> u32;
    pub fn sceKernelUtilsMd5Digest(data: *mut u8, size: u32, digest: *mut u8) -> i32;
    pub fn sceKernelUtilsMd5BlockInit(ctx: *mut SceKernelUtilsMd5Context) -> i32;
    pub fn sceKernelUtilsMd5BlockUpdate(
        ctx: *mut SceKernelUtilsMd5Context,
        data: *mut u8,
        size: u32,
    ) -> i32;
    pub fn sceKernelUtilsMd5BlockResult(ctx: *mut SceKernelUtilsMd5Context, digest: *mut u8)
        -> i32;
    pub fn sceKernelUtilsSha1Digest(data: *mut u8, size: u32, digest: *mut u8) -> i32;
    pub fn sceKernelUtilsSha1BlockInit(ctx: *mut SceKernelUtilsSha1Context) -> i32;
    pub fn sceKernelUtilsSha1BlockUpdate(
        ctx: *mut SceKernelUtilsSha1Context,
        data: *mut u8,
        size: u32,
    ) -> i32;
    pub fn sceKernelUtilsSha1BlockResult(
        ctx: *mut SceKernelUtilsSha1Context,
        digest: *mut u8,
    ) -> i32;

    pub fn sceKernelRegisterSubIntrHandler(
        int_no: i32,
        no: i32,
        handler: *mut c_void,
        arg: *mut c_void,
    ) -> i32;
    pub fn sceKernelReleaseSubIntrHandler(int_no: i32, no: i32) -> i32;
    pub fn sceKernelEnableSubIntr(int_no: i32, no: i32) -> i32;
    pub fn sceKernelDisableSubIntr(int_no: i32, no: i32) -> i32;
    pub fn QueryIntrHandlerInfo(
        intr_code: SceUid,
        sub_intr_code: SceUid,
        data: *mut IntrHandlerOptionParam,
    ) -> i32;

    pub fn sceKernelCpuSuspendIntr() -> u32;
    pub fn sceKernelCpuResumeIntr(flags: u32);
    pub fn sceKernelCpuResumeIntrWithSync(flags: u32);
    pub fn sceKernelIsCpuIntrSuspended(flags: u32) -> i32;
    pub fn sceKernelIsCpuIntrEnable() -> i32;

    pub fn sceKernelLoadModule(
        path: *const u8,
        flags: i32,
        option: *mut SceKernelLMOption,
    ) -> SceUid;
    pub fn sceKernelLoadModuleMs(
        path: *const u8,
        flags: i32,
        option: *mut SceKernelLMOption,
    ) -> SceUid;
    pub fn sceKernelLoadModuleByID(
        fid: SceUid,
        flags: i32,
        option: *mut SceKernelLMOption,
    ) -> SceUid;
    pub fn sceKernelLoadModuleBufferUsbWlan(
        buf_size: usize,
        buf: *mut c_void,
        flags: i32,
        option: *mut SceKernelLMOption,
    ) -> SceUid;
    pub fn sceKernelStartModule(
        mod_id: SceUid,
        arg_size: usize,
        argp: *mut c_void,
        status: *mut i32,
        option: *mut SceKernelSMOption,
    ) -> i32;
    pub fn sceKernelStopModule(
        mod_id: SceUid,
        arg_size: usize,
        argp: *mut c_void,
        status: *mut i32,
        option: *mut SceKernelSMOption,
    ) -> i32;
    pub fn sceKernelUnloadModule(mod_id: SceUid) -> i32;
    pub fn sceKernelSelfStopUnloadModule(unknown: i32, arg_size: usize, argp: *mut c_void) -> i32;
    pub fn sceKernelStopUnloadSelfModule(
        arg_size: usize,
        argp: *mut c_void,
        status: *mut i32,
        option: *mut SceKernelSMOption,
    ) -> i32;
    pub fn sceKernelQueryModuleInfo(mod_id: SceUid, info: *mut SceKernelModuleInfo) -> i32;
    pub fn sceKernelGetModuleIdList(
        read_buf: *mut SceUid,
        read_buf_size: i32,
        id_count: *mut i32,
    ) -> i32;

    pub fn sceKernelVolatileMemLock(unk: i32, ptr: *mut *mut c_void, size: *mut i32) -> i32;
    pub fn sceKernelVolatileMemTryLock(unk: i32, ptr: *mut *mut c_void, size: *mut i32) -> i32;
    pub fn sceKernelVolatileMemUnlock(unk: i32) -> i32;

    pub fn sceKernelStdin() -> SceUid;
    pub fn sceKernelStdout() -> SceUid;
    pub fn sceKernelStderr() -> SceUid;

    pub fn sceKernelGetThreadmanIdType(uid: SceUid) -> SceKernelIdListType;
    pub fn sceKernelCreateThread(
        name: *const u8,
        entry: SceKernelThreadEntry,
        init_priority: i32,
        stack_size: i32,
        attr: i32,
        option: *mut SceKernelThreadOptParam,
    ) -> SceUid;
    pub fn sceKernelDeleteThread(thid: SceUid) -> i32;
    pub fn sceKernelStartThread(id: SceUid, arg_len: usize, arg_p: *mut c_void) -> i32;
    pub fn sceKernelExitThread(status: i32) -> i32;
    pub fn sceKernelExitDeleteThread(status: i32) -> i32;
    pub fn sceKernelTerminateThread(thid: SceUid) -> i32;
    pub fn sceKernelTerminateDeleteThread(thid: SceUid) -> i32;
    pub fn sceKernelSuspendDispatchThread() -> i32;
    pub fn sceKernelResumeDispatchThread(state: i32) -> i32;
    pub fn sceKernelSleepThread() -> i32;
    pub fn sceKernelSleepThreadCB() -> i32;
    pub fn sceKernelWakeupThread(thid: SceUid) -> i32;
    pub fn sceKernelCancelWakeupThread(thid: SceUid) -> i32;
    pub fn sceKernelSuspendThread(thid: SceUid) -> i32;
    pub fn sceKernelResumeThread(thid: SceUid) -> i32;
    pub fn sceKernelWaitThreadEnd(thid: SceUid, timeout: *mut u32) -> i32;
    pub fn sceKernelWaitThreadEndCB(thid: SceUid, timeout: *mut u32) -> i32;
    pub fn sceKernelDelayThread(delay: u32) -> i32;
    pub fn sceKernelDelayThreadCB(delay: u32) -> i32;
    pub fn sceKernelDelaySysClockThread(delay: *mut SceKernelSysClock) -> i32;
    pub fn sceKernelDelaySysClockThreadCB(delay: *mut SceKernelSysClock) -> i32;
    pub fn sceKernelChangeCurrentThreadAttr(unknown: i32, attr: i32) -> i32;
    pub fn sceKernelChangeThreadPriority(thid: SceUid, priority: i32) -> i32;
    pub fn sceKernelRotateThreadReadyQueue(priority: i32) -> i32;
    pub fn sceKernelReleaseWaitThread(thid: SceUid) -> i32;
    pub fn sceKernelGetThreadId() -> i32;
    pub fn sceKernelGetThreadCurrentPriority() -> i32;
    pub fn sceKernelGetThreadExitStatus(thid: SceUid) -> i32;
    pub fn sceKernelCheckThreadStack() -> i32;
    pub fn sceKernelGetThreadStackFreeSize(thid: SceUid) -> i32;
    pub fn sceKernelReferThreadStatus(thid: SceUid, info: *mut SceKernelThreadInfo) -> i32;
    pub fn sceKernelReferThreadRunStatus(
        thid: SceUid,
        status: *mut SceKernelThreadRunStatus,
    ) -> i32;
    pub fn sceKernelCreateSema(
        name: *const u8,
        attr: u32,
        init_val: i32,
        max_val: i32,
        option: *mut SceKernelSemaOptParam,
    ) -> SceUid;
    pub fn sceKernelDeleteSema(sema_id: SceUid) -> i32;
    pub fn sceKernelSignalSema(sema_id: SceUid, signal: i32) -> i32;
    pub fn sceKernelWaitSema(sema_id: SceUid, signal: i32, timeout: *mut u32) -> i32;
    pub fn sceKernelWaitSemaCB(sema_id: SceUid, signal: i32, timeout: *mut u32) -> i32;
    pub fn sceKernelPollSema(sema_id: SceUid, signal: i32) -> i32;
    pub fn sceKernelReferSemaStatus(sema_id: SceUid, info: *mut SceKernelSemaInfo) -> i32;
    pub fn sceKernelCreateEventFlag(
        name: *const u8,
        attr: i32,
        bits: i32,
        opt: *mut SceKernelEventFlagOptParam,
    ) -> SceUid;
    pub fn sceKernelSetEventFlag(ev_id: SceUid, bits: u32) -> i32;
    pub fn sceKernelClearEventFlag(ev_id: SceUid, bits: u32) -> i32;
    pub fn sceKernelPollEventFlag(ev_id: SceUid, bits: u32, wait: i32, out_bits: *mut u32) -> i32;
    pub fn sceKernelWaitEventFlag(
        ev_id: SceUid,
        bits: u32,
        wait: i32,
        out_bits: *mut u32,
        timeout: *mut u32,
    ) -> i32;
    pub fn sceKernelWaitEventFlagCB(
        ev_id: SceUid,
        bits: u32,
        wait: i32,
        out_bits: *mut u32,
        timeout: *mut u32,
    ) -> i32;
    pub fn sceKernelDeleteEventFlag(ev_id: SceUid) -> i32;
    pub fn sceKernelReferEventFlagStatus(event: SceUid, status: *mut SceKernelEventFlagInfo)
        -> i32;
    pub fn sceKernelCreateMbx(
        name: *const u8,
        attr: u32,
        option: *mut SceKernelMbxOptParam,
    ) -> SceUid;
    pub fn sceKernelDeleteMbx(mbx_id: SceUid) -> i32;
    pub fn sceKernelSendMbx(mbx_id: SceUid, message: *mut c_void) -> i32;
    pub fn sceKernelReceiveMbx(mbx_id: SceUid, message: *mut *mut c_void, timeout: *mut u32)
        -> i32;
    pub fn sceKernelReceiveMbxCB(
        mbx_id: SceUid,
        message: *mut *mut c_void,
        timeout: *mut u32,
    ) -> i32;
    pub fn sceKernelPollMbx(mbx_id: SceUid, pmessage: *mut *mut c_void) -> i32;
    pub fn sceKernelCancelReceiveMbx(mbx_id: SceUid, num: *mut i32) -> i32;
    pub fn sceKernelReferMbxStatus(mbx_id: SceUid, info: *mut SceKernelMbxInfo) -> i32;
    pub fn sceKernelSetAlarm(
        clock: u32,
        handler: SceKernelAlarmHandler,
        common: *mut c_void,
    ) -> SceUid;
    pub fn sceKernelSetSysClockAlarm(
        clock: *mut SceKernelSysClock,
        handler: *mut SceKernelAlarmHandler,
        common: *mut c_void,
    ) -> SceUid;
    pub fn sceKernelCancelAlarm(alarm_id: SceUid) -> i32;
    pub fn sceKernelReferAlarmStatus(alarm_id: SceUid, info: *mut SceKernelAlarmInfo) -> i32;
    pub fn sceKernelCreateCallback(
        name: *const u8,
        func: SceKernelCallbackFunction,
        arg: *mut c_void,
    ) -> SceUid;
    pub fn sceKernelReferCallbackStatus(cb: SceUid, status: *mut SceKernelCallbackInfo) -> i32;
    pub fn sceKernelDeleteCallback(cb: SceUid) -> i32;
    pub fn sceKernelNotifyCallback(cb: SceUid, arg2: i32) -> i32;
    pub fn sceKernelCancelCallback(cb: SceUid) -> i32;
    pub fn sceKernelGetCallbackCount(cb: SceUid) -> i32;
    pub fn sceKernelCheckCallback() -> i32;
    pub fn sceKernelGetThreadmanIdList(
        type_: SceKernelIdListType,
        read_buf: *mut SceUid,
        read_buf_size: i32,
        id_count: *mut i32,
    ) -> i32;
    pub fn sceKernelReferSystemStatus(status: *mut SceKernelSystemStatus) -> i32;
    pub fn sceKernelCreateMsgPipe(
        name: *const u8,
        part: i32,
        attr: i32,
        unk1: *mut c_void,
        opt: *mut c_void,
    ) -> SceUid;
    pub fn sceKernelDeleteMsgPipe(uid: SceUid) -> i32;
    pub fn sceKernelSendMsgPipe(
        uid: SceUid,
        message: *mut c_void,
        size: u32,
        unk1: i32,
        unk2: *mut c_void,
        timeout: *mut u32,
    ) -> i32;
    pub fn sceKernelSendMsgPipeCB(
        uid: SceUid,
        message: *mut c_void,
        size: u32,
        unk1: i32,
        unk2: *mut c_void,
        timeout: *mut u32,
    ) -> i32;
    pub fn sceKernelTrySendMsgPipe(
        uid: SceUid,
        message: *mut c_void,
        size: u32,
        unk1: i32,
        unk2: *mut c_void,
    ) -> i32;
    pub fn sceKernelReceiveMsgPipe(
        uid: SceUid,
        message: *mut c_void,
        size: u32,
        unk1: i32,
        unk2: *mut c_void,
        timeout: *mut u32,
    ) -> i32;
    pub fn sceKernelReceiveMsgPipeCB(
        uid: SceUid,
        message: *mut c_void,
        size: u32,
        unk1: i32,
        unk2: *mut c_void,
        timeout: *mut u32,
    ) -> i32;
    pub fn sceKernelTryReceiveMsgPipe(
        uid: SceUid,
        message: *mut c_void,
        size: u32,
        unk1: i32,
        unk2: *mut c_void,
    ) -> i32;
    pub fn sceKernelCancelMsgPipe(uid: SceUid, send: *mut i32, recv: *mut i32) -> i32;
    pub fn sceKernelReferMsgPipeStatus(uid: SceUid, info: *mut SceKernelMppInfo) -> i32;
    pub fn sceKernelCreateVpl(
        name: *const u8,
        part: i32,
        attr: i32,
        size: u32,
        opt: *mut SceKernelVplOptParam,
    ) -> SceUid;
    pub fn sceKernelDeleteVpl(uid: SceUid) -> i32;
    pub fn sceKernelAllocateVpl(
        uid: SceUid,
        size: u32,
        data: *mut *mut c_void,
        timeout: *mut u32,
    ) -> i32;
    pub fn sceKernelAllocateVplCB(
        uid: SceUid,
        size: u32,
        data: *mut *mut c_void,
        timeout: *mut u32,
    ) -> i32;
    pub fn sceKernelTryAllocateVpl(uid: SceUid, size: u32, data: *mut *mut c_void) -> i32;
    pub fn sceKernelFreeVpl(uid: SceUid, data: *mut c_void) -> i32;
    pub fn sceKernelCancelVpl(uid: SceUid, num: *mut i32) -> i32;
    pub fn sceKernelReferVplStatus(uid: SceUid, info: *mut SceKernelVplInfo) -> i32;
    pub fn sceKernelCreateFpl(
        name: *const u8,
        part: i32,
        attr: i32,
        size: u32,
        blocks: u32,
        opt: *mut SceKernelFplOptParam,
    ) -> i32;
    pub fn sceKernelDeleteFpl(uid: SceUid) -> i32;
    pub fn sceKernelAllocateFpl(uid: SceUid, data: *mut *mut c_void, timeout: *mut u32) -> i32;
    pub fn sceKernelAllocateFplCB(uid: SceUid, data: *mut *mut c_void, timeout: *mut u32) -> i32;
    pub fn sceKernelTryAllocateFpl(uid: SceUid, data: *mut *mut c_void) -> i32;
    pub fn sceKernelFreeFpl(uid: SceUid, data: *mut c_void) -> i32;
    pub fn sceKernelCancelFpl(uid: SceUid, pnum: *mut i32) -> i32;
    pub fn sceKernelReferFplStatus(uid: SceUid, info: *mut SceKernelFplInfo) -> i32;
    pub fn sceKernelUSec2SysClock(usec: u32, clock: *mut SceKernelSysClock) -> i32;
    pub fn sceKernelUSec2SysClockWide(usec: u32) -> i64;
    pub fn sceKernelSysClock2USec(
        clock: *mut SceKernelSysClock,
        low: *mut u32,
        high: *mut u32,
    ) -> i32;
    pub fn sceKernelSysClock2USecWide(clock: i64, low: *mut u32, high: *mut u32) -> i32;
    pub fn sceKernelGetSystemTime(time: *mut SceKernelSysClock) -> i32;
    pub fn sceKernelGetSystemTimeWide() -> i64;
    pub fn sceKernelGetSystemTimeLow() -> u32;
    pub fn sceKernelCreateVTimer(name: *const u8, opt: *mut SceKernelVTimerOptParam) -> SceUid;
    pub fn sceKernelDeleteVTimer(uid: SceUid) -> i32;
    pub fn sceKernelGetVTimerBase(uid: SceUid, base: *mut SceKernelSysClock) -> i32;
    pub fn sceKernelGetVTimerBaseWide(uid: SceUid) -> i64;
    pub fn sceKernelGetVTimerTime(uid: SceUid, time: *mut SceKernelSysClock) -> i32;
    pub fn sceKernelGetVTimerTimeWide(uid: SceUid) -> i64;
    pub fn sceKernelSetVTimerTime(uid: SceUid, time: *mut SceKernelSysClock) -> i32;
    pub fn sceKernelSetVTimerTimeWide(uid: SceUid, time: i64) -> i64;
    pub fn sceKernelStartVTimer(uid: SceUid) -> i32;
    pub fn sceKernelStopVTimer(uid: SceUid) -> i32;
    pub fn sceKernelSetVTimerHandler(
        uid: SceUid,
        time: *mut SceKernelSysClock,
        handler: SceKernelVTimerHandler,
        common: *mut c_void,
    ) -> i32;
    pub fn sceKernelSetVTimerHandlerWide(
        uid: SceUid,
        time: i64,
        handler: SceKernelVTimerHandlerWide,
        common: *mut c_void,
    ) -> i32;
    pub fn sceKernelCancelVTimerHandler(uid: SceUid) -> i32;
    pub fn sceKernelReferVTimerStatus(uid: SceUid, info: *mut SceKernelVTimerInfo) -> i32;
    pub fn sceKernelRegisterThreadEventHandler(
        name: *const u8,
        thread_id: SceUid,
        mask: i32,
        handler: SceKernelThreadEventHandler,
        common: *mut c_void,
    ) -> SceUid;
    pub fn sceKernelReleaseThreadEventHandler(uid: SceUid) -> i32;
    pub fn sceKernelReferThreadEventHandlerStatus(
        uid: SceUid,
        info: *mut SceKernelThreadEventHandlerInfo,
    ) -> i32;
    pub fn sceKernelReferThreadProfiler() -> *mut DebugProfilerRegs;
    pub fn sceKernelReferGlobalProfiler() -> *mut DebugProfilerRegs;

    pub fn sceUsbStart(driver_name: *const u8, size: i32, args: *mut c_void) -> i32;
    pub fn sceUsbStop(driver_name: *const u8, size: i32, args: *mut c_void) -> i32;
    pub fn sceUsbActivate(pid: u32) -> i32;
    pub fn sceUsbDeactivate(pid: u32) -> i32;
    pub fn sceUsbGetState() -> i32;
    pub fn sceUsbGetDrvState(driver_name: *const u8) -> i32;
}

extern "C" {
    pub fn sceUsbCamSetupStill(param: *mut UsbCamSetupStillParam) -> i32;
    pub fn sceUsbCamSetupStillEx(param: *mut UsbCamSetupStillExParam) -> i32;
    pub fn sceUsbCamStillInputBlocking(buf: *mut u8, size: usize) -> i32;
    pub fn sceUsbCamStillInput(buf: *mut u8, size: usize) -> i32;
    pub fn sceUsbCamStillWaitInputEnd() -> i32;
    pub fn sceUsbCamStillPollInputEnd() -> i32;
    pub fn sceUsbCamStillCancelInput() -> i32;
    pub fn sceUsbCamStillGetInputLength() -> i32;
    pub fn sceUsbCamSetupVideo(
        param: *mut UsbCamSetupVideoParam,
        work_area: *mut c_void,
        work_area_size: i32,
    ) -> i32;
    pub fn sceUsbCamSetupVideoEx(
        param: *mut UsbCamSetupVideoExParam,
        work_area: *mut c_void,
        work_area_size: i32,
    ) -> i32;
    pub fn sceUsbCamStartVideo() -> i32;
    pub fn sceUsbCamStopVideo() -> i32;
    pub fn sceUsbCamReadVideoFrameBlocking(buf: *mut u8, size: usize) -> i32;
    pub fn sceUsbCamReadVideoFrame(buf: *mut u8, size: usize) -> i32;
    pub fn sceUsbCamWaitReadVideoFrameEnd() -> i32;
    pub fn sceUsbCamPollReadVideoFrameEnd() -> i32;
    pub fn sceUsbCamGetReadVideoFrameSize() -> i32;
    pub fn sceUsbCamSetSaturation(saturation: i32) -> i32;
    pub fn sceUsbCamSetBrightness(brightness: i32) -> i32;
    pub fn sceUsbCamSetContrast(contrast: i32) -> i32;
    pub fn sceUsbCamSetSharpness(sharpness: i32) -> i32;
    pub fn sceUsbCamSetImageEffectMode(effect_mode: UsbCamEffectMode) -> i32;
    pub fn sceUsbCamSetEvLevel(exposure_level: UsbCamEvLevel) -> i32;
    pub fn sceUsbCamSetReverseMode(reverse_flags: i32) -> i32;
    pub fn sceUsbCamSetZoom(zoom: i32) -> i32;
    pub fn sceUsbCamGetSaturation(saturation: *mut i32) -> i32;
    pub fn sceUsbCamGetBrightness(brightness: *mut i32) -> i32;
    pub fn sceUsbCamGetContrast(contrast: *mut i32) -> i32;
    pub fn sceUsbCamGetSharpness(sharpness: *mut i32) -> i32;
    pub fn sceUsbCamGetImageEffectMode(effect_mode: *mut UsbCamEffectMode) -> i32;
    pub fn sceUsbCamGetEvLevel(exposure_level: *mut UsbCamEvLevel) -> i32;
    pub fn sceUsbCamGetReverseMode(reverse_flags: *mut i32) -> i32;
    pub fn sceUsbCamGetZoom(zoom: *mut i32) -> i32;
    pub fn sceUsbCamAutoImageReverseSW(on: i32) -> i32;
    pub fn sceUsbCamGetAutoImageReverseState() -> i32;
    pub fn sceUsbCamGetLensDirection() -> i32;

    pub fn sceUsbstorBootRegisterNotify(event_flag: SceUid) -> i32;
    pub fn sceUsbstorBootUnregisterNotify(event_flag: u32) -> i32;
    pub fn sceUsbstorBootSetCapacity(size: u32) -> i32;

    pub fn scePowerRegisterCallback(slot: i32, cbid: SceUid) -> i32;
    pub fn scePowerUnregisterCallback(slot: i32) -> i32;
    pub fn scePowerIsPowerOnline() -> i32;
    pub fn scePowerIsBatteryExist() -> i32;
    pub fn scePowerIsBatteryCharging() -> i32;
    pub fn scePowerGetBatteryChargingStatus() -> i32;
    pub fn scePowerIsLowBattery() -> i32;
    pub fn scePowerGetBatteryLifePercent() -> i32;
    pub fn scePowerGetBatteryLifeTime() -> i32;
    pub fn scePowerGetBatteryTemp() -> i32;
    pub fn scePowerGetBatteryElec() -> i32;
    pub fn scePowerGetBatteryVolt() -> i32;
    pub fn scePowerSetCpuClockFrequency(cpufreq: i32) -> i32;
    pub fn scePowerSetBusClockFrequency(busfreq: i32) -> i32;
    pub fn scePowerGetCpuClockFrequency() -> i32;
    pub fn scePowerGetCpuClockFrequencyInt() -> i32;
    pub fn scePowerGetCpuClockFrequencyFloat() -> f32;
    pub fn scePowerGetBusClockFrequency() -> i32;
    pub fn scePowerGetBusClockFrequencyInt() -> i32;
    pub fn scePowerGetBusClockFrequencyFloat() -> f32;
    pub fn scePowerSetClockFrequency(pllfreq: i32, cpufreq: i32, busfreq: i32) -> i32;
    pub fn scePowerLock(unknown: i32) -> i32;
    pub fn scePowerUnlock(unknown: i32) -> i32;
    pub fn scePowerTick(t: PowerTick) -> i32;
    pub fn scePowerGetIdleTimer() -> i32;
    pub fn scePowerIdleTimerEnable(unknown: i32) -> i32;
    pub fn scePowerIdleTimerDisable(unknown: i32) -> i32;
    pub fn scePowerRequestStandby() -> i32;
    pub fn scePowerRequestSuspend() -> i32;

    pub fn sceWlanDevIsPowerOn() -> i32;
    pub fn sceWlanGetSwitchState() -> i32;
    pub fn sceWlanGetEtherAddr(ether_addr: *mut u8) -> i32;

    pub fn sceWlanDevAttach() -> i32;
    pub fn sceWlanDevDetach() -> i32;

    pub fn sceRtcGetTickResolution() -> u32;
    pub fn sceRtcGetCurrentTick(tick: *mut u64) -> i32;
    pub fn sceRtcGetCurrentClock(tm: *mut ScePspDateTime, tz: i32) -> i32;
    pub fn sceRtcGetCurrentClockLocalTime(tm: *mut ScePspDateTime) -> i32;
    pub fn sceRtcConvertUtcToLocalTime(tick_utc: *const u64, tick_local: *mut u64) -> i32;
    pub fn sceRtcConvertLocalTimeToUTC(tick_local: *const u64, tick_utc: *mut u64) -> i32;
    pub fn sceRtcIsLeapYear(year: i32) -> i32;
    pub fn sceRtcGetDaysInMonth(year: i32, month: i32) -> i32;
    pub fn sceRtcGetDayOfWeek(year: i32, month: i32, day: i32) -> i32;
    pub fn sceRtcCheckValid(date: *const ScePspDateTime) -> i32;
    pub fn sceRtcSetTick(date: *mut ScePspDateTime, tick: *const u64) -> i32;
    pub fn sceRtcGetTick(date: *const ScePspDateTime, tick: *mut u64) -> i32;
    pub fn sceRtcCompareTick(tick1: *const u64, tick2: *const u64) -> i32;
    pub fn sceRtcTickAddTicks(dest_tick: *mut u64, src_tick: *const u64, num_ticks: u64) -> i32;
    pub fn sceRtcTickAddMicroseconds(dest_tick: *mut u64, src_tick: *const u64, num_ms: u64)
        -> i32;
    pub fn sceRtcTickAddSeconds(dest_tick: *mut u64, src_tick: *const u64, num_seconds: u64)
        -> i32;
    pub fn sceRtcTickAddMinutes(dest_tick: *mut u64, src_tick: *const u64, num_minutes: u64)
        -> i32;
    pub fn sceRtcTickAddHours(dest_tick: *mut u64, src_tick: *const u64, num_hours: u64) -> i32;
    pub fn sceRtcTickAddDays(dest_tick: *mut u64, src_tick: *const u64, num_days: u64) -> i32;
    pub fn sceRtcTickAddWeeks(dest_tick: *mut u64, src_tick: *const u64, num_weeks: u64) -> i32;
    pub fn sceRtcTickAddMonths(dest_tick: *mut u64, src_tick: *const u64, num_months: u64) -> i32;
    pub fn sceRtcTickAddYears(dest_tick: *mut u64, src_tick: *const u64, num_years: u64) -> i32;
    pub fn sceRtcSetTime_t(date: *mut ScePspDateTime, time: u32) -> i32;
    pub fn sceRtcGetTime_t(date: *const ScePspDateTime, time: *mut u32) -> i32;
    pub fn sceRtcSetTime64_t(date: *mut ScePspDateTime, time: u64) -> i32;
    pub fn sceRtcGetTime64_t(date: *const ScePspDateTime, time: *mut u64) -> i32;
    pub fn sceRtcSetDosTime(date: *mut ScePspDateTime, dos_time: u32) -> i32;
    pub fn sceRtcGetDosTime(date: *mut ScePspDateTime, dos_time: u32) -> i32;
    pub fn sceRtcSetWin32FileTime(date: *mut ScePspDateTime, time: *mut u64) -> i32;
    pub fn sceRtcGetWin32FileTime(date: *mut ScePspDateTime, time: *mut u64) -> i32;
    pub fn sceRtcParseDateTime(dest_tick: *mut u64, date_string: *const u8) -> i32;
    pub fn sceRtcFormatRFC3339(
        psz_date_time: *mut c_char,
        p_utc: *const u64,
        time_zone_minutes: i32,
    ) -> i32;
    pub fn sceRtcFormatRFC3339LocalTime(psz_date_time: *mut c_char, p_utc: *const u64) -> i32;
    pub fn sceRtcParseRFC3339(p_utc: *mut u64, psz_date_time: *const u8) -> i32;
    pub fn sceRtcFormatRFC2822(
        psz_date_time: *mut c_char,
        p_utc: *const u64,
        time_zone_minutes: i32,
    ) -> i32;
    pub fn sceRtcFormatRFC2822LocalTime(psz_date_time: *mut c_char, p_utc: *const u64) -> i32;

    pub fn sceIoOpen(file: *const u8, flags: i32, permissions: IoPermissions) -> SceUid;
    pub fn sceIoOpenAsync(file: *const u8, flags: i32, permissions: IoPermissions) -> SceUid;
    pub fn sceIoClose(fd: SceUid) -> i32;
    pub fn sceIoCloseAsync(fd: SceUid) -> i32;
    pub fn sceIoRead(fd: SceUid, data: *mut c_void, size: u32) -> i32;
    pub fn sceIoReadAsync(fd: SceUid, data: *mut c_void, size: u32) -> i32;
    pub fn sceIoWrite(fd: SceUid, data: *const c_void, size: usize) -> i32;
    pub fn sceIoWriteAsync(fd: SceUid, data: *const c_void, size: u32) -> i32;
    pub fn sceIoLseek(fd: SceUid, offset: i64, whence: IoWhence) -> i64;
    pub fn sceIoLseekAsync(fd: SceUid, offset: i64, whence: IoWhence) -> i32;
    pub fn sceIoLseek32(fd: SceUid, offset: i32, whence: IoWhence) -> i32;
    pub fn sceIoLseek32Async(fd: SceUid, offset: i32, whence: IoWhence) -> i32;
    pub fn sceIoRemove(file: *const u8) -> i32;
    pub fn sceIoMkdir(dir: *const u8, mode: IoPermissions) -> i32;
    pub fn sceIoRmdir(path: *const u8) -> i32;
    pub fn sceIoChdir(path: *const u8) -> i32;
    pub fn sceIoRename(oldname: *const u8, newname: *const u8) -> i32;
    pub fn sceIoDopen(dirname: *const u8) -> SceUid;
    pub fn sceIoDread(fd: SceUid, dir: *mut SceIoDirent) -> i32;
    pub fn sceIoDclose(fd: SceUid) -> i32;
    pub fn sceIoDevctl(
        dev: *const u8,
        cmd: u32,
        indata: *mut c_void,
        inlen: i32,
        outdata: *mut c_void,
        outlen: i32,
    ) -> i32;
    pub fn sceIoAssign(
        dev1: *const u8,
        dev2: *const u8,
        dev3: *const u8,
        mode: IoAssignPerms,
        unk1: *mut c_void,
        unk2: i32,
    ) -> i32;
    pub fn sceIoUnassign(dev: *const u8) -> i32;
    pub fn sceIoGetstat(file: *const u8, stat: *mut SceIoStat) -> i32;
    pub fn sceIoChstat(file: *const u8, stat: *mut SceIoStat, bits: i32) -> i32;
    pub fn sceIoIoctl(
        fd: SceUid,
        cmd: u32,
        indata: *mut c_void,
        inlen: i32,
        outdata: *mut c_void,
        outlen: i32,
    ) -> i32;
    pub fn sceIoIoctlAsync(
        fd: SceUid,
        cmd: u32,
        indata: *mut c_void,
        inlen: i32,
        outdata: *mut c_void,
        outlen: i32,
    ) -> i32;
    pub fn sceIoSync(device: *const u8, unk: u32) -> i32;
    pub fn sceIoWaitAsync(fd: SceUid, res: *mut i64) -> i32;
    pub fn sceIoWaitAsyncCB(fd: SceUid, res: *mut i64) -> i32;
    pub fn sceIoPollAsync(fd: SceUid, res: *mut i64) -> i32;
    pub fn sceIoGetAsyncStat(fd: SceUid, poll: i32, res: *mut i64) -> i32;
    pub fn sceIoCancel(fd: SceUid) -> i32;
    pub fn sceIoGetDevType(fd: SceUid) -> i32;
    pub fn sceIoChangeAsyncPriority(fd: SceUid, pri: i32) -> i32;
    pub fn sceIoSetAsyncCallback(fd: SceUid, cb: SceUid, argp: *mut c_void) -> i32;

    pub fn sceJpegInitMJpeg() -> i32;
    pub fn sceJpegFinishMJpeg() -> i32;
    pub fn sceJpegCreateMJpeg(width: i32, height: i32) -> i32;
    pub fn sceJpegDeleteMJpeg() -> i32;
    pub fn sceJpegDecodeMJpeg(jpeg_buf: *mut u8, size: usize, rgba: *mut c_void, unk: u32) -> i32;

    pub fn sceUmdCheckMedium() -> i32;
    pub fn sceUmdGetDiscInfo(info: *mut UmdInfo) -> i32;
    pub fn sceUmdActivate(unit: i32, drive: *const u8) -> i32;
    pub fn sceUmdDeactivate(unit: i32, drive: *const u8) -> i32;
    pub fn sceUmdWaitDriveStat(state: i32) -> i32;
    pub fn sceUmdWaitDriveStatWithTimer(state: i32, timeout: u32) -> i32;
    pub fn sceUmdWaitDriveStatCB(state: i32, timeout: u32) -> i32;
    pub fn sceUmdCancelWaitDriveStat() -> i32;
    pub fn sceUmdGetDriveStat() -> i32;
    pub fn sceUmdGetErrorStat() -> i32;
    pub fn sceUmdRegisterUMDCallBack(cbid: i32) -> i32;
    pub fn sceUmdUnRegisterUMDCallBack(cbid: i32) -> i32;
    pub fn sceUmdReplacePermit() -> i32;
    pub fn sceUmdReplaceProhibit() -> i32;

    pub fn sceMpegInit() -> i32;
    pub fn sceMpegFinish();
    pub fn sceMpegRingbufferQueryMemSize(packets: i32) -> i32;
    pub fn sceMpegRingbufferConstruct(
        ringbuffer: *mut SceMpegRingbuffer,
        packets: i32,
        data: *mut c_void,
        size: i32,
        callback: SceMpegRingbufferCb,
        cb_param: *mut c_void,
    ) -> i32;
    pub fn sceMpegRingbufferDestruct(ringbuffer: *mut SceMpegRingbuffer);
    pub fn sceMpegRingbufferAvailableSize(ringbuffer: *mut SceMpegRingbuffer) -> i32;
    pub fn sceMpegRingbufferPut(
        ringbuffer: *mut SceMpegRingbuffer,
        num_packets: i32,
        available: i32,
    ) -> i32;
    pub fn sceMpegQueryMemSize(unk: i32) -> i32;
    pub fn sceMpegCreate(
        handle: SceMpeg,
        data: *mut c_void,
        size: i32,
        ringbuffer: *mut SceMpegRingbuffer,
        frame_width: i32,
        unk1: i32,
        unk2: i32,
    ) -> i32;
    pub fn sceMpegDelete(handle: SceMpeg);
    pub fn sceMpegQueryStreamOffset(handle: SceMpeg, buffer: *mut c_void, offset: *mut i32) -> i32;
    pub fn sceMpegQueryStreamSize(buffer: *mut c_void, size: *mut i32) -> i32;
    pub fn sceMpegRegistStream(handle: SceMpeg, stream_id: i32, unk: i32) -> SceMpegStream;
    pub fn sceMpegUnRegistStream(handle: SceMpeg, stream: SceMpegStream);
    pub fn sceMpegFlushAllStream(handle: SceMpeg) -> i32;
    pub fn sceMpegMallocAvcEsBuf(handle: SceMpeg) -> *mut c_void;
    pub fn sceMpegFreeAvcEsBuf(handle: SceMpeg, buf: *mut c_void);
    pub fn sceMpegQueryAtracEsSize(handle: SceMpeg, es_size: *mut i32, out_size: *mut i32) -> i32;
    pub fn sceMpegInitAu(handle: SceMpeg, es_buffer: *mut c_void, au: *mut SceMpegAu) -> i32;
    pub fn sceMpegGetAvcAu(
        handle: SceMpeg,
        stream: SceMpegStream,
        au: *mut SceMpegAu,
        unk: *mut i32,
    ) -> i32;
    pub fn sceMpegAvcDecodeMode(handle: SceMpeg, mode: *mut SceMpegAvcMode) -> i32;
    pub fn sceMpegAvcDecode(
        handle: SceMpeg,
        au: *mut SceMpegAu,
        iframe_width: i32,
        buffer: *mut c_void,
        init: *mut i32,
    ) -> i32;
    pub fn sceMpegAvcDecodeStop(
        handle: SceMpeg,
        frame_width: i32,
        buffer: *mut c_void,
        status: *mut i32,
    ) -> i32;
    pub fn sceMpegGetAtracAu(
        handle: SceMpeg,
        stream: SceMpegStream,
        au: *mut SceMpegAu,
        unk: *mut c_void,
    ) -> i32;
    pub fn sceMpegAtracDecode(
        handle: SceMpeg,
        au: *mut SceMpegAu,
        buffer: *mut c_void,
        init: i32,
    ) -> i32;

    pub fn sceMpegBaseYCrCbCopyVme(yuv_buffer: *mut c_void, buffer: *mut i32, type_: i32) -> i32;
    pub fn sceMpegBaseCscInit(width: i32) -> i32;
    pub fn sceMpegBaseCscVme(
        rgb_buffer: *mut c_void,
        rgb_buffer2: *mut c_void,
        width: i32,
        y_cr_cb_buffer: *mut SceMpegYCrCbBuffer,
    ) -> i32;
    pub fn sceMpegbase_BEA18F91(lli: *mut SceMpegLLI) -> i32;

    pub fn sceHprmPeekCurrentKey(key: *mut i32) -> i32;
    pub fn sceHprmPeekLatch(latch: *mut [u32; 4]) -> i32;
    pub fn sceHprmReadLatch(latch: *mut [u32; 4]) -> i32;
    pub fn sceHprmIsHeadphoneExist() -> i32;
    pub fn sceHprmIsRemoteExist() -> i32;
    pub fn sceHprmIsMicrophoneExist() -> i32;

    pub fn sceGuDepthBuffer(zbp: *mut c_void, zbw: i32);
    pub fn sceGuDispBuffer(width: i32, height: i32, dispbp: *mut c_void, dispbw: i32);
    pub fn sceGuDrawBuffer(psm: DisplayPixelFormat, fbp: *mut c_void, fbw: i32);
    pub fn sceGuDrawBufferList(psm: DisplayPixelFormat, fbp: *mut c_void, fbw: i32);
    pub fn sceGuDisplay(state: bool) -> bool;
    pub fn sceGuDepthFunc(function: DepthFunc);
    pub fn sceGuDepthMask(mask: i32);
    pub fn sceGuDepthOffset(offset: i32);
    pub fn sceGuDepthRange(near: i32, far: i32);
    pub fn sceGuFog(near: f32, far: f32, color: u32);
    pub fn sceGuInit();
    pub fn sceGuTerm();
    pub fn sceGuBreak(mode: i32);
    pub fn sceGuContinue();
    pub fn sceGuSetCallback(signal: GuCallbackId, callback: GuCallback) -> GuCallback;
    pub fn sceGuSignal(behavior: SignalBehavior, signal: i32);
    pub fn sceGuSendCommandf(cmd: GeCommand, argument: f32);
    pub fn sceGuSendCommandi(cmd: GeCommand, argument: i32);
    pub fn sceGuGetMemory(size: i32) -> *mut c_void;
    pub fn sceGuStart(context_type: GuContextType, list: *mut c_void);
    pub fn sceGuFinish() -> i32;
    pub fn sceGuFinishId(id: u32) -> i32;
    pub fn sceGuCallList(list: *const c_void);
    pub fn sceGuCallMode(mode: i32);
    pub fn sceGuCheckList() -> i32;
    pub fn sceGuSendList(mode: GuQueueMode, list: *const c_void, context: *mut GeContext);
    pub fn sceGuSwapBuffers() -> *mut c_void;
    pub fn sceGuSync(mode: GuSyncMode, behavior: GuSyncBehavior) -> GeListState;
    pub fn sceGuDrawArray(
        prim: GuPrimitive,
        vtype: i32,
        count: i32,
        indices: *const c_void,
        vertices: *const c_void,
    );
    pub fn sceGuBeginObject(
        vtype: i32,
        count: i32,
        indices: *const c_void,
        vertices: *const c_void,
    );
    pub fn sceGuEndObject();
    pub fn sceGuSetStatus(state: GuState, status: i32);
    pub fn sceGuGetStatus(state: GuState) -> bool;
    pub fn sceGuSetAllStatus(status: i32);
    pub fn sceGuGetAllStatus() -> i32;
    pub fn sceGuEnable(state: GuState);
    pub fn sceGuDisable(state: GuState);
    pub fn sceGuLight(light: i32, type_: LightType, components: i32, position: &ScePspFVector3);
    pub fn sceGuLightAtt(light: i32, atten0: f32, atten1: f32, atten2: f32);
    pub fn sceGuLightColor(light: i32, component: i32, color: u32);
    pub fn sceGuLightMode(mode: LightMode);
    pub fn sceGuLightSpot(light: i32, direction: &ScePspFVector3, exponent: f32, cutoff: f32);
    pub fn sceGuClear(flags: i32);
    pub fn sceGuClearColor(color: u32);
    pub fn sceGuClearDepth(depth: u32);
    pub fn sceGuClearStencil(stencil: u32);
    pub fn sceGuPixelMask(mask: u32);
    pub fn sceGuColor(color: u32);
    pub fn sceGuColorFunc(func: ColorFunc, color: u32, mask: u32);
    pub fn sceGuColorMaterial(components: i32);
    pub fn sceGuAlphaFunc(func: AlphaFunc, value: i32, mask: i32);
    pub fn sceGuAmbient(color: u32);
    pub fn sceGuAmbientColor(color: u32);
    pub fn sceGuBlendFunc(op: BlendOp, src: BlendSrc, dest: BlendDst, src_fix: u32, dest_fix: u32);
    pub fn sceGuMaterial(components: i32, color: u32);
    pub fn sceGuModelColor(emissive: u32, ambient: u32, diffuse: u32, specular: u32);
    pub fn sceGuStencilFunc(func: StencilFunc, ref_: i32, mask: i32);
    pub fn sceGuStencilOp(fail: StencilOperation, zfail: StencilOperation, zpass: StencilOperation);
    pub fn sceGuSpecular(power: f32);
    pub fn sceGuFrontFace(order: FrontFaceDirection);
    pub fn sceGuLogicalOp(op: LogicalOperation);
    pub fn sceGuSetDither(matrix: &ScePspIMatrix4);
    pub fn sceGuShadeModel(mode: ShadingModel);
    pub fn sceGuCopyImage(
        psm: DisplayPixelFormat,
        sx: i32,
        sy: i32,
        width: i32,
        height: i32,
        srcw: i32,
        src: *mut c_void,
        dx: i32,
        dy: i32,
        destw: i32,
        dest: *mut c_void,
    );
    pub fn sceGuTexEnvColor(color: u32);
    pub fn sceGuTexFilter(min: TextureFilter, mag: TextureFilter);
    pub fn sceGuTexFlush();
    pub fn sceGuTexFunc(tfx: TextureEffect, tcc: TextureColorComponent);
    pub fn sceGuTexImage(
        mipmap: MipmapLevel,
        width: i32,
        height: i32,
        tbw: i32,
        tbp: *const c_void,
    );
    pub fn sceGuTexLevelMode(mode: TextureLevelMode, bias: f32);
    pub fn sceGuTexMapMode(mode: TextureMapMode, a1: u32, a2: u32);
    pub fn sceGuTexMode(tpsm: TexturePixelFormat, maxmips: i32, a2: i32, swizzle: i32);
    pub fn sceGuTexOffset(u: f32, v: f32);
    pub fn sceGuTexProjMapMode(mode: TextureProjectionMapMode);
    pub fn sceGuTexScale(u: f32, v: f32);
    pub fn sceGuTexSlope(slope: f32);
    pub fn sceGuTexSync();
    pub fn sceGuTexWrap(u: GuTexWrapMode, v: GuTexWrapMode);
    pub fn sceGuClutLoad(num_blocks: i32, cbp: *const c_void);
    pub fn sceGuClutMode(cpsm: ClutPixelFormat, shift: u32, mask: u32, a3: u32);
    pub fn sceGuOffset(x: u32, y: u32);
    pub fn sceGuScissor(x: i32, y: i32, w: i32, h: i32);
    pub fn sceGuViewport(cx: i32, cy: i32, width: i32, height: i32);
    pub fn sceGuDrawBezier(
        v_type: i32,
        u_count: i32,
        v_count: i32,
        indices: *const c_void,
        vertices: *const c_void,
    );
    pub fn sceGuPatchDivide(ulevel: u32, vlevel: u32);
    pub fn sceGuPatchFrontFace(a0: u32);
    pub fn sceGuPatchPrim(prim: PatchPrimitive);
    pub fn sceGuDrawSpline(
        v_type: i32,
        u_count: i32,
        v_count: i32,
        u_edge: i32,
        v_edge: i32,
        indices: *const c_void,
        vertices: *const c_void,
    );
    pub fn sceGuSetMatrix(type_: MatrixMode, matrix: &ScePspFMatrix4);
    pub fn sceGuBoneMatrix(index: u32, matrix: &ScePspFMatrix4);
    pub fn sceGuMorphWeight(index: i32, weight: f32);
    pub fn sceGuDrawArrayN(
        primitive_type: GuPrimitive,
        v_type: i32,
        count: i32,
        a3: i32,
        indices: *const c_void,
        vertices: *const c_void,
    );

    pub fn sceGumDrawArray(
        prim: GuPrimitive,
        v_type: i32,
        count: i32,
        indices: *const c_void,
        vertices: *const c_void,
    );
    pub fn sceGumDrawArrayN(
        prim: GuPrimitive,
        v_type: i32,
        count: i32,
        a3: i32,
        indices: *const c_void,
        vertices: *const c_void,
    );
    pub fn sceGumDrawBezier(
        v_type: i32,
        u_count: i32,
        v_count: i32,
        indices: *const c_void,
        vertices: *const c_void,
    );
    pub fn sceGumDrawSpline(
        v_type: i32,
        u_count: i32,
        v_count: i32,
        u_edge: i32,
        v_edge: i32,
        indices: *const c_void,
        vertices: *const c_void,
    );
    pub fn sceGumFastInverse();
    pub fn sceGumFullInverse();
    pub fn sceGumLoadIdentity();
    pub fn sceGumLoadMatrix(m: &ScePspFMatrix4);
    pub fn sceGumLookAt(eye: &ScePspFVector3, center: &ScePspFVector3, up: &ScePspFVector3);
    pub fn sceGumMatrixMode(mode: MatrixMode);
    pub fn sceGumMultMatrix(m: &ScePspFMatrix4);
    pub fn sceGumOrtho(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32);
    pub fn sceGumPerspective(fovy: f32, aspect: f32, near: f32, far: f32);
    pub fn sceGumPopMatrix();
    pub fn sceGumPushMatrix();
    pub fn sceGumRotateX(angle: f32);
    pub fn sceGumRotateY(angle: f32);
    pub fn sceGumRotateZ(angle: f32);
    pub fn sceGumRotateXYZ(v: &ScePspFVector3);
    pub fn sceGumRotateZYX(v: &ScePspFVector3);
    pub fn sceGumScale(v: &ScePspFVector3);
    pub fn sceGumStoreMatrix(m: &mut ScePspFMatrix4);
    pub fn sceGumTranslate(v: &ScePspFVector3);
    pub fn sceGumUpdateMatrix();

    pub fn sceMp3ReserveMp3Handle(args: *mut SceMp3InitArg) -> i32;
    pub fn sceMp3ReleaseMp3Handle(handle: Mp3Handle) -> i32;
    pub fn sceMp3InitResource() -> i32;
    pub fn sceMp3TermResource() -> i32;
    pub fn sceMp3Init(handle: Mp3Handle) -> i32;
    pub fn sceMp3Decode(handle: Mp3Handle, dst: *mut *mut i16) -> i32;
    pub fn sceMp3GetInfoToAddStreamData(
        handle: Mp3Handle,
        dst: *mut *mut u8,
        to_write: *mut i32,
        src_pos: *mut i32,
    ) -> i32;
    pub fn sceMp3NotifyAddStreamData(handle: Mp3Handle, size: i32) -> i32;
    pub fn sceMp3CheckStreamDataNeeded(handle: Mp3Handle) -> i32;
    pub fn sceMp3SetLoopNum(handle: Mp3Handle, loop_: i32) -> i32;
    pub fn sceMp3GetLoopNum(handle: Mp3Handle) -> i32;
    pub fn sceMp3GetSumDecodedSample(handle: Mp3Handle) -> i32;
    pub fn sceMp3GetMaxOutputSample(handle: Mp3Handle) -> i32;
    pub fn sceMp3GetSamplingRate(handle: Mp3Handle) -> i32;
    pub fn sceMp3GetBitRate(handle: Mp3Handle) -> i32;
    pub fn sceMp3GetMp3ChannelNum(handle: Mp3Handle) -> i32;
    pub fn sceMp3ResetPlayPosition(handle: Mp3Handle) -> i32;

    pub fn sceRegOpenRegistry(reg: *mut Key, mode: i32, handle: *mut RegHandle) -> i32;
    pub fn sceRegFlushRegistry(handle: RegHandle) -> i32;
    pub fn sceRegCloseRegistry(handle: RegHandle) -> i32;
    pub fn sceRegOpenCategory(
        handle: RegHandle,
        name: *const u8,
        mode: i32,
        dir_handle: *mut RegHandle,
    ) -> i32;
    pub fn sceRegRemoveCategory(handle: RegHandle, name: *const u8) -> i32;
    pub fn sceRegCloseCategory(dir_handle: RegHandle) -> i32;
    pub fn sceRegFlushCategory(dir_handle: RegHandle) -> i32;
    pub fn sceRegGetKeyInfo(
        dir_handle: RegHandle,
        name: *const u8,
        key_handle: *mut RegHandle,
        type_: *mut KeyType,
        size: *mut usize,
    ) -> i32;
    pub fn sceRegGetKeyInfoByName(
        dir_handle: RegHandle,
        name: *const u8,
        type_: *mut KeyType,
        size: *mut usize,
    ) -> i32;
    pub fn sceRegGetKeyValue(
        dir_handle: RegHandle,
        key_handle: RegHandle,
        buf: *mut c_void,
        size: usize,
    ) -> i32;
    pub fn sceRegGetKeyValueByName(
        dir_handle: RegHandle,
        name: *const u8,
        buf: *mut c_void,
        size: usize,
    ) -> i32;
    pub fn sceRegSetKeyValue(
        dir_handle: RegHandle,
        name: *const u8,
        buf: *const c_void,
        size: usize,
    ) -> i32;
    pub fn sceRegGetKeysNum(dir_handle: RegHandle, num: *mut i32) -> i32;
    pub fn sceRegGetKeys(dir_handle: RegHandle, buf: *mut u8, num: i32) -> i32;
    pub fn sceRegCreateKey(dir_handle: RegHandle, name: *const u8, type_: i32, size: usize) -> i32;
    pub fn sceRegRemoveRegistry(key: *mut Key) -> i32;

    pub fn sceOpenPSIDGetOpenPSID(openpsid: *mut OpenPSID) -> i32;

    pub fn sceUtilityMsgDialogInitStart(params: *mut UtilityMsgDialogParams) -> i32;
    pub fn sceUtilityMsgDialogShutdownStart();
    pub fn sceUtilityMsgDialogGetStatus() -> i32;
    pub fn sceUtilityMsgDialogUpdate(n: i32);
    pub fn sceUtilityMsgDialogAbort() -> i32;
    pub fn sceUtilityNetconfInitStart(data: *mut UtilityNetconfData) -> i32;
    pub fn sceUtilityNetconfShutdownStart() -> i32;
    pub fn sceUtilityNetconfUpdate(unknown: i32) -> i32;
    pub fn sceUtilityNetconfGetStatus() -> i32;
    pub fn sceUtilityCheckNetParam(id: i32) -> i32;
    pub fn sceUtilityGetNetParam(conf: i32, param: NetParam, data: *mut UtilityNetData) -> i32;
    pub fn sceUtilitySavedataInitStart(params: *mut SceUtilitySavedataParam) -> i32;
    pub fn sceUtilitySavedataGetStatus() -> i32;
    pub fn sceUtilitySavedataShutdownStart() -> i32;
    pub fn sceUtilitySavedataUpdate(unknown: i32);
    pub fn sceUtilityGameSharingInitStart(params: *mut UtilityGameSharingParams) -> i32;
    pub fn sceUtilityGameSharingShutdownStart();
    pub fn sceUtilityGameSharingGetStatus() -> i32;
    pub fn sceUtilityGameSharingUpdate(n: i32);
    pub fn sceUtilityHtmlViewerInitStart(params: *mut UtilityHtmlViewerParam) -> i32;
    pub fn sceUtilityHtmlViewerShutdownStart() -> i32;
    pub fn sceUtilityHtmlViewerUpdate(n: i32) -> i32;
    pub fn sceUtilityHtmlViewerGetStatus() -> i32;
    pub fn sceUtilitySetSystemParamInt(id: SystemParamId, value: i32) -> i32;
    pub fn sceUtilitySetSystemParamString(id: SystemParamId, str: *const u8) -> i32;
    pub fn sceUtilityGetSystemParamInt(id: SystemParamId, value: *mut i32) -> i32;
    pub fn sceUtilityGetSystemParamString(id: SystemParamId, str: *mut u8, len: i32) -> i32;
    pub fn sceUtilityOskInitStart(params: *mut SceUtilityOskParams) -> i32;
    pub fn sceUtilityOskShutdownStart() -> i32;
    pub fn sceUtilityOskUpdate(n: i32) -> i32;
    pub fn sceUtilityOskGetStatus() -> i32;
    pub fn sceUtilityLoadNetModule(module: NetModule) -> i32;
    pub fn sceUtilityUnloadNetModule(module: NetModule) -> i32;
    pub fn sceUtilityLoadAvModule(module: AvModule) -> i32;
    pub fn sceUtilityUnloadAvModule(module: AvModule) -> i32;
    pub fn sceUtilityLoadUsbModule(module: UsbModule) -> i32;
    pub fn sceUtilityUnloadUsbModule(module: UsbModule) -> i32;
    pub fn sceUtilityLoadModule(module: Module) -> i32;
    pub fn sceUtilityUnloadModule(module: Module) -> i32;
    pub fn sceUtilityCreateNetParam(conf: i32) -> i32;
    pub fn sceUtilitySetNetParam(param: NetParam, val: *const c_void) -> i32;
    pub fn sceUtilityCopyNetParam(src: i32, dest: i32) -> i32;
    pub fn sceUtilityDeleteNetParam(conf: i32) -> i32;

    pub fn sceNetInit(
        poolsize: i32,
        calloutprio: i32,
        calloutstack: i32,
        netintrprio: i32,
        netintrstack: i32,
    ) -> i32;
    pub fn sceNetTerm() -> i32;
    pub fn sceNetFreeThreadinfo(thid: i32) -> i32;
    pub fn sceNetThreadAbort(thid: i32) -> i32;
    pub fn sceNetEtherStrton(name: *mut u8, mac: *mut u8);
    pub fn sceNetEtherNtostr(mac: *mut u8, name: *mut u8);
    pub fn sceNetGetLocalEtherAddr(mac: *mut u8) -> i32;
    pub fn sceNetGetMallocStat(stat: *mut SceNetMallocStat) -> i32;

    pub fn sceNetAdhocctlInit(
        stacksize: i32,
        priority: i32,
        adhoc_id: *mut SceNetAdhocctlAdhocId,
    ) -> i32;
    pub fn sceNetAdhocctlTerm() -> i32;
    pub fn sceNetAdhocctlConnect(name: *const u8) -> i32;
    pub fn sceNetAdhocctlDisconnect() -> i32;
    pub fn sceNetAdhocctlGetState(event: *mut i32) -> i32;
    pub fn sceNetAdhocctlCreate(name: *const u8) -> i32;
    pub fn sceNetAdhocctlJoin(scaninfo: *mut SceNetAdhocctlScanInfo) -> i32;
    pub fn sceNetAdhocctlGetAdhocId(id: *mut SceNetAdhocctlAdhocId) -> i32;
    pub fn sceNetAdhocctlCreateEnterGameMode(
        name: *const u8,
        unknown: i32,
        num: i32,
        macs: *mut u8,
        timeout: u32,
        unknown2: i32,
    ) -> i32;
    pub fn sceNetAdhocctlJoinEnterGameMode(
        name: *const u8,
        hostmac: *mut u8,
        timeout: u32,
        unknown: i32,
    ) -> i32;
    pub fn sceNetAdhocctlGetGameModeInfo(gamemodeinfo: *mut SceNetAdhocctlGameModeInfo) -> i32;
    pub fn sceNetAdhocctlExitGameMode() -> i32;
    pub fn sceNetAdhocctlGetPeerList(length: *mut i32, buf: *mut c_void) -> i32;
    pub fn sceNetAdhocctlGetPeerInfo(
        mac: *mut u8,
        size: i32,
        peerinfo: *mut SceNetAdhocctlPeerInfo,
    ) -> i32;
    pub fn sceNetAdhocctlScan() -> i32;
    pub fn sceNetAdhocctlGetScanInfo(length: *mut i32, buf: *mut c_void) -> i32;
    pub fn sceNetAdhocctlAddHandler(handler: SceNetAdhocctlHandler, unknown: *mut c_void) -> i32;
    pub fn sceNetAdhocctlDelHandler(id: i32) -> i32;
    pub fn sceNetAdhocctlGetNameByAddr(mac: *mut u8, nickname: *mut u8) -> i32;
    pub fn sceNetAdhocctlGetAddrByName(
        nickname: *mut u8,
        length: *mut i32,
        buf: *mut c_void,
    ) -> i32;
    pub fn sceNetAdhocctlGetParameter(params: *mut SceNetAdhocctlParams) -> i32;

    pub fn sceNetAdhocInit() -> i32;
    pub fn sceNetAdhocTerm() -> i32;
    pub fn sceNetAdhocPdpCreate(mac: *mut u8, port: u16, buf_size: u32, unk1: i32) -> i32;
    pub fn sceNetAdhocPdpDelete(id: i32, unk1: i32) -> i32;
    pub fn sceNetAdhocPdpSend(
        id: i32,
        dest_mac_addr: *mut u8,
        port: u16,
        data: *mut c_void,
        len: u32,
        timeout: u32,
        nonblock: i32,
    ) -> i32;
    pub fn sceNetAdhocPdpRecv(
        id: i32,
        src_mac_addr: *mut u8,
        port: *mut u16,
        data: *mut c_void,
        data_length: *mut c_void,
        timeout: u32,
        nonblock: i32,
    ) -> i32;
    pub fn sceNetAdhocGetPdpStat(size: *mut i32, stat: *mut SceNetAdhocPdpStat) -> i32;
    pub fn sceNetAdhocGameModeCreateMaster(data: *mut c_void, size: i32) -> i32;
    pub fn sceNetAdhocGameModeCreateReplica(mac: *mut u8, data: *mut c_void, size: i32) -> i32;
    pub fn sceNetAdhocGameModeUpdateMaster() -> i32;
    pub fn sceNetAdhocGameModeUpdateReplica(id: i32, unk1: i32) -> i32;
    pub fn sceNetAdhocGameModeDeleteMaster() -> i32;
    pub fn sceNetAdhocGameModeDeleteReplica(id: i32) -> i32;
    pub fn sceNetAdhocPtpOpen(
        srcmac: *mut u8,
        srcport: u16,
        destmac: *mut u8,
        destport: u16,
        buf_size: u32,
        delay: u32,
        count: i32,
        unk1: i32,
    ) -> i32;
    pub fn sceNetAdhocPtpConnect(id: i32, timeout: u32, nonblock: i32) -> i32;
    pub fn sceNetAdhocPtpListen(
        srcmac: *mut u8,
        srcport: u16,
        buf_size: u32,
        delay: u32,
        count: i32,
        queue: i32,
        unk1: i32,
    ) -> i32;
    pub fn sceNetAdhocPtpAccept(
        id: i32,
        mac: *mut u8,
        port: *mut u16,
        timeout: u32,
        nonblock: i32,
    ) -> i32;
    pub fn sceNetAdhocPtpSend(
        id: i32,
        data: *mut c_void,
        data_size: *mut i32,
        timeout: u32,
        nonblock: i32,
    ) -> i32;
    pub fn sceNetAdhocPtpRecv(
        id: i32,
        data: *mut c_void,
        data_size: *mut i32,
        timeout: u32,
        nonblock: i32,
    ) -> i32;
    pub fn sceNetAdhocPtpFlush(id: i32, timeout: u32, nonblock: i32) -> i32;
    pub fn sceNetAdhocPtpClose(id: i32, unk1: i32) -> i32;
    pub fn sceNetAdhocGetPtpStat(size: *mut i32, stat: *mut SceNetAdhocPtpStat) -> i32;
}

extern "C" {
    pub fn sceNetAdhocMatchingInit(memsize: i32) -> i32;
    pub fn sceNetAdhocMatchingTerm() -> i32;
    pub fn sceNetAdhocMatchingCreate(
        mode: AdhocMatchingMode,
        max_peers: i32,
        port: u16,
        buf_size: i32,
        hello_delay: u32,
        ping_delay: u32,
        init_count: i32,
        msg_delay: u32,
        callback: AdhocMatchingCallback,
    ) -> i32;
    pub fn sceNetAdhocMatchingDelete(matching_id: i32) -> i32;
    pub fn sceNetAdhocMatchingStart(
        matching_id: i32,
        evth_pri: i32,
        evth_stack: i32,
        inth_pri: i32,
        inth_stack: i32,
        opt_len: i32,
        opt_data: *mut c_void,
    ) -> i32;
    pub fn sceNetAdhocMatchingStop(matching_id: i32) -> i32;
    pub fn sceNetAdhocMatchingSelectTarget(
        matching_id: i32,
        mac: *mut u8,
        opt_len: i32,
        opt_data: *mut c_void,
    ) -> i32;
    pub fn sceNetAdhocMatchingCancelTarget(matching_id: i32, mac: *mut u8) -> i32;
    pub fn sceNetAdhocMatchingCancelTargetWithOpt(
        matching_id: i32,
        mac: *mut u8,
        opt_len: i32,
        opt_data: *mut c_void,
    ) -> i32;
    pub fn sceNetAdhocMatchingSendData(
        matching_id: i32,
        mac: *mut u8,
        data_len: i32,
        data: *mut c_void,
    ) -> i32;
    pub fn sceNetAdhocMatchingAbortSendData(matching_id: i32, mac: *mut u8) -> i32;
    pub fn sceNetAdhocMatchingSetHelloOpt(
        matching_id: i32,
        opt_len: i32,
        opt_data: *mut c_void,
    ) -> i32;
    pub fn sceNetAdhocMatchingGetHelloOpt(
        matching_id: i32,
        opt_len: *mut i32,
        opt_data: *mut c_void,
    ) -> i32;
    pub fn sceNetAdhocMatchingGetMembers(
        matching_id: i32,
        length: *mut i32,
        buf: *mut c_void,
    ) -> i32;
    pub fn sceNetAdhocMatchingGetPoolMaxAlloc() -> i32;
    pub fn sceNetAdhocMatchingGetPoolStat(poolstat: *mut AdhocPoolStat) -> i32;
}

extern "C" {
    pub fn sceNetApctlInit(stack_size: i32, init_priority: i32) -> i32;
    pub fn sceNetApctlTerm() -> i32;
    pub fn sceNetApctlGetInfo(code: ApctlInfo, pinfo: *mut SceNetApctlInfo) -> i32;
    pub fn sceNetApctlAddHandler(handler: SceNetApctlHandler, parg: *mut c_void) -> i32;
    pub fn sceNetApctlDelHandler(handler_id: i32) -> i32;
    pub fn sceNetApctlConnect(conn_index: i32) -> i32;
    pub fn sceNetApctlDisconnect() -> i32;
    pub fn sceNetApctlGetState(pstate: *mut ApctlState) -> i32;

    pub fn sceNetInetInit() -> i32;
    pub fn sceNetInetTerm() -> i32;
    pub fn sceNetInetAccept(s: i32, addr: *mut sockaddr, addr_len: *mut socklen_t) -> i32;
    pub fn sceNetInetBind(s: i32, my_addr: *const sockaddr, addr_len: socklen_t) -> i32;
    pub fn sceNetInetConnect(s: i32, serv_addr: *const sockaddr, addr_len: socklen_t) -> i32;
    pub fn sceNetInetGetsockopt(
        s: i32,
        level: i32,
        opt_name: i32,
        opt_val: *mut c_void,
        optl_en: *mut socklen_t,
    ) -> i32;
    pub fn sceNetInetListen(s: i32, backlog: i32) -> i32;
    pub fn sceNetInetRecv(s: i32, buf: *mut c_void, len: usize, flags: i32) -> usize;
    pub fn sceNetInetRecvfrom(
        s: i32,
        buf: *mut c_void,
        flags: usize,
        arg1: i32,
        from: *mut sockaddr,
        from_len: *mut socklen_t,
    ) -> usize;
    pub fn sceNetInetSend(s: i32, buf: *const c_void, len: usize, flags: i32) -> usize;
    pub fn sceNetInetSendto(
        s: i32,
        buf: *const c_void,
        len: usize,
        flags: i32,
        to: *const sockaddr,
        to_len: socklen_t,
    ) -> usize;
    pub fn sceNetInetSetsockopt(
        s: i32,
        level: i32,
        opt_name: i32,
        opt_val: *const c_void,
        opt_len: socklen_t,
    ) -> i32;
    pub fn sceNetInetShutdown(s: i32, how: i32) -> i32;
    pub fn sceNetInetSocket(domain: i32, type_: i32, protocol: i32) -> i32;
    pub fn sceNetInetClose(s: i32) -> i32;
    pub fn sceNetInetGetErrno() -> i32;

    pub fn sceSslInit(unknown1: i32) -> i32;
    pub fn sceSslEnd() -> i32;
    pub fn sceSslGetUsedMemoryMax(memory: *mut u32) -> i32;
    pub fn sceSslGetUsedMemoryCurrent(memory: *mut u32) -> i32;

    pub fn sceHttpInit(unknown1: u32) -> i32;
    pub fn sceHttpEnd() -> i32;
    pub fn sceHttpCreateTemplate(agent: *mut u8, unknown1: i32, unknown2: i32) -> i32;
    pub fn sceHttpDeleteTemplate(templateid: i32) -> i32;
    pub fn sceHttpCreateConnection(
        templateid: i32,
        host: *mut u8,
        unknown1: *mut u8,
        port: u16,
        unknown2: i32,
    ) -> i32;
    pub fn sceHttpCreateConnectionWithURL(templateid: i32, url: *const u8, unknown1: i32) -> i32;
    pub fn sceHttpDeleteConnection(connection_id: i32) -> i32;
    pub fn sceHttpCreateRequest(
        connection_id: i32,
        method: HttpMethod,
        path: *mut u8,
        content_length: u64,
    ) -> i32;
    pub fn sceHttpCreateRequestWithURL(
        connection_id: i32,
        method: HttpMethod,
        url: *mut u8,
        content_length: u64,
    ) -> i32;
    pub fn sceHttpDeleteRequest(request_id: i32) -> i32;
    pub fn sceHttpSendRequest(request_id: i32, data: *mut c_void, data_size: u32) -> i32;
    pub fn sceHttpAbortRequest(request_id: i32) -> i32;
    pub fn sceHttpReadData(request_id: i32, data: *mut c_void, data_size: u32) -> i32;
    pub fn sceHttpGetContentLength(request_id: i32, content_length: *mut u64) -> i32;
    pub fn sceHttpGetStatusCode(request_id: i32, status_code: *mut i32) -> i32;
    pub fn sceHttpSetResolveTimeOut(id: i32, timeout: u32) -> i32;
    pub fn sceHttpSetResolveRetry(id: i32, count: i32) -> i32;
    pub fn sceHttpSetConnectTimeOut(id: i32, timeout: u32) -> i32;
    pub fn sceHttpSetSendTimeOut(id: i32, timeout: u32) -> i32;
    pub fn sceHttpSetRecvTimeOut(id: i32, timeout: u32) -> i32;
    pub fn sceHttpEnableKeepAlive(id: i32) -> i32;
    pub fn sceHttpDisableKeepAlive(id: i32) -> i32;
    pub fn sceHttpEnableRedirect(id: i32) -> i32;
    pub fn sceHttpDisableRedirect(id: i32) -> i32;
    pub fn sceHttpEnableCookie(id: i32) -> i32;
    pub fn sceHttpDisableCookie(id: i32) -> i32;
    pub fn sceHttpSaveSystemCookie() -> i32;
    pub fn sceHttpLoadSystemCookie() -> i32;
    pub fn sceHttpAddExtraHeader(id: i32, name: *mut u8, value: *mut u8, unknown1: i32) -> i32;
    pub fn sceHttpDeleteHeader(id: i32, name: *const u8) -> i32;
    pub fn sceHttpsInit(unknown1: i32, unknown2: i32, unknown3: i32, unknown4: i32) -> i32;
    pub fn sceHttpsEnd() -> i32;
    pub fn sceHttpsLoadDefaultCert(unknown1: i32, unknown2: i32) -> i32;
    pub fn sceHttpDisableAuth(id: i32) -> i32;
    pub fn sceHttpDisableCache(id: i32) -> i32;
    pub fn sceHttpEnableAuth(id: i32) -> i32;
    pub fn sceHttpEnableCache(id: i32) -> i32;
    pub fn sceHttpEndCache() -> i32;
    pub fn sceHttpGetAllHeader(request: i32, header: *mut *mut u8, header_size: *mut u32) -> i32;
    pub fn sceHttpGetNetworkErrno(request: i32, err_num: *mut i32) -> i32;
    pub fn sceHttpGetProxy(
        id: i32,
        activate_flag: *mut i32,
        mode: *mut i32,
        proxy_host: *mut u8,
        len: usize,
        proxy_port: *mut u16,
    ) -> i32;
    pub fn sceHttpInitCache(max_size: usize) -> i32;
    pub fn sceHttpSetAuthInfoCB(id: i32, cbfunc: HttpPasswordCB) -> i32;
    pub fn sceHttpSetProxy(
        id: i32,
        activate_flag: i32,
        mode: i32,
        new_proxy_host: *const u8,
        new_proxy_port: u16,
    ) -> i32;
    pub fn sceHttpSetResHeaderMaxSize(id: i32, header_size: u32) -> i32;
    pub fn sceHttpSetMallocFunction(
        malloc_func: HttpMallocFunction,
        free_func: HttpFreeFunction,
        realloc_func: HttpReallocFunction,
    ) -> i32;

    pub fn sceNetResolverInit() -> i32;
    pub fn sceNetResolverCreate(rid: *mut i32, buf: *mut c_void, buf_length: u32) -> i32;
    pub fn sceNetResolverDelete(rid: i32) -> i32;
    pub fn sceNetResolverStartNtoA(
        rid: i32,
        hostname: *const u8,
        addr: *mut in_addr,
        timeout: u32,
        retry: i32,
    ) -> i32;
    pub fn sceNetResolverStartAtoN(
        rid: i32,
        addr: *const in_addr,
        hostname: *mut u8,
        hostname_len: u32,
        timeout: u32,
        retry: i32,
    ) -> i32;
    pub fn sceNetResolverStop(rid: i32) -> i32;
    pub fn sceNetResolverTerm() -> i32;
}
