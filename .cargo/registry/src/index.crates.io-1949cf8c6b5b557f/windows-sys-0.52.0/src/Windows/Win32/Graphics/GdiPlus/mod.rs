::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathArc(path : *mut GpPath, x : f32, y : f32, width : f32, height : f32, startangle : f32, sweepangle : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathArcI(path : *mut GpPath, x : i32, y : i32, width : i32, height : i32, startangle : f32, sweepangle : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathBezier(path : *mut GpPath, x1 : f32, y1 : f32, x2 : f32, y2 : f32, x3 : f32, y3 : f32, x4 : f32, y4 : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathBezierI(path : *mut GpPath, x1 : i32, y1 : i32, x2 : i32, y2 : i32, x3 : i32, y3 : i32, x4 : i32, y4 : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathBeziers(path : *mut GpPath, points : *const PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathBeziersI(path : *mut GpPath, points : *const Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathClosedCurve(path : *mut GpPath, points : *const PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathClosedCurve2(path : *mut GpPath, points : *const PointF, count : i32, tension : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathClosedCurve2I(path : *mut GpPath, points : *const Point, count : i32, tension : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathClosedCurveI(path : *mut GpPath, points : *const Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathCurve(path : *mut GpPath, points : *const PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathCurve2(path : *mut GpPath, points : *const PointF, count : i32, tension : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathCurve2I(path : *mut GpPath, points : *const Point, count : i32, tension : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathCurve3(path : *mut GpPath, points : *const PointF, count : i32, offset : i32, numberofsegments : i32, tension : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathCurve3I(path : *mut GpPath, points : *const Point, count : i32, offset : i32, numberofsegments : i32, tension : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathCurveI(path : *mut GpPath, points : *const Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathEllipse(path : *mut GpPath, x : f32, y : f32, width : f32, height : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathEllipseI(path : *mut GpPath, x : i32, y : i32, width : i32, height : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathLine(path : *mut GpPath, x1 : f32, y1 : f32, x2 : f32, y2 : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathLine2(path : *mut GpPath, points : *const PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathLine2I(path : *mut GpPath, points : *const Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathLineI(path : *mut GpPath, x1 : i32, y1 : i32, x2 : i32, y2 : i32) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipAddPathPath(path : *mut GpPath, addingpath : *const GpPath, connect : super::super::Foundation:: BOOL) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathPie(path : *mut GpPath, x : f32, y : f32, width : f32, height : f32, startangle : f32, sweepangle : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathPieI(path : *mut GpPath, x : i32, y : i32, width : i32, height : i32, startangle : f32, sweepangle : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathPolygon(path : *mut GpPath, points : *const PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathPolygonI(path : *mut GpPath, points : *const Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathRectangle(path : *mut GpPath, x : f32, y : f32, width : f32, height : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathRectangleI(path : *mut GpPath, x : i32, y : i32, width : i32, height : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathRectangles(path : *mut GpPath, rects : *const RectF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathRectanglesI(path : *mut GpPath, rects : *const Rect, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathString(path : *mut GpPath, string : ::windows_sys::core::PCWSTR, length : i32, family : *const GpFontFamily, style : i32, emsize : f32, layoutrect : *const RectF, format : *const GpStringFormat) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAddPathStringI(path : *mut GpPath, string : ::windows_sys::core::PCWSTR, length : i32, family : *const GpFontFamily, style : i32, emsize : f32, layoutrect : *const Rect, format : *const GpStringFormat) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipAlloc(size : usize) -> *mut ::core::ffi::c_void);
::windows_targets::link!("gdiplus.dll" "system" fn GdipBeginContainer(graphics : *mut GpGraphics, dstrect : *const RectF, srcrect : *const RectF, unit : Unit, state : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipBeginContainer2(graphics : *mut GpGraphics, state : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipBeginContainerI(graphics : *mut GpGraphics, dstrect : *const Rect, srcrect : *const Rect, unit : Unit, state : *mut u32) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipBitmapApplyEffect(bitmap : *mut GpBitmap, effect : *mut CGpEffect, roi : *mut super::super::Foundation:: RECT, useauxdata : super::super::Foundation:: BOOL, auxdata : *mut *mut ::core::ffi::c_void, auxdatasize : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipBitmapConvertFormat(pinputbitmap : *mut GpBitmap, format : i32, dithertype : DitherType, palettetype : PaletteType, palette : *mut ColorPalette, alphathresholdpercent : f32) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipBitmapCreateApplyEffect(inputbitmaps : *mut *mut GpBitmap, numinputs : i32, effect : *mut CGpEffect, roi : *mut super::super::Foundation:: RECT, outputrect : *mut super::super::Foundation:: RECT, outputbitmap : *mut *mut GpBitmap, useauxdata : super::super::Foundation:: BOOL, auxdata : *mut *mut ::core::ffi::c_void, auxdatasize : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipBitmapGetHistogram(bitmap : *mut GpBitmap, format : HistogramFormat, numberofentries : u32, channel0 : *mut u32, channel1 : *mut u32, channel2 : *mut u32, channel3 : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipBitmapGetHistogramSize(format : HistogramFormat, numberofentries : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipBitmapGetPixel(bitmap : *mut GpBitmap, x : i32, y : i32, color : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipBitmapLockBits(bitmap : *mut GpBitmap, rect : *const Rect, flags : u32, format : i32, lockedbitmapdata : *mut BitmapData) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipBitmapSetPixel(bitmap : *mut GpBitmap, x : i32, y : i32, color : u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipBitmapSetResolution(bitmap : *mut GpBitmap, xdpi : f32, ydpi : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipBitmapUnlockBits(bitmap : *mut GpBitmap, lockedbitmapdata : *mut BitmapData) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipClearPathMarkers(path : *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCloneBitmapArea(x : f32, y : f32, width : f32, height : f32, format : i32, srcbitmap : *mut GpBitmap, dstbitmap : *mut *mut GpBitmap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCloneBitmapAreaI(x : i32, y : i32, width : i32, height : i32, format : i32, srcbitmap : *mut GpBitmap, dstbitmap : *mut *mut GpBitmap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCloneBrush(brush : *mut GpBrush, clonebrush : *mut *mut GpBrush) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCloneCustomLineCap(customcap : *mut GpCustomLineCap, clonedcap : *mut *mut GpCustomLineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCloneFont(font : *mut GpFont, clonefont : *mut *mut GpFont) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCloneFontFamily(fontfamily : *mut GpFontFamily, clonedfontfamily : *mut *mut GpFontFamily) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCloneImage(image : *mut GpImage, cloneimage : *mut *mut GpImage) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCloneImageAttributes(imageattr : *const GpImageAttributes, cloneimageattr : *mut *mut GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCloneMatrix(matrix : *mut Matrix, clonematrix : *mut *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipClonePath(path : *mut GpPath, clonepath : *mut *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipClonePen(pen : *mut GpPen, clonepen : *mut *mut GpPen) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCloneRegion(region : *mut GpRegion, cloneregion : *mut *mut GpRegion) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCloneStringFormat(format : *const GpStringFormat, newformat : *mut *mut GpStringFormat) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipClosePathFigure(path : *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipClosePathFigures(path : *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCombineRegionPath(region : *mut GpRegion, path : *mut GpPath, combinemode : CombineMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCombineRegionRect(region : *mut GpRegion, rect : *const RectF, combinemode : CombineMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCombineRegionRectI(region : *mut GpRegion, rect : *const Rect, combinemode : CombineMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCombineRegionRegion(region : *mut GpRegion, region2 : *mut GpRegion, combinemode : CombineMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipComment(graphics : *mut GpGraphics, sizedata : u32, data : *const u8) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipConvertToEmfPlus(refgraphics : *const GpGraphics, metafile : *mut GpMetafile, conversionfailureflag : *mut i32, emftype : EmfType, description : ::windows_sys::core::PCWSTR, out_metafile : *mut *mut GpMetafile) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipConvertToEmfPlusToFile(refgraphics : *const GpGraphics, metafile : *mut GpMetafile, conversionfailureflag : *mut i32, filename : ::windows_sys::core::PCWSTR, emftype : EmfType, description : ::windows_sys::core::PCWSTR, out_metafile : *mut *mut GpMetafile) -> Status);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn GdipConvertToEmfPlusToStream(refgraphics : *const GpGraphics, metafile : *mut GpMetafile, conversionfailureflag : *mut i32, stream : super::super::System::Com:: IStream, emftype : EmfType, description : ::windows_sys::core::PCWSTR, out_metafile : *mut *mut GpMetafile) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipCreateAdjustableArrowCap(height : f32, width : f32, isfilled : super::super::Foundation:: BOOL, cap : *mut *mut GpAdjustableArrowCap) -> Status);
#[cfg(feature = "Win32_Graphics_DirectDraw")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_DirectDraw\"`"] fn GdipCreateBitmapFromDirectDrawSurface(surface : super::DirectDraw:: IDirectDrawSurface7, bitmap : *mut *mut GpBitmap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateBitmapFromFile(filename : ::windows_sys::core::PCWSTR, bitmap : *mut *mut GpBitmap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateBitmapFromFileICM(filename : ::windows_sys::core::PCWSTR, bitmap : *mut *mut GpBitmap) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipCreateBitmapFromGdiDib(gdibitmapinfo : *const super::Gdi:: BITMAPINFO, gdibitmapdata : *mut ::core::ffi::c_void, bitmap : *mut *mut GpBitmap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateBitmapFromGraphics(width : i32, height : i32, target : *mut GpGraphics, bitmap : *mut *mut GpBitmap) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipCreateBitmapFromHBITMAP(hbm : super::Gdi:: HBITMAP, hpal : super::Gdi:: HPALETTE, bitmap : *mut *mut GpBitmap) -> Status);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn GdipCreateBitmapFromHICON(hicon : super::super::UI::WindowsAndMessaging:: HICON, bitmap : *mut *mut GpBitmap) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipCreateBitmapFromResource(hinstance : super::super::Foundation:: HINSTANCE, lpbitmapname : ::windows_sys::core::PCWSTR, bitmap : *mut *mut GpBitmap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateBitmapFromScan0(width : i32, height : i32, stride : i32, format : i32, scan0 : *const u8, bitmap : *mut *mut GpBitmap) -> Status);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn GdipCreateBitmapFromStream(stream : super::super::System::Com:: IStream, bitmap : *mut *mut GpBitmap) -> Status);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn GdipCreateBitmapFromStreamICM(stream : super::super::System::Com:: IStream, bitmap : *mut *mut GpBitmap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateCachedBitmap(bitmap : *mut GpBitmap, graphics : *mut GpGraphics, cachedbitmap : *mut *mut GpCachedBitmap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateCustomLineCap(fillpath : *mut GpPath, strokepath : *mut GpPath, basecap : LineCap, baseinset : f32, customcap : *mut *mut GpCustomLineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateEffect(guid : ::windows_sys::core::GUID, effect : *mut *mut CGpEffect) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateFont(fontfamily : *const GpFontFamily, emsize : f32, style : i32, unit : Unit, font : *mut *mut GpFont) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateFontFamilyFromName(name : ::windows_sys::core::PCWSTR, fontcollection : *mut GpFontCollection, fontfamily : *mut *mut GpFontFamily) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipCreateFontFromDC(hdc : super::Gdi:: HDC, font : *mut *mut GpFont) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipCreateFontFromLogfontA(hdc : super::Gdi:: HDC, logfont : *const super::Gdi:: LOGFONTA, font : *mut *mut GpFont) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipCreateFontFromLogfontW(hdc : super::Gdi:: HDC, logfont : *const super::Gdi:: LOGFONTW, font : *mut *mut GpFont) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipCreateFromHDC(hdc : super::Gdi:: HDC, graphics : *mut *mut GpGraphics) -> Status);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GdipCreateFromHDC2(hdc : super::Gdi:: HDC, hdevice : super::super::Foundation:: HANDLE, graphics : *mut *mut GpGraphics) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipCreateFromHWND(hwnd : super::super::Foundation:: HWND, graphics : *mut *mut GpGraphics) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipCreateFromHWNDICM(hwnd : super::super::Foundation:: HWND, graphics : *mut *mut GpGraphics) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipCreateHBITMAPFromBitmap(bitmap : *mut GpBitmap, hbmreturn : *mut super::Gdi:: HBITMAP, background : u32) -> Status);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn GdipCreateHICONFromBitmap(bitmap : *mut GpBitmap, hbmreturn : *mut super::super::UI::WindowsAndMessaging:: HICON) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipCreateHalftonePalette() -> super::Gdi:: HPALETTE);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateHatchBrush(hatchstyle : HatchStyle, forecol : u32, backcol : u32, brush : *mut *mut GpHatch) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateImageAttributes(imageattr : *mut *mut GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateLineBrush(point1 : *const PointF, point2 : *const PointF, color1 : u32, color2 : u32, wrapmode : WrapMode, linegradient : *mut *mut GpLineGradient) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateLineBrushFromRect(rect : *const RectF, color1 : u32, color2 : u32, mode : LinearGradientMode, wrapmode : WrapMode, linegradient : *mut *mut GpLineGradient) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateLineBrushFromRectI(rect : *const Rect, color1 : u32, color2 : u32, mode : LinearGradientMode, wrapmode : WrapMode, linegradient : *mut *mut GpLineGradient) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipCreateLineBrushFromRectWithAngle(rect : *const RectF, color1 : u32, color2 : u32, angle : f32, isanglescalable : super::super::Foundation:: BOOL, wrapmode : WrapMode, linegradient : *mut *mut GpLineGradient) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipCreateLineBrushFromRectWithAngleI(rect : *const Rect, color1 : u32, color2 : u32, angle : f32, isanglescalable : super::super::Foundation:: BOOL, wrapmode : WrapMode, linegradient : *mut *mut GpLineGradient) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateLineBrushI(point1 : *const Point, point2 : *const Point, color1 : u32, color2 : u32, wrapmode : WrapMode, linegradient : *mut *mut GpLineGradient) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateMatrix(matrix : *mut *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateMatrix2(m11 : f32, m12 : f32, m21 : f32, m22 : f32, dx : f32, dy : f32, matrix : *mut *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateMatrix3(rect : *const RectF, dstplg : *const PointF, matrix : *mut *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateMatrix3I(rect : *const Rect, dstplg : *const Point, matrix : *mut *mut Matrix) -> Status);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GdipCreateMetafileFromEmf(hemf : super::Gdi:: HENHMETAFILE, deleteemf : super::super::Foundation:: BOOL, metafile : *mut *mut GpMetafile) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateMetafileFromFile(file : ::windows_sys::core::PCWSTR, metafile : *mut *mut GpMetafile) -> Status);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn GdipCreateMetafileFromStream(stream : super::super::System::Com:: IStream, metafile : *mut *mut GpMetafile) -> Status);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GdipCreateMetafileFromWmf(hwmf : super::Gdi:: HMETAFILE, deletewmf : super::super::Foundation:: BOOL, wmfplaceablefileheader : *const WmfPlaceableFileHeader, metafile : *mut *mut GpMetafile) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateMetafileFromWmfFile(file : ::windows_sys::core::PCWSTR, wmfplaceablefileheader : *const WmfPlaceableFileHeader, metafile : *mut *mut GpMetafile) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreatePath(brushmode : FillMode, path : *mut *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreatePath2(param0 : *const PointF, param1 : *const u8, param2 : i32, param3 : FillMode, path : *mut *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreatePath2I(param0 : *const Point, param1 : *const u8, param2 : i32, param3 : FillMode, path : *mut *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreatePathGradient(points : *const PointF, count : i32, wrapmode : WrapMode, polygradient : *mut *mut GpPathGradient) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreatePathGradientFromPath(path : *const GpPath, polygradient : *mut *mut GpPathGradient) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreatePathGradientI(points : *const Point, count : i32, wrapmode : WrapMode, polygradient : *mut *mut GpPathGradient) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreatePathIter(iterator : *mut *mut GpPathIterator, path : *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreatePen1(color : u32, width : f32, unit : Unit, pen : *mut *mut GpPen) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreatePen2(brush : *mut GpBrush, width : f32, unit : Unit, pen : *mut *mut GpPen) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateRegion(region : *mut *mut GpRegion) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipCreateRegionHrgn(hrgn : super::Gdi:: HRGN, region : *mut *mut GpRegion) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateRegionPath(path : *mut GpPath, region : *mut *mut GpRegion) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateRegionRect(rect : *const RectF, region : *mut *mut GpRegion) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateRegionRectI(rect : *const Rect, region : *mut *mut GpRegion) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateRegionRgnData(regiondata : *const u8, size : i32, region : *mut *mut GpRegion) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateSolidFill(color : u32, brush : *mut *mut GpSolidFill) -> Status);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn GdipCreateStreamOnFile(filename : ::windows_sys::core::PCWSTR, access : u32, stream : *mut super::super::System::Com:: IStream) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateStringFormat(formatattributes : i32, language : u16, format : *mut *mut GpStringFormat) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateTexture(image : *mut GpImage, wrapmode : WrapMode, texture : *mut *mut GpTexture) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateTexture2(image : *mut GpImage, wrapmode : WrapMode, x : f32, y : f32, width : f32, height : f32, texture : *mut *mut GpTexture) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateTexture2I(image : *mut GpImage, wrapmode : WrapMode, x : i32, y : i32, width : i32, height : i32, texture : *mut *mut GpTexture) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateTextureIA(image : *mut GpImage, imageattributes : *const GpImageAttributes, x : f32, y : f32, width : f32, height : f32, texture : *mut *mut GpTexture) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipCreateTextureIAI(image : *mut GpImage, imageattributes : *const GpImageAttributes, x : i32, y : i32, width : i32, height : i32, texture : *mut *mut GpTexture) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDeleteBrush(brush : *mut GpBrush) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDeleteCachedBitmap(cachedbitmap : *mut GpCachedBitmap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDeleteCustomLineCap(customcap : *mut GpCustomLineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDeleteEffect(effect : *mut CGpEffect) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDeleteFont(font : *mut GpFont) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDeleteFontFamily(fontfamily : *mut GpFontFamily) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDeleteGraphics(graphics : *mut GpGraphics) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDeleteMatrix(matrix : *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDeletePath(path : *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDeletePathIter(iterator : *mut GpPathIterator) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDeletePen(pen : *mut GpPen) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDeletePrivateFontCollection(fontcollection : *mut *mut GpFontCollection) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDeleteRegion(region : *mut GpRegion) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDeleteStringFormat(format : *mut GpStringFormat) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDisposeImage(image : *mut GpImage) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDisposeImageAttributes(imageattr : *mut GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawArc(graphics : *mut GpGraphics, pen : *mut GpPen, x : f32, y : f32, width : f32, height : f32, startangle : f32, sweepangle : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawArcI(graphics : *mut GpGraphics, pen : *mut GpPen, x : i32, y : i32, width : i32, height : i32, startangle : f32, sweepangle : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawBezier(graphics : *mut GpGraphics, pen : *mut GpPen, x1 : f32, y1 : f32, x2 : f32, y2 : f32, x3 : f32, y3 : f32, x4 : f32, y4 : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawBezierI(graphics : *mut GpGraphics, pen : *mut GpPen, x1 : i32, y1 : i32, x2 : i32, y2 : i32, x3 : i32, y3 : i32, x4 : i32, y4 : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawBeziers(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawBeziersI(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawCachedBitmap(graphics : *mut GpGraphics, cachedbitmap : *mut GpCachedBitmap, x : i32, y : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawClosedCurve(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawClosedCurve2(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const PointF, count : i32, tension : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawClosedCurve2I(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const Point, count : i32, tension : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawClosedCurveI(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawCurve(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawCurve2(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const PointF, count : i32, tension : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawCurve2I(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const Point, count : i32, tension : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawCurve3(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const PointF, count : i32, offset : i32, numberofsegments : i32, tension : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawCurve3I(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const Point, count : i32, offset : i32, numberofsegments : i32, tension : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawCurveI(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawDriverString(graphics : *mut GpGraphics, text : *const u16, length : i32, font : *const GpFont, brush : *const GpBrush, positions : *const PointF, flags : i32, matrix : *const Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawEllipse(graphics : *mut GpGraphics, pen : *mut GpPen, x : f32, y : f32, width : f32, height : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawEllipseI(graphics : *mut GpGraphics, pen : *mut GpPen, x : i32, y : i32, width : i32, height : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawImage(graphics : *mut GpGraphics, image : *mut GpImage, x : f32, y : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawImageFX(graphics : *mut GpGraphics, image : *mut GpImage, source : *mut RectF, xform : *mut Matrix, effect : *mut CGpEffect, imageattributes : *mut GpImageAttributes, srcunit : Unit) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawImageI(graphics : *mut GpGraphics, image : *mut GpImage, x : i32, y : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawImagePointRect(graphics : *mut GpGraphics, image : *mut GpImage, x : f32, y : f32, srcx : f32, srcy : f32, srcwidth : f32, srcheight : f32, srcunit : Unit) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawImagePointRectI(graphics : *mut GpGraphics, image : *mut GpImage, x : i32, y : i32, srcx : i32, srcy : i32, srcwidth : i32, srcheight : i32, srcunit : Unit) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawImagePoints(graphics : *mut GpGraphics, image : *mut GpImage, dstpoints : *const PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawImagePointsI(graphics : *mut GpGraphics, image : *mut GpImage, dstpoints : *const Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawImagePointsRect(graphics : *mut GpGraphics, image : *mut GpImage, points : *const PointF, count : i32, srcx : f32, srcy : f32, srcwidth : f32, srcheight : f32, srcunit : Unit, imageattributes : *const GpImageAttributes, callback : isize, callbackdata : *mut ::core::ffi::c_void) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawImagePointsRectI(graphics : *mut GpGraphics, image : *mut GpImage, points : *const Point, count : i32, srcx : i32, srcy : i32, srcwidth : i32, srcheight : i32, srcunit : Unit, imageattributes : *const GpImageAttributes, callback : isize, callbackdata : *mut ::core::ffi::c_void) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawImageRect(graphics : *mut GpGraphics, image : *mut GpImage, x : f32, y : f32, width : f32, height : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawImageRectI(graphics : *mut GpGraphics, image : *mut GpImage, x : i32, y : i32, width : i32, height : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawImageRectRect(graphics : *mut GpGraphics, image : *mut GpImage, dstx : f32, dsty : f32, dstwidth : f32, dstheight : f32, srcx : f32, srcy : f32, srcwidth : f32, srcheight : f32, srcunit : Unit, imageattributes : *const GpImageAttributes, callback : isize, callbackdata : *mut ::core::ffi::c_void) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawImageRectRectI(graphics : *mut GpGraphics, image : *mut GpImage, dstx : i32, dsty : i32, dstwidth : i32, dstheight : i32, srcx : i32, srcy : i32, srcwidth : i32, srcheight : i32, srcunit : Unit, imageattributes : *const GpImageAttributes, callback : isize, callbackdata : *mut ::core::ffi::c_void) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawLine(graphics : *mut GpGraphics, pen : *mut GpPen, x1 : f32, y1 : f32, x2 : f32, y2 : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawLineI(graphics : *mut GpGraphics, pen : *mut GpPen, x1 : i32, y1 : i32, x2 : i32, y2 : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawLines(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawLinesI(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawPath(graphics : *mut GpGraphics, pen : *mut GpPen, path : *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawPie(graphics : *mut GpGraphics, pen : *mut GpPen, x : f32, y : f32, width : f32, height : f32, startangle : f32, sweepangle : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawPieI(graphics : *mut GpGraphics, pen : *mut GpPen, x : i32, y : i32, width : i32, height : i32, startangle : f32, sweepangle : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawPolygon(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawPolygonI(graphics : *mut GpGraphics, pen : *mut GpPen, points : *const Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawRectangle(graphics : *mut GpGraphics, pen : *mut GpPen, x : f32, y : f32, width : f32, height : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawRectangleI(graphics : *mut GpGraphics, pen : *mut GpPen, x : i32, y : i32, width : i32, height : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawRectangles(graphics : *mut GpGraphics, pen : *mut GpPen, rects : *const RectF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawRectanglesI(graphics : *mut GpGraphics, pen : *mut GpPen, rects : *const Rect, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipDrawString(graphics : *mut GpGraphics, string : ::windows_sys::core::PCWSTR, length : i32, font : *const GpFont, layoutrect : *const RectF, stringformat : *const GpStringFormat, brush : *const GpBrush) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipEmfToWmfBits(hemf : super::Gdi:: HENHMETAFILE, cbdata16 : u32, pdata16 : *mut u8, imapmode : i32, eflags : i32) -> u32);
::windows_targets::link!("gdiplus.dll" "system" fn GdipEndContainer(graphics : *mut GpGraphics, state : u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipEnumerateMetafileDestPoint(graphics : *mut GpGraphics, metafile : *const GpMetafile, destpoint : *const PointF, callback : isize, callbackdata : *mut ::core::ffi::c_void, imageattributes : *const GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipEnumerateMetafileDestPointI(graphics : *mut GpGraphics, metafile : *const GpMetafile, destpoint : *const Point, callback : isize, callbackdata : *mut ::core::ffi::c_void, imageattributes : *const GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipEnumerateMetafileDestPoints(graphics : *mut GpGraphics, metafile : *const GpMetafile, destpoints : *const PointF, count : i32, callback : isize, callbackdata : *mut ::core::ffi::c_void, imageattributes : *const GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipEnumerateMetafileDestPointsI(graphics : *mut GpGraphics, metafile : *const GpMetafile, destpoints : *const Point, count : i32, callback : isize, callbackdata : *mut ::core::ffi::c_void, imageattributes : *const GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipEnumerateMetafileDestRect(graphics : *mut GpGraphics, metafile : *const GpMetafile, destrect : *const RectF, callback : isize, callbackdata : *mut ::core::ffi::c_void, imageattributes : *const GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipEnumerateMetafileDestRectI(graphics : *mut GpGraphics, metafile : *const GpMetafile, destrect : *const Rect, callback : isize, callbackdata : *mut ::core::ffi::c_void, imageattributes : *const GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipEnumerateMetafileSrcRectDestPoint(graphics : *mut GpGraphics, metafile : *const GpMetafile, destpoint : *const PointF, srcrect : *const RectF, srcunit : Unit, callback : isize, callbackdata : *mut ::core::ffi::c_void, imageattributes : *const GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipEnumerateMetafileSrcRectDestPointI(graphics : *mut GpGraphics, metafile : *const GpMetafile, destpoint : *const Point, srcrect : *const Rect, srcunit : Unit, callback : isize, callbackdata : *mut ::core::ffi::c_void, imageattributes : *const GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipEnumerateMetafileSrcRectDestPoints(graphics : *mut GpGraphics, metafile : *const GpMetafile, destpoints : *const PointF, count : i32, srcrect : *const RectF, srcunit : Unit, callback : isize, callbackdata : *mut ::core::ffi::c_void, imageattributes : *const GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipEnumerateMetafileSrcRectDestPointsI(graphics : *mut GpGraphics, metafile : *const GpMetafile, destpoints : *const Point, count : i32, srcrect : *const Rect, srcunit : Unit, callback : isize, callbackdata : *mut ::core::ffi::c_void, imageattributes : *const GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipEnumerateMetafileSrcRectDestRect(graphics : *mut GpGraphics, metafile : *const GpMetafile, destrect : *const RectF, srcrect : *const RectF, srcunit : Unit, callback : isize, callbackdata : *mut ::core::ffi::c_void, imageattributes : *const GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipEnumerateMetafileSrcRectDestRectI(graphics : *mut GpGraphics, metafile : *const GpMetafile, destrect : *const Rect, srcrect : *const Rect, srcunit : Unit, callback : isize, callbackdata : *mut ::core::ffi::c_void, imageattributes : *const GpImageAttributes) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillClosedCurve(graphics : *mut GpGraphics, brush : *mut GpBrush, points : *const PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillClosedCurve2(graphics : *mut GpGraphics, brush : *mut GpBrush, points : *const PointF, count : i32, tension : f32, fillmode : FillMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillClosedCurve2I(graphics : *mut GpGraphics, brush : *mut GpBrush, points : *const Point, count : i32, tension : f32, fillmode : FillMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillClosedCurveI(graphics : *mut GpGraphics, brush : *mut GpBrush, points : *const Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillEllipse(graphics : *mut GpGraphics, brush : *mut GpBrush, x : f32, y : f32, width : f32, height : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillEllipseI(graphics : *mut GpGraphics, brush : *mut GpBrush, x : i32, y : i32, width : i32, height : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillPath(graphics : *mut GpGraphics, brush : *mut GpBrush, path : *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillPie(graphics : *mut GpGraphics, brush : *mut GpBrush, x : f32, y : f32, width : f32, height : f32, startangle : f32, sweepangle : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillPieI(graphics : *mut GpGraphics, brush : *mut GpBrush, x : i32, y : i32, width : i32, height : i32, startangle : f32, sweepangle : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillPolygon(graphics : *mut GpGraphics, brush : *mut GpBrush, points : *const PointF, count : i32, fillmode : FillMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillPolygon2(graphics : *mut GpGraphics, brush : *mut GpBrush, points : *const PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillPolygon2I(graphics : *mut GpGraphics, brush : *mut GpBrush, points : *const Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillPolygonI(graphics : *mut GpGraphics, brush : *mut GpBrush, points : *const Point, count : i32, fillmode : FillMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillRectangle(graphics : *mut GpGraphics, brush : *mut GpBrush, x : f32, y : f32, width : f32, height : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillRectangleI(graphics : *mut GpGraphics, brush : *mut GpBrush, x : i32, y : i32, width : i32, height : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillRectangles(graphics : *mut GpGraphics, brush : *mut GpBrush, rects : *const RectF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillRectanglesI(graphics : *mut GpGraphics, brush : *mut GpBrush, rects : *const Rect, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFillRegion(graphics : *mut GpGraphics, brush : *mut GpBrush, region : *mut GpRegion) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFindFirstImageItem(image : *mut GpImage, item : *mut ImageItemData) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFindNextImageItem(image : *mut GpImage, item : *mut ImageItemData) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFlattenPath(path : *mut GpPath, matrix : *mut Matrix, flatness : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFlush(graphics : *mut GpGraphics, intention : FlushIntention) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipFree(ptr : *mut ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipGetAdjustableArrowCapFillState(cap : *mut GpAdjustableArrowCap, fillstate : *mut super::super::Foundation:: BOOL) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetAdjustableArrowCapHeight(cap : *mut GpAdjustableArrowCap, height : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetAdjustableArrowCapMiddleInset(cap : *mut GpAdjustableArrowCap, middleinset : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetAdjustableArrowCapWidth(cap : *mut GpAdjustableArrowCap, width : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetAllPropertyItems(image : *mut GpImage, totalbuffersize : u32, numproperties : u32, allitems : *mut PropertyItem) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetBrushType(brush : *mut GpBrush, r#type : *mut BrushType) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetCellAscent(family : *const GpFontFamily, style : i32, cellascent : *mut u16) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetCellDescent(family : *const GpFontFamily, style : i32, celldescent : *mut u16) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetClip(graphics : *mut GpGraphics, region : *mut GpRegion) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetClipBounds(graphics : *mut GpGraphics, rect : *mut RectF) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetClipBoundsI(graphics : *mut GpGraphics, rect : *mut Rect) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetCompositingMode(graphics : *mut GpGraphics, compositingmode : *mut CompositingMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetCompositingQuality(graphics : *mut GpGraphics, compositingquality : *mut CompositingQuality) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetCustomLineCapBaseCap(customcap : *mut GpCustomLineCap, basecap : *mut LineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetCustomLineCapBaseInset(customcap : *mut GpCustomLineCap, inset : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetCustomLineCapStrokeCaps(customcap : *mut GpCustomLineCap, startcap : *mut LineCap, endcap : *mut LineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetCustomLineCapStrokeJoin(customcap : *mut GpCustomLineCap, linejoin : *mut LineJoin) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetCustomLineCapType(customcap : *mut GpCustomLineCap, captype : *mut CustomLineCapType) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetCustomLineCapWidthScale(customcap : *mut GpCustomLineCap, widthscale : *mut f32) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipGetDC(graphics : *mut GpGraphics, hdc : *mut super::Gdi:: HDC) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetDpiX(graphics : *mut GpGraphics, dpi : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetDpiY(graphics : *mut GpGraphics, dpi : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetEffectParameterSize(effect : *mut CGpEffect, size : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetEffectParameters(effect : *mut CGpEffect, size : *mut u32, params : *mut ::core::ffi::c_void) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetEmHeight(family : *const GpFontFamily, style : i32, emheight : *mut u16) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetEncoderParameterList(image : *mut GpImage, clsidencoder : *const ::windows_sys::core::GUID, size : u32, buffer : *mut EncoderParameters) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetEncoderParameterListSize(image : *mut GpImage, clsidencoder : *const ::windows_sys::core::GUID, size : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetFamily(font : *mut GpFont, family : *mut *mut GpFontFamily) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetFamilyName(family : *const GpFontFamily, name : ::windows_sys::core::PWSTR, language : u16) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetFontCollectionFamilyCount(fontcollection : *mut GpFontCollection, numfound : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetFontCollectionFamilyList(fontcollection : *const GpFontCollection, numsought : i32, gpfamilies : *mut *mut GpFontFamily, numfound : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetFontHeight(font : *const GpFont, graphics : *const GpGraphics, height : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetFontHeightGivenDPI(font : *const GpFont, dpi : f32, height : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetFontSize(font : *mut GpFont, size : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetFontStyle(font : *mut GpFont, style : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetFontUnit(font : *mut GpFont, unit : *mut Unit) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetGenericFontFamilyMonospace(nativefamily : *mut *mut GpFontFamily) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetGenericFontFamilySansSerif(nativefamily : *mut *mut GpFontFamily) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetGenericFontFamilySerif(nativefamily : *mut *mut GpFontFamily) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetHatchBackgroundColor(brush : *mut GpHatch, backcol : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetHatchForegroundColor(brush : *mut GpHatch, forecol : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetHatchStyle(brush : *mut GpHatch, hatchstyle : *mut HatchStyle) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipGetHemfFromMetafile(metafile : *mut GpMetafile, hemf : *mut super::Gdi:: HENHMETAFILE) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageAttributesAdjustedPalette(imageattr : *mut GpImageAttributes, colorpalette : *mut ColorPalette, coloradjusttype : ColorAdjustType) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageBounds(image : *mut GpImage, srcrect : *mut RectF, srcunit : *mut Unit) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageDecoders(numdecoders : u32, size : u32, decoders : *mut ImageCodecInfo) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageDecodersSize(numdecoders : *mut u32, size : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageDimension(image : *mut GpImage, width : *mut f32, height : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageEncoders(numencoders : u32, size : u32, encoders : *mut ImageCodecInfo) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageEncodersSize(numencoders : *mut u32, size : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageFlags(image : *mut GpImage, flags : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageGraphicsContext(image : *mut GpImage, graphics : *mut *mut GpGraphics) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageHeight(image : *mut GpImage, height : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageHorizontalResolution(image : *mut GpImage, resolution : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageItemData(image : *mut GpImage, item : *mut ImageItemData) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImagePalette(image : *mut GpImage, palette : *mut ColorPalette, size : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImagePaletteSize(image : *mut GpImage, size : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImagePixelFormat(image : *mut GpImage, format : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageRawFormat(image : *mut GpImage, format : *mut ::windows_sys::core::GUID) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageThumbnail(image : *mut GpImage, thumbwidth : u32, thumbheight : u32, thumbimage : *mut *mut GpImage, callback : isize, callbackdata : *mut ::core::ffi::c_void) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageType(image : *mut GpImage, r#type : *mut ImageType) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageVerticalResolution(image : *mut GpImage, resolution : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetImageWidth(image : *mut GpImage, width : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetInterpolationMode(graphics : *mut GpGraphics, interpolationmode : *mut InterpolationMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetLineBlend(brush : *mut GpLineGradient, blend : *mut f32, positions : *mut f32, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetLineBlendCount(brush : *mut GpLineGradient, count : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetLineColors(brush : *mut GpLineGradient, colors : *mut u32) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipGetLineGammaCorrection(brush : *mut GpLineGradient, usegammacorrection : *mut super::super::Foundation:: BOOL) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetLinePresetBlend(brush : *mut GpLineGradient, blend : *mut u32, positions : *mut f32, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetLinePresetBlendCount(brush : *mut GpLineGradient, count : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetLineRect(brush : *mut GpLineGradient, rect : *mut RectF) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetLineRectI(brush : *mut GpLineGradient, rect : *mut Rect) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetLineSpacing(family : *const GpFontFamily, style : i32, linespacing : *mut u16) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetLineTransform(brush : *mut GpLineGradient, matrix : *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetLineWrapMode(brush : *mut GpLineGradient, wrapmode : *mut WrapMode) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipGetLogFontA(font : *mut GpFont, graphics : *mut GpGraphics, logfonta : *mut super::Gdi:: LOGFONTA) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipGetLogFontW(font : *mut GpFont, graphics : *mut GpGraphics, logfontw : *mut super::Gdi:: LOGFONTW) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetMatrixElements(matrix : *const Matrix, matrixout : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetMetafileDownLevelRasterizationLimit(metafile : *const GpMetafile, metafilerasterizationlimitdpi : *mut u32) -> Status);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GdipGetMetafileHeaderFromEmf(hemf : super::Gdi:: HENHMETAFILE, header : *mut MetafileHeader) -> Status);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GdipGetMetafileHeaderFromFile(filename : ::windows_sys::core::PCWSTR, header : *mut MetafileHeader) -> Status);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GdipGetMetafileHeaderFromMetafile(metafile : *mut GpMetafile, header : *mut MetafileHeader) -> Status);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_System_Com"))]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_System_Com\"`"] fn GdipGetMetafileHeaderFromStream(stream : super::super::System::Com:: IStream, header : *mut MetafileHeader) -> Status);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn GdipGetMetafileHeaderFromWmf(hwmf : super::Gdi:: HMETAFILE, wmfplaceablefileheader : *const WmfPlaceableFileHeader, header : *mut MetafileHeader) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetNearestColor(graphics : *mut GpGraphics, argb : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPageScale(graphics : *mut GpGraphics, scale : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPageUnit(graphics : *mut GpGraphics, unit : *mut Unit) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathData(path : *mut GpPath, pathdata : *mut PathData) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathFillMode(path : *mut GpPath, fillmode : *mut FillMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientBlend(brush : *mut GpPathGradient, blend : *mut f32, positions : *mut f32, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientBlendCount(brush : *mut GpPathGradient, count : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientCenterColor(brush : *mut GpPathGradient, colors : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientCenterPoint(brush : *mut GpPathGradient, points : *mut PointF) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientCenterPointI(brush : *mut GpPathGradient, points : *mut Point) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientFocusScales(brush : *mut GpPathGradient, xscale : *mut f32, yscale : *mut f32) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipGetPathGradientGammaCorrection(brush : *mut GpPathGradient, usegammacorrection : *mut super::super::Foundation:: BOOL) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientPath(brush : *mut GpPathGradient, path : *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientPointCount(brush : *mut GpPathGradient, count : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientPresetBlend(brush : *mut GpPathGradient, blend : *mut u32, positions : *mut f32, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientPresetBlendCount(brush : *mut GpPathGradient, count : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientRect(brush : *mut GpPathGradient, rect : *mut RectF) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientRectI(brush : *mut GpPathGradient, rect : *mut Rect) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientSurroundColorCount(brush : *mut GpPathGradient, count : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientSurroundColorsWithCount(brush : *const GpPathGradient, color : *mut u32, count : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientTransform(brush : *mut GpPathGradient, matrix : *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathGradientWrapMode(brush : *mut GpPathGradient, wrapmode : *mut WrapMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathLastPoint(path : *mut GpPath, lastpoint : *mut PointF) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathPoints(param0 : *mut GpPath, points : *mut PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathPointsI(param0 : *mut GpPath, points : *mut Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathTypes(path : *const GpPath, types : *mut u8, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathWorldBounds(path : *mut GpPath, bounds : *mut RectF, matrix : *const Matrix, pen : *const GpPen) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPathWorldBoundsI(path : *mut GpPath, bounds : *mut Rect, matrix : *const Matrix, pen : *const GpPen) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenBrushFill(pen : *mut GpPen, brush : *mut *mut GpBrush) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenColor(pen : *mut GpPen, argb : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenCompoundArray(pen : *mut GpPen, dash : *mut f32, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenCompoundCount(pen : *mut GpPen, count : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenCustomEndCap(pen : *mut GpPen, customcap : *mut *mut GpCustomLineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenCustomStartCap(pen : *mut GpPen, customcap : *mut *mut GpCustomLineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenDashArray(pen : *mut GpPen, dash : *mut f32, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenDashCap197819(pen : *mut GpPen, dashcap : *mut DashCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenDashCount(pen : *mut GpPen, count : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenDashOffset(pen : *mut GpPen, offset : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenDashStyle(pen : *mut GpPen, dashstyle : *mut DashStyle) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenEndCap(pen : *mut GpPen, endcap : *mut LineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenFillType(pen : *mut GpPen, r#type : *mut PenType) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenLineJoin(pen : *mut GpPen, linejoin : *mut LineJoin) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenMiterLimit(pen : *mut GpPen, miterlimit : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenMode(pen : *mut GpPen, penmode : *mut PenAlignment) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenStartCap(pen : *mut GpPen, startcap : *mut LineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenTransform(pen : *mut GpPen, matrix : *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenUnit(pen : *mut GpPen, unit : *mut Unit) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPenWidth(pen : *mut GpPen, width : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPixelOffsetMode(graphics : *mut GpGraphics, pixeloffsetmode : *mut PixelOffsetMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPointCount(path : *mut GpPath, count : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPropertyCount(image : *mut GpImage, numofproperty : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPropertyIdList(image : *mut GpImage, numofproperty : u32, list : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPropertyItem(image : *mut GpImage, propid : u32, propsize : u32, buffer : *mut PropertyItem) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPropertyItemSize(image : *mut GpImage, propid : u32, size : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetPropertySize(image : *mut GpImage, totalbuffersize : *mut u32, numproperties : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetRegionBounds(region : *mut GpRegion, graphics : *mut GpGraphics, rect : *mut RectF) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetRegionBoundsI(region : *mut GpRegion, graphics : *mut GpGraphics, rect : *mut Rect) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetRegionData(region : *mut GpRegion, buffer : *mut u8, buffersize : u32, sizefilled : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetRegionDataSize(region : *mut GpRegion, buffersize : *mut u32) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipGetRegionHRgn(region : *mut GpRegion, graphics : *mut GpGraphics, hrgn : *mut super::Gdi:: HRGN) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetRegionScans(region : *mut GpRegion, rects : *mut RectF, count : *mut i32, matrix : *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetRegionScansCount(region : *mut GpRegion, count : *mut u32, matrix : *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetRegionScansI(region : *mut GpRegion, rects : *mut Rect, count : *mut i32, matrix : *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetRenderingOrigin(graphics : *mut GpGraphics, x : *mut i32, y : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetSmoothingMode(graphics : *mut GpGraphics, smoothingmode : *mut SmoothingMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetSolidFillColor(brush : *mut GpSolidFill, color : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetStringFormatAlign(format : *const GpStringFormat, align : *mut StringAlignment) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetStringFormatDigitSubstitution(format : *const GpStringFormat, language : *mut u16, substitute : *mut StringDigitSubstitute) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetStringFormatFlags(format : *const GpStringFormat, flags : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetStringFormatHotkeyPrefix(format : *const GpStringFormat, hotkeyprefix : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetStringFormatLineAlign(format : *const GpStringFormat, align : *mut StringAlignment) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetStringFormatMeasurableCharacterRangeCount(format : *const GpStringFormat, count : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetStringFormatTabStopCount(format : *const GpStringFormat, count : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetStringFormatTabStops(format : *const GpStringFormat, count : i32, firsttaboffset : *mut f32, tabstops : *mut f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetStringFormatTrimming(format : *const GpStringFormat, trimming : *mut StringTrimming) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetTextContrast(graphics : *mut GpGraphics, contrast : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetTextRenderingHint(graphics : *mut GpGraphics, mode : *mut TextRenderingHint) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetTextureImage(brush : *mut GpTexture, image : *mut *mut GpImage) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetTextureTransform(brush : *mut GpTexture, matrix : *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetTextureWrapMode(brush : *mut GpTexture, wrapmode : *mut WrapMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetVisibleClipBounds(graphics : *mut GpGraphics, rect : *mut RectF) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetVisibleClipBoundsI(graphics : *mut GpGraphics, rect : *mut Rect) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGetWorldTransform(graphics : *mut GpGraphics, matrix : *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGraphicsClear(graphics : *mut GpGraphics, color : u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipGraphicsSetAbort(pgraphics : *mut GpGraphics, piabort : GdiplusAbort) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipImageForceValidation(image : *mut GpImage) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipImageGetFrameCount(image : *mut GpImage, dimensionid : *const ::windows_sys::core::GUID, count : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipImageGetFrameDimensionsCount(image : *mut GpImage, count : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipImageGetFrameDimensionsList(image : *mut GpImage, dimensionids : *mut ::windows_sys::core::GUID, count : u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipImageRotateFlip(image : *mut GpImage, rftype : RotateFlipType) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipImageSelectActiveFrame(image : *mut GpImage, dimensionid : *const ::windows_sys::core::GUID, frameindex : u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipImageSetAbort(pimage : *mut GpImage, piabort : GdiplusAbort) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipInitializePalette(palette : *mut ColorPalette, palettetype : PaletteType, optimalcolors : i32, usetransparentcolor : super::super::Foundation:: BOOL, bitmap : *mut GpBitmap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipInvertMatrix(matrix : *mut Matrix) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsClipEmpty(graphics : *mut GpGraphics, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsEmptyRegion(region : *mut GpRegion, graphics : *mut GpGraphics, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsEqualRegion(region : *mut GpRegion, region2 : *mut GpRegion, graphics : *mut GpGraphics, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsInfiniteRegion(region : *mut GpRegion, graphics : *mut GpGraphics, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsMatrixEqual(matrix : *const Matrix, matrix2 : *const Matrix, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsMatrixIdentity(matrix : *const Matrix, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsMatrixInvertible(matrix : *const Matrix, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsOutlineVisiblePathPoint(path : *mut GpPath, x : f32, y : f32, pen : *mut GpPen, graphics : *mut GpGraphics, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsOutlineVisiblePathPointI(path : *mut GpPath, x : i32, y : i32, pen : *mut GpPen, graphics : *mut GpGraphics, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsStyleAvailable(family : *const GpFontFamily, style : i32, isstyleavailable : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsVisibleClipEmpty(graphics : *mut GpGraphics, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsVisiblePathPoint(path : *mut GpPath, x : f32, y : f32, graphics : *mut GpGraphics, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsVisiblePathPointI(path : *mut GpPath, x : i32, y : i32, graphics : *mut GpGraphics, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsVisiblePoint(graphics : *mut GpGraphics, x : f32, y : f32, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsVisiblePointI(graphics : *mut GpGraphics, x : i32, y : i32, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsVisibleRect(graphics : *mut GpGraphics, x : f32, y : f32, width : f32, height : f32, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsVisibleRectI(graphics : *mut GpGraphics, x : i32, y : i32, width : i32, height : i32, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsVisibleRegionPoint(region : *mut GpRegion, x : f32, y : f32, graphics : *mut GpGraphics, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsVisibleRegionPointI(region : *mut GpRegion, x : i32, y : i32, graphics : *mut GpGraphics, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsVisibleRegionRect(region : *mut GpRegion, x : f32, y : f32, width : f32, height : f32, graphics : *mut GpGraphics, result : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipIsVisibleRegionRectI(region : *mut GpRegion, x : i32, y : i32, width : i32, height : i32, graphics : *mut GpGraphics, result : *mut super::super::Foundation:: BOOL) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipLoadImageFromFile(filename : ::windows_sys::core::PCWSTR, image : *mut *mut GpImage) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipLoadImageFromFileICM(filename : ::windows_sys::core::PCWSTR, image : *mut *mut GpImage) -> Status);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn GdipLoadImageFromStream(stream : super::super::System::Com:: IStream, image : *mut *mut GpImage) -> Status);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn GdipLoadImageFromStreamICM(stream : super::super::System::Com:: IStream, image : *mut *mut GpImage) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipMeasureCharacterRanges(graphics : *mut GpGraphics, string : ::windows_sys::core::PCWSTR, length : i32, font : *const GpFont, layoutrect : *const RectF, stringformat : *const GpStringFormat, regioncount : i32, regions : *mut *mut GpRegion) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipMeasureDriverString(graphics : *mut GpGraphics, text : *const u16, length : i32, font : *const GpFont, positions : *const PointF, flags : i32, matrix : *const Matrix, boundingbox : *mut RectF) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipMeasureString(graphics : *mut GpGraphics, string : ::windows_sys::core::PCWSTR, length : i32, font : *const GpFont, layoutrect : *const RectF, stringformat : *const GpStringFormat, boundingbox : *mut RectF, codepointsfitted : *mut i32, linesfilled : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipMultiplyLineTransform(brush : *mut GpLineGradient, matrix : *const Matrix, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipMultiplyMatrix(matrix : *mut Matrix, matrix2 : *mut Matrix, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipMultiplyPathGradientTransform(brush : *mut GpPathGradient, matrix : *const Matrix, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipMultiplyPenTransform(pen : *mut GpPen, matrix : *const Matrix, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipMultiplyTextureTransform(brush : *mut GpTexture, matrix : *const Matrix, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipMultiplyWorldTransform(graphics : *mut GpGraphics, matrix : *const Matrix, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipNewInstalledFontCollection(fontcollection : *mut *mut GpFontCollection) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipNewPrivateFontCollection(fontcollection : *mut *mut GpFontCollection) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipPathIterCopyData(iterator : *mut GpPathIterator, resultcount : *mut i32, points : *mut PointF, types : *mut u8, startindex : i32, endindex : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipPathIterEnumerate(iterator : *mut GpPathIterator, resultcount : *mut i32, points : *mut PointF, types : *mut u8, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipPathIterGetCount(iterator : *mut GpPathIterator, count : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipPathIterGetSubpathCount(iterator : *mut GpPathIterator, count : *mut i32) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipPathIterHasCurve(iterator : *mut GpPathIterator, hascurve : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipPathIterIsValid(iterator : *mut GpPathIterator, valid : *mut super::super::Foundation:: BOOL) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipPathIterNextMarker(iterator : *mut GpPathIterator, resultcount : *mut i32, startindex : *mut i32, endindex : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipPathIterNextMarkerPath(iterator : *mut GpPathIterator, resultcount : *mut i32, path : *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipPathIterNextPathType(iterator : *mut GpPathIterator, resultcount : *mut i32, pathtype : *mut u8, startindex : *mut i32, endindex : *mut i32) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipPathIterNextSubpath(iterator : *mut GpPathIterator, resultcount : *mut i32, startindex : *mut i32, endindex : *mut i32, isclosed : *mut super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipPathIterNextSubpathPath(iterator : *mut GpPathIterator, resultcount : *mut i32, path : *mut GpPath, isclosed : *mut super::super::Foundation:: BOOL) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipPathIterRewind(iterator : *mut GpPathIterator) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipPlayMetafileRecord(metafile : *const GpMetafile, recordtype : EmfPlusRecordType, flags : u32, datasize : u32, data : *const u8) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipPrivateAddFontFile(fontcollection : *mut GpFontCollection, filename : ::windows_sys::core::PCWSTR) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipPrivateAddMemoryFont(fontcollection : *mut GpFontCollection, memory : *const ::core::ffi::c_void, length : i32) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipRecordMetafile(referencehdc : super::Gdi:: HDC, r#type : EmfType, framerect : *const RectF, frameunit : MetafileFrameUnit, description : ::windows_sys::core::PCWSTR, metafile : *mut *mut GpMetafile) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipRecordMetafileFileName(filename : ::windows_sys::core::PCWSTR, referencehdc : super::Gdi:: HDC, r#type : EmfType, framerect : *const RectF, frameunit : MetafileFrameUnit, description : ::windows_sys::core::PCWSTR, metafile : *mut *mut GpMetafile) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipRecordMetafileFileNameI(filename : ::windows_sys::core::PCWSTR, referencehdc : super::Gdi:: HDC, r#type : EmfType, framerect : *const Rect, frameunit : MetafileFrameUnit, description : ::windows_sys::core::PCWSTR, metafile : *mut *mut GpMetafile) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipRecordMetafileI(referencehdc : super::Gdi:: HDC, r#type : EmfType, framerect : *const Rect, frameunit : MetafileFrameUnit, description : ::windows_sys::core::PCWSTR, metafile : *mut *mut GpMetafile) -> Status);
#[cfg(all(feature = "Win32_Graphics_Gdi", feature = "Win32_System_Com"))]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`, `\"Win32_System_Com\"`"] fn GdipRecordMetafileStream(stream : super::super::System::Com:: IStream, referencehdc : super::Gdi:: HDC, r#type : EmfType, framerect : *const RectF, frameunit : MetafileFrameUnit, description : ::windows_sys::core::PCWSTR, metafile : *mut *mut GpMetafile) -> Status);
#[cfg(all(feature = "Win32_Graphics_Gdi", feature = "Win32_System_Com"))]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`, `\"Win32_System_Com\"`"] fn GdipRecordMetafileStreamI(stream : super::super::System::Com:: IStream, referencehdc : super::Gdi:: HDC, r#type : EmfType, framerect : *const Rect, frameunit : MetafileFrameUnit, description : ::windows_sys::core::PCWSTR, metafile : *mut *mut GpMetafile) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipReleaseDC(graphics : *mut GpGraphics, hdc : super::Gdi:: HDC) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipRemovePropertyItem(image : *mut GpImage, propid : u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipResetClip(graphics : *mut GpGraphics) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipResetImageAttributes(imageattr : *mut GpImageAttributes, r#type : ColorAdjustType) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipResetLineTransform(brush : *mut GpLineGradient) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipResetPageTransform(graphics : *mut GpGraphics) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipResetPath(path : *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipResetPathGradientTransform(brush : *mut GpPathGradient) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipResetPenTransform(pen : *mut GpPen) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipResetTextureTransform(brush : *mut GpTexture) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipResetWorldTransform(graphics : *mut GpGraphics) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipRestoreGraphics(graphics : *mut GpGraphics, state : u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipReversePath(path : *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipRotateLineTransform(brush : *mut GpLineGradient, angle : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipRotateMatrix(matrix : *mut Matrix, angle : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipRotatePathGradientTransform(brush : *mut GpPathGradient, angle : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipRotatePenTransform(pen : *mut GpPen, angle : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipRotateTextureTransform(brush : *mut GpTexture, angle : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipRotateWorldTransform(graphics : *mut GpGraphics, angle : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSaveAdd(image : *mut GpImage, encoderparams : *const EncoderParameters) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSaveAddImage(image : *mut GpImage, newimage : *mut GpImage, encoderparams : *const EncoderParameters) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSaveGraphics(graphics : *mut GpGraphics, state : *mut u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSaveImageToFile(image : *mut GpImage, filename : ::windows_sys::core::PCWSTR, clsidencoder : *const ::windows_sys::core::GUID, encoderparams : *const EncoderParameters) -> Status);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn GdipSaveImageToStream(image : *mut GpImage, stream : super::super::System::Com:: IStream, clsidencoder : *const ::windows_sys::core::GUID, encoderparams : *const EncoderParameters) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipScaleLineTransform(brush : *mut GpLineGradient, sx : f32, sy : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipScaleMatrix(matrix : *mut Matrix, scalex : f32, scaley : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipScalePathGradientTransform(brush : *mut GpPathGradient, sx : f32, sy : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipScalePenTransform(pen : *mut GpPen, sx : f32, sy : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipScaleTextureTransform(brush : *mut GpTexture, sx : f32, sy : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipScaleWorldTransform(graphics : *mut GpGraphics, sx : f32, sy : f32, order : MatrixOrder) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipSetAdjustableArrowCapFillState(cap : *mut GpAdjustableArrowCap, fillstate : super::super::Foundation:: BOOL) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetAdjustableArrowCapHeight(cap : *mut GpAdjustableArrowCap, height : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetAdjustableArrowCapMiddleInset(cap : *mut GpAdjustableArrowCap, middleinset : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetAdjustableArrowCapWidth(cap : *mut GpAdjustableArrowCap, width : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetClipGraphics(graphics : *mut GpGraphics, srcgraphics : *mut GpGraphics, combinemode : CombineMode) -> Status);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GdipSetClipHrgn(graphics : *mut GpGraphics, hrgn : super::Gdi:: HRGN, combinemode : CombineMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetClipPath(graphics : *mut GpGraphics, path : *mut GpPath, combinemode : CombineMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetClipRect(graphics : *mut GpGraphics, x : f32, y : f32, width : f32, height : f32, combinemode : CombineMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetClipRectI(graphics : *mut GpGraphics, x : i32, y : i32, width : i32, height : i32, combinemode : CombineMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetClipRegion(graphics : *mut GpGraphics, region : *mut GpRegion, combinemode : CombineMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetCompositingMode(graphics : *mut GpGraphics, compositingmode : CompositingMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetCompositingQuality(graphics : *mut GpGraphics, compositingquality : CompositingQuality) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetCustomLineCapBaseCap(customcap : *mut GpCustomLineCap, basecap : LineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetCustomLineCapBaseInset(customcap : *mut GpCustomLineCap, inset : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetCustomLineCapStrokeCaps(customcap : *mut GpCustomLineCap, startcap : LineCap, endcap : LineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetCustomLineCapStrokeJoin(customcap : *mut GpCustomLineCap, linejoin : LineJoin) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetCustomLineCapWidthScale(customcap : *mut GpCustomLineCap, widthscale : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetEffectParameters(effect : *mut CGpEffect, params : *const ::core::ffi::c_void, size : u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetEmpty(region : *mut GpRegion) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipSetImageAttributesCachedBackground(imageattr : *mut GpImageAttributes, enableflag : super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipSetImageAttributesColorKeys(imageattr : *mut GpImageAttributes, r#type : ColorAdjustType, enableflag : super::super::Foundation:: BOOL, colorlow : u32, colorhigh : u32) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipSetImageAttributesColorMatrix(imageattr : *mut GpImageAttributes, r#type : ColorAdjustType, enableflag : super::super::Foundation:: BOOL, colormatrix : *const ColorMatrix, graymatrix : *const ColorMatrix, flags : ColorMatrixFlags) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipSetImageAttributesGamma(imageattr : *mut GpImageAttributes, r#type : ColorAdjustType, enableflag : super::super::Foundation:: BOOL, gamma : f32) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipSetImageAttributesNoOp(imageattr : *mut GpImageAttributes, r#type : ColorAdjustType, enableflag : super::super::Foundation:: BOOL) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipSetImageAttributesOutputChannel(imageattr : *mut GpImageAttributes, r#type : ColorAdjustType, enableflag : super::super::Foundation:: BOOL, channelflags : ColorChannelFlags) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipSetImageAttributesOutputChannelColorProfile(imageattr : *mut GpImageAttributes, r#type : ColorAdjustType, enableflag : super::super::Foundation:: BOOL, colorprofilefilename : ::windows_sys::core::PCWSTR) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipSetImageAttributesRemapTable(imageattr : *mut GpImageAttributes, r#type : ColorAdjustType, enableflag : super::super::Foundation:: BOOL, mapsize : u32, map : *const ColorMap) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipSetImageAttributesThreshold(imageattr : *mut GpImageAttributes, r#type : ColorAdjustType, enableflag : super::super::Foundation:: BOOL, threshold : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetImageAttributesToIdentity(imageattr : *mut GpImageAttributes, r#type : ColorAdjustType) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipSetImageAttributesWrapMode(imageattr : *mut GpImageAttributes, wrap : WrapMode, argb : u32, clamp : super::super::Foundation:: BOOL) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetImagePalette(image : *mut GpImage, palette : *const ColorPalette) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetInfinite(region : *mut GpRegion) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetInterpolationMode(graphics : *mut GpGraphics, interpolationmode : InterpolationMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetLineBlend(brush : *mut GpLineGradient, blend : *const f32, positions : *const f32, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetLineColors(brush : *mut GpLineGradient, color1 : u32, color2 : u32) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipSetLineGammaCorrection(brush : *mut GpLineGradient, usegammacorrection : super::super::Foundation:: BOOL) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetLineLinearBlend(brush : *mut GpLineGradient, focus : f32, scale : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetLinePresetBlend(brush : *mut GpLineGradient, blend : *const u32, positions : *const f32, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetLineSigmaBlend(brush : *mut GpLineGradient, focus : f32, scale : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetLineTransform(brush : *mut GpLineGradient, matrix : *const Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetLineWrapMode(brush : *mut GpLineGradient, wrapmode : WrapMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetMatrixElements(matrix : *mut Matrix, m11 : f32, m12 : f32, m21 : f32, m22 : f32, dx : f32, dy : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetMetafileDownLevelRasterizationLimit(metafile : *mut GpMetafile, metafilerasterizationlimitdpi : u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPageScale(graphics : *mut GpGraphics, scale : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPageUnit(graphics : *mut GpGraphics, unit : Unit) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPathFillMode(path : *mut GpPath, fillmode : FillMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPathGradientBlend(brush : *mut GpPathGradient, blend : *const f32, positions : *const f32, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPathGradientCenterColor(brush : *mut GpPathGradient, colors : u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPathGradientCenterPoint(brush : *mut GpPathGradient, points : *const PointF) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPathGradientCenterPointI(brush : *mut GpPathGradient, points : *const Point) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPathGradientFocusScales(brush : *mut GpPathGradient, xscale : f32, yscale : f32) -> Status);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdipSetPathGradientGammaCorrection(brush : *mut GpPathGradient, usegammacorrection : super::super::Foundation:: BOOL) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPathGradientLinearBlend(brush : *mut GpPathGradient, focus : f32, scale : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPathGradientPath(brush : *mut GpPathGradient, path : *const GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPathGradientPresetBlend(brush : *mut GpPathGradient, blend : *const u32, positions : *const f32, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPathGradientSigmaBlend(brush : *mut GpPathGradient, focus : f32, scale : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPathGradientSurroundColorsWithCount(brush : *mut GpPathGradient, color : *const u32, count : *mut i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPathGradientTransform(brush : *mut GpPathGradient, matrix : *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPathGradientWrapMode(brush : *mut GpPathGradient, wrapmode : WrapMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPathMarker(path : *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenBrushFill(pen : *mut GpPen, brush : *mut GpBrush) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenColor(pen : *mut GpPen, argb : u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenCompoundArray(pen : *mut GpPen, dash : *const f32, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenCustomEndCap(pen : *mut GpPen, customcap : *mut GpCustomLineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenCustomStartCap(pen : *mut GpPen, customcap : *mut GpCustomLineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenDashArray(pen : *mut GpPen, dash : *const f32, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenDashCap197819(pen : *mut GpPen, dashcap : DashCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenDashOffset(pen : *mut GpPen, offset : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenDashStyle(pen : *mut GpPen, dashstyle : DashStyle) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenEndCap(pen : *mut GpPen, endcap : LineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenLineCap197819(pen : *mut GpPen, startcap : LineCap, endcap : LineCap, dashcap : DashCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenLineJoin(pen : *mut GpPen, linejoin : LineJoin) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenMiterLimit(pen : *mut GpPen, miterlimit : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenMode(pen : *mut GpPen, penmode : PenAlignment) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenStartCap(pen : *mut GpPen, startcap : LineCap) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenTransform(pen : *mut GpPen, matrix : *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenUnit(pen : *mut GpPen, unit : Unit) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPenWidth(pen : *mut GpPen, width : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPixelOffsetMode(graphics : *mut GpGraphics, pixeloffsetmode : PixelOffsetMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetPropertyItem(image : *mut GpImage, item : *const PropertyItem) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetRenderingOrigin(graphics : *mut GpGraphics, x : i32, y : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetSmoothingMode(graphics : *mut GpGraphics, smoothingmode : SmoothingMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetSolidFillColor(brush : *mut GpSolidFill, color : u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetStringFormatAlign(format : *mut GpStringFormat, align : StringAlignment) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetStringFormatDigitSubstitution(format : *mut GpStringFormat, language : u16, substitute : StringDigitSubstitute) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetStringFormatFlags(format : *mut GpStringFormat, flags : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetStringFormatHotkeyPrefix(format : *mut GpStringFormat, hotkeyprefix : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetStringFormatLineAlign(format : *mut GpStringFormat, align : StringAlignment) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetStringFormatMeasurableCharacterRanges(format : *mut GpStringFormat, rangecount : i32, ranges : *const CharacterRange) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetStringFormatTabStops(format : *mut GpStringFormat, firsttaboffset : f32, count : i32, tabstops : *const f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetStringFormatTrimming(format : *mut GpStringFormat, trimming : StringTrimming) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetTextContrast(graphics : *mut GpGraphics, contrast : u32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetTextRenderingHint(graphics : *mut GpGraphics, mode : TextRenderingHint) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetTextureTransform(brush : *mut GpTexture, matrix : *const Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetTextureWrapMode(brush : *mut GpTexture, wrapmode : WrapMode) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipSetWorldTransform(graphics : *mut GpGraphics, matrix : *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipShearMatrix(matrix : *mut Matrix, shearx : f32, sheary : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipStartPathFigure(path : *mut GpPath) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipStringFormatGetGenericDefault(format : *mut *mut GpStringFormat) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipStringFormatGetGenericTypographic(format : *mut *mut GpStringFormat) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTestControl(control : GpTestControlEnum, param1 : *mut ::core::ffi::c_void) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTransformMatrixPoints(matrix : *mut Matrix, pts : *mut PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTransformMatrixPointsI(matrix : *mut Matrix, pts : *mut Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTransformPath(path : *mut GpPath, matrix : *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTransformPoints(graphics : *mut GpGraphics, destspace : CoordinateSpace, srcspace : CoordinateSpace, points : *mut PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTransformPointsI(graphics : *mut GpGraphics, destspace : CoordinateSpace, srcspace : CoordinateSpace, points : *mut Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTransformRegion(region : *mut GpRegion, matrix : *mut Matrix) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTranslateClip(graphics : *mut GpGraphics, dx : f32, dy : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTranslateClipI(graphics : *mut GpGraphics, dx : i32, dy : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTranslateLineTransform(brush : *mut GpLineGradient, dx : f32, dy : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTranslateMatrix(matrix : *mut Matrix, offsetx : f32, offsety : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTranslatePathGradientTransform(brush : *mut GpPathGradient, dx : f32, dy : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTranslatePenTransform(pen : *mut GpPen, dx : f32, dy : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTranslateRegion(region : *mut GpRegion, dx : f32, dy : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTranslateRegionI(region : *mut GpRegion, dx : i32, dy : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTranslateTextureTransform(brush : *mut GpTexture, dx : f32, dy : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipTranslateWorldTransform(graphics : *mut GpGraphics, dx : f32, dy : f32, order : MatrixOrder) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipVectorTransformMatrixPoints(matrix : *mut Matrix, pts : *mut PointF, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipVectorTransformMatrixPointsI(matrix : *mut Matrix, pts : *mut Point, count : i32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipWarpPath(path : *mut GpPath, matrix : *mut Matrix, points : *const PointF, count : i32, srcx : f32, srcy : f32, srcwidth : f32, srcheight : f32, warpmode : WarpMode, flatness : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipWidenPath(nativepath : *mut GpPath, pen : *mut GpPen, matrix : *mut Matrix, flatness : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdipWindingModeOutline(path : *mut GpPath, matrix : *mut Matrix, flatness : f32) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdiplusNotificationHook(token : *mut usize) -> Status);
::windows_targets::link!("gdiplus.dll" "system" fn GdiplusNotificationUnhook(token : usize) -> ());
::windows_targets::link!("gdiplus.dll" "system" fn GdiplusShutdown(token : usize) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdiplus.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GdiplusStartup(token : *mut usize, input : *const GdiplusStartupInput, output : *mut GdiplusStartupOutput) -> Status);
pub type GdiplusAbort = *mut ::core::ffi::c_void;
pub type IImageBytes = *mut ::core::ffi::c_void;
pub const ALPHA_SHIFT: u32 = 24u32;
pub const Aborted: Status = 9i32;
pub const AccessDenied: Status = 12i32;
pub const AdjustBlackSaturation: CurveAdjustments = 7i32;
pub const AdjustContrast: CurveAdjustments = 2i32;
pub const AdjustDensity: CurveAdjustments = 1i32;
pub const AdjustExposure: CurveAdjustments = 0i32;
pub const AdjustHighlight: CurveAdjustments = 3i32;
pub const AdjustMidtone: CurveAdjustments = 5i32;
pub const AdjustShadow: CurveAdjustments = 4i32;
pub const AdjustWhiteSaturation: CurveAdjustments = 6i32;
pub const BLUE_SHIFT: u32 = 0u32;
pub const BlurEffectGuid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x633c80a4_1843_482b_9ef2_be2834c5fdd4);
pub const BrightnessContrastEffectGuid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd3a1dbe1_8ec4_4c17_9f4c_ea97ad1c343d);
pub const BrushTypeHatchFill: BrushType = 1i32;
pub const BrushTypeLinearGradient: BrushType = 4i32;
pub const BrushTypePathGradient: BrushType = 3i32;
pub const BrushTypeSolidColor: BrushType = 0i32;
pub const BrushTypeTextureFill: BrushType = 2i32;
pub const CodecIImageBytes: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x025d1823_6c7d_447b_bbdb_a3cbc3dfa2fc);
pub const ColorAdjustTypeAny: ColorAdjustType = 6i32;
pub const ColorAdjustTypeBitmap: ColorAdjustType = 1i32;
pub const ColorAdjustTypeBrush: ColorAdjustType = 2i32;
pub const ColorAdjustTypeCount: ColorAdjustType = 5i32;
pub const ColorAdjustTypeDefault: ColorAdjustType = 0i32;
pub const ColorAdjustTypePen: ColorAdjustType = 3i32;
pub const ColorAdjustTypeText: ColorAdjustType = 4i32;
pub const ColorBalanceEffectGuid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x537e597d_251e_48da_9664_29ca496b70f8);
pub const ColorChannelFlagsC: ColorChannelFlags = 0i32;
pub const ColorChannelFlagsK: ColorChannelFlags = 3i32;
pub const ColorChannelFlagsLast: ColorChannelFlags = 4i32;
pub const ColorChannelFlagsM: ColorChannelFlags = 1i32;
pub const ColorChannelFlagsY: ColorChannelFlags = 2i32;
pub const ColorCurveEffectGuid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdd6a0022_58e4_4a67_9d9b_d48eb881a53d);
pub const ColorLUTEffectGuid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa7ce72a9_0f7f_40d7_b3cc_d0c02d5c3212);
pub const ColorMatrixEffectGuid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x718f2615_7933_40e3_a511_5f68fe14dd74);
pub const ColorMatrixFlagsAltGray: ColorMatrixFlags = 2i32;
pub const ColorMatrixFlagsDefault: ColorMatrixFlags = 0i32;
pub const ColorMatrixFlagsSkipGrays: ColorMatrixFlags = 1i32;
pub const ColorModeARGB32: ColorMode = 0i32;
pub const ColorModeARGB64: ColorMode = 1i32;
pub const CombineModeComplement: CombineMode = 5i32;
pub const CombineModeExclude: CombineMode = 4i32;
pub const CombineModeIntersect: CombineMode = 1i32;
pub const CombineModeReplace: CombineMode = 0i32;
pub const CombineModeUnion: CombineMode = 2i32;
pub const CombineModeXor: CombineMode = 3i32;
pub const CompositingModeSourceCopy: CompositingMode = 1i32;
pub const CompositingModeSourceOver: CompositingMode = 0i32;
pub const CompositingQualityAssumeLinear: CompositingQuality = 4i32;
pub const CompositingQualityDefault: CompositingQuality = 0i32;
pub const CompositingQualityGammaCorrected: CompositingQuality = 3i32;
pub const CompositingQualityHighQuality: CompositingQuality = 2i32;
pub const CompositingQualityHighSpeed: CompositingQuality = 1i32;
pub const CompositingQualityInvalid: CompositingQuality = -1i32;
pub const ConvertToEmfPlusFlagsDefault: ConvertToEmfPlusFlags = 0i32;
pub const ConvertToEmfPlusFlagsInvalidRecord: ConvertToEmfPlusFlags = 4i32;
pub const ConvertToEmfPlusFlagsRopUsed: ConvertToEmfPlusFlags = 1i32;
pub const ConvertToEmfPlusFlagsText: ConvertToEmfPlusFlags = 2i32;
pub const CoordinateSpaceDevice: CoordinateSpace = 2i32;
pub const CoordinateSpacePage: CoordinateSpace = 1i32;
pub const CoordinateSpaceWorld: CoordinateSpace = 0i32;
pub const CurveChannelAll: CurveChannel = 0i32;
pub const CurveChannelBlue: CurveChannel = 3i32;
pub const CurveChannelGreen: CurveChannel = 2i32;
pub const CurveChannelRed: CurveChannel = 1i32;
pub const CustomLineCapTypeAdjustableArrow: CustomLineCapType = 1i32;
pub const CustomLineCapTypeDefault: CustomLineCapType = 0i32;
pub const DashCapFlat: DashCap = 0i32;
pub const DashCapRound: DashCap = 2i32;
pub const DashCapTriangle: DashCap = 3i32;
pub const DashStyleCustom: DashStyle = 5i32;
pub const DashStyleDash: DashStyle = 1i32;
pub const DashStyleDashDot: DashStyle = 3i32;
pub const DashStyleDashDotDot: DashStyle = 4i32;
pub const DashStyleDot: DashStyle = 2i32;
pub const DashStyleSolid: DashStyle = 0i32;
pub const DebugEventLevelFatal: DebugEventLevel = 0i32;
pub const DebugEventLevelWarning: DebugEventLevel = 1i32;
pub const DitherTypeDualSpiral4x4: DitherType = 7i32;
pub const DitherTypeDualSpiral8x8: DitherType = 8i32;
pub const DitherTypeErrorDiffusion: DitherType = 9i32;
pub const DitherTypeMax: DitherType = 10i32;
pub const DitherTypeNone: DitherType = 0i32;
pub const DitherTypeOrdered16x16: DitherType = 4i32;
pub const DitherTypeOrdered4x4: DitherType = 2i32;
pub const DitherTypeOrdered8x8: DitherType = 3i32;
pub const DitherTypeSolid: DitherType = 1i32;
pub const DitherTypeSpiral4x4: DitherType = 5i32;
pub const DitherTypeSpiral8x8: DitherType = 6i32;
pub const DriverStringOptionsCmapLookup: DriverStringOptions = 1i32;
pub const DriverStringOptionsLimitSubpixel: DriverStringOptions = 8i32;
pub const DriverStringOptionsRealizedAdvance: DriverStringOptions = 4i32;
pub const DriverStringOptionsVertical: DriverStringOptions = 2i32;
pub const EmfPlusRecordTotal: EmfPlusRecordType = 16443i32;
pub const EmfPlusRecordTypeBeginContainer: EmfPlusRecordType = 16423i32;
pub const EmfPlusRecordTypeBeginContainerNoParams: EmfPlusRecordType = 16424i32;
pub const EmfPlusRecordTypeClear: EmfPlusRecordType = 16393i32;
pub const EmfPlusRecordTypeComment: EmfPlusRecordType = 16387i32;
pub const EmfPlusRecordTypeDrawArc: EmfPlusRecordType = 16402i32;
pub const EmfPlusRecordTypeDrawBeziers: EmfPlusRecordType = 16409i32;
pub const EmfPlusRecordTypeDrawClosedCurve: EmfPlusRecordType = 16407i32;
pub const EmfPlusRecordTypeDrawCurve: EmfPlusRecordType = 16408i32;
pub const EmfPlusRecordTypeDrawDriverString: EmfPlusRecordType = 16438i32;
pub const EmfPlusRecordTypeDrawEllipse: EmfPlusRecordType = 16399i32;
pub const EmfPlusRecordTypeDrawImage: EmfPlusRecordType = 16410i32;
pub const EmfPlusRecordTypeDrawImagePoints: EmfPlusRecordType = 16411i32;
pub const EmfPlusRecordTypeDrawLines: EmfPlusRecordType = 16397i32;
pub const EmfPlusRecordTypeDrawPath: EmfPlusRecordType = 16405i32;
pub const EmfPlusRecordTypeDrawPie: EmfPlusRecordType = 16401i32;
pub const EmfPlusRecordTypeDrawRects: EmfPlusRecordType = 16395i32;
pub const EmfPlusRecordTypeDrawString: EmfPlusRecordType = 16412i32;
pub const EmfPlusRecordTypeEndContainer: EmfPlusRecordType = 16425i32;
pub const EmfPlusRecordTypeEndOfFile: EmfPlusRecordType = 16386i32;
pub const EmfPlusRecordTypeFillClosedCurve: EmfPlusRecordType = 16406i32;
pub const EmfPlusRecordTypeFillEllipse: EmfPlusRecordType = 16398i32;
pub const EmfPlusRecordTypeFillPath: EmfPlusRecordType = 16404i32;
pub const EmfPlusRecordTypeFillPie: EmfPlusRecordType = 16400i32;
pub const EmfPlusRecordTypeFillPolygon: EmfPlusRecordType = 16396i32;
pub const EmfPlusRecordTypeFillRects: EmfPlusRecordType = 16394i32;
pub const EmfPlusRecordTypeFillRegion: EmfPlusRecordType = 16403i32;
pub const EmfPlusRecordTypeGetDC: EmfPlusRecordType = 16388i32;
pub const EmfPlusRecordTypeHeader: EmfPlusRecordType = 16385i32;
pub const EmfPlusRecordTypeInvalid: EmfPlusRecordType = 16384i32;
pub const EmfPlusRecordTypeMax: EmfPlusRecordType = 16442i32;
pub const EmfPlusRecordTypeMin: EmfPlusRecordType = 16385i32;
pub const EmfPlusRecordTypeMultiFormatEnd: EmfPlusRecordType = 16391i32;
pub const EmfPlusRecordTypeMultiFormatSection: EmfPlusRecordType = 16390i32;
pub const EmfPlusRecordTypeMultiFormatStart: EmfPlusRecordType = 16389i32;
pub const EmfPlusRecordTypeMultiplyWorldTransform: EmfPlusRecordType = 16428i32;
pub const EmfPlusRecordTypeObject: EmfPlusRecordType = 16392i32;
pub const EmfPlusRecordTypeOffsetClip: EmfPlusRecordType = 16437i32;
pub const EmfPlusRecordTypeResetClip: EmfPlusRecordType = 16433i32;
pub const EmfPlusRecordTypeResetWorldTransform: EmfPlusRecordType = 16427i32;
pub const EmfPlusRecordTypeRestore: EmfPlusRecordType = 16422i32;
pub const EmfPlusRecordTypeRotateWorldTransform: EmfPlusRecordType = 16431i32;
pub const EmfPlusRecordTypeSave: EmfPlusRecordType = 16421i32;
pub const EmfPlusRecordTypeScaleWorldTransform: EmfPlusRecordType = 16430i32;
pub const EmfPlusRecordTypeSerializableObject: EmfPlusRecordType = 16440i32;
pub const EmfPlusRecordTypeSetAntiAliasMode: EmfPlusRecordType = 16414i32;
pub const EmfPlusRecordTypeSetClipPath: EmfPlusRecordType = 16435i32;
pub const EmfPlusRecordTypeSetClipRect: EmfPlusRecordType = 16434i32;
pub const EmfPlusRecordTypeSetClipRegion: EmfPlusRecordType = 16436i32;
pub const EmfPlusRecordTypeSetCompositingMode: EmfPlusRecordType = 16419i32;
pub const EmfPlusRecordTypeSetCompositingQuality: EmfPlusRecordType = 16420i32;
pub const EmfPlusRecordTypeSetInterpolationMode: EmfPlusRecordType = 16417i32;
pub const EmfPlusRecordTypeSetPageTransform: EmfPlusRecordType = 16432i32;
pub const EmfPlusRecordTypeSetPixelOffsetMode: EmfPlusRecordType = 16418i32;
pub const EmfPlusRecordTypeSetRenderingOrigin: EmfPlusRecordType = 16413i32;
pub const EmfPlusRecordTypeSetTSClip: EmfPlusRecordType = 16442i32;
pub const EmfPlusRecordTypeSetTSGraphics: EmfPlusRecordType = 16441i32;
pub const EmfPlusRecordTypeSetTextContrast: EmfPlusRecordType = 16416i32;
pub const EmfPlusRecordTypeSetTextRenderingHint: EmfPlusRecordType = 16415i32;
pub const EmfPlusRecordTypeSetWorldTransform: EmfPlusRecordType = 16426i32;
pub const EmfPlusRecordTypeStrokeFillPath: EmfPlusRecordType = 16439i32;
pub const EmfPlusRecordTypeTranslateWorldTransform: EmfPlusRecordType = 16429i32;
pub const EmfRecordTypeAbortPath: EmfPlusRecordType = 68i32;
pub const EmfRecordTypeAlphaBlend: EmfPlusRecordType = 114i32;
pub const EmfRecordTypeAngleArc: EmfPlusRecordType = 41i32;
pub const EmfRecordTypeArc: EmfPlusRecordType = 45i32;
pub const EmfRecordTypeArcTo: EmfPlusRecordType = 55i32;
pub const EmfRecordTypeBeginPath: EmfPlusRecordType = 59i32;
pub const EmfRecordTypeBitBlt: EmfPlusRecordType = 76i32;
pub const EmfRecordTypeChord: EmfPlusRecordType = 46i32;
pub const EmfRecordTypeCloseFigure: EmfPlusRecordType = 61i32;
pub const EmfRecordTypeColorCorrectPalette: EmfPlusRecordType = 111i32;
pub const EmfRecordTypeColorMatchToTargetW: EmfPlusRecordType = 121i32;
pub const EmfRecordTypeCreateBrushIndirect: EmfPlusRecordType = 39i32;
pub const EmfRecordTypeCreateColorSpace: EmfPlusRecordType = 99i32;
pub const EmfRecordTypeCreateColorSpaceW: EmfPlusRecordType = 122i32;
pub const EmfRecordTypeCreateDIBPatternBrushPt: EmfPlusRecordType = 94i32;
pub const EmfRecordTypeCreateMonoBrush: EmfPlusRecordType = 93i32;
pub const EmfRecordTypeCreatePalette: EmfPlusRecordType = 49i32;
pub const EmfRecordTypeCreatePen: EmfPlusRecordType = 38i32;
pub const EmfRecordTypeDeleteColorSpace: EmfPlusRecordType = 101i32;
pub const EmfRecordTypeDeleteObject: EmfPlusRecordType = 40i32;
pub const EmfRecordTypeDrawEscape: EmfPlusRecordType = 105i32;
pub const EmfRecordTypeEOF: EmfPlusRecordType = 14i32;
pub const EmfRecordTypeEllipse: EmfPlusRecordType = 42i32;
pub const EmfRecordTypeEndPath: EmfPlusRecordType = 60i32;
pub const EmfRecordTypeExcludeClipRect: EmfPlusRecordType = 29i32;
pub const EmfRecordTypeExtCreateFontIndirect: EmfPlusRecordType = 82i32;
pub const EmfRecordTypeExtCreatePen: EmfPlusRecordType = 95i32;
pub const EmfRecordTypeExtEscape: EmfPlusRecordType = 106i32;
pub const EmfRecordTypeExtFloodFill: EmfPlusRecordType = 53i32;
pub const EmfRecordTypeExtSelectClipRgn: EmfPlusRecordType = 75i32;
pub const EmfRecordTypeExtTextOutA: EmfPlusRecordType = 83i32;
pub const EmfRecordTypeExtTextOutW: EmfPlusRecordType = 84i32;
pub const EmfRecordTypeFillPath: EmfPlusRecordType = 62i32;
pub const EmfRecordTypeFillRgn: EmfPlusRecordType = 71i32;
pub const EmfRecordTypeFlattenPath: EmfPlusRecordType = 65i32;
pub const EmfRecordTypeForceUFIMapping: EmfPlusRecordType = 109i32;
pub const EmfRecordTypeFrameRgn: EmfPlusRecordType = 72i32;
pub const EmfRecordTypeGLSBoundedRecord: EmfPlusRecordType = 103i32;
pub const EmfRecordTypeGLSRecord: EmfPlusRecordType = 102i32;
pub const EmfRecordTypeGdiComment: EmfPlusRecordType = 70i32;
pub const EmfRecordTypeGradientFill: EmfPlusRecordType = 118i32;
pub const EmfRecordTypeHeader: EmfPlusRecordType = 1i32;
pub const EmfRecordTypeIntersectClipRect: EmfPlusRecordType = 30i32;
pub const EmfRecordTypeInvertRgn: EmfPlusRecordType = 73i32;
pub const EmfRecordTypeLineTo: EmfPlusRecordType = 54i32;
pub const EmfRecordTypeMaskBlt: EmfPlusRecordType = 78i32;
pub const EmfRecordTypeMax: EmfPlusRecordType = 122i32;
pub const EmfRecordTypeMin: EmfPlusRecordType = 1i32;
pub const EmfRecordTypeModifyWorldTransform: EmfPlusRecordType = 36i32;
pub const EmfRecordTypeMoveToEx: EmfPlusRecordType = 27i32;
pub const EmfRecordTypeNamedEscape: EmfPlusRecordType = 110i32;
pub const EmfRecordTypeOffsetClipRgn: EmfPlusRecordType = 26i32;
pub const EmfRecordTypePaintRgn: EmfPlusRecordType = 74i32;
pub const EmfRecordTypePie: EmfPlusRecordType = 47i32;
pub const EmfRecordTypePixelFormat: EmfPlusRecordType = 104i32;
pub const EmfRecordTypePlgBlt: EmfPlusRecordType = 79i32;
pub const EmfRecordTypePolyBezier: EmfPlusRecordType = 2i32;
pub const EmfRecordTypePolyBezier16: EmfPlusRecordType = 85i32;
pub const EmfRecordTypePolyBezierTo: EmfPlusRecordType = 5i32;
pub const EmfRecordTypePolyBezierTo16: EmfPlusRecordType = 88i32;
pub const EmfRecordTypePolyDraw: EmfPlusRecordType = 56i32;
pub const EmfRecordTypePolyDraw16: EmfPlusRecordType = 92i32;
pub const EmfRecordTypePolyLineTo: EmfPlusRecordType = 6i32;
pub const EmfRecordTypePolyPolygon: EmfPlusRecordType = 8i32;
pub const EmfRecordTypePolyPolygon16: EmfPlusRecordType = 91i32;
pub const EmfRecordTypePolyPolyline: EmfPlusRecordType = 7i32;
pub const EmfRecordTypePolyPolyline16: EmfPlusRecordType = 90i32;
pub const EmfRecordTypePolyTextOutA: EmfPlusRecordType = 96i32;
pub const EmfRecordTypePolyTextOutW: EmfPlusRecordType = 97i32;
pub const EmfRecordTypePolygon: EmfPlusRecordType = 3i32;
pub const EmfRecordTypePolygon16: EmfPlusRecordType = 86i32;
pub const EmfRecordTypePolyline: EmfPlusRecordType = 4i32;
pub const EmfRecordTypePolyline16: EmfPlusRecordType = 87i32;
pub const EmfRecordTypePolylineTo16: EmfPlusRecordType = 89i32;
pub const EmfRecordTypeRealizePalette: EmfPlusRecordType = 52i32;
pub const EmfRecordTypeRectangle: EmfPlusRecordType = 43i32;
pub const EmfRecordTypeReserved_069: EmfPlusRecordType = 69i32;
pub const EmfRecordTypeReserved_117: EmfPlusRecordType = 117i32;
pub const EmfRecordTypeResizePalette: EmfPlusRecordType = 51i32;
pub const EmfRecordTypeRestoreDC: EmfPlusRecordType = 34i32;
pub const EmfRecordTypeRoundRect: EmfPlusRecordType = 44i32;
pub const EmfRecordTypeSaveDC: EmfPlusRecordType = 33i32;
pub const EmfRecordTypeScaleViewportExtEx: EmfPlusRecordType = 31i32;
pub const EmfRecordTypeScaleWindowExtEx: EmfPlusRecordType = 32i32;
pub const EmfRecordTypeSelectClipPath: EmfPlusRecordType = 67i32;
pub const EmfRecordTypeSelectObject: EmfPlusRecordType = 37i32;
pub const EmfRecordTypeSelectPalette: EmfPlusRecordType = 48i32;
pub const EmfRecordTypeSetArcDirection: EmfPlusRecordType = 57i32;
pub const EmfRecordTypeSetBkColor: EmfPlusRecordType = 25i32;
pub const EmfRecordTypeSetBkMode: EmfPlusRecordType = 18i32;
pub const EmfRecordTypeSetBrushOrgEx: EmfPlusRecordType = 13i32;
pub const EmfRecordTypeSetColorAdjustment: EmfPlusRecordType = 23i32;
pub const EmfRecordTypeSetColorSpace: EmfPlusRecordType = 100i32;
pub const EmfRecordTypeSetDIBitsToDevice: EmfPlusRecordType = 80i32;
pub const EmfRecordTypeSetICMMode: EmfPlusRecordType = 98i32;
pub const EmfRecordTypeSetICMProfileA: EmfPlusRecordType = 112i32;
pub const EmfRecordTypeSetICMProfileW: EmfPlusRecordType = 113i32;
pub const EmfRecordTypeSetLayout: EmfPlusRecordType = 115i32;
pub const EmfRecordTypeSetLinkedUFIs: EmfPlusRecordType = 119i32;
pub const EmfRecordTypeSetMapMode: EmfPlusRecordType = 17i32;
pub const EmfRecordTypeSetMapperFlags: EmfPlusRecordType = 16i32;
pub const EmfRecordTypeSetMetaRgn: EmfPlusRecordType = 28i32;
pub const EmfRecordTypeSetMiterLimit: EmfPlusRecordType = 58i32;
pub const EmfRecordTypeSetPaletteEntries: EmfPlusRecordType = 50i32;
pub const EmfRecordTypeSetPixelV: EmfPlusRecordType = 15i32;
pub const EmfRecordTypeSetPolyFillMode: EmfPlusRecordType = 19i32;
pub const EmfRecordTypeSetROP2: EmfPlusRecordType = 20i32;
pub const EmfRecordTypeSetStretchBltMode: EmfPlusRecordType = 21i32;
pub const EmfRecordTypeSetTextAlign: EmfPlusRecordType = 22i32;
pub const EmfRecordTypeSetTextColor: EmfPlusRecordType = 24i32;
pub const EmfRecordTypeSetTextJustification: EmfPlusRecordType = 120i32;
pub const EmfRecordTypeSetViewportExtEx: EmfPlusRecordType = 11i32;
pub const EmfRecordTypeSetViewportOrgEx: EmfPlusRecordType = 12i32;
pub const EmfRecordTypeSetWindowExtEx: EmfPlusRecordType = 9i32;
pub const EmfRecordTypeSetWindowOrgEx: EmfPlusRecordType = 10i32;
pub const EmfRecordTypeSetWorldTransform: EmfPlusRecordType = 35i32;
pub const EmfRecordTypeSmallTextOut: EmfPlusRecordType = 108i32;
pub const EmfRecordTypeStartDoc: EmfPlusRecordType = 107i32;
pub const EmfRecordTypeStretchBlt: EmfPlusRecordType = 77i32;
pub const EmfRecordTypeStretchDIBits: EmfPlusRecordType = 81i32;
pub const EmfRecordTypeStrokeAndFillPath: EmfPlusRecordType = 63i32;
pub const EmfRecordTypeStrokePath: EmfPlusRecordType = 64i32;
pub const EmfRecordTypeTransparentBlt: EmfPlusRecordType = 116i32;
pub const EmfRecordTypeWidenPath: EmfPlusRecordType = 66i32;
pub const EmfToWmfBitsFlagsDefault: EmfToWmfBitsFlags = 0i32;
pub const EmfToWmfBitsFlagsEmbedEmf: EmfToWmfBitsFlags = 1i32;
pub const EmfToWmfBitsFlagsIncludePlaceable: EmfToWmfBitsFlags = 2i32;
pub const EmfToWmfBitsFlagsNoXORClip: EmfToWmfBitsFlags = 4i32;
pub const EmfTypeEmfOnly: EmfType = 3i32;
pub const EmfTypeEmfPlusDual: EmfType = 5i32;
pub const EmfTypeEmfPlusOnly: EmfType = 4i32;
pub const EncoderChrominanceTable: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e455dc_09b3_4316_8260_676ada32481c);
pub const EncoderColorDepth: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x66087055_ad66_4c7c_9a18_38a2310b8337);
pub const EncoderColorSpace: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xae7a62a0_ee2c_49d8_9d07_1ba8a927596e);
pub const EncoderCompression: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe09d739d_ccd4_44ee_8eba_3fbf8be4fc58);
pub const EncoderImageItems: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x63875e13_1f1d_45ab_9195_a29b6066a650);
pub const EncoderLuminanceTable: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xedb33bce_0266_4a77_b904_27216099e717);
pub const EncoderParameterValueTypeASCII: EncoderParameterValueType = 2i32;
pub const EncoderParameterValueTypeByte: EncoderParameterValueType = 1i32;
pub const EncoderParameterValueTypeLong: EncoderParameterValueType = 4i32;
pub const EncoderParameterValueTypeLongRange: EncoderParameterValueType = 6i32;
pub const EncoderParameterValueTypePointer: EncoderParameterValueType = 9i32;
pub const EncoderParameterValueTypeRational: EncoderParameterValueType = 5i32;
pub const EncoderParameterValueTypeRationalRange: EncoderParameterValueType = 8i32;
pub const EncoderParameterValueTypeShort: EncoderParameterValueType = 3i32;
pub const EncoderParameterValueTypeUndefined: EncoderParameterValueType = 7i32;
pub const EncoderQuality: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1d5be4b5_fa4a_452d_9cdd_5db35105e7eb);
pub const EncoderRenderMethod: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6d42c53a_229a_4825_8bb7_5c99e2b9a8b8);
pub const EncoderSaveAsCMYK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa219bbc9_0a9d_4005_a3ee_3a421b8bb06c);
pub const EncoderSaveFlag: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x292266fc_ac40_47bf_8cfc_a85b89a655de);
pub const EncoderScanMethod: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3a4e2661_3109_4e56_8536_42c156e7dcfa);
pub const EncoderTransformation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8d0eb2d1_a58e_4ea8_aa14_108074b7b6f9);
pub const EncoderValueColorTypeCMYK: EncoderValue = 0i32;
pub const EncoderValueColorTypeGray: EncoderValue = 24i32;
pub const EncoderValueColorTypeRGB: EncoderValue = 25i32;
pub const EncoderValueColorTypeYCCK: EncoderValue = 1i32;
pub const EncoderValueCompressionCCITT3: EncoderValue = 3i32;
pub const EncoderValueCompressionCCITT4: EncoderValue = 4i32;
pub const EncoderValueCompressionLZW: EncoderValue = 2i32;
pub const EncoderValueCompressionNone: EncoderValue = 6i32;
pub const EncoderValueCompressionRle: EncoderValue = 5i32;
pub const EncoderValueFlush: EncoderValue = 20i32;
pub const EncoderValueFrameDimensionPage: EncoderValue = 23i32;
pub const EncoderValueFrameDimensionResolution: EncoderValue = 22i32;
pub const EncoderValueFrameDimensionTime: EncoderValue = 21i32;
pub const EncoderValueLastFrame: EncoderValue = 19i32;
pub const EncoderValueMultiFrame: EncoderValue = 18i32;
pub const EncoderValueRenderNonProgressive: EncoderValue = 12i32;
pub const EncoderValueRenderProgressive: EncoderValue = 11i32;
pub const EncoderValueScanMethodInterlaced: EncoderValue = 7i32;
pub const EncoderValueScanMethodNonInterlaced: EncoderValue = 8i32;
pub const EncoderValueTransformFlipHorizontal: EncoderValue = 16i32;
pub const EncoderValueTransformFlipVertical: EncoderValue = 17i32;
pub const EncoderValueTransformRotate180: EncoderValue = 14i32;
pub const EncoderValueTransformRotate270: EncoderValue = 15i32;
pub const EncoderValueTransformRotate90: EncoderValue = 13i32;
pub const EncoderValueVersionGif87: EncoderValue = 9i32;
pub const EncoderValueVersionGif89: EncoderValue = 10i32;
pub const EncoderVersion: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x24d18c76_814a_41a4_bf53_1c219cccf797);
pub const FileNotFound: Status = 10i32;
pub const FillModeAlternate: FillMode = 0i32;
pub const FillModeWinding: FillMode = 1i32;
pub const FlatnessDefault: f32 = 0.25f32;
pub const FlushIntentionFlush: FlushIntention = 0i32;
pub const FlushIntentionSync: FlushIntention = 1i32;
pub const FontFamilyNotFound: Status = 14i32;
pub const FontStyleBold: FontStyle = 1i32;
pub const FontStyleBoldItalic: FontStyle = 3i32;
pub const FontStyleItalic: FontStyle = 2i32;
pub const FontStyleNotFound: Status = 15i32;
pub const FontStyleRegular: FontStyle = 0i32;
pub const FontStyleStrikeout: FontStyle = 8i32;
pub const FontStyleUnderline: FontStyle = 4i32;
pub const FormatIDImageInformation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe5836cbe_5eef_4f1d_acde_ae4c43b608ce);
pub const FormatIDJpegAppHeaders: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1c4afdcd_6177_43cf_abc7_5f51af39ee85);
pub const FrameDimensionPage: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7462dc86_6180_4c7e_8e3f_ee7333a7a483);
pub const FrameDimensionResolution: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x84236f7b_3bd3_428f_8dab_4ea1439ca315);
pub const FrameDimensionTime: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6aedbd6d_3fb5_418a_83a6_7f45229dc872);
pub const GDIP_EMFPLUSFLAGS_DISPLAY: u32 = 1u32;
pub const GDIP_EMFPLUS_RECORD_BASE: u32 = 16384u32;
pub const GDIP_WMF_RECORD_BASE: u32 = 65536u32;
pub const GREEN_SHIFT: u32 = 8u32;
pub const GdiplusNotInitialized: Status = 18i32;
pub const GdiplusStartupDefault: GdiplusStartupParams = 0i32;
pub const GdiplusStartupNoSetRound: GdiplusStartupParams = 1i32;
pub const GdiplusStartupSetPSValue: GdiplusStartupParams = 2i32;
pub const GdiplusStartupTransparencyMask: GdiplusStartupParams = -16777216i32;
pub const GenericError: Status = 1i32;
pub const GenericFontFamilyMonospace: GenericFontFamily = 2i32;
pub const GenericFontFamilySansSerif: GenericFontFamily = 1i32;
pub const GenericFontFamilySerif: GenericFontFamily = 0i32;
pub const HatchStyle05Percent: HatchStyle = 6i32;
pub const HatchStyle10Percent: HatchStyle = 7i32;
pub const HatchStyle20Percent: HatchStyle = 8i32;
pub const HatchStyle25Percent: HatchStyle = 9i32;
pub const HatchStyle30Percent: HatchStyle = 10i32;
pub const HatchStyle40Percent: HatchStyle = 11i32;
pub const HatchStyle50Percent: HatchStyle = 12i32;
pub const HatchStyle60Percent: HatchStyle = 13i32;
pub const HatchStyle70Percent: HatchStyle = 14i32;
pub const HatchStyle75Percent: HatchStyle = 15i32;
pub const HatchStyle80Percent: HatchStyle = 16i32;
pub const HatchStyle90Percent: HatchStyle = 17i32;
pub const HatchStyleBackwardDiagonal: HatchStyle = 3i32;
pub const HatchStyleCross: HatchStyle = 4i32;
pub const HatchStyleDarkDownwardDiagonal: HatchStyle = 20i32;
pub const HatchStyleDarkHorizontal: HatchStyle = 29i32;
pub const HatchStyleDarkUpwardDiagonal: HatchStyle = 21i32;
pub const HatchStyleDarkVertical: HatchStyle = 28i32;
pub const HatchStyleDashedDownwardDiagonal: HatchStyle = 30i32;
pub const HatchStyleDashedHorizontal: HatchStyle = 32i32;
pub const HatchStyleDashedUpwardDiagonal: HatchStyle = 31i32;
pub const HatchStyleDashedVertical: HatchStyle = 33i32;
pub const HatchStyleDiagonalBrick: HatchStyle = 38i32;
pub const HatchStyleDiagonalCross: HatchStyle = 5i32;
pub const HatchStyleDivot: HatchStyle = 42i32;
pub const HatchStyleDottedDiamond: HatchStyle = 44i32;
pub const HatchStyleDottedGrid: HatchStyle = 43i32;
pub const HatchStyleForwardDiagonal: HatchStyle = 2i32;
pub const HatchStyleHorizontal: HatchStyle = 0i32;
pub const HatchStyleHorizontalBrick: HatchStyle = 39i32;
pub const HatchStyleLargeCheckerBoard: HatchStyle = 50i32;
pub const HatchStyleLargeConfetti: HatchStyle = 35i32;
pub const HatchStyleLargeGrid: HatchStyle = 4i32;
pub const HatchStyleLightDownwardDiagonal: HatchStyle = 18i32;
pub const HatchStyleLightHorizontal: HatchStyle = 25i32;
pub const HatchStyleLightUpwardDiagonal: HatchStyle = 19i32;
pub const HatchStyleLightVertical: HatchStyle = 24i32;
pub const HatchStyleMax: HatchStyle = 52i32;
pub const HatchStyleMin: HatchStyle = 0i32;
pub const HatchStyleNarrowHorizontal: HatchStyle = 27i32;
pub const HatchStyleNarrowVertical: HatchStyle = 26i32;
pub const HatchStyleOutlinedDiamond: HatchStyle = 51i32;
pub const HatchStylePlaid: HatchStyle = 41i32;
pub const HatchStyleShingle: HatchStyle = 45i32;
pub const HatchStyleSmallCheckerBoard: HatchStyle = 49i32;
pub const HatchStyleSmallConfetti: HatchStyle = 34i32;
pub const HatchStyleSmallGrid: HatchStyle = 48i32;
pub const HatchStyleSolidDiamond: HatchStyle = 52i32;
pub const HatchStyleSphere: HatchStyle = 47i32;
pub const HatchStyleTotal: HatchStyle = 53i32;
pub const HatchStyleTrellis: HatchStyle = 46i32;
pub const HatchStyleVertical: HatchStyle = 1i32;
pub const HatchStyleWave: HatchStyle = 37i32;
pub const HatchStyleWeave: HatchStyle = 40i32;
pub const HatchStyleWideDownwardDiagonal: HatchStyle = 22i32;
pub const HatchStyleWideUpwardDiagonal: HatchStyle = 23i32;
pub const HatchStyleZigZag: HatchStyle = 36i32;
pub const HistogramFormatA: HistogramFormat = 7i32;
pub const HistogramFormatARGB: HistogramFormat = 0i32;
pub const HistogramFormatB: HistogramFormat = 4i32;
pub const HistogramFormatG: HistogramFormat = 5i32;
pub const HistogramFormatGray: HistogramFormat = 3i32;
pub const HistogramFormatPARGB: HistogramFormat = 1i32;
pub const HistogramFormatR: HistogramFormat = 6i32;
pub const HistogramFormatRGB: HistogramFormat = 2i32;
pub const HotkeyPrefixHide: HotkeyPrefix = 2i32;
pub const HotkeyPrefixNone: HotkeyPrefix = 0i32;
pub const HotkeyPrefixShow: HotkeyPrefix = 1i32;
pub const HueSaturationLightnessEffectGuid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8b2dd6c3_eb07_4d87_a5f0_7108e26a9c5f);
pub const ImageCodecFlagsBlockingDecode: ImageCodecFlags = 32i32;
pub const ImageCodecFlagsBuiltin: ImageCodecFlags = 65536i32;
pub const ImageCodecFlagsDecoder: ImageCodecFlags = 2i32;
pub const ImageCodecFlagsEncoder: ImageCodecFlags = 1i32;
pub const ImageCodecFlagsSeekableEncode: ImageCodecFlags = 16i32;
pub const ImageCodecFlagsSupportBitmap: ImageCodecFlags = 4i32;
pub const ImageCodecFlagsSupportVector: ImageCodecFlags = 8i32;
pub const ImageCodecFlagsSystem: ImageCodecFlags = 131072i32;
pub const ImageCodecFlagsUser: ImageCodecFlags = 262144i32;
pub const ImageFlagsCaching: ImageFlags = 131072i32;
pub const ImageFlagsColorSpaceCMYK: ImageFlags = 32i32;
pub const ImageFlagsColorSpaceGRAY: ImageFlags = 64i32;
pub const ImageFlagsColorSpaceRGB: ImageFlags = 16i32;
pub const ImageFlagsColorSpaceYCBCR: ImageFlags = 128i32;
pub const ImageFlagsColorSpaceYCCK: ImageFlags = 256i32;
pub const ImageFlagsHasAlpha: ImageFlags = 2i32;
pub const ImageFlagsHasRealDPI: ImageFlags = 4096i32;
pub const ImageFlagsHasRealPixelSize: ImageFlags = 8192i32;
pub const ImageFlagsHasTranslucent: ImageFlags = 4i32;
pub const ImageFlagsNone: ImageFlags = 0i32;
pub const ImageFlagsPartiallyScalable: ImageFlags = 8i32;
pub const ImageFlagsReadOnly: ImageFlags = 65536i32;
pub const ImageFlagsScalable: ImageFlags = 1i32;
pub const ImageFormatBMP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96b3cab_0728_11d3_9d7b_0000f81ef32e);
pub const ImageFormatEMF: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96b3cac_0728_11d3_9d7b_0000f81ef32e);
pub const ImageFormatEXIF: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96b3cb2_0728_11d3_9d7b_0000f81ef32e);
pub const ImageFormatGIF: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96b3cb0_0728_11d3_9d7b_0000f81ef32e);
pub const ImageFormatHEIF: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96b3cb6_0728_11d3_9d7b_0000f81ef32e);
pub const ImageFormatIcon: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96b3cb5_0728_11d3_9d7b_0000f81ef32e);
pub const ImageFormatJPEG: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96b3cae_0728_11d3_9d7b_0000f81ef32e);
pub const ImageFormatMemoryBMP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96b3caa_0728_11d3_9d7b_0000f81ef32e);
pub const ImageFormatPNG: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96b3caf_0728_11d3_9d7b_0000f81ef32e);
pub const ImageFormatTIFF: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96b3cb1_0728_11d3_9d7b_0000f81ef32e);
pub const ImageFormatUndefined: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96b3ca9_0728_11d3_9d7b_0000f81ef32e);
pub const ImageFormatWEBP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96b3cb7_0728_11d3_9d7b_0000f81ef32e);
pub const ImageFormatWMF: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96b3cad_0728_11d3_9d7b_0000f81ef32e);
pub const ImageLockModeRead: ImageLockMode = 1i32;
pub const ImageLockModeUserInputBuf: ImageLockMode = 4i32;
pub const ImageLockModeWrite: ImageLockMode = 2i32;
pub const ImageTypeBitmap: ImageType = 1i32;
pub const ImageTypeMetafile: ImageType = 2i32;
pub const ImageTypeUnknown: ImageType = 0i32;
pub const InsufficientBuffer: Status = 5i32;
pub const InterpolationModeBicubic: InterpolationMode = 4i32;
pub const InterpolationModeBilinear: InterpolationMode = 3i32;
pub const InterpolationModeDefault: InterpolationMode = 0i32;
pub const InterpolationModeHighQuality: InterpolationMode = 2i32;
pub const InterpolationModeHighQualityBicubic: InterpolationMode = 7i32;
pub const InterpolationModeHighQualityBilinear: InterpolationMode = 6i32;
pub const InterpolationModeInvalid: InterpolationMode = -1i32;
pub const InterpolationModeLowQuality: InterpolationMode = 1i32;
pub const InterpolationModeNearestNeighbor: InterpolationMode = 5i32;
pub const InvalidParameter: Status = 2i32;
pub const ItemDataPositionAfterBits: ItemDataPosition = 2i32;
pub const ItemDataPositionAfterHeader: ItemDataPosition = 0i32;
pub const ItemDataPositionAfterPalette: ItemDataPosition = 1i32;
pub const LevelsEffectGuid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x99c354ec_2a31_4f3a_8c34_17a803b33a25);
pub const LineCapAnchorMask: LineCap = 240i32;
pub const LineCapArrowAnchor: LineCap = 20i32;
pub const LineCapCustom: LineCap = 255i32;
pub const LineCapDiamondAnchor: LineCap = 19i32;
pub const LineCapFlat: LineCap = 0i32;
pub const LineCapNoAnchor: LineCap = 16i32;
pub const LineCapRound: LineCap = 2i32;
pub const LineCapRoundAnchor: LineCap = 18i32;
pub const LineCapSquare: LineCap = 1i32;
pub const LineCapSquareAnchor: LineCap = 17i32;
pub const LineCapTriangle: LineCap = 3i32;
pub const LineJoinBevel: LineJoin = 1i32;
pub const LineJoinMiter: LineJoin = 0i32;
pub const LineJoinMiterClipped: LineJoin = 3i32;
pub const LineJoinRound: LineJoin = 2i32;
pub const LinearGradientModeBackwardDiagonal: LinearGradientMode = 3i32;
pub const LinearGradientModeForwardDiagonal: LinearGradientMode = 2i32;
pub const LinearGradientModeHorizontal: LinearGradientMode = 0i32;
pub const LinearGradientModeVertical: LinearGradientMode = 1i32;
pub const MatrixOrderAppend: MatrixOrder = 1i32;
pub const MatrixOrderPrepend: MatrixOrder = 0i32;
pub const MetafileFrameUnitDocument: MetafileFrameUnit = 5i32;
pub const MetafileFrameUnitGdi: MetafileFrameUnit = 7i32;
pub const MetafileFrameUnitInch: MetafileFrameUnit = 4i32;
pub const MetafileFrameUnitMillimeter: MetafileFrameUnit = 6i32;
pub const MetafileFrameUnitPixel: MetafileFrameUnit = 2i32;
pub const MetafileFrameUnitPoint: MetafileFrameUnit = 3i32;
pub const MetafileTypeEmf: MetafileType = 3i32;
pub const MetafileTypeEmfPlusDual: MetafileType = 5i32;
pub const MetafileTypeEmfPlusOnly: MetafileType = 4i32;
pub const MetafileTypeInvalid: MetafileType = 0i32;
pub const MetafileTypeWmf: MetafileType = 1i32;
pub const MetafileTypeWmfPlaceable: MetafileType = 2i32;
pub const NotImplemented: Status = 6i32;
pub const NotTrueTypeFont: Status = 16i32;
pub const ObjectBusy: Status = 4i32;
pub const ObjectTypeBrush: ObjectType = 1i32;
pub const ObjectTypeCustomLineCap: ObjectType = 9i32;
pub const ObjectTypeFont: ObjectType = 6i32;
pub const ObjectTypeGraphics: ObjectType = 10i32;
pub const ObjectTypeImage: ObjectType = 5i32;
pub const ObjectTypeImageAttributes: ObjectType = 8i32;
pub const ObjectTypeInvalid: ObjectType = 0i32;
pub const ObjectTypeMax: ObjectType = 10i32;
pub const ObjectTypeMin: ObjectType = 1i32;
pub const ObjectTypePath: ObjectType = 3i32;
pub const ObjectTypePen: ObjectType = 2i32;
pub const ObjectTypeRegion: ObjectType = 4i32;
pub const ObjectTypeStringFormat: ObjectType = 7i32;
pub const Ok: Status = 0i32;
pub const OutOfMemory: Status = 3i32;
pub const PaletteFlagsGrayScale: PaletteFlags = 2i32;
pub const PaletteFlagsHalftone: PaletteFlags = 4i32;
pub const PaletteFlagsHasAlpha: PaletteFlags = 1i32;
pub const PaletteTypeCustom: PaletteType = 0i32;
pub const PaletteTypeFixedBW: PaletteType = 2i32;
pub const PaletteTypeFixedHalftone125: PaletteType = 6i32;
pub const PaletteTypeFixedHalftone216: PaletteType = 7i32;
pub const PaletteTypeFixedHalftone252: PaletteType = 8i32;
pub const PaletteTypeFixedHalftone256: PaletteType = 9i32;
pub const PaletteTypeFixedHalftone27: PaletteType = 4i32;
pub const PaletteTypeFixedHalftone64: PaletteType = 5i32;
pub const PaletteTypeFixedHalftone8: PaletteType = 3i32;
pub const PaletteTypeOptimal: PaletteType = 1i32;
pub const PathPointTypeBezier: PathPointType = 3i32;
pub const PathPointTypeBezier3: PathPointType = 3i32;
pub const PathPointTypeCloseSubpath: PathPointType = 128i32;
pub const PathPointTypeDashMode: PathPointType = 16i32;
pub const PathPointTypeLine: PathPointType = 1i32;
pub const PathPointTypePathMarker: PathPointType = 32i32;
pub const PathPointTypePathTypeMask: PathPointType = 7i32;
pub const PathPointTypeStart: PathPointType = 0i32;
pub const PenAlignmentCenter: PenAlignment = 0i32;
pub const PenAlignmentInset: PenAlignment = 1i32;
pub const PenTypeHatchFill: PenType = 1i32;
pub const PenTypeLinearGradient: PenType = 4i32;
pub const PenTypePathGradient: PenType = 3i32;
pub const PenTypeSolidColor: PenType = 0i32;
pub const PenTypeTextureFill: PenType = 2i32;
pub const PenTypeUnknown: PenType = -1i32;
pub const PixelFormatAlpha: u32 = 262144u32;
pub const PixelFormatCanonical: u32 = 2097152u32;
pub const PixelFormatDontCare: u32 = 0u32;
pub const PixelFormatExtended: u32 = 1048576u32;
pub const PixelFormatGDI: u32 = 131072u32;
pub const PixelFormatIndexed: u32 = 65536u32;
pub const PixelFormatMax: u32 = 16u32;
pub const PixelFormatPAlpha: u32 = 524288u32;
pub const PixelFormatUndefined: u32 = 0u32;
pub const PixelOffsetModeDefault: PixelOffsetMode = 0i32;
pub const PixelOffsetModeHalf: PixelOffsetMode = 4i32;
pub const PixelOffsetModeHighQuality: PixelOffsetMode = 2i32;
pub const PixelOffsetModeHighSpeed: PixelOffsetMode = 1i32;
pub const PixelOffsetModeInvalid: PixelOffsetMode = -1i32;
pub const PixelOffsetModeNone: PixelOffsetMode = 3i32;
pub const ProfileNotFound: Status = 21i32;
pub const PropertyNotFound: Status = 19i32;
pub const PropertyNotSupported: Status = 20i32;
pub const PropertyTagArtist: u32 = 315u32;
pub const PropertyTagBitsPerSample: u32 = 258u32;
pub const PropertyTagCellHeight: u32 = 265u32;
pub const PropertyTagCellWidth: u32 = 264u32;
pub const PropertyTagChrominanceTable: u32 = 20625u32;
pub const PropertyTagColorMap: u32 = 320u32;
pub const PropertyTagColorTransferFunction: u32 = 20506u32;
pub const PropertyTagCompression: u32 = 259u32;
pub const PropertyTagCopyright: u32 = 33432u32;
pub const PropertyTagDateTime: u32 = 306u32;
pub const PropertyTagDocumentName: u32 = 269u32;
pub const PropertyTagDotRange: u32 = 336u32;
pub const PropertyTagEquipMake: u32 = 271u32;
pub const PropertyTagEquipModel: u32 = 272u32;
pub const PropertyTagExifAperture: u32 = 37378u32;
pub const PropertyTagExifBrightness: u32 = 37379u32;
pub const PropertyTagExifCfaPattern: u32 = 41730u32;
pub const PropertyTagExifColorSpace: u32 = 40961u32;
pub const PropertyTagExifCompBPP: u32 = 37122u32;
pub const PropertyTagExifCompConfig: u32 = 37121u32;
pub const PropertyTagExifContrast: u32 = 41992u32;
pub const PropertyTagExifCustomRendered: u32 = 41985u32;
pub const PropertyTagExifDTDigSS: u32 = 37522u32;
pub const PropertyTagExifDTDigitized: u32 = 36868u32;
pub const PropertyTagExifDTOrig: u32 = 36867u32;
pub const PropertyTagExifDTOrigSS: u32 = 37521u32;
pub const PropertyTagExifDTSubsec: u32 = 37520u32;
pub const PropertyTagExifDeviceSettingDesc: u32 = 41995u32;
pub const PropertyTagExifDigitalZoomRatio: u32 = 41988u32;
pub const PropertyTagExifExposureBias: u32 = 37380u32;
pub const PropertyTagExifExposureIndex: u32 = 41493u32;
pub const PropertyTagExifExposureMode: u32 = 41986u32;
pub const PropertyTagExifExposureProg: u32 = 34850u32;
pub const PropertyTagExifExposureTime: u32 = 33434u32;
pub const PropertyTagExifFNumber: u32 = 33437u32;
pub const PropertyTagExifFPXVer: u32 = 40960u32;
pub const PropertyTagExifFileSource: u32 = 41728u32;
pub const PropertyTagExifFlash: u32 = 37385u32;
pub const PropertyTagExifFlashEnergy: u32 = 41483u32;
pub const PropertyTagExifFocalLength: u32 = 37386u32;
pub const PropertyTagExifFocalLengthIn35mmFilm: u32 = 41989u32;
pub const PropertyTagExifFocalResUnit: u32 = 41488u32;
pub const PropertyTagExifFocalXRes: u32 = 41486u32;
pub const PropertyTagExifFocalYRes: u32 = 41487u32;
pub const PropertyTagExifGainControl: u32 = 41991u32;
pub const PropertyTagExifIFD: u32 = 34665u32;
pub const PropertyTagExifISOSpeed: u32 = 34855u32;
pub const PropertyTagExifInterop: u32 = 40965u32;
pub const PropertyTagExifLightSource: u32 = 37384u32;
pub const PropertyTagExifMakerNote: u32 = 37500u32;
pub const PropertyTagExifMaxAperture: u32 = 37381u32;
pub const PropertyTagExifMeteringMode: u32 = 37383u32;
pub const PropertyTagExifOECF: u32 = 34856u32;
pub const PropertyTagExifPixXDim: u32 = 40962u32;
pub const PropertyTagExifPixYDim: u32 = 40963u32;
pub const PropertyTagExifRelatedWav: u32 = 40964u32;
pub const PropertyTagExifSaturation: u32 = 41993u32;
pub const PropertyTagExifSceneCaptureType: u32 = 41990u32;
pub const PropertyTagExifSceneType: u32 = 41729u32;
pub const PropertyTagExifSensingMethod: u32 = 41495u32;
pub const PropertyTagExifSharpness: u32 = 41994u32;
pub const PropertyTagExifShutterSpeed: u32 = 37377u32;
pub const PropertyTagExifSpatialFR: u32 = 41484u32;
pub const PropertyTagExifSpectralSense: u32 = 34852u32;
pub const PropertyTagExifSubjectArea: u32 = 37396u32;
pub const PropertyTagExifSubjectDist: u32 = 37382u32;
pub const PropertyTagExifSubjectDistanceRange: u32 = 41996u32;
pub const PropertyTagExifSubjectLoc: u32 = 41492u32;
pub const PropertyTagExifUniqueImageID: u32 = 42016u32;
pub const PropertyTagExifUserComment: u32 = 37510u32;
pub const PropertyTagExifVer: u32 = 36864u32;
pub const PropertyTagExifWhiteBalance: u32 = 41987u32;
pub const PropertyTagExtraSamples: u32 = 338u32;
pub const PropertyTagFillOrder: u32 = 266u32;
pub const PropertyTagFrameDelay: u32 = 20736u32;
pub const PropertyTagFreeByteCounts: u32 = 289u32;
pub const PropertyTagFreeOffset: u32 = 288u32;
pub const PropertyTagGamma: u32 = 769u32;
pub const PropertyTagGlobalPalette: u32 = 20738u32;
pub const PropertyTagGpsAltitude: u32 = 6u32;
pub const PropertyTagGpsAltitudeRef: u32 = 5u32;
pub const PropertyTagGpsAreaInformation: u32 = 28u32;
pub const PropertyTagGpsDate: u32 = 29u32;
pub const PropertyTagGpsDestBear: u32 = 24u32;
pub const PropertyTagGpsDestBearRef: u32 = 23u32;
pub const PropertyTagGpsDestDist: u32 = 26u32;
pub const PropertyTagGpsDestDistRef: u32 = 25u32;
pub const PropertyTagGpsDestLat: u32 = 20u32;
pub const PropertyTagGpsDestLatRef: u32 = 19u32;
pub const PropertyTagGpsDestLong: u32 = 22u32;
pub const PropertyTagGpsDestLongRef: u32 = 21u32;
pub const PropertyTagGpsDifferential: u32 = 30u32;
pub const PropertyTagGpsGpsDop: u32 = 11u32;
pub const PropertyTagGpsGpsMeasureMode: u32 = 10u32;
pub const PropertyTagGpsGpsSatellites: u32 = 8u32;
pub const PropertyTagGpsGpsStatus: u32 = 9u32;
pub const PropertyTagGpsGpsTime: u32 = 7u32;
pub const PropertyTagGpsIFD: u32 = 34853u32;
pub const PropertyTagGpsImgDir: u32 = 17u32;
pub const PropertyTagGpsImgDirRef: u32 = 16u32;
pub const PropertyTagGpsLatitude: u32 = 2u32;
pub const PropertyTagGpsLatitudeRef: u32 = 1u32;
pub const PropertyTagGpsLongitude: u32 = 4u32;
pub const PropertyTagGpsLongitudeRef: u32 = 3u32;
pub const PropertyTagGpsMapDatum: u32 = 18u32;
pub const PropertyTagGpsProcessingMethod: u32 = 27u32;
pub const PropertyTagGpsSpeed: u32 = 13u32;
pub const PropertyTagGpsSpeedRef: u32 = 12u32;
pub const PropertyTagGpsTrack: u32 = 15u32;
pub const PropertyTagGpsTrackRef: u32 = 14u32;
pub const PropertyTagGpsVer: u32 = 0u32;
pub const PropertyTagGrayResponseCurve: u32 = 291u32;
pub const PropertyTagGrayResponseUnit: u32 = 290u32;
pub const PropertyTagGridSize: u32 = 20497u32;
pub const PropertyTagHalftoneDegree: u32 = 20492u32;
pub const PropertyTagHalftoneHints: u32 = 321u32;
pub const PropertyTagHalftoneLPI: u32 = 20490u32;
pub const PropertyTagHalftoneLPIUnit: u32 = 20491u32;
pub const PropertyTagHalftoneMisc: u32 = 20494u32;
pub const PropertyTagHalftoneScreen: u32 = 20495u32;
pub const PropertyTagHalftoneShape: u32 = 20493u32;
pub const PropertyTagHostComputer: u32 = 316u32;
pub const PropertyTagICCProfile: u32 = 34675u32;
pub const PropertyTagICCProfileDescriptor: u32 = 770u32;
pub const PropertyTagImageDescription: u32 = 270u32;
pub const PropertyTagImageHeight: u32 = 257u32;
pub const PropertyTagImageTitle: u32 = 800u32;
pub const PropertyTagImageWidth: u32 = 256u32;
pub const PropertyTagIndexBackground: u32 = 20739u32;
pub const PropertyTagIndexTransparent: u32 = 20740u32;
pub const PropertyTagInkNames: u32 = 333u32;
pub const PropertyTagInkSet: u32 = 332u32;
pub const PropertyTagJPEGACTables: u32 = 521u32;
pub const PropertyTagJPEGDCTables: u32 = 520u32;
pub const PropertyTagJPEGInterFormat: u32 = 513u32;
pub const PropertyTagJPEGInterLength: u32 = 514u32;
pub const PropertyTagJPEGLosslessPredictors: u32 = 517u32;
pub const PropertyTagJPEGPointTransforms: u32 = 518u32;
pub const PropertyTagJPEGProc: u32 = 512u32;
pub const PropertyTagJPEGQTables: u32 = 519u32;
pub const PropertyTagJPEGQuality: u32 = 20496u32;
pub const PropertyTagJPEGRestartInterval: u32 = 515u32;
pub const PropertyTagLoopCount: u32 = 20737u32;
pub const PropertyTagLuminanceTable: u32 = 20624u32;
pub const PropertyTagMaxSampleValue: u32 = 281u32;
pub const PropertyTagMinSampleValue: u32 = 280u32;
pub const PropertyTagNewSubfileType: u32 = 254u32;
pub const PropertyTagNumberOfInks: u32 = 334u32;
pub const PropertyTagOrientation: u32 = 274u32;
pub const PropertyTagPageName: u32 = 285u32;
pub const PropertyTagPageNumber: u32 = 297u32;
pub const PropertyTagPaletteHistogram: u32 = 20755u32;
pub const PropertyTagPhotometricInterp: u32 = 262u32;
pub const PropertyTagPixelPerUnitX: u32 = 20753u32;
pub const PropertyTagPixelPerUnitY: u32 = 20754u32;
pub const PropertyTagPixelUnit: u32 = 20752u32;
pub const PropertyTagPlanarConfig: u32 = 284u32;
pub const PropertyTagPredictor: u32 = 317u32;
pub const PropertyTagPrimaryChromaticities: u32 = 319u32;
pub const PropertyTagPrintFlags: u32 = 20485u32;
pub const PropertyTagPrintFlagsBleedWidth: u32 = 20488u32;
pub const PropertyTagPrintFlagsBleedWidthScale: u32 = 20489u32;
pub const PropertyTagPrintFlagsCrop: u32 = 20487u32;
pub const PropertyTagPrintFlagsVersion: u32 = 20486u32;
pub const PropertyTagREFBlackWhite: u32 = 532u32;
pub const PropertyTagResolutionUnit: u32 = 296u32;
pub const PropertyTagResolutionXLengthUnit: u32 = 20483u32;
pub const PropertyTagResolutionXUnit: u32 = 20481u32;
pub const PropertyTagResolutionYLengthUnit: u32 = 20484u32;
pub const PropertyTagResolutionYUnit: u32 = 20482u32;
pub const PropertyTagRowsPerStrip: u32 = 278u32;
pub const PropertyTagSMaxSampleValue: u32 = 341u32;
pub const PropertyTagSMinSampleValue: u32 = 340u32;
pub const PropertyTagSRGBRenderingIntent: u32 = 771u32;
pub const PropertyTagSampleFormat: u32 = 339u32;
pub const PropertyTagSamplesPerPixel: u32 = 277u32;
pub const PropertyTagSoftwareUsed: u32 = 305u32;
pub const PropertyTagStripBytesCount: u32 = 279u32;
pub const PropertyTagStripOffsets: u32 = 273u32;
pub const PropertyTagSubfileType: u32 = 255u32;
pub const PropertyTagT4Option: u32 = 292u32;
pub const PropertyTagT6Option: u32 = 293u32;
pub const PropertyTagTargetPrinter: u32 = 337u32;
pub const PropertyTagThreshHolding: u32 = 263u32;
pub const PropertyTagThumbnailArtist: u32 = 20532u32;
pub const PropertyTagThumbnailBitsPerSample: u32 = 20514u32;
pub const PropertyTagThumbnailColorDepth: u32 = 20501u32;
pub const PropertyTagThumbnailCompressedSize: u32 = 20505u32;
pub const PropertyTagThumbnailCompression: u32 = 20515u32;
pub const PropertyTagThumbnailCopyRight: u32 = 20539u32;
pub const PropertyTagThumbnailData: u32 = 20507u32;
pub const PropertyTagThumbnailDateTime: u32 = 20531u32;
pub const PropertyTagThumbnailEquipMake: u32 = 20518u32;
pub const PropertyTagThumbnailEquipModel: u32 = 20519u32;
pub const PropertyTagThumbnailFormat: u32 = 20498u32;
pub const PropertyTagThumbnailHeight: u32 = 20500u32;
pub const PropertyTagThumbnailImageDescription: u32 = 20517u32;
pub const PropertyTagThumbnailImageHeight: u32 = 20513u32;
pub const PropertyTagThumbnailImageWidth: u32 = 20512u32;
pub const PropertyTagThumbnailOrientation: u32 = 20521u32;
pub const PropertyTagThumbnailPhotometricInterp: u32 = 20516u32;
pub const PropertyTagThumbnailPlanarConfig: u32 = 20527u32;
pub const PropertyTagThumbnailPlanes: u32 = 20502u32;
pub const PropertyTagThumbnailPrimaryChromaticities: u32 = 20534u32;
pub const PropertyTagThumbnailRawBytes: u32 = 20503u32;
pub const PropertyTagThumbnailRefBlackWhite: u32 = 20538u32;
pub const PropertyTagThumbnailResolutionUnit: u32 = 20528u32;
pub const PropertyTagThumbnailResolutionX: u32 = 20525u32;
pub const PropertyTagThumbnailResolutionY: u32 = 20526u32;
pub const PropertyTagThumbnailRowsPerStrip: u32 = 20523u32;
pub const PropertyTagThumbnailSamplesPerPixel: u32 = 20522u32;
pub const PropertyTagThumbnailSize: u32 = 20504u32;
pub const PropertyTagThumbnailSoftwareUsed: u32 = 20530u32;
pub const PropertyTagThumbnailStripBytesCount: u32 = 20524u32;
pub const PropertyTagThumbnailStripOffsets: u32 = 20520u32;
pub const PropertyTagThumbnailTransferFunction: u32 = 20529u32;
pub const PropertyTagThumbnailWhitePoint: u32 = 20533u32;
pub const PropertyTagThumbnailWidth: u32 = 20499u32;
pub const PropertyTagThumbnailYCbCrCoefficients: u32 = 20535u32;
pub const PropertyTagThumbnailYCbCrPositioning: u32 = 20537u32;
pub const PropertyTagThumbnailYCbCrSubsampling: u32 = 20536u32;
pub const PropertyTagTileByteCounts: u32 = 325u32;
pub const PropertyTagTileLength: u32 = 323u32;
pub const PropertyTagTileOffset: u32 = 324u32;
pub const PropertyTagTileWidth: u32 = 322u32;
pub const PropertyTagTransferFuncition: u32 = 301u32;
pub const PropertyTagTransferRange: u32 = 342u32;
pub const PropertyTagTypeASCII: u32 = 2u32;
pub const PropertyTagTypeByte: u32 = 1u32;
pub const PropertyTagTypeLong: u32 = 4u32;
pub const PropertyTagTypeRational: u32 = 5u32;
pub const PropertyTagTypeSLONG: u32 = 9u32;
pub const PropertyTagTypeSRational: u32 = 10u32;
pub const PropertyTagTypeShort: u32 = 3u32;
pub const PropertyTagTypeUndefined: u32 = 7u32;
pub const PropertyTagWhitePoint: u32 = 318u32;
pub const PropertyTagXPosition: u32 = 286u32;
pub const PropertyTagXResolution: u32 = 282u32;
pub const PropertyTagYCbCrCoefficients: u32 = 529u32;
pub const PropertyTagYCbCrPositioning: u32 = 531u32;
pub const PropertyTagYCbCrSubsampling: u32 = 530u32;
pub const PropertyTagYPosition: u32 = 287u32;
pub const PropertyTagYResolution: u32 = 283u32;
pub const QualityModeDefault: QualityMode = 0i32;
pub const QualityModeHigh: QualityMode = 2i32;
pub const QualityModeInvalid: QualityMode = -1i32;
pub const QualityModeLow: QualityMode = 1i32;
pub const RED_SHIFT: u32 = 16u32;
pub const RedEyeCorrectionEffectGuid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x74d29d05_69a4_4266_9549_3cc52836b632);
pub const Rotate180FlipNone: RotateFlipType = 2i32;
pub const Rotate180FlipX: RotateFlipType = 6i32;
pub const Rotate180FlipXY: RotateFlipType = 0i32;
pub const Rotate180FlipY: RotateFlipType = 4i32;
pub const Rotate270FlipNone: RotateFlipType = 3i32;
pub const Rotate270FlipX: RotateFlipType = 7i32;
pub const Rotate270FlipXY: RotateFlipType = 1i32;
pub const Rotate270FlipY: RotateFlipType = 5i32;
pub const Rotate90FlipNone: RotateFlipType = 1i32;
pub const Rotate90FlipX: RotateFlipType = 5i32;
pub const Rotate90FlipXY: RotateFlipType = 3i32;
pub const Rotate90FlipY: RotateFlipType = 7i32;
pub const RotateNoneFlipNone: RotateFlipType = 0i32;
pub const RotateNoneFlipX: RotateFlipType = 4i32;
pub const RotateNoneFlipXY: RotateFlipType = 2i32;
pub const RotateNoneFlipY: RotateFlipType = 6i32;
pub const SharpenEffectGuid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x63cbf3ee_c526_402c_8f71_62c540bf5142);
pub const SmoothingModeAntiAlias: SmoothingMode = 4i32;
pub const SmoothingModeAntiAlias8x4: SmoothingMode = 4i32;
pub const SmoothingModeAntiAlias8x8: SmoothingMode = 5i32;
pub const SmoothingModeDefault: SmoothingMode = 0i32;
pub const SmoothingModeHighQuality: SmoothingMode = 2i32;
pub const SmoothingModeHighSpeed: SmoothingMode = 1i32;
pub const SmoothingModeInvalid: SmoothingMode = -1i32;
pub const SmoothingModeNone: SmoothingMode = 3i32;
pub const StringAlignmentCenter: StringAlignment = 1i32;
pub const StringAlignmentFar: StringAlignment = 2i32;
pub const StringAlignmentNear: StringAlignment = 0i32;
pub const StringDigitSubstituteNational: StringDigitSubstitute = 2i32;
pub const StringDigitSubstituteNone: StringDigitSubstitute = 1i32;
pub const StringDigitSubstituteTraditional: StringDigitSubstitute = 3i32;
pub const StringDigitSubstituteUser: StringDigitSubstitute = 0i32;
pub const StringFormatFlagsBypassGDI: StringFormatFlags = -2147483648i32;
pub const StringFormatFlagsDirectionRightToLeft: StringFormatFlags = 1i32;
pub const StringFormatFlagsDirectionVertical: StringFormatFlags = 2i32;
pub const StringFormatFlagsDisplayFormatControl: StringFormatFlags = 32i32;
pub const StringFormatFlagsLineLimit: StringFormatFlags = 8192i32;
pub const StringFormatFlagsMeasureTrailingSpaces: StringFormatFlags = 2048i32;
pub const StringFormatFlagsNoClip: StringFormatFlags = 16384i32;
pub const StringFormatFlagsNoFitBlackBox: StringFormatFlags = 4i32;
pub const StringFormatFlagsNoFontFallback: StringFormatFlags = 1024i32;
pub const StringFormatFlagsNoWrap: StringFormatFlags = 4096i32;
pub const StringTrimmingCharacter: StringTrimming = 1i32;
pub const StringTrimmingEllipsisCharacter: StringTrimming = 3i32;
pub const StringTrimmingEllipsisPath: StringTrimming = 5i32;
pub const StringTrimmingEllipsisWord: StringTrimming = 4i32;
pub const StringTrimmingNone: StringTrimming = 0i32;
pub const StringTrimmingWord: StringTrimming = 2i32;
pub const TestControlForceBilinear: GpTestControlEnum = 0i32;
pub const TestControlGetBuildNumber: GpTestControlEnum = 2i32;
pub const TestControlNoICM: GpTestControlEnum = 1i32;
pub const TextRenderingHintAntiAlias: TextRenderingHint = 4i32;
pub const TextRenderingHintAntiAliasGridFit: TextRenderingHint = 3i32;
pub const TextRenderingHintClearTypeGridFit: TextRenderingHint = 5i32;
pub const TextRenderingHintSingleBitPerPixel: TextRenderingHint = 2i32;
pub const TextRenderingHintSingleBitPerPixelGridFit: TextRenderingHint = 1i32;
pub const TextRenderingHintSystemDefault: TextRenderingHint = 0i32;
pub const TintEffectGuid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1077af00_2848_4441_9489_44ad4c2d7a2c);
pub const UnitDisplay: Unit = 1i32;
pub const UnitDocument: Unit = 5i32;
pub const UnitInch: Unit = 4i32;
pub const UnitMillimeter: Unit = 6i32;
pub const UnitPixel: Unit = 2i32;
pub const UnitPoint: Unit = 3i32;
pub const UnitWorld: Unit = 0i32;
pub const UnknownImageFormat: Status = 13i32;
pub const UnsupportedGdiplusVersion: Status = 17i32;
pub const ValueOverflow: Status = 11i32;
pub const WarpModeBilinear: WarpMode = 1i32;
pub const WarpModePerspective: WarpMode = 0i32;
pub const Win32Error: Status = 7i32;
pub const WmfRecordTypeAbortDoc: EmfPlusRecordType = 65618i32;
pub const WmfRecordTypeAnimatePalette: EmfPlusRecordType = 66614i32;
pub const WmfRecordTypeArc: EmfPlusRecordType = 67607i32;
pub const WmfRecordTypeBitBlt: EmfPlusRecordType = 67874i32;
pub const WmfRecordTypeChord: EmfPlusRecordType = 67632i32;
pub const WmfRecordTypeCreateBitmap: EmfPlusRecordType = 67326i32;
pub const WmfRecordTypeCreateBitmapIndirect: EmfPlusRecordType = 66301i32;
pub const WmfRecordTypeCreateBrush: EmfPlusRecordType = 65784i32;
pub const WmfRecordTypeCreateBrushIndirect: EmfPlusRecordType = 66300i32;
pub const WmfRecordTypeCreateFontIndirect: EmfPlusRecordType = 66299i32;
pub const WmfRecordTypeCreatePalette: EmfPlusRecordType = 65783i32;
pub const WmfRecordTypeCreatePatternBrush: EmfPlusRecordType = 66041i32;
pub const WmfRecordTypeCreatePenIndirect: EmfPlusRecordType = 66298i32;
pub const WmfRecordTypeCreateRegion: EmfPlusRecordType = 67327i32;
pub const WmfRecordTypeDIBBitBlt: EmfPlusRecordType = 67904i32;
pub const WmfRecordTypeDIBCreatePatternBrush: EmfPlusRecordType = 65858i32;
pub const WmfRecordTypeDIBStretchBlt: EmfPlusRecordType = 68417i32;
pub const WmfRecordTypeDeleteObject: EmfPlusRecordType = 66032i32;
pub const WmfRecordTypeDrawText: EmfPlusRecordType = 67119i32;
pub const WmfRecordTypeEllipse: EmfPlusRecordType = 66584i32;
pub const WmfRecordTypeEndDoc: EmfPlusRecordType = 65630i32;
pub const WmfRecordTypeEndPage: EmfPlusRecordType = 65616i32;
pub const WmfRecordTypeEscape: EmfPlusRecordType = 67110i32;
pub const WmfRecordTypeExcludeClipRect: EmfPlusRecordType = 66581i32;
pub const WmfRecordTypeExtFloodFill: EmfPlusRecordType = 66888i32;
pub const WmfRecordTypeExtTextOut: EmfPlusRecordType = 68146i32;
pub const WmfRecordTypeFillRegion: EmfPlusRecordType = 66088i32;
pub const WmfRecordTypeFloodFill: EmfPlusRecordType = 66585i32;
pub const WmfRecordTypeFrameRegion: EmfPlusRecordType = 66601i32;
pub const WmfRecordTypeIntersectClipRect: EmfPlusRecordType = 66582i32;
pub const WmfRecordTypeInvertRegion: EmfPlusRecordType = 65834i32;
pub const WmfRecordTypeLineTo: EmfPlusRecordType = 66067i32;
pub const WmfRecordTypeMoveTo: EmfPlusRecordType = 66068i32;
pub const WmfRecordTypeOffsetClipRgn: EmfPlusRecordType = 66080i32;
pub const WmfRecordTypeOffsetViewportOrg: EmfPlusRecordType = 66065i32;
pub const WmfRecordTypeOffsetWindowOrg: EmfPlusRecordType = 66063i32;
pub const WmfRecordTypePaintRegion: EmfPlusRecordType = 65835i32;
pub const WmfRecordTypePatBlt: EmfPlusRecordType = 67101i32;
pub const WmfRecordTypePie: EmfPlusRecordType = 67610i32;
pub const WmfRecordTypePolyPolygon: EmfPlusRecordType = 66872i32;
pub const WmfRecordTypePolygon: EmfPlusRecordType = 66340i32;
pub const WmfRecordTypePolyline: EmfPlusRecordType = 66341i32;
pub const WmfRecordTypeRealizePalette: EmfPlusRecordType = 65589i32;
pub const WmfRecordTypeRectangle: EmfPlusRecordType = 66587i32;
pub const WmfRecordTypeResetDC: EmfPlusRecordType = 65868i32;
pub const WmfRecordTypeResizePalette: EmfPlusRecordType = 65849i32;
pub const WmfRecordTypeRestoreDC: EmfPlusRecordType = 65831i32;
pub const WmfRecordTypeRoundRect: EmfPlusRecordType = 67100i32;
pub const WmfRecordTypeSaveDC: EmfPlusRecordType = 65566i32;
pub const WmfRecordTypeScaleViewportExt: EmfPlusRecordType = 66578i32;
pub const WmfRecordTypeScaleWindowExt: EmfPlusRecordType = 66576i32;
pub const WmfRecordTypeSelectClipRegion: EmfPlusRecordType = 65836i32;
pub const WmfRecordTypeSelectObject: EmfPlusRecordType = 65837i32;
pub const WmfRecordTypeSelectPalette: EmfPlusRecordType = 66100i32;
pub const WmfRecordTypeSetBkColor: EmfPlusRecordType = 66049i32;
pub const WmfRecordTypeSetBkMode: EmfPlusRecordType = 65794i32;
pub const WmfRecordTypeSetDIBToDev: EmfPlusRecordType = 68915i32;
pub const WmfRecordTypeSetLayout: EmfPlusRecordType = 65865i32;
pub const WmfRecordTypeSetMapMode: EmfPlusRecordType = 65795i32;
pub const WmfRecordTypeSetMapperFlags: EmfPlusRecordType = 66097i32;
pub const WmfRecordTypeSetPalEntries: EmfPlusRecordType = 65591i32;
pub const WmfRecordTypeSetPixel: EmfPlusRecordType = 66591i32;
pub const WmfRecordTypeSetPolyFillMode: EmfPlusRecordType = 65798i32;
pub const WmfRecordTypeSetROP2: EmfPlusRecordType = 65796i32;
pub const WmfRecordTypeSetRelAbs: EmfPlusRecordType = 65797i32;
pub const WmfRecordTypeSetStretchBltMode: EmfPlusRecordType = 65799i32;
pub const WmfRecordTypeSetTextAlign: EmfPlusRecordType = 65838i32;
pub const WmfRecordTypeSetTextCharExtra: EmfPlusRecordType = 65800i32;
pub const WmfRecordTypeSetTextColor: EmfPlusRecordType = 66057i32;
pub const WmfRecordTypeSetTextJustification: EmfPlusRecordType = 66058i32;
pub const WmfRecordTypeSetViewportExt: EmfPlusRecordType = 66062i32;
pub const WmfRecordTypeSetViewportOrg: EmfPlusRecordType = 66061i32;
pub const WmfRecordTypeSetWindowExt: EmfPlusRecordType = 66060i32;
pub const WmfRecordTypeSetWindowOrg: EmfPlusRecordType = 66059i32;
pub const WmfRecordTypeStartDoc: EmfPlusRecordType = 65869i32;
pub const WmfRecordTypeStartPage: EmfPlusRecordType = 65615i32;
pub const WmfRecordTypeStretchBlt: EmfPlusRecordType = 68387i32;
pub const WmfRecordTypeStretchDIB: EmfPlusRecordType = 69443i32;
pub const WmfRecordTypeTextOut: EmfPlusRecordType = 66849i32;
pub const WrapModeClamp: WrapMode = 4i32;
pub const WrapModeTile: WrapMode = 0i32;
pub const WrapModeTileFlipX: WrapMode = 1i32;
pub const WrapModeTileFlipXY: WrapMode = 3i32;
pub const WrapModeTileFlipY: WrapMode = 2i32;
pub const WrongState: Status = 8i32;
pub type BrushType = i32;
pub type ColorAdjustType = i32;
pub type ColorChannelFlags = i32;
pub type ColorMatrixFlags = i32;
pub type ColorMode = i32;
pub type CombineMode = i32;
pub type CompositingMode = i32;
pub type CompositingQuality = i32;
pub type ConvertToEmfPlusFlags = i32;
pub type CoordinateSpace = i32;
pub type CurveAdjustments = i32;
pub type CurveChannel = i32;
pub type CustomLineCapType = i32;
pub type DashCap = i32;
pub type DashStyle = i32;
pub type DebugEventLevel = i32;
pub type DitherType = i32;
pub type DriverStringOptions = i32;
pub type EmfPlusRecordType = i32;
pub type EmfToWmfBitsFlags = i32;
pub type EmfType = i32;
pub type EncoderParameterValueType = i32;
pub type EncoderValue = i32;
pub type FillMode = i32;
pub type FlushIntention = i32;
pub type FontStyle = i32;
pub type GdiplusStartupParams = i32;
pub type GenericFontFamily = i32;
pub type GpTestControlEnum = i32;
pub type HatchStyle = i32;
pub type HistogramFormat = i32;
pub type HotkeyPrefix = i32;
pub type ImageCodecFlags = i32;
pub type ImageFlags = i32;
pub type ImageLockMode = i32;
pub type ImageType = i32;
pub type InterpolationMode = i32;
pub type ItemDataPosition = i32;
pub type LineCap = i32;
pub type LineJoin = i32;
pub type LinearGradientMode = i32;
pub type MatrixOrder = i32;
pub type MetafileFrameUnit = i32;
pub type MetafileType = i32;
pub type ObjectType = i32;
pub type PaletteFlags = i32;
pub type PaletteType = i32;
pub type PathPointType = i32;
pub type PenAlignment = i32;
pub type PenType = i32;
pub type PixelOffsetMode = i32;
pub type QualityMode = i32;
pub type RotateFlipType = i32;
pub type SmoothingMode = i32;
pub type Status = i32;
pub type StringAlignment = i32;
pub type StringDigitSubstitute = i32;
pub type StringFormatFlags = i32;
pub type StringTrimming = i32;
pub type TextRenderingHint = i32;
pub type Unit = i32;
pub type WarpMode = i32;
pub type WrapMode = i32;
pub type Bitmap = isize;
#[repr(C)]
pub struct BitmapData {
    pub Width: u32,
    pub Height: u32,
    pub Stride: i32,
    pub PixelFormat: i32,
    pub Scan0: *mut ::core::ffi::c_void,
    pub Reserved: usize,
}
impl ::core::marker::Copy for BitmapData {}
impl ::core::clone::Clone for BitmapData {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct Blur {
    pub Base: Effect,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for Blur {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for Blur {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BlurParams {
    pub radius: f32,
    pub expandEdge: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BlurParams {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BlurParams {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BrightnessContrast {
    pub Base: Effect,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BrightnessContrast {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BrightnessContrast {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BrightnessContrastParams {
    pub brightnessLevel: i32,
    pub contrastLevel: i32,
}
impl ::core::marker::Copy for BrightnessContrastParams {}
impl ::core::clone::Clone for BrightnessContrastParams {
    fn clone(&self) -> Self {
        *self
    }
}
pub type CGpEffect = isize;
pub type CachedBitmap = isize;
#[repr(C)]
pub struct CharacterRange {
    pub First: i32,
    pub Length: i32,
}
impl ::core::marker::Copy for CharacterRange {}
impl ::core::clone::Clone for CharacterRange {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct Color {
    pub Argb: u32,
}
impl Color {
    pub const AliceBlue: i32 = -984833i32;
    pub const AntiqueWhite: i32 = -332841i32;
    pub const Aqua: i32 = -16711681i32;
    pub const Aquamarine: i32 = -8388652i32;
    pub const Azure: i32 = -983041i32;
    pub const Beige: i32 = -657956i32;
    pub const Bisque: i32 = -6972i32;
    pub const Black: i32 = -16777216i32;
    pub const BlanchedAlmond: i32 = -5171i32;
    pub const Blue: i32 = -16776961i32;
    pub const BlueViolet: i32 = -7722014i32;
    pub const Brown: i32 = -5952982i32;
    pub const BurlyWood: i32 = -2180985i32;
    pub const CadetBlue: i32 = -10510688i32;
    pub const Chartreuse: i32 = -8388864i32;
    pub const Chocolate: i32 = -2987746i32;
    pub const Coral: i32 = -32944i32;
    pub const CornflowerBlue: i32 = -10185235i32;
    pub const Cornsilk: i32 = -1828i32;
    pub const Crimson: i32 = -2354116i32;
    pub const Cyan: i32 = -16711681i32;
    pub const DarkBlue: i32 = -16777077i32;
    pub const DarkCyan: i32 = -16741493i32;
    pub const DarkGoldenrod: i32 = -4684277i32;
    pub const DarkGray: i32 = -5658199i32;
    pub const DarkGreen: i32 = -16751616i32;
    pub const DarkKhaki: i32 = -4343957i32;
    pub const DarkMagenta: i32 = -7667573i32;
    pub const DarkOliveGreen: i32 = -11179217i32;
    pub const DarkOrange: i32 = -29696i32;
    pub const DarkOrchid: i32 = -6737204i32;
    pub const DarkRed: i32 = -7667712i32;
    pub const DarkSalmon: i32 = -1468806i32;
    pub const DarkSeaGreen: i32 = -7357301i32;
    pub const DarkSlateBlue: i32 = -12042869i32;
    pub const DarkSlateGray: i32 = -13676721i32;
    pub const DarkTurquoise: i32 = -16724271i32;
    pub const DarkViolet: i32 = -7077677i32;
    pub const DeepPink: i32 = -60269i32;
    pub const DeepSkyBlue: i32 = -16728065i32;
    pub const DimGray: i32 = -9868951i32;
    pub const DodgerBlue: i32 = -14774017i32;
    pub const Firebrick: i32 = -5103070i32;
    pub const FloralWhite: i32 = -1296i32;
    pub const ForestGreen: i32 = -14513374i32;
    pub const Fuchsia: i32 = -65281i32;
    pub const Gainsboro: i32 = -2302756i32;
    pub const GhostWhite: i32 = -460545i32;
    pub const Gold: i32 = -10496i32;
    pub const Goldenrod: i32 = -2448096i32;
    pub const Gray: i32 = -8355712i32;
    pub const Green: i32 = -16744448i32;
    pub const GreenYellow: i32 = -5374161i32;
    pub const Honeydew: i32 = -983056i32;
    pub const HotPink: i32 = -38476i32;
    pub const IndianRed: i32 = -3318692i32;
    pub const Indigo: i32 = -11861886i32;
    pub const Ivory: i32 = -16i32;
    pub const Khaki: i32 = -989556i32;
    pub const Lavender: i32 = -1644806i32;
    pub const LavenderBlush: i32 = -3851i32;
    pub const LawnGreen: i32 = -8586240i32;
    pub const LemonChiffon: i32 = -1331i32;
    pub const LightBlue: i32 = -5383962i32;
    pub const LightCoral: i32 = -1015680i32;
    pub const LightCyan: i32 = -2031617i32;
    pub const LightGoldenrodYellow: i32 = -329006i32;
    pub const LightGray: i32 = -2894893i32;
    pub const LightGreen: i32 = -7278960i32;
    pub const LightPink: i32 = -18751i32;
    pub const LightSalmon: i32 = -24454i32;
    pub const LightSeaGreen: i32 = -14634326i32;
    pub const LightSkyBlue: i32 = -7876870i32;
    pub const LightSlateGray: i32 = -8943463i32;
    pub const LightSteelBlue: i32 = -5192482i32;
    pub const LightYellow: i32 = -32i32;
    pub const Lime: i32 = -16711936i32;
    pub const LimeGreen: i32 = -13447886i32;
    pub const Linen: i32 = -331546i32;
    pub const Magenta: i32 = -65281i32;
    pub const Maroon: i32 = -8388608i32;
    pub const MediumAquamarine: i32 = -10039894i32;
    pub const MediumBlue: i32 = -16777011i32;
    pub const MediumOrchid: i32 = -4565549i32;
    pub const MediumPurple: i32 = -7114533i32;
    pub const MediumSeaGreen: i32 = -12799119i32;
    pub const MediumSlateBlue: i32 = -8689426i32;
    pub const MediumSpringGreen: i32 = -16713062i32;
    pub const MediumTurquoise: i32 = -12004916i32;
    pub const MediumVioletRed: i32 = -3730043i32;
    pub const MidnightBlue: i32 = -15132304i32;
    pub const MintCream: i32 = -655366i32;
    pub const MistyRose: i32 = -6943i32;
    pub const Moccasin: i32 = -6987i32;
    pub const NavajoWhite: i32 = -8531i32;
    pub const Navy: i32 = -16777088i32;
    pub const OldLace: i32 = -133658i32;
    pub const Olive: i32 = -8355840i32;
    pub const OliveDrab: i32 = -9728477i32;
    pub const Orange: i32 = -23296i32;
    pub const OrangeRed: i32 = -47872i32;
    pub const Orchid: i32 = -2461482i32;
    pub const PaleGoldenrod: i32 = -1120086i32;
    pub const PaleGreen: i32 = -6751336i32;
    pub const PaleTurquoise: i32 = -5247250i32;
    pub const PaleVioletRed: i32 = -2396013i32;
    pub const PapayaWhip: i32 = -4139i32;
    pub const PeachPuff: i32 = -9543i32;
    pub const Peru: i32 = -3308225i32;
    pub const Pink: i32 = -16181i32;
    pub const Plum: i32 = -2252579i32;
    pub const PowderBlue: i32 = -5185306i32;
    pub const Purple: i32 = -8388480i32;
    pub const Red: i32 = -65536i32;
    pub const RosyBrown: i32 = -4419697i32;
    pub const RoyalBlue: i32 = -12490271i32;
    pub const SaddleBrown: i32 = -7650029i32;
    pub const Salmon: i32 = -360334i32;
    pub const SandyBrown: i32 = -744352i32;
    pub const SeaGreen: i32 = -13726889i32;
    pub const SeaShell: i32 = -2578i32;
    pub const Sienna: i32 = -6270419i32;
    pub const Silver: i32 = -4144960i32;
    pub const SkyBlue: i32 = -7876885i32;
    pub const SlateBlue: i32 = -9807155i32;
    pub const SlateGray: i32 = -9404272i32;
    pub const Snow: i32 = -1286i32;
    pub const SpringGreen: i32 = -16711809i32;
    pub const SteelBlue: i32 = -12156236i32;
    pub const Tan: i32 = -2968436i32;
    pub const Teal: i32 = -16744320i32;
    pub const Thistle: i32 = -2572328i32;
    pub const Tomato: i32 = -40121i32;
    pub const Transparent: i32 = 16777215i32;
    pub const Turquoise: i32 = -12525360i32;
    pub const Violet: i32 = -1146130i32;
    pub const Wheat: i32 = -663885i32;
    pub const White: i32 = -1i32;
    pub const WhiteSmoke: i32 = -657931i32;
    pub const Yellow: i32 = -256i32;
    pub const YellowGreen: i32 = -6632142i32;
    pub const AlphaShift: i32 = 24i32;
    pub const RedShift: i32 = 16i32;
    pub const GreenShift: i32 = 8i32;
    pub const BlueShift: i32 = 0i32;
    pub const AlphaMask: i32 = -16777216i32;
    pub const RedMask: i32 = 16711680i32;
    pub const GreenMask: i32 = 65280i32;
    pub const BlueMask: i32 = 255i32;
}
impl ::core::marker::Copy for Color {}
impl ::core::clone::Clone for Color {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ColorBalance {
    pub Base: Effect,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ColorBalance {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ColorBalance {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ColorBalanceParams {
    pub cyanRed: i32,
    pub magentaGreen: i32,
    pub yellowBlue: i32,
}
impl ::core::marker::Copy for ColorBalanceParams {}
impl ::core::clone::Clone for ColorBalanceParams {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ColorCurve {
    pub Base: Effect,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ColorCurve {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ColorCurve {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ColorCurveParams {
    pub adjustment: CurveAdjustments,
    pub channel: CurveChannel,
    pub adjustValue: i32,
}
impl ::core::marker::Copy for ColorCurveParams {}
impl ::core::clone::Clone for ColorCurveParams {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ColorLUT {
    pub Base: Effect,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ColorLUT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ColorLUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ColorLUTParams {
    pub lutB: [u8; 256],
    pub lutG: [u8; 256],
    pub lutR: [u8; 256],
    pub lutA: [u8; 256],
}
impl ::core::marker::Copy for ColorLUTParams {}
impl ::core::clone::Clone for ColorLUTParams {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ColorMap {
    pub oldColor: Color,
    pub newColor: Color,
}
impl ::core::marker::Copy for ColorMap {}
impl ::core::clone::Clone for ColorMap {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ColorMatrix {
    pub m: [f32; 25],
}
impl ::core::marker::Copy for ColorMatrix {}
impl ::core::clone::Clone for ColorMatrix {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ColorMatrixEffect {
    pub Base: Effect,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ColorMatrixEffect {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ColorMatrixEffect {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ColorPalette {
    pub Flags: u32,
    pub Count: u32,
    pub Entries: [u32; 1],
}
impl ::core::marker::Copy for ColorPalette {}
impl ::core::clone::Clone for ColorPalette {
    fn clone(&self) -> Self {
        *self
    }
}
pub type CustomLineCap = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ENHMETAHEADER3 {
    pub iType: u32,
    pub nSize: u32,
    pub rclBounds: super::super::Foundation::RECTL,
    pub rclFrame: super::super::Foundation::RECTL,
    pub dSignature: u32,
    pub nVersion: u32,
    pub nBytes: u32,
    pub nRecords: u32,
    pub nHandles: u16,
    pub sReserved: u16,
    pub nDescription: u32,
    pub offDescription: u32,
    pub nPalEntries: u32,
    pub szlDevice: super::super::Foundation::SIZE,
    pub szlMillimeters: super::super::Foundation::SIZE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ENHMETAHEADER3 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ENHMETAHEADER3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct Effect {
    pub lpVtbl: *mut *mut ::core::ffi::c_void,
    pub nativeEffect: *mut CGpEffect,
    pub auxDataSize: i32,
    pub auxData: *mut ::core::ffi::c_void,
    pub useAuxData: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for Effect {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for Effect {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EncoderParameter {
    pub Guid: ::windows_sys::core::GUID,
    pub NumberOfValues: u32,
    pub Type: u32,
    pub Value: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for EncoderParameter {}
impl ::core::clone::Clone for EncoderParameter {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EncoderParameters {
    pub Count: u32,
    pub Parameter: [EncoderParameter; 1],
}
impl ::core::marker::Copy for EncoderParameters {}
impl ::core::clone::Clone for EncoderParameters {
    fn clone(&self) -> Self {
        *self
    }
}
pub type Font = isize;
pub type FontCollection = isize;
pub type FontFamily = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct GdiplusStartupInput {
    pub GdiplusVersion: u32,
    pub DebugEventCallback: isize,
    pub SuppressBackgroundThread: super::super::Foundation::BOOL,
    pub SuppressExternalCodecs: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for GdiplusStartupInput {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for GdiplusStartupInput {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct GdiplusStartupInputEx {
    pub Base: GdiplusStartupInput,
    pub StartupParameters: i32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for GdiplusStartupInputEx {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for GdiplusStartupInputEx {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GdiplusStartupOutput {
    pub NotificationHook: isize,
    pub NotificationUnhook: isize,
}
impl ::core::marker::Copy for GdiplusStartupOutput {}
impl ::core::clone::Clone for GdiplusStartupOutput {
    fn clone(&self) -> Self {
        *self
    }
}
pub type GpAdjustableArrowCap = isize;
pub type GpBitmap = isize;
pub type GpBrush = isize;
pub type GpCachedBitmap = isize;
pub type GpCustomLineCap = isize;
pub type GpFont = isize;
pub type GpFontCollection = isize;
pub type GpFontFamily = isize;
pub type GpGraphics = isize;
pub type GpHatch = isize;
pub type GpImage = isize;
pub type GpImageAttributes = isize;
pub type GpInstalledFontCollection = isize;
pub type GpLineGradient = isize;
pub type GpMetafile = isize;
pub type GpPath = isize;
pub type GpPathGradient = isize;
pub type GpPathIterator = isize;
pub type GpPen = isize;
pub type GpPrivateFontCollection = isize;
pub type GpRegion = isize;
pub type GpSolidFill = isize;
pub type GpStringFormat = isize;
pub type GpTexture = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct HueSaturationLightness {
    pub Base: Effect,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for HueSaturationLightness {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for HueSaturationLightness {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct HueSaturationLightnessParams {
    pub hueLevel: i32,
    pub saturationLevel: i32,
    pub lightnessLevel: i32,
}
impl ::core::marker::Copy for HueSaturationLightnessParams {}
impl ::core::clone::Clone for HueSaturationLightnessParams {
    fn clone(&self) -> Self {
        *self
    }
}
pub type Image = isize;
#[repr(C)]
pub struct ImageCodecInfo {
    pub Clsid: ::windows_sys::core::GUID,
    pub FormatID: ::windows_sys::core::GUID,
    pub CodecName: ::windows_sys::core::PCWSTR,
    pub DllName: ::windows_sys::core::PCWSTR,
    pub FormatDescription: ::windows_sys::core::PCWSTR,
    pub FilenameExtension: ::windows_sys::core::PCWSTR,
    pub MimeType: ::windows_sys::core::PCWSTR,
    pub Flags: u32,
    pub Version: u32,
    pub SigCount: u32,
    pub SigSize: u32,
    pub SigPattern: *const u8,
    pub SigMask: *const u8,
}
impl ::core::marker::Copy for ImageCodecInfo {}
impl ::core::clone::Clone for ImageCodecInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ImageItemData {
    pub Size: u32,
    pub Position: u32,
    pub Desc: *mut ::core::ffi::c_void,
    pub DescSize: u32,
    pub Data: *mut ::core::ffi::c_void,
    pub DataSize: u32,
    pub Cookie: u32,
}
impl ::core::marker::Copy for ImageItemData {}
impl ::core::clone::Clone for ImageItemData {
    fn clone(&self) -> Self {
        *self
    }
}
pub type InstalledFontCollection = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct Levels {
    pub Base: Effect,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for Levels {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for Levels {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LevelsParams {
    pub highlight: i32,
    pub midtone: i32,
    pub shadow: i32,
}
impl ::core::marker::Copy for LevelsParams {}
impl ::core::clone::Clone for LevelsParams {
    fn clone(&self) -> Self {
        *self
    }
}
pub type Matrix = isize;
pub type Metafile = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub struct MetafileHeader {
    pub Type: MetafileType,
    pub Size: u32,
    pub Version: u32,
    pub EmfPlusFlags: u32,
    pub DpiX: f32,
    pub DpiY: f32,
    pub X: i32,
    pub Y: i32,
    pub Width: i32,
    pub Height: i32,
    pub Anonymous: MetafileHeader_0,
    pub EmfPlusHeaderSize: i32,
    pub LogicalDpiX: i32,
    pub LogicalDpiY: i32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for MetafileHeader {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for MetafileHeader {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub union MetafileHeader_0 {
    pub WmfHeader: super::Gdi::METAHEADER,
    pub EmfHeader: ENHMETAHEADER3,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for MetafileHeader_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for MetafileHeader_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PWMFRect16 {
    pub Left: i16,
    pub Top: i16,
    pub Right: i16,
    pub Bottom: i16,
}
impl ::core::marker::Copy for PWMFRect16 {}
impl ::core::clone::Clone for PWMFRect16 {
    fn clone(&self) -> Self {
        *self
    }
}
pub type PathData = isize;
#[repr(C)]
pub struct Point {
    pub X: i32,
    pub Y: i32,
}
impl ::core::marker::Copy for Point {}
impl ::core::clone::Clone for Point {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PointF {
    pub X: f32,
    pub Y: f32,
}
impl ::core::marker::Copy for PointF {}
impl ::core::clone::Clone for PointF {
    fn clone(&self) -> Self {
        *self
    }
}
pub type PrivateFontCollection = isize;
#[repr(C)]
pub struct PropertyItem {
    pub id: u32,
    pub length: u32,
    pub r#type: u16,
    pub value: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for PropertyItem {}
impl ::core::clone::Clone for PropertyItem {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct Rect {
    pub X: i32,
    pub Y: i32,
    pub Width: i32,
    pub Height: i32,
}
impl ::core::marker::Copy for Rect {}
impl ::core::clone::Clone for Rect {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RectF {
    pub X: f32,
    pub Y: f32,
    pub Width: f32,
    pub Height: f32,
}
impl ::core::marker::Copy for RectF {}
impl ::core::clone::Clone for RectF {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RedEyeCorrection {
    pub Base: Effect,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RedEyeCorrection {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RedEyeCorrection {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RedEyeCorrectionParams {
    pub numberOfAreas: u32,
    pub areas: *mut super::super::Foundation::RECT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RedEyeCorrectionParams {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RedEyeCorrectionParams {
    fn clone(&self) -> Self {
        *self
    }
}
pub type Region = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct Sharpen {
    pub Base: Effect,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for Sharpen {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for Sharpen {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SharpenParams {
    pub radius: f32,
    pub amount: f32,
}
impl ::core::marker::Copy for SharpenParams {}
impl ::core::clone::Clone for SharpenParams {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct Size {
    pub Width: i32,
    pub Height: i32,
}
impl ::core::marker::Copy for Size {}
impl ::core::clone::Clone for Size {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SizeF {
    pub Width: f32,
    pub Height: f32,
}
impl ::core::marker::Copy for SizeF {}
impl ::core::clone::Clone for SizeF {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct Tint {
    pub Base: Effect,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for Tint {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for Tint {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TintParams {
    pub hue: i32,
    pub amount: i32,
}
impl ::core::marker::Copy for TintParams {}
impl ::core::clone::Clone for TintParams {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(2))]
pub struct WmfPlaceableFileHeader {
    pub Key: u32,
    pub Hmf: i16,
    pub BoundingBox: PWMFRect16,
    pub Inch: i16,
    pub Reserved: u32,
    pub Checksum: i16,
}
impl ::core::marker::Copy for WmfPlaceableFileHeader {}
impl ::core::clone::Clone for WmfPlaceableFileHeader {
    fn clone(&self) -> Self {
        *self
    }
}
pub type DebugEventProc = ::core::option::Option<unsafe extern "system" fn(level: DebugEventLevel, message: ::windows_sys::core::PCSTR) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DrawImageAbort = ::core::option::Option<unsafe extern "system" fn() -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type EnumerateMetafileProc = ::core::option::Option<unsafe extern "system" fn(param0: EmfPlusRecordType, param1: u32, param2: u32, param3: *const u8, param4: *mut ::core::ffi::c_void) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type GetThumbnailImageAbort = ::core::option::Option<unsafe extern "system" fn() -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type ImageAbort = ::core::option::Option<unsafe extern "system" fn(param0: *mut ::core::ffi::c_void) -> super::super::Foundation::BOOL>;
pub type NotificationHookProc = ::core::option::Option<unsafe extern "system" fn(token: *mut usize) -> Status>;
pub type NotificationUnhookProc = ::core::option::Option<unsafe extern "system" fn(token: usize) -> ()>;
