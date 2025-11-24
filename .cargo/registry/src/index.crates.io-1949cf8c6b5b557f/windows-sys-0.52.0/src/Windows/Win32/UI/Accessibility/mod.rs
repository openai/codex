#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("oleacc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AccNotifyTouchInteraction(hwndapp : super::super::Foundation:: HWND, hwndtarget : super::super::Foundation:: HWND, pttarget : super::super::Foundation:: POINT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("oleacc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AccSetRunningUtilityState(hwndapp : super::super::Foundation:: HWND, dwutilitystatemask : u32, dwutilitystate : ACC_UTILITY_STATE_FLAGS) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("oleacc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn AccessibleChildren(pacccontainer : IAccessible, ichildstart : i32, cchildren : i32, rgvarchildren : *mut super::super::System::Variant:: VARIANT, pcobtained : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("oleacc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn AccessibleObjectFromEvent(hwnd : super::super::Foundation:: HWND, dwid : u32, dwchildid : u32, ppacc : *mut IAccessible, pvarchild : *mut super::super::System::Variant:: VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("oleacc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn AccessibleObjectFromPoint(ptscreen : super::super::Foundation:: POINT, ppacc : *mut IAccessible, pvarchild : *mut super::super::System::Variant:: VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("oleacc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AccessibleObjectFromWindow(hwnd : super::super::Foundation:: HWND, dwid : u32, riid : *const ::windows_sys::core::GUID, ppvobject : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("oleacc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateStdAccessibleObject(hwnd : super::super::Foundation:: HWND, idobject : i32, riid : *const ::windows_sys::core::GUID, ppvobject : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("oleacc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateStdAccessibleProxyA(hwnd : super::super::Foundation:: HWND, pclassname : ::windows_sys::core::PCSTR, idobject : i32, riid : *const ::windows_sys::core::GUID, ppvobject : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("oleacc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateStdAccessibleProxyW(hwnd : super::super::Foundation:: HWND, pclassname : ::windows_sys::core::PCWSTR, idobject : i32, riid : *const ::windows_sys::core::GUID, ppvobject : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn DockPattern_SetDockPosition(hobj : HUIAPATTERNOBJECT, dockposition : DockPosition) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn ExpandCollapsePattern_Collapse(hobj : HUIAPATTERNOBJECT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn ExpandCollapsePattern_Expand(hobj : HUIAPATTERNOBJECT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("oleacc.dll" "system" fn GetOleaccVersionInfo(pver : *mut u32, pbuild : *mut u32) -> ());
::windows_targets::link!("oleacc.dll" "system" fn GetRoleTextA(lrole : u32, lpszrole : ::windows_sys::core::PSTR, cchrolemax : u32) -> u32);
::windows_targets::link!("oleacc.dll" "system" fn GetRoleTextW(lrole : u32, lpszrole : ::windows_sys::core::PWSTR, cchrolemax : u32) -> u32);
::windows_targets::link!("oleacc.dll" "system" fn GetStateTextA(lstatebit : u32, lpszstate : ::windows_sys::core::PSTR, cchstate : u32) -> u32);
::windows_targets::link!("oleacc.dll" "system" fn GetStateTextW(lstatebit : u32, lpszstate : ::windows_sys::core::PWSTR, cchstate : u32) -> u32);
::windows_targets::link!("uiautomationcore.dll" "system" fn GridPattern_GetItem(hobj : HUIAPATTERNOBJECT, row : i32, column : i32, presult : *mut HUIANODE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn InvokePattern_Invoke(hobj : HUIAPATTERNOBJECT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsWinEventHookInstalled(event : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn ItemContainerPattern_FindItemByProperty(hobj : HUIAPATTERNOBJECT, hnodestartafter : HUIANODE, propertyid : i32, value : super::super::System::Variant:: VARIANT, pfound : *mut HUIANODE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn LegacyIAccessiblePattern_DoDefaultAction(hobj : HUIAPATTERNOBJECT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn LegacyIAccessiblePattern_GetIAccessible(hobj : HUIAPATTERNOBJECT, paccessible : *mut IAccessible) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn LegacyIAccessiblePattern_Select(hobj : HUIAPATTERNOBJECT, flagsselect : i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn LegacyIAccessiblePattern_SetValue(hobj : HUIAPATTERNOBJECT, szvalue : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("oleacc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LresultFromObject(riid : *const ::windows_sys::core::GUID, wparam : super::super::Foundation:: WPARAM, punk : ::windows_sys::core::IUnknown) -> super::super::Foundation:: LRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn MultipleViewPattern_GetViewName(hobj : HUIAPATTERNOBJECT, viewid : i32, ppstr : *mut ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn MultipleViewPattern_SetCurrentView(hobj : HUIAPATTERNOBJECT, viewid : i32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NotifyWinEvent(event : u32, hwnd : super::super::Foundation:: HWND, idobject : i32, idchild : i32) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("oleacc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ObjectFromLresult(lresult : super::super::Foundation:: LRESULT, riid : *const ::windows_sys::core::GUID, wparam : super::super::Foundation:: WPARAM, ppvobject : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn RangeValuePattern_SetValue(hobj : HUIAPATTERNOBJECT, val : f64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn RegisterPointerInputTarget(hwnd : super::super::Foundation:: HWND, pointertype : super::WindowsAndMessaging:: POINTER_INPUT_TYPE) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn RegisterPointerInputTargetEx(hwnd : super::super::Foundation:: HWND, pointertype : super::WindowsAndMessaging:: POINTER_INPUT_TYPE, fobserve : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
::windows_targets::link!("uiautomationcore.dll" "system" fn ScrollItemPattern_ScrollIntoView(hobj : HUIAPATTERNOBJECT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn ScrollPattern_Scroll(hobj : HUIAPATTERNOBJECT, horizontalamount : ScrollAmount, verticalamount : ScrollAmount) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn ScrollPattern_SetScrollPercent(hobj : HUIAPATTERNOBJECT, horizontalpercent : f64, verticalpercent : f64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn SelectionItemPattern_AddToSelection(hobj : HUIAPATTERNOBJECT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn SelectionItemPattern_RemoveFromSelection(hobj : HUIAPATTERNOBJECT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn SelectionItemPattern_Select(hobj : HUIAPATTERNOBJECT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetWinEventHook(eventmin : u32, eventmax : u32, hmodwineventproc : super::super::Foundation:: HMODULE, pfnwineventproc : WINEVENTPROC, idprocess : u32, idthread : u32, dwflags : u32) -> HWINEVENTHOOK);
::windows_targets::link!("uiautomationcore.dll" "system" fn SynchronizedInputPattern_Cancel(hobj : HUIAPATTERNOBJECT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn SynchronizedInputPattern_StartListening(hobj : HUIAPATTERNOBJECT, inputtype : SynchronizedInputType) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn TextPattern_GetSelection(hobj : HUIAPATTERNOBJECT, pretval : *mut *mut super::super::System::Com:: SAFEARRAY) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn TextPattern_GetVisibleRanges(hobj : HUIAPATTERNOBJECT, pretval : *mut *mut super::super::System::Com:: SAFEARRAY) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextPattern_RangeFromChild(hobj : HUIAPATTERNOBJECT, hnodechild : HUIANODE, pretval : *mut HUIATEXTRANGE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextPattern_RangeFromPoint(hobj : HUIAPATTERNOBJECT, point : UiaPoint, pretval : *mut HUIATEXTRANGE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextPattern_get_DocumentRange(hobj : HUIAPATTERNOBJECT, pretval : *mut HUIATEXTRANGE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextPattern_get_SupportedTextSelection(hobj : HUIAPATTERNOBJECT, pretval : *mut SupportedTextSelection) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextRange_AddToSelection(hobj : HUIATEXTRANGE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextRange_Clone(hobj : HUIATEXTRANGE, pretval : *mut HUIATEXTRANGE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TextRange_Compare(hobj : HUIATEXTRANGE, range : HUIATEXTRANGE, pretval : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextRange_CompareEndpoints(hobj : HUIATEXTRANGE, endpoint : TextPatternRangeEndpoint, targetrange : HUIATEXTRANGE, targetendpoint : TextPatternRangeEndpoint, pretval : *mut i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextRange_ExpandToEnclosingUnit(hobj : HUIATEXTRANGE, unit : TextUnit) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn TextRange_FindAttribute(hobj : HUIATEXTRANGE, attributeid : i32, val : super::super::System::Variant:: VARIANT, backward : super::super::Foundation:: BOOL, pretval : *mut HUIATEXTRANGE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TextRange_FindText(hobj : HUIATEXTRANGE, text : ::windows_sys::core::BSTR, backward : super::super::Foundation:: BOOL, ignorecase : super::super::Foundation:: BOOL, pretval : *mut HUIATEXTRANGE) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn TextRange_GetAttributeValue(hobj : HUIATEXTRANGE, attributeid : i32, pretval : *mut super::super::System::Variant:: VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn TextRange_GetBoundingRectangles(hobj : HUIATEXTRANGE, pretval : *mut *mut super::super::System::Com:: SAFEARRAY) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn TextRange_GetChildren(hobj : HUIATEXTRANGE, pretval : *mut *mut super::super::System::Com:: SAFEARRAY) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextRange_GetEnclosingElement(hobj : HUIATEXTRANGE, pretval : *mut HUIANODE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextRange_GetText(hobj : HUIATEXTRANGE, maxlength : i32, pretval : *mut ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextRange_Move(hobj : HUIATEXTRANGE, unit : TextUnit, count : i32, pretval : *mut i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextRange_MoveEndpointByRange(hobj : HUIATEXTRANGE, endpoint : TextPatternRangeEndpoint, targetrange : HUIATEXTRANGE, targetendpoint : TextPatternRangeEndpoint) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextRange_MoveEndpointByUnit(hobj : HUIATEXTRANGE, endpoint : TextPatternRangeEndpoint, unit : TextUnit, count : i32, pretval : *mut i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextRange_RemoveFromSelection(hobj : HUIATEXTRANGE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TextRange_ScrollIntoView(hobj : HUIATEXTRANGE, aligntotop : super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TextRange_Select(hobj : HUIATEXTRANGE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TogglePattern_Toggle(hobj : HUIAPATTERNOBJECT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TransformPattern_Move(hobj : HUIAPATTERNOBJECT, x : f64, y : f64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TransformPattern_Resize(hobj : HUIAPATTERNOBJECT, width : f64, height : f64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn TransformPattern_Rotate(hobj : HUIAPATTERNOBJECT, degrees : f64) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn UiaAddEvent(hnode : HUIANODE, eventid : i32, pcallback : *mut UiaEventCallback, scope : TreeScope, pproperties : *mut i32, cproperties : i32, prequest : *mut UiaCacheRequest, phevent : *mut HUIAEVENT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UiaClientsAreListening() -> super::super::Foundation:: BOOL);
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaDisconnectAllProviders() -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaDisconnectProvider(pprovider : IRawElementProviderSimple) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UiaEventAddWindow(hevent : HUIAEVENT, hwnd : super::super::Foundation:: HWND) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UiaEventRemoveWindow(hevent : HUIAEVENT, hwnd : super::super::Foundation:: HWND) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn UiaFind(hnode : HUIANODE, pparams : *mut UiaFindParams, prequest : *mut UiaCacheRequest, pprequesteddata : *mut *mut super::super::System::Com:: SAFEARRAY, ppoffsets : *mut *mut super::super::System::Com:: SAFEARRAY, pptreestructures : *mut *mut super::super::System::Com:: SAFEARRAY) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UiaGetErrorDescription(pdescription : *mut ::windows_sys::core::BSTR) -> super::super::Foundation:: BOOL);
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaGetPatternProvider(hnode : HUIANODE, patternid : i32, phobj : *mut HUIAPATTERNOBJECT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn UiaGetPropertyValue(hnode : HUIANODE, propertyid : i32, pvalue : *mut super::super::System::Variant:: VARIANT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaGetReservedMixedAttributeValue(punkmixedattributevalue : *mut ::windows_sys::core::IUnknown) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaGetReservedNotSupportedValue(punknotsupportedvalue : *mut ::windows_sys::core::IUnknown) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaGetRootNode(phnode : *mut HUIANODE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn UiaGetRuntimeId(hnode : HUIANODE, pruntimeid : *mut *mut super::super::System::Com:: SAFEARRAY) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn UiaGetUpdatedCache(hnode : HUIANODE, prequest : *mut UiaCacheRequest, normalizestate : NormalizeState, pnormalizecondition : *mut UiaCondition, pprequesteddata : *mut *mut super::super::System::Com:: SAFEARRAY, pptreestructure : *mut ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn UiaHPatternObjectFromVariant(pvar : *mut super::super::System::Variant:: VARIANT, phobj : *mut HUIAPATTERNOBJECT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn UiaHTextRangeFromVariant(pvar : *mut super::super::System::Variant:: VARIANT, phtextrange : *mut HUIATEXTRANGE) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn UiaHUiaNodeFromVariant(pvar : *mut super::super::System::Variant:: VARIANT, phnode : *mut HUIANODE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UiaHasServerSideProvider(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UiaHostProviderFromHwnd(hwnd : super::super::Foundation:: HWND, ppprovider : *mut IRawElementProviderSimple) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn UiaIAccessibleFromProvider(pprovider : IRawElementProviderSimple, dwflags : u32, ppaccessible : *mut IAccessible, pvarchild : *mut super::super::System::Variant:: VARIANT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaLookupId(r#type : AutomationIdentifierType, pguid : *const ::windows_sys::core::GUID) -> i32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn UiaNavigate(hnode : HUIANODE, direction : NavigateDirection, pcondition : *mut UiaCondition, prequest : *mut UiaCacheRequest, pprequesteddata : *mut *mut super::super::System::Com:: SAFEARRAY, pptreestructure : *mut ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn UiaNodeFromFocus(prequest : *mut UiaCacheRequest, pprequesteddata : *mut *mut super::super::System::Com:: SAFEARRAY, pptreestructure : *mut ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UiaNodeFromHandle(hwnd : super::super::Foundation:: HWND, phnode : *mut HUIANODE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn UiaNodeFromPoint(x : f64, y : f64, prequest : *mut UiaCacheRequest, pprequesteddata : *mut *mut super::super::System::Com:: SAFEARRAY, pptreestructure : *mut ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaNodeFromProvider(pprovider : IRawElementProviderSimple, phnode : *mut HUIANODE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UiaNodeRelease(hnode : HUIANODE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UiaPatternRelease(hobj : HUIAPATTERNOBJECT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UiaProviderForNonClient(hwnd : super::super::Foundation:: HWND, idobject : i32, idchild : i32, ppprovider : *mut IRawElementProviderSimple) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn UiaProviderFromIAccessible(paccessible : IAccessible, idchild : i32, dwflags : u32, ppprovider : *mut IRawElementProviderSimple) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaRaiseActiveTextPositionChangedEvent(provider : IRawElementProviderSimple, textrange : ITextRangeProvider) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaRaiseAsyncContentLoadedEvent(pprovider : IRawElementProviderSimple, asynccontentloadedstate : AsyncContentLoadedState, percentcomplete : f64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaRaiseAutomationEvent(pprovider : IRawElementProviderSimple, id : UIA_EVENT_ID) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn UiaRaiseAutomationPropertyChangedEvent(pprovider : IRawElementProviderSimple, id : UIA_PROPERTY_ID, oldvalue : super::super::System::Variant:: VARIANT, newvalue : super::super::System::Variant:: VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn UiaRaiseChangesEvent(pprovider : IRawElementProviderSimple, eventidcount : i32, puiachanges : *mut UiaChangeInfo) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaRaiseNotificationEvent(provider : IRawElementProviderSimple, notificationkind : NotificationKind, notificationprocessing : NotificationProcessing, displaystring : ::windows_sys::core::BSTR, activityid : ::windows_sys::core::BSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaRaiseStructureChangedEvent(pprovider : IRawElementProviderSimple, structurechangetype : StructureChangeType, pruntimeid : *mut i32, cruntimeidlen : i32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn UiaRaiseTextEditTextChangedEvent(pprovider : IRawElementProviderSimple, texteditchangetype : TextEditChangeType, pchangeddata : *mut super::super::System::Com:: SAFEARRAY) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn UiaRegisterProviderCallback(pcallback : *mut UiaProviderCallback) -> ());
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaRemoveEvent(hevent : HUIAEVENT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UiaReturnRawElementProvider(hwnd : super::super::Foundation:: HWND, wparam : super::super::Foundation:: WPARAM, lparam : super::super::Foundation:: LPARAM, el : IRawElementProviderSimple) -> super::super::Foundation:: LRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn UiaSetFocus(hnode : HUIANODE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UiaTextRangeRelease(hobj : HUIATEXTRANGE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UnhookWinEvent(hwineventhook : HWINEVENTHOOK) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn UnregisterPointerInputTarget(hwnd : super::super::Foundation:: HWND, pointertype : super::WindowsAndMessaging:: POINTER_INPUT_TYPE) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn UnregisterPointerInputTargetEx(hwnd : super::super::Foundation:: HWND, pointertype : super::WindowsAndMessaging:: POINTER_INPUT_TYPE) -> super::super::Foundation:: BOOL);
::windows_targets::link!("uiautomationcore.dll" "system" fn ValuePattern_SetValue(hobj : HUIAPATTERNOBJECT, pval : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn VirtualizedItemPattern_Realize(hobj : HUIAPATTERNOBJECT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("oleacc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn WindowFromAccessibleObject(param0 : IAccessible, phwnd : *mut super::super::Foundation:: HWND) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn WindowPattern_Close(hobj : HUIAPATTERNOBJECT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("uiautomationcore.dll" "system" fn WindowPattern_SetWindowVisualState(hobj : HUIAPATTERNOBJECT, state : WindowVisualState) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("uiautomationcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WindowPattern_WaitForInputIdle(hobj : HUIAPATTERNOBJECT, milliseconds : i32, presult : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
pub type IAccIdentity = *mut ::core::ffi::c_void;
pub type IAccPropServer = *mut ::core::ffi::c_void;
pub type IAccPropServices = *mut ::core::ffi::c_void;
pub type IAccessible = *mut ::core::ffi::c_void;
pub type IAccessibleEx = *mut ::core::ffi::c_void;
pub type IAccessibleHandler = *mut ::core::ffi::c_void;
pub type IAccessibleHostingElementProviders = *mut ::core::ffi::c_void;
pub type IAccessibleWindowlessSite = *mut ::core::ffi::c_void;
pub type IAnnotationProvider = *mut ::core::ffi::c_void;
pub type ICustomNavigationProvider = *mut ::core::ffi::c_void;
pub type IDockProvider = *mut ::core::ffi::c_void;
pub type IDragProvider = *mut ::core::ffi::c_void;
pub type IDropTargetProvider = *mut ::core::ffi::c_void;
pub type IExpandCollapseProvider = *mut ::core::ffi::c_void;
pub type IGridItemProvider = *mut ::core::ffi::c_void;
pub type IGridProvider = *mut ::core::ffi::c_void;
pub type IInvokeProvider = *mut ::core::ffi::c_void;
pub type IItemContainerProvider = *mut ::core::ffi::c_void;
pub type ILegacyIAccessibleProvider = *mut ::core::ffi::c_void;
pub type IMultipleViewProvider = *mut ::core::ffi::c_void;
pub type IObjectModelProvider = *mut ::core::ffi::c_void;
pub type IProxyProviderWinEventHandler = *mut ::core::ffi::c_void;
pub type IProxyProviderWinEventSink = *mut ::core::ffi::c_void;
pub type IRangeValueProvider = *mut ::core::ffi::c_void;
pub type IRawElementProviderAdviseEvents = *mut ::core::ffi::c_void;
pub type IRawElementProviderFragment = *mut ::core::ffi::c_void;
pub type IRawElementProviderFragmentRoot = *mut ::core::ffi::c_void;
pub type IRawElementProviderHostingAccessibles = *mut ::core::ffi::c_void;
pub type IRawElementProviderHwndOverride = *mut ::core::ffi::c_void;
pub type IRawElementProviderSimple = *mut ::core::ffi::c_void;
pub type IRawElementProviderSimple2 = *mut ::core::ffi::c_void;
pub type IRawElementProviderSimple3 = *mut ::core::ffi::c_void;
pub type IRawElementProviderWindowlessSite = *mut ::core::ffi::c_void;
pub type IRichEditUiaInformation = *mut ::core::ffi::c_void;
pub type IRicheditWindowlessAccessibility = *mut ::core::ffi::c_void;
pub type IScrollItemProvider = *mut ::core::ffi::c_void;
pub type IScrollProvider = *mut ::core::ffi::c_void;
pub type ISelectionItemProvider = *mut ::core::ffi::c_void;
pub type ISelectionProvider = *mut ::core::ffi::c_void;
pub type ISelectionProvider2 = *mut ::core::ffi::c_void;
pub type ISpreadsheetItemProvider = *mut ::core::ffi::c_void;
pub type ISpreadsheetProvider = *mut ::core::ffi::c_void;
pub type IStylesProvider = *mut ::core::ffi::c_void;
pub type ISynchronizedInputProvider = *mut ::core::ffi::c_void;
pub type ITableItemProvider = *mut ::core::ffi::c_void;
pub type ITableProvider = *mut ::core::ffi::c_void;
pub type ITextChildProvider = *mut ::core::ffi::c_void;
pub type ITextEditProvider = *mut ::core::ffi::c_void;
pub type ITextProvider = *mut ::core::ffi::c_void;
pub type ITextProvider2 = *mut ::core::ffi::c_void;
pub type ITextRangeProvider = *mut ::core::ffi::c_void;
pub type ITextRangeProvider2 = *mut ::core::ffi::c_void;
pub type IToggleProvider = *mut ::core::ffi::c_void;
pub type ITransformProvider = *mut ::core::ffi::c_void;
pub type ITransformProvider2 = *mut ::core::ffi::c_void;
pub type IUIAutomation = *mut ::core::ffi::c_void;
pub type IUIAutomation2 = *mut ::core::ffi::c_void;
pub type IUIAutomation3 = *mut ::core::ffi::c_void;
pub type IUIAutomation4 = *mut ::core::ffi::c_void;
pub type IUIAutomation5 = *mut ::core::ffi::c_void;
pub type IUIAutomation6 = *mut ::core::ffi::c_void;
pub type IUIAutomationActiveTextPositionChangedEventHandler = *mut ::core::ffi::c_void;
pub type IUIAutomationAndCondition = *mut ::core::ffi::c_void;
pub type IUIAutomationAnnotationPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationBoolCondition = *mut ::core::ffi::c_void;
pub type IUIAutomationCacheRequest = *mut ::core::ffi::c_void;
pub type IUIAutomationChangesEventHandler = *mut ::core::ffi::c_void;
pub type IUIAutomationCondition = *mut ::core::ffi::c_void;
pub type IUIAutomationCustomNavigationPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationDockPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationDragPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationDropTargetPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationElement = *mut ::core::ffi::c_void;
pub type IUIAutomationElement2 = *mut ::core::ffi::c_void;
pub type IUIAutomationElement3 = *mut ::core::ffi::c_void;
pub type IUIAutomationElement4 = *mut ::core::ffi::c_void;
pub type IUIAutomationElement5 = *mut ::core::ffi::c_void;
pub type IUIAutomationElement6 = *mut ::core::ffi::c_void;
pub type IUIAutomationElement7 = *mut ::core::ffi::c_void;
pub type IUIAutomationElement8 = *mut ::core::ffi::c_void;
pub type IUIAutomationElement9 = *mut ::core::ffi::c_void;
pub type IUIAutomationElementArray = *mut ::core::ffi::c_void;
pub type IUIAutomationEventHandler = *mut ::core::ffi::c_void;
pub type IUIAutomationEventHandlerGroup = *mut ::core::ffi::c_void;
pub type IUIAutomationExpandCollapsePattern = *mut ::core::ffi::c_void;
pub type IUIAutomationFocusChangedEventHandler = *mut ::core::ffi::c_void;
pub type IUIAutomationGridItemPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationGridPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationInvokePattern = *mut ::core::ffi::c_void;
pub type IUIAutomationItemContainerPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationLegacyIAccessiblePattern = *mut ::core::ffi::c_void;
pub type IUIAutomationMultipleViewPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationNotCondition = *mut ::core::ffi::c_void;
pub type IUIAutomationNotificationEventHandler = *mut ::core::ffi::c_void;
pub type IUIAutomationObjectModelPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationOrCondition = *mut ::core::ffi::c_void;
pub type IUIAutomationPatternHandler = *mut ::core::ffi::c_void;
pub type IUIAutomationPatternInstance = *mut ::core::ffi::c_void;
pub type IUIAutomationPropertyChangedEventHandler = *mut ::core::ffi::c_void;
pub type IUIAutomationPropertyCondition = *mut ::core::ffi::c_void;
pub type IUIAutomationProxyFactory = *mut ::core::ffi::c_void;
pub type IUIAutomationProxyFactoryEntry = *mut ::core::ffi::c_void;
pub type IUIAutomationProxyFactoryMapping = *mut ::core::ffi::c_void;
pub type IUIAutomationRangeValuePattern = *mut ::core::ffi::c_void;
pub type IUIAutomationRegistrar = *mut ::core::ffi::c_void;
pub type IUIAutomationScrollItemPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationScrollPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationSelectionItemPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationSelectionPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationSelectionPattern2 = *mut ::core::ffi::c_void;
pub type IUIAutomationSpreadsheetItemPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationSpreadsheetPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationStructureChangedEventHandler = *mut ::core::ffi::c_void;
pub type IUIAutomationStylesPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationSynchronizedInputPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationTableItemPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationTablePattern = *mut ::core::ffi::c_void;
pub type IUIAutomationTextChildPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationTextEditPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationTextEditTextChangedEventHandler = *mut ::core::ffi::c_void;
pub type IUIAutomationTextPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationTextPattern2 = *mut ::core::ffi::c_void;
pub type IUIAutomationTextRange = *mut ::core::ffi::c_void;
pub type IUIAutomationTextRange2 = *mut ::core::ffi::c_void;
pub type IUIAutomationTextRange3 = *mut ::core::ffi::c_void;
pub type IUIAutomationTextRangeArray = *mut ::core::ffi::c_void;
pub type IUIAutomationTogglePattern = *mut ::core::ffi::c_void;
pub type IUIAutomationTransformPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationTransformPattern2 = *mut ::core::ffi::c_void;
pub type IUIAutomationTreeWalker = *mut ::core::ffi::c_void;
pub type IUIAutomationValuePattern = *mut ::core::ffi::c_void;
pub type IUIAutomationVirtualizedItemPattern = *mut ::core::ffi::c_void;
pub type IUIAutomationWindowPattern = *mut ::core::ffi::c_void;
pub type IValueProvider = *mut ::core::ffi::c_void;
pub type IVirtualizedItemProvider = *mut ::core::ffi::c_void;
pub type IWindowProvider = *mut ::core::ffi::c_void;
pub const ANNO_CONTAINER: AnnoScope = 1i32;
pub const ANNO_THIS: AnnoScope = 0i32;
pub const ANRUS_ON_SCREEN_KEYBOARD_ACTIVE: ACC_UTILITY_STATE_FLAGS = 1u32;
pub const ANRUS_PRIORITY_AUDIO_ACTIVE: ACC_UTILITY_STATE_FLAGS = 4u32;
pub const ANRUS_PRIORITY_AUDIO_ACTIVE_NODUCK: ACC_UTILITY_STATE_FLAGS = 8u32;
pub const ANRUS_PRIORITY_AUDIO_DYNAMIC_DUCK: u32 = 16u32;
pub const ANRUS_TOUCH_MODIFICATION_ACTIVE: ACC_UTILITY_STATE_FLAGS = 2u32;
pub const AcceleratorKey_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x514865df_2557_4cb9_aeed_6ced084ce52c);
pub const AccessKey_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x06827b12_a7f9_4a15_917c_ffa5ad3eb0a7);
pub const ActiveEnd_End: ActiveEnd = 2i32;
pub const ActiveEnd_None: ActiveEnd = 0i32;
pub const ActiveEnd_Start: ActiveEnd = 1i32;
pub const ActiveTextPositionChanged_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa5c09e9c_c77d_4f25_b491_e5bb7017cbd4);
pub const AnimationStyle_BlinkingBackground: AnimationStyle = 2i32;
pub const AnimationStyle_LasVegasLights: AnimationStyle = 1i32;
pub const AnimationStyle_MarchingBlackAnts: AnimationStyle = 4i32;
pub const AnimationStyle_MarchingRedAnts: AnimationStyle = 5i32;
pub const AnimationStyle_None: AnimationStyle = 0i32;
pub const AnimationStyle_Other: AnimationStyle = -1i32;
pub const AnimationStyle_Shimmer: AnimationStyle = 6i32;
pub const AnimationStyle_SparkleText: AnimationStyle = 3i32;
pub const AnnotationObjects_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x310910c8_7c6e_4f20_becd_4aaf6d191156);
pub const AnnotationType_AdvancedProofingIssue: UIA_ANNOTATIONTYPE = 60020u32;
pub const AnnotationType_Author: UIA_ANNOTATIONTYPE = 60019u32;
pub const AnnotationType_CircularReferenceError: UIA_ANNOTATIONTYPE = 60022u32;
pub const AnnotationType_Comment: UIA_ANNOTATIONTYPE = 60003u32;
pub const AnnotationType_ConflictingChange: UIA_ANNOTATIONTYPE = 60018u32;
pub const AnnotationType_DataValidationError: UIA_ANNOTATIONTYPE = 60021u32;
pub const AnnotationType_DeletionChange: UIA_ANNOTATIONTYPE = 60012u32;
pub const AnnotationType_EditingLockedChange: UIA_ANNOTATIONTYPE = 60016u32;
pub const AnnotationType_Endnote: UIA_ANNOTATIONTYPE = 60009u32;
pub const AnnotationType_ExternalChange: UIA_ANNOTATIONTYPE = 60017u32;
pub const AnnotationType_Footer: UIA_ANNOTATIONTYPE = 60007u32;
pub const AnnotationType_Footnote: UIA_ANNOTATIONTYPE = 60010u32;
pub const AnnotationType_FormatChange: UIA_ANNOTATIONTYPE = 60014u32;
pub const AnnotationType_FormulaError: UIA_ANNOTATIONTYPE = 60004u32;
pub const AnnotationType_GrammarError: UIA_ANNOTATIONTYPE = 60002u32;
pub const AnnotationType_Header: UIA_ANNOTATIONTYPE = 60006u32;
pub const AnnotationType_Highlighted: UIA_ANNOTATIONTYPE = 60008u32;
pub const AnnotationType_InsertionChange: UIA_ANNOTATIONTYPE = 60011u32;
pub const AnnotationType_Mathematics: UIA_ANNOTATIONTYPE = 60023u32;
pub const AnnotationType_MoveChange: UIA_ANNOTATIONTYPE = 60013u32;
pub const AnnotationType_Sensitive: UIA_ANNOTATIONTYPE = 60024u32;
pub const AnnotationType_SpellingError: UIA_ANNOTATIONTYPE = 60001u32;
pub const AnnotationType_TrackChanges: UIA_ANNOTATIONTYPE = 60005u32;
pub const AnnotationType_Unknown: UIA_ANNOTATIONTYPE = 60000u32;
pub const AnnotationType_UnsyncedChange: UIA_ANNOTATIONTYPE = 60015u32;
pub const AnnotationTypes_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x64b71f76_53c4_4696_a219_20e940c9a176);
pub const Annotation_AdvancedProofingIssue_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdac7b72c_c0f2_4b84_b90d_5fafc0f0ef1c);
pub const Annotation_AnnotationTypeId_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x20ae484f_69ef_4c48_8f5b_c4938b206ac7);
pub const Annotation_AnnotationTypeName_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9b818892_5ac9_4af9_aa96_f58a77b058e3);
pub const Annotation_Author_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf161d3a7_f81b_4128_b17f_71f690914520);
pub const Annotation_Author_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7a528462_9c5c_4a03_a974_8b307a9937f2);
pub const Annotation_CircularReferenceError_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x25bd9cf4_1745_4659_ba67_727f0318c616);
pub const Annotation_Comment_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfd2fda30_26b3_4c06_8bc7_98f1532e46fd);
pub const Annotation_ConflictingChange_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x98af8802_517c_459f_af13_016d3fab877e);
pub const Annotation_Custom_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9ec82750_3931_4952_85bc_1dbff78a43e3);
pub const Annotation_DataValidationError_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc8649fa8_9775_437e_ad46_e709d93c2343);
pub const Annotation_DateTime_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x99b5ca5d_1acf_414b_a4d0_6b350b047578);
pub const Annotation_DeletionChange_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbe3d5b05_951d_42e7_901d_adc8c2cf34d0);
pub const Annotation_EditingLockedChange_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc31f3e1c_7423_4dac_8348_41f099ff6f64);
pub const Annotation_Endnote_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7565725c_2d99_4839_960d_33d3b866aba5);
pub const Annotation_ExternalChange_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x75a05b31_5f11_42fd_887d_dfa010db2392);
pub const Annotation_Footer_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcceab046_1833_47aa_8080_701ed0b0c832);
pub const Annotation_Footnote_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3de10e21_4125_42db_8620_be8083080624);
pub const Annotation_FormatChange_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xeb247345_d4f1_41ce_8e52_f79b69635e48);
pub const Annotation_FormulaError_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x95611982_0cab_46d5_a2f0_e30d1905f8bf);
pub const Annotation_GrammarError_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x757a048d_4518_41c6_854c_dc009b7cfb53);
pub const Annotation_Header_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x867b409b_b216_4472_a219_525e310681f8);
pub const Annotation_Highlighted_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x757c884e_8083_4081_8b9c_e87f5072f0e4);
pub const Annotation_InsertionChange_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0dbeb3a6_df15_4164_a3c0_e21a8ce931c4);
pub const Annotation_Mathematics_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xeaab634b_26d0_40c1_8073_57ca1c633c9b);
pub const Annotation_MoveChange_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9da587eb_23e5_4490_b385_1a22ddc8b187);
pub const Annotation_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf6c72ad7_356c_4850_9291_316f608a8c84);
pub const Annotation_Sensitive_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x37f4c04f_0f12_4464_929c_828fd15292e3);
pub const Annotation_SpellingError_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xae85567e_9ece_423f_81b7_96c43d53e50e);
pub const Annotation_Target_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb71b302d_2104_44ad_9c5c_092b4907d70f);
pub const Annotation_TrackChanges_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x21e6e888_dc14_4016_ac27_190553c8c470);
pub const Annotation_UnsyncedChange_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1851116a_0e47_4b30_8cb5_d7dae4fbcd1b);
pub const AppBar_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6114908d_cc02_4d37_875b_b530c7139554);
pub const AriaProperties_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4213678c_e025_4922_beb5_e43ba08e6221);
pub const AriaRole_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdd207b95_be4a_4e0d_b727_63ace94b6916);
pub const Assertive: LiveSetting = 2i32;
pub const AsyncContentLoadedState_Beginning: AsyncContentLoadedState = 0i32;
pub const AsyncContentLoadedState_Completed: AsyncContentLoadedState = 2i32;
pub const AsyncContentLoadedState_Progress: AsyncContentLoadedState = 1i32;
pub const AsyncContentLoaded_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5fdee11c_d2fa_4fb9_904e_5cbee894d5ef);
pub const AutomationElementMode_Full: AutomationElementMode = 1i32;
pub const AutomationElementMode_None: AutomationElementMode = 0i32;
pub const AutomationFocusChanged_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb68a1f17_f60d_41a7_a3cc_b05292155fe0);
pub const AutomationId_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc82c0500_b60e_4310_a267_303c531f8ee5);
pub const AutomationIdentifierType_Annotation: AutomationIdentifierType = 6i32;
pub const AutomationIdentifierType_Changes: AutomationIdentifierType = 7i32;
pub const AutomationIdentifierType_ControlType: AutomationIdentifierType = 3i32;
pub const AutomationIdentifierType_Event: AutomationIdentifierType = 2i32;
pub const AutomationIdentifierType_LandmarkType: AutomationIdentifierType = 5i32;
pub const AutomationIdentifierType_Pattern: AutomationIdentifierType = 1i32;
pub const AutomationIdentifierType_Property: AutomationIdentifierType = 0i32;
pub const AutomationIdentifierType_Style: AutomationIdentifierType = 8i32;
pub const AutomationIdentifierType_TextAttribute: AutomationIdentifierType = 4i32;
pub const AutomationPropertyChanged_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2527fba1_8d7a_4630_a4cc_e66315942f52);
pub const BoundingRectangle_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7bbfe8b2_3bfc_48dd_b729_c794b846e9a1);
pub const BulletStyle_DashBullet: BulletStyle = 5i32;
pub const BulletStyle_FilledRoundBullet: BulletStyle = 2i32;
pub const BulletStyle_FilledSquareBullet: BulletStyle = 4i32;
pub const BulletStyle_HollowRoundBullet: BulletStyle = 1i32;
pub const BulletStyle_HollowSquareBullet: BulletStyle = 3i32;
pub const BulletStyle_None: BulletStyle = 0i32;
pub const BulletStyle_Other: BulletStyle = -1i32;
pub const Button_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5a78e369_c6a1_4f33_a9d7_79f20d0c788e);
pub const CAccPropServices: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb5f8350b_0548_48b1_a6ee_88bd00b4a5e7);
pub const CLSID_AccPropServices: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb5f8350b_0548_48b1_a6ee_88bd00b4a5e7);
pub const CUIAutomation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xff48dba4_60ef_4201_aa87_54103eef594e);
pub const CUIAutomation8: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe22ad333_b25f_460c_83d0_0581107395c9);
pub const CUIAutomationRegistrar: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6e29fabf_9977_42d1_8d0e_ca7e61ad87e6);
pub const Calendar_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8913eb88_00e5_46bc_8e4e_14a786e165a1);
pub const CapStyle_AllCap: CapStyle = 2i32;
pub const CapStyle_AllPetiteCaps: CapStyle = 3i32;
pub const CapStyle_None: CapStyle = 0i32;
pub const CapStyle_Other: CapStyle = -1i32;
pub const CapStyle_PetiteCaps: CapStyle = 4i32;
pub const CapStyle_SmallCap: CapStyle = 1i32;
pub const CapStyle_Titling: CapStyle = 6i32;
pub const CapStyle_Unicase: CapStyle = 5i32;
pub const CaretBidiMode_LTR: CaretBidiMode = 0i32;
pub const CaretBidiMode_RTL: CaretBidiMode = 1i32;
pub const CaretPosition_BeginningOfLine: CaretPosition = 2i32;
pub const CaretPosition_EndOfLine: CaretPosition = 1i32;
pub const CaretPosition_Unknown: CaretPosition = 0i32;
pub const CenterPoint_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0cb00c08_540c_4edb_9445_26359ea69785);
pub const Changes_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7df26714_614f_4e05_9488_716c5ba19436);
pub const Changes_Summary_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x313d65a6_e60f_4d62_9861_55afd728d207);
pub const CheckBox_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfb50f922_a3db_49c0_8bc3_06dad55778e2);
pub const ClassName_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x157b7215_894f_4b65_84e2_aac0da08b16b);
pub const ClickablePoint_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0196903b_b203_4818_a9f3_f08e675f2341);
pub const CoalesceEventsOptions_Disabled: CoalesceEventsOptions = 0i32;
pub const CoalesceEventsOptions_Enabled: CoalesceEventsOptions = 1i32;
pub const ComboBox_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x54cb426c_2f33_4fff_aaa1_aef60dac5deb);
pub const ConditionType_And: ConditionType = 3i32;
pub const ConditionType_False: ConditionType = 1i32;
pub const ConditionType_Not: ConditionType = 5i32;
pub const ConditionType_Or: ConditionType = 4i32;
pub const ConditionType_Property: ConditionType = 2i32;
pub const ConditionType_True: ConditionType = 0i32;
pub const ConnectionRecoveryBehaviorOptions_Disabled: ConnectionRecoveryBehaviorOptions = 0i32;
pub const ConnectionRecoveryBehaviorOptions_Enabled: ConnectionRecoveryBehaviorOptions = 1i32;
pub const ControlType_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xca774fea_28ac_4bc2_94ca_acec6d6c10a3);
pub const ControllerFor_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x51124c8a_a5d2_4f13_9be6_7fa8ba9d3a90);
pub const Culture_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe2d74f27_3d79_4dc2_b88b_3044963a8afb);
pub const CustomNavigation_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xafea938a_621e_4054_bb2c_2f46114dac3f);
pub const Custom_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf29ea0c3_adb7_430a_ba90_e52c7313e6ed);
pub const DISPID_ACC_CHILD: i32 = -5002i32;
pub const DISPID_ACC_CHILDCOUNT: i32 = -5001i32;
pub const DISPID_ACC_DEFAULTACTION: i32 = -5013i32;
pub const DISPID_ACC_DESCRIPTION: i32 = -5005i32;
pub const DISPID_ACC_DODEFAULTACTION: i32 = -5018i32;
pub const DISPID_ACC_FOCUS: i32 = -5011i32;
pub const DISPID_ACC_HELP: i32 = -5008i32;
pub const DISPID_ACC_HELPTOPIC: i32 = -5009i32;
pub const DISPID_ACC_HITTEST: i32 = -5017i32;
pub const DISPID_ACC_KEYBOARDSHORTCUT: i32 = -5010i32;
pub const DISPID_ACC_LOCATION: i32 = -5015i32;
pub const DISPID_ACC_NAME: i32 = -5003i32;
pub const DISPID_ACC_NAVIGATE: i32 = -5016i32;
pub const DISPID_ACC_PARENT: i32 = -5000i32;
pub const DISPID_ACC_ROLE: i32 = -5006i32;
pub const DISPID_ACC_SELECT: i32 = -5014i32;
pub const DISPID_ACC_SELECTION: i32 = -5012i32;
pub const DISPID_ACC_STATE: i32 = -5007i32;
pub const DISPID_ACC_VALUE: i32 = -5004i32;
pub const DataGrid_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x84b783af_d103_4b0a_8415_e73942410f4b);
pub const DataItem_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa0177842_d94f_42a5_814b_6068addc8da5);
pub const DescribedBy_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7c5865b8_9992_40fd_8db0_6bf1d317f998);
pub const DockPosition_Bottom: DockPosition = 2i32;
pub const DockPosition_Fill: DockPosition = 4i32;
pub const DockPosition_Left: DockPosition = 1i32;
pub const DockPosition_None: DockPosition = 5i32;
pub const DockPosition_Right: DockPosition = 3i32;
pub const DockPosition_Top: DockPosition = 0i32;
pub const Dock_DockPosition_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6d67f02e_c0b0_4b10_b5b9_18d6ecf98760);
pub const Dock_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9cbaa846_83c8_428d_827f_7e6063fe0620);
pub const Document_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3cd6bb6f_6f08_4562_b229_e4e2fc7a9eb4);
pub const Drag_DragCancel_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc3ede6fa_3451_4e0f_9e71_df9c280a4657);
pub const Drag_DragComplete_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x38e96188_ef1f_463e_91ca_3a7792c29caf);
pub const Drag_DragStart_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x883a480b_3aa9_429d_95e4_d9c8d011f0dd);
pub const Drag_DropEffect_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x646f2779_48d3_4b23_8902_4bf100005df3);
pub const Drag_DropEffects_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf5d61156_7ce6_49be_a836_9269dcec920f);
pub const Drag_GrabbedItems_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x77c1562c_7b86_4b21_9ed7_3cefda6f4c43);
pub const Drag_IsGrabbed_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x45f206f3_75cc_4cca_a9b9_fcdfb982d8a2);
pub const Drag_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc0bee21f_ccb3_4fed_995b_114f6e3d2728);
pub const DropTarget_DragEnter_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xaad9319b_032c_4a88_961d_1cf579581e34);
pub const DropTarget_DragLeave_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0f82eb15_24a2_4988_9217_de162aee272b);
pub const DropTarget_DropTargetEffect_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8bb75975_a0ca_4981_b818_87fc66e9509d);
pub const DropTarget_DropTargetEffects_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbc1dd4ed_cb89_45f1_a592_e03b08ae790f);
pub const DropTarget_Dropped_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x622cead8_1edb_4a3d_abbc_be2211ff68b5);
pub const DropTarget_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0bcbec56_bd34_4b7b_9fd5_2659905ea3dc);
pub const Edit_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6504a5c8_2c86_4f87_ae7b_1abddc810cf9);
pub const EventArgsType_ActiveTextPositionChanged: EventArgsType = 8i32;
pub const EventArgsType_AsyncContentLoaded: EventArgsType = 3i32;
pub const EventArgsType_Changes: EventArgsType = 6i32;
pub const EventArgsType_Notification: EventArgsType = 7i32;
pub const EventArgsType_PropertyChanged: EventArgsType = 1i32;
pub const EventArgsType_Simple: EventArgsType = 0i32;
pub const EventArgsType_StructureChanged: EventArgsType = 2i32;
pub const EventArgsType_StructuredMarkup: EventArgsType = 9i32;
pub const EventArgsType_TextEditTextChanged: EventArgsType = 5i32;
pub const EventArgsType_WindowClosed: EventArgsType = 4i32;
pub const ExpandCollapseState_Collapsed: ExpandCollapseState = 0i32;
pub const ExpandCollapseState_Expanded: ExpandCollapseState = 1i32;
pub const ExpandCollapseState_LeafNode: ExpandCollapseState = 3i32;
pub const ExpandCollapseState_PartiallyExpanded: ExpandCollapseState = 2i32;
pub const ExpandCollapse_ExpandCollapseState_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x275a4c48_85a7_4f69_aba0_af157610002b);
pub const ExpandCollapse_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xae05efa2_f9d1_428a_834c_53a5c52f9b8b);
pub const FillColor_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6e0ec4d0_e2a8_4a56_9de7_953389933b39);
pub const FillType_Color: FillType = 1i32;
pub const FillType_Gradient: FillType = 2i32;
pub const FillType_None: FillType = 0i32;
pub const FillType_Pattern: FillType = 4i32;
pub const FillType_Picture: FillType = 3i32;
pub const FillType_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc6fc74e4_8cb9_429c_a9e1_9bc4ac372b62);
pub const FlowDirections_BottomToTop: FlowDirections = 2i32;
pub const FlowDirections_Default: FlowDirections = 0i32;
pub const FlowDirections_RightToLeft: FlowDirections = 1i32;
pub const FlowDirections_Vertical: FlowDirections = 4i32;
pub const FlowsFrom_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x05c6844f_19de_48f8_95fa_880d5b0fd615);
pub const FlowsTo_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe4f33d20_559a_47fb_a830_f9cb4ff1a70a);
pub const FrameworkId_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdbfd9900_7e1a_4f58_b61b_7063120f773b);
pub const FullDescription_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0d4450ff_6aef_4f33_95dd_7befa72a4391);
pub const GridItem_ColumnSpan_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x583ea3f5_86d0_4b08_a6ec_2c5463ffc109);
pub const GridItem_Column_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc774c15c_62c0_4519_8bdc_47be573c8ad5);
pub const GridItem_Parent_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9d912252_b97f_4ecc_8510_ea0e33427c72);
pub const GridItem_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2d5c877_a462_4957_a2a5_2c96b303bc63);
pub const GridItem_RowSpan_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4582291c_466b_4e93_8e83_3d1715ec0c5e);
pub const GridItem_Row_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6223972a_c945_4563_9329_fdc974af2553);
pub const Grid_ColumnCount_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfe96f375_44aa_4536_ac7a_2a75d71a3efc);
pub const Grid_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x260a2ccb_93a8_4e44_a4c1_3df397f2b02b);
pub const Grid_RowCount_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2a9505bf_c2eb_4fb6_b356_8245ae53703e);
pub const Group_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xad50aa1c_e8c8_4774_ae1b_dd86df0b3bdc);
pub const HCF_AVAILABLE: HIGHCONTRASTW_FLAGS = 2u32;
pub const HCF_CONFIRMHOTKEY: HIGHCONTRASTW_FLAGS = 8u32;
pub const HCF_HIGHCONTRASTON: HIGHCONTRASTW_FLAGS = 1u32;
pub const HCF_HOTKEYACTIVE: HIGHCONTRASTW_FLAGS = 4u32;
pub const HCF_HOTKEYAVAILABLE: HIGHCONTRASTW_FLAGS = 64u32;
pub const HCF_HOTKEYSOUND: HIGHCONTRASTW_FLAGS = 16u32;
pub const HCF_INDICATOR: HIGHCONTRASTW_FLAGS = 32u32;
pub const HCF_OPTION_NOTHEMECHANGE: HIGHCONTRASTW_FLAGS = 4096u32;
pub const HasKeyboardFocus_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcf8afd39_3f46_4800_9656_b2bf12529905);
pub const HeaderItem_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe6bc12cb_7c8e_49cf_b168_4a93a32bebb0);
pub const Header_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5b90cbce_78fb_4614_82b6_554d74718e67);
pub const HeadingLevel1: UIA_HEADINGLEVEL_ID = 80051u32;
pub const HeadingLevel2: UIA_HEADINGLEVEL_ID = 80052u32;
pub const HeadingLevel3: UIA_HEADINGLEVEL_ID = 80053u32;
pub const HeadingLevel4: UIA_HEADINGLEVEL_ID = 80054u32;
pub const HeadingLevel5: UIA_HEADINGLEVEL_ID = 80055u32;
pub const HeadingLevel6: UIA_HEADINGLEVEL_ID = 80056u32;
pub const HeadingLevel7: UIA_HEADINGLEVEL_ID = 80057u32;
pub const HeadingLevel8: UIA_HEADINGLEVEL_ID = 80058u32;
pub const HeadingLevel9: UIA_HEADINGLEVEL_ID = 80059u32;
pub const HeadingLevel_None: UIA_HEADINGLEVEL_ID = 80050u32;
pub const HeadingLevel_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x29084272_aaaf_4a30_8796_3c12f62b6bbb);
pub const HelpText_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x08555685_0977_45c7_a7a6_abaf5684121a);
pub const HorizontalTextAlignment_Centered: HorizontalTextAlignment = 1i32;
pub const HorizontalTextAlignment_Justified: HorizontalTextAlignment = 3i32;
pub const HorizontalTextAlignment_Left: HorizontalTextAlignment = 0i32;
pub const HorizontalTextAlignment_Right: HorizontalTextAlignment = 2i32;
pub const HostedFragmentRootsInvalidated_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe6bdb03e_0921_4ec5_8dcf_eae877b0426b);
pub const Hyperlink_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8a56022c_b00d_4d15_8ff0_5b6b266e5e02);
pub const IIS_ControlAccessible: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x38c682a6_9731_43f2_9fae_e901e641b101);
pub const IIS_IsOleaccProxy: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x902697fa_80e4_4560_802a_a13f22a64709);
pub const Image_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2d3736e4_6b16_4c57_a962_f93260a75243);
pub const InputDiscarded_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7f36c367_7b18_417c_97e3_9d58ddc944ab);
pub const InputReachedOtherElement_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xed201d8a_4e6c_415e_a874_2460c9b66ba8);
pub const InputReachedTarget_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x93ed549a_0549_40f0_bedb_28e44f7de2a3);
pub const Invoke_Invoked_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdfd699f0_c915_49dd_b422_dde785c3d24b);
pub const Invoke_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd976c2fc_66ea_4a6e_b28f_c24c7546ad37);
pub const IsAnnotationPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0b5b3238_6d5c_41b6_bcc4_5e807f6551c4);
pub const IsContentElement_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4bda64a8_f5d8_480b_8155_ef2e89adb672);
pub const IsControlElement_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x95f35085_abcc_4afd_a5f4_dbb46c230fdb);
pub const IsCustomNavigationPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8f8e80d4_2351_48e0_874a_54aa7313889a);
pub const IsDataValidForForm_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x445ac684_c3fc_4dd9_acf8_845a579296ba);
pub const IsDialog_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9d0dfb9b_8436_4501_bbbb_e534a4fb3b3f);
pub const IsDockPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2600a4c4_2ff8_4c96_ae31_8fe619a13c6c);
pub const IsDragPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe997a7b7_1d39_4ca7_be0f_277fcf5605cc);
pub const IsDropTargetPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0686b62e_8e19_4aaf_873d_384f6d3b92be);
pub const IsEnabled_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2109427f_da60_4fed_bf1b_264bdce6eb3a);
pub const IsExpandCollapsePatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x929d3806_5287_4725_aa16_222afc63d595);
pub const IsGridItemPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5a43e524_f9a2_4b12_84c8_b48a3efedd34);
pub const IsGridPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5622c26c_f0ef_4f3b_97cb_714c0868588b);
pub const IsInvokePatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4e725738_8364_4679_aa6c_f3f41931f750);
pub const IsItemContainerPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x624b5ca7_fe40_4957_a019_20c4cf11920f);
pub const IsKeyboardFocusable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf7b8552a_0859_4b37_b9cb_51e72092f29f);
pub const IsLegacyIAccessiblePatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd8ebd0c7_929a_4ee7_8d3a_d3d94413027b);
pub const IsMultipleViewPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xff0a31eb_8e25_469d_8d6e_e771a27c1b90);
pub const IsObjectModelPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6b21d89b_2841_412f_8ef2_15ca952318ba);
pub const IsOffscreen_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x03c3d160_db79_42db_a2ef_1c231eede507);
pub const IsPassword_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe8482eb1_687c_497b_bebc_03be53ec1454);
pub const IsPeripheral_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xda758276_7ed5_49d4_8e68_ecc9a2d300dd);
pub const IsRangeValuePatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfda4244a_eb4d_43ff_b5ad_ed36d373ec4c);
pub const IsRequiredForForm_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4f5f43cf_59fb_4bde_a270_602e5e1141e9);
pub const IsScrollItemPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1cad1a05_0927_4b76_97e1_0fcdb209b98a);
pub const IsScrollPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3ebb7b4a_828a_4b57_9d22_2fea1632ed0d);
pub const IsSelectionItemPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8becd62d_0bc3_4109_bee2_8e6715290e68);
pub const IsSelectionPattern2Available_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x490806fb_6e89_4a47_8319_d266e511f021);
pub const IsSelectionPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf588acbe_c769_4838_9a60_2686dc1188c4);
pub const IsSpreadsheetItemPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9fe79b2a_2f94_43fd_996b_549e316f4acd);
pub const IsSpreadsheetPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6ff43732_e4b4_4555_97bc_ecdbbc4d1888);
pub const IsStructuredMarkupPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb0d4c196_2c0b_489c_b165_a405928c6f3d);
pub const IsStylesPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x27f353d3_459c_4b59_a490_50611dacafb5);
pub const IsSynchronizedInputPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x75d69cc5_d2bf_4943_876e_b45b62a6cc66);
pub const IsTableItemPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xeb36b40d_8ea4_489b_a013_e60d5951fe34);
pub const IsTablePatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcb83575f_45c2_4048_9c76_159715a139df);
pub const IsTextChildPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x559e65df_30ff_43b5_b5ed_5b283b80c7e9);
pub const IsTextEditPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7843425c_8b32_484c_9ab5_e3200571ffda);
pub const IsTextPattern2Available_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x41cf921d_e3f1_4b22_9c81_e1c3ed331c22);
pub const IsTextPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfbe2d69d_aff6_4a45_82e2_fc92a82f5917);
pub const IsTogglePatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x78686d53_fcd0_4b83_9b78_5832ce63bb5b);
pub const IsTransformPattern2Available_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x25980b4b_be04_4710_ab4a_fda31dbd2895);
pub const IsTransformPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa7f78804_d68b_4077_a5c6_7a5ea1ac31c5);
pub const IsValuePatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0b5020a7_2119_473b_be37_5ceb98bbfb22);
pub const IsVirtualizedItemPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x302cb151_2ac8_45d6_977b_d2b3a5a53f20);
pub const IsWindowPatternAvailable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe7a57bb1_5888_4155_98dc_b422fd57f2bc);
pub const ItemContainer_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3d13da0f_8b9a_4a99_85fa_c5c9a69f1ed4);
pub const ItemStatus_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x51de0321_3973_43e7_8913_0b08e813c37f);
pub const ItemType_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcdda434d_6222_413b_a68a_325dd1d40f39);
pub const LIBID_Accessibility: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1ea4dbf0_3c3b_11cf_810c_00aa00389b71);
pub const LabeledBy_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe5b8924b_fc8a_4a35_8031_cf78ac43e55e);
pub const LandmarkType_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x454045f2_6f61_49f7_a4f8_b5f0cf82da1e);
pub const LayoutInvalidated_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xed7d6544_a6bd_4595_9bae_3d28946cc715);
pub const LegacyIAccessible_ChildId_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9a191b5d_9ef2_4787_a459_dcde885dd4e8);
pub const LegacyIAccessible_DefaultAction_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3b331729_eaad_4502_b85f_92615622913c);
pub const LegacyIAccessible_Description_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x46448418_7d70_4ea9_9d27_b7e775cf2ad7);
pub const LegacyIAccessible_Help_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x94402352_161c_4b77_a98d_a872cc33947a);
pub const LegacyIAccessible_KeyboardShortcut_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8f6909ac_00b8_4259_a41c_966266d43a8a);
pub const LegacyIAccessible_Name_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcaeb063d_40ae_4869_aa5a_1b8e5d666739);
pub const LegacyIAccessible_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x54cc0a9f_3395_48af_ba8d_73f85690f3e0);
pub const LegacyIAccessible_Role_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6856e59f_cbaf_4e31_93e8_bcbf6f7e491c);
pub const LegacyIAccessible_Selection_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8aa8b1e0_0891_40cc_8b06_90d7d4166219);
pub const LegacyIAccessible_State_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdf985854_2281_4340_ab9c_c60e2c5803f6);
pub const LegacyIAccessible_Value_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb5c5b0b6_8217_4a77_97a5_190a85ed0156);
pub const Level_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x242ac529_cd36_400f_aad9_7876ef3af627);
pub const ListItem_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7b3717f2_44d1_4a58_98a8_f12a9b8f78e2);
pub const List_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9b149ee1_7cca_4cfc_9af1_cac7bddd3031);
pub const LiveRegionChanged_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x102d5e90_e6a9_41b6_b1c5_a9b1929d9510);
pub const LiveSetting_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc12bcd8e_2a8e_4950_8ae7_3625111d58eb);
pub const LocalizedControlType_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8763404f_a1bd_452a_89c4_3f01d3833806);
pub const LocalizedLandmarkType_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7ac81980_eafb_4fb2_bf91_f485bef5e8e1);
pub const MSAA_MENU_SIG: i32 = -1441927155i32;
pub const MenuBar_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcc384250_0e7b_4ae8_95ae_a08f261b52ee);
pub const MenuClosed_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3cf1266e_1582_4041_acd7_88a35a965297);
pub const MenuItem_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf45225d3_d0a0_49d8_9834_9a000d2aeddc);
pub const MenuModeEnd_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9ecd4c9f_80dd_47b8_8267_5aec06bb2cff);
pub const MenuModeStart_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x18d7c631_166a_4ac9_ae3b_ef4b5420e681);
pub const MenuOpened_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xebe2e945_66ca_4ed1_9ff8_2ad7df0a1b08);
pub const Menu_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2e9b1440_0ea8_41fd_b374_c1ea6f503cd1);
pub const MultipleView_CurrentView_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7a81a67a_b94f_4875_918b_65c8d2f998e5);
pub const MultipleView_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x547a6ae4_113f_47c4_850f_db4dfa466b1d);
pub const MultipleView_SupportedViews_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8d5db9fd_ce3c_4ae7_b788_400a3c645547);
pub const NAVDIR_DOWN: u32 = 2u32;
pub const NAVDIR_FIRSTCHILD: u32 = 7u32;
pub const NAVDIR_LASTCHILD: u32 = 8u32;
pub const NAVDIR_LEFT: u32 = 3u32;
pub const NAVDIR_MAX: u32 = 9u32;
pub const NAVDIR_MIN: u32 = 0u32;
pub const NAVDIR_NEXT: u32 = 5u32;
pub const NAVDIR_PREVIOUS: u32 = 6u32;
pub const NAVDIR_RIGHT: u32 = 4u32;
pub const NAVDIR_UP: u32 = 1u32;
pub const Name_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc3a6921b_4a99_44f1_bca6_61187052c431);
pub const NavigateDirection_FirstChild: NavigateDirection = 3i32;
pub const NavigateDirection_LastChild: NavigateDirection = 4i32;
pub const NavigateDirection_NextSibling: NavigateDirection = 1i32;
pub const NavigateDirection_Parent: NavigateDirection = 0i32;
pub const NavigateDirection_PreviousSibling: NavigateDirection = 2i32;
pub const NewNativeWindowHandle_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5196b33b_380a_4982_95e1_91f3ef60e024);
pub const NormalizeState_Custom: NormalizeState = 2i32;
pub const NormalizeState_None: NormalizeState = 0i32;
pub const NormalizeState_View: NormalizeState = 1i32;
pub const NotificationKind_ActionAborted: NotificationKind = 3i32;
pub const NotificationKind_ActionCompleted: NotificationKind = 2i32;
pub const NotificationKind_ItemAdded: NotificationKind = 0i32;
pub const NotificationKind_ItemRemoved: NotificationKind = 1i32;
pub const NotificationKind_Other: NotificationKind = 4i32;
pub const NotificationProcessing_All: NotificationProcessing = 2i32;
pub const NotificationProcessing_CurrentThenMostRecent: NotificationProcessing = 4i32;
pub const NotificationProcessing_ImportantAll: NotificationProcessing = 0i32;
pub const NotificationProcessing_ImportantMostRecent: NotificationProcessing = 1i32;
pub const NotificationProcessing_MostRecent: NotificationProcessing = 3i32;
pub const Notification_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x72c5a2f7_9788_480f_b8eb_4dee00f6186f);
pub const ObjectModel_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3e04acfe_08fc_47ec_96bc_353fa3b34aa7);
pub const Off: LiveSetting = 0i32;
pub const OptimizeForVisualContent_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6a852250_c75a_4e5d_b858_e381b0f78861);
pub const OrientationType_Horizontal: OrientationType = 1i32;
pub const OrientationType_None: OrientationType = 0i32;
pub const OrientationType_Vertical: OrientationType = 2i32;
pub const Orientation_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa01eee62_3884_4415_887e_678ec21e39ba);
pub const OutlineColor_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc395d6c0_4b55_4762_a073_fd303a634f52);
pub const OutlineStyles_Embossed: OutlineStyles = 8i32;
pub const OutlineStyles_Engraved: OutlineStyles = 4i32;
pub const OutlineStyles_None: OutlineStyles = 0i32;
pub const OutlineStyles_Outline: OutlineStyles = 1i32;
pub const OutlineStyles_Shadow: OutlineStyles = 2i32;
pub const OutlineThickness_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x13e67cc7_dac2_4888_bdd3_375c62fa9618);
pub const PROPID_ACC_DEFAULTACTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x180c072b_c27f_43c7_9922_f63562a4632b);
pub const PROPID_ACC_DESCRIPTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d48dfe4_bd3f_491f_a648_492d6f20c588);
pub const PROPID_ACC_DESCRIPTIONMAP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1ff1435f_8a14_477b_b226_a0abe279975d);
pub const PROPID_ACC_DODEFAULTACTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1ba09523_2e3b_49a6_a059_59682a3c48fd);
pub const PROPID_ACC_FOCUS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6eb335df_1c29_4127_b12c_dee9fd157f2b);
pub const PROPID_ACC_HELP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc831e11f_44db_4a99_9768_cb8f978b7231);
pub const PROPID_ACC_HELPTOPIC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x787d1379_8ede_440b_8aec_11f7bf9030b3);
pub const PROPID_ACC_KEYBOARDSHORTCUT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7d9bceee_7d1e_4979_9382_5180f4172c34);
pub const PROPID_ACC_NAME: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x608d3df8_8128_4aa7_a428_f55e49267291);
pub const PROPID_ACC_NAV_DOWN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x031670ed_3cdf_48d2_9613_138f2dd8a668);
pub const PROPID_ACC_NAV_FIRSTCHILD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcfd02558_557b_4c67_84f9_2a09fce40749);
pub const PROPID_ACC_NAV_LASTCHILD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x302ecaa5_48d5_4f8d_b671_1a8d20a77832);
pub const PROPID_ACC_NAV_LEFT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x228086cb_82f1_4a39_8705_dcdc0fff92f5);
pub const PROPID_ACC_NAV_NEXT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1cdc5455_8cd9_4c92_a371_3939a2fe3eee);
pub const PROPID_ACC_NAV_PREV: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x776d3891_c73b_4480_b3f6_076a16a15af6);
pub const PROPID_ACC_NAV_RIGHT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcd211d9f_e1cb_4fe5_a77c_920b884d095b);
pub const PROPID_ACC_NAV_UP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x016e1a2b_1a4e_4767_8612_3386f66935ec);
pub const PROPID_ACC_PARENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x474c22b6_ffc2_467a_b1b5_e958b4657330);
pub const PROPID_ACC_ROLE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcb905ff2_7bd1_4c05_b3c8_e6c241364d70);
pub const PROPID_ACC_ROLEMAP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf79acda2_140d_4fe6_8914_208476328269);
pub const PROPID_ACC_SELECTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb99d073c_d731_405b_9061_d95e8f842984);
pub const PROPID_ACC_STATE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa8d4d5b0_0a21_42d0_a5c0_514e984f457b);
pub const PROPID_ACC_STATEMAP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x43946c5e_0ac0_4042_b525_07bbdbe17fa7);
pub const PROPID_ACC_VALUE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x123fe443_211a_4615_9527_c45a7e93717a);
pub const PROPID_ACC_VALUEMAP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xda1c3d79_fc5c_420e_b399_9d1533549e75);
pub const Pane_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5c2b3f5b_9182_42a3_8dec_8c04c1ee634d);
pub const Polite: LiveSetting = 1i32;
pub const PositionInSet_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x33d1dc54_641e_4d76_a6b1_13f341c1f896);
pub const ProcessId_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x40499998_9c31_4245_a403_87320e59eaf6);
pub const ProgressBar_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x228c9f86_c36c_47bb_9fb6_a5834bfc53a4);
pub const PropertyConditionFlags_IgnoreCase: PropertyConditionFlags = 1i32;
pub const PropertyConditionFlags_MatchSubstring: PropertyConditionFlags = 2i32;
pub const PropertyConditionFlags_None: PropertyConditionFlags = 0i32;
pub const ProviderDescription_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdca5708a_c16b_4cd9_b889_beb16a804904);
pub const ProviderOptions_ClientSideProvider: ProviderOptions = 1i32;
pub const ProviderOptions_HasNativeIAccessible: ProviderOptions = 128i32;
pub const ProviderOptions_NonClientAreaProvider: ProviderOptions = 4i32;
pub const ProviderOptions_OverrideProvider: ProviderOptions = 8i32;
pub const ProviderOptions_ProviderOwnsSetFocus: ProviderOptions = 16i32;
pub const ProviderOptions_RefuseNonClientSupport: ProviderOptions = 64i32;
pub const ProviderOptions_ServerSideProvider: ProviderOptions = 2i32;
pub const ProviderOptions_UseClientCoordinates: ProviderOptions = 256i32;
pub const ProviderOptions_UseComThreading: ProviderOptions = 32i32;
pub const ProviderType_BaseHwnd: ProviderType = 0i32;
pub const ProviderType_NonClientArea: ProviderType = 2i32;
pub const ProviderType_Proxy: ProviderType = 1i32;
pub const ROLE_SYSTEM_ALERT: u32 = 8u32;
pub const ROLE_SYSTEM_ANIMATION: u32 = 54u32;
pub const ROLE_SYSTEM_APPLICATION: u32 = 14u32;
pub const ROLE_SYSTEM_BORDER: u32 = 19u32;
pub const ROLE_SYSTEM_BUTTONDROPDOWN: u32 = 56u32;
pub const ROLE_SYSTEM_BUTTONDROPDOWNGRID: u32 = 58u32;
pub const ROLE_SYSTEM_BUTTONMENU: u32 = 57u32;
pub const ROLE_SYSTEM_CARET: u32 = 7u32;
pub const ROLE_SYSTEM_CELL: u32 = 29u32;
pub const ROLE_SYSTEM_CHARACTER: u32 = 32u32;
pub const ROLE_SYSTEM_CHART: u32 = 17u32;
pub const ROLE_SYSTEM_CHECKBUTTON: u32 = 44u32;
pub const ROLE_SYSTEM_CLIENT: u32 = 10u32;
pub const ROLE_SYSTEM_CLOCK: u32 = 61u32;
pub const ROLE_SYSTEM_COLUMN: u32 = 27u32;
pub const ROLE_SYSTEM_COLUMNHEADER: u32 = 25u32;
pub const ROLE_SYSTEM_COMBOBOX: u32 = 46u32;
pub const ROLE_SYSTEM_CURSOR: u32 = 6u32;
pub const ROLE_SYSTEM_DIAGRAM: u32 = 53u32;
pub const ROLE_SYSTEM_DIAL: u32 = 49u32;
pub const ROLE_SYSTEM_DIALOG: u32 = 18u32;
pub const ROLE_SYSTEM_DOCUMENT: u32 = 15u32;
pub const ROLE_SYSTEM_DROPLIST: u32 = 47u32;
pub const ROLE_SYSTEM_EQUATION: u32 = 55u32;
pub const ROLE_SYSTEM_GRAPHIC: u32 = 40u32;
pub const ROLE_SYSTEM_GRIP: u32 = 4u32;
pub const ROLE_SYSTEM_GROUPING: u32 = 20u32;
pub const ROLE_SYSTEM_HELPBALLOON: u32 = 31u32;
pub const ROLE_SYSTEM_HOTKEYFIELD: u32 = 50u32;
pub const ROLE_SYSTEM_INDICATOR: u32 = 39u32;
pub const ROLE_SYSTEM_IPADDRESS: u32 = 63u32;
pub const ROLE_SYSTEM_LINK: u32 = 30u32;
pub const ROLE_SYSTEM_LIST: u32 = 33u32;
pub const ROLE_SYSTEM_LISTITEM: u32 = 34u32;
pub const ROLE_SYSTEM_MENUBAR: u32 = 2u32;
pub const ROLE_SYSTEM_MENUITEM: u32 = 12u32;
pub const ROLE_SYSTEM_MENUPOPUP: u32 = 11u32;
pub const ROLE_SYSTEM_OUTLINE: u32 = 35u32;
pub const ROLE_SYSTEM_OUTLINEBUTTON: u32 = 64u32;
pub const ROLE_SYSTEM_OUTLINEITEM: u32 = 36u32;
pub const ROLE_SYSTEM_PAGETAB: u32 = 37u32;
pub const ROLE_SYSTEM_PAGETABLIST: u32 = 60u32;
pub const ROLE_SYSTEM_PANE: u32 = 16u32;
pub const ROLE_SYSTEM_PROGRESSBAR: u32 = 48u32;
pub const ROLE_SYSTEM_PROPERTYPAGE: u32 = 38u32;
pub const ROLE_SYSTEM_PUSHBUTTON: u32 = 43u32;
pub const ROLE_SYSTEM_RADIOBUTTON: u32 = 45u32;
pub const ROLE_SYSTEM_ROW: u32 = 28u32;
pub const ROLE_SYSTEM_ROWHEADER: u32 = 26u32;
pub const ROLE_SYSTEM_SCROLLBAR: u32 = 3u32;
pub const ROLE_SYSTEM_SEPARATOR: u32 = 21u32;
pub const ROLE_SYSTEM_SLIDER: u32 = 51u32;
pub const ROLE_SYSTEM_SOUND: u32 = 5u32;
pub const ROLE_SYSTEM_SPINBUTTON: u32 = 52u32;
pub const ROLE_SYSTEM_SPLITBUTTON: u32 = 62u32;
pub const ROLE_SYSTEM_STATICTEXT: u32 = 41u32;
pub const ROLE_SYSTEM_STATUSBAR: u32 = 23u32;
pub const ROLE_SYSTEM_TABLE: u32 = 24u32;
pub const ROLE_SYSTEM_TEXT: u32 = 42u32;
pub const ROLE_SYSTEM_TITLEBAR: u32 = 1u32;
pub const ROLE_SYSTEM_TOOLBAR: u32 = 22u32;
pub const ROLE_SYSTEM_TOOLTIP: u32 = 13u32;
pub const ROLE_SYSTEM_WHITESPACE: u32 = 59u32;
pub const ROLE_SYSTEM_WINDOW: u32 = 9u32;
pub const RadioButton_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3bdb49db_fe2c_4483_b3e1_e57f219440c6);
pub const RangeValue_IsReadOnly_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x25fa1055_debf_4373_a79e_1f1a1908d3c4);
pub const RangeValue_LargeChange_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa1f96325_3a3d_4b44_8e1f_4a46d9844019);
pub const RangeValue_Maximum_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x19319914_f979_4b35_a1a6_d37e05433473);
pub const RangeValue_Minimum_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x78cbd3b2_684d_4860_af93_d1f95cb022fd);
pub const RangeValue_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x18b00d87_b1c9_476a_bfbd_5f0bdb926f63);
pub const RangeValue_SmallChange_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x81c2c457_3941_4107_9975_139760f7c072);
pub const RangeValue_Value_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x131f5d98_c50c_489d_abe5_ae220898c5f7);
pub const Rotation_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x767cdc7d_aec0_4110_ad32_30edd403492e);
pub const RowOrColumnMajor_ColumnMajor: RowOrColumnMajor = 1i32;
pub const RowOrColumnMajor_Indeterminate: RowOrColumnMajor = 2i32;
pub const RowOrColumnMajor_RowMajor: RowOrColumnMajor = 0i32;
pub const RuntimeId_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa39eebfa_7fba_4c89_b4d4_b99e2de7d160);
pub const SELFLAG_ADDSELECTION: u32 = 8u32;
pub const SELFLAG_EXTENDSELECTION: u32 = 4u32;
pub const SELFLAG_NONE: u32 = 0u32;
pub const SELFLAG_REMOVESELECTION: u32 = 16u32;
pub const SELFLAG_TAKEFOCUS: u32 = 1u32;
pub const SELFLAG_TAKESELECTION: u32 = 2u32;
pub const SELFLAG_VALID: u32 = 31u32;
pub const SERKF_AVAILABLE: SERIALKEYS_FLAGS = 2u32;
pub const SERKF_INDICATOR: SERIALKEYS_FLAGS = 4u32;
pub const SERKF_SERIALKEYSON: SERIALKEYS_FLAGS = 1u32;
pub const SID_ControlElementProvider: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf4791d68_e254_4ba3_9a53_26a5c5497946);
pub const SID_IsUIAutomationObject: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb96fdb85_7204_4724_842b_c7059dedb9d0);
pub const SKF_AUDIBLEFEEDBACK: STICKYKEYS_FLAGS = 64u32;
pub const SKF_AVAILABLE: STICKYKEYS_FLAGS = 2u32;
pub const SKF_CONFIRMHOTKEY: STICKYKEYS_FLAGS = 8u32;
pub const SKF_HOTKEYACTIVE: STICKYKEYS_FLAGS = 4u32;
pub const SKF_HOTKEYSOUND: STICKYKEYS_FLAGS = 16u32;
pub const SKF_INDICATOR: STICKYKEYS_FLAGS = 32u32;
pub const SKF_LALTLATCHED: STICKYKEYS_FLAGS = 268435456u32;
pub const SKF_LALTLOCKED: STICKYKEYS_FLAGS = 1048576u32;
pub const SKF_LCTLLATCHED: STICKYKEYS_FLAGS = 67108864u32;
pub const SKF_LCTLLOCKED: STICKYKEYS_FLAGS = 262144u32;
pub const SKF_LSHIFTLATCHED: STICKYKEYS_FLAGS = 16777216u32;
pub const SKF_LSHIFTLOCKED: STICKYKEYS_FLAGS = 65536u32;
pub const SKF_LWINLATCHED: STICKYKEYS_FLAGS = 1073741824u32;
pub const SKF_LWINLOCKED: STICKYKEYS_FLAGS = 4194304u32;
pub const SKF_RALTLATCHED: STICKYKEYS_FLAGS = 536870912u32;
pub const SKF_RALTLOCKED: STICKYKEYS_FLAGS = 2097152u32;
pub const SKF_RCTLLATCHED: STICKYKEYS_FLAGS = 134217728u32;
pub const SKF_RCTLLOCKED: STICKYKEYS_FLAGS = 524288u32;
pub const SKF_RSHIFTLATCHED: STICKYKEYS_FLAGS = 33554432u32;
pub const SKF_RSHIFTLOCKED: STICKYKEYS_FLAGS = 131072u32;
pub const SKF_RWINLATCHED: STICKYKEYS_FLAGS = 2147483648u32;
pub const SKF_RWINLOCKED: STICKYKEYS_FLAGS = 8388608u32;
pub const SKF_STICKYKEYSON: STICKYKEYS_FLAGS = 1u32;
pub const SKF_TRISTATE: STICKYKEYS_FLAGS = 128u32;
pub const SKF_TWOKEYSOFF: STICKYKEYS_FLAGS = 256u32;
pub const SSF_AVAILABLE: SOUNDSENTRY_FLAGS = 2u32;
pub const SSF_INDICATOR: SOUNDSENTRY_FLAGS = 4u32;
pub const SSF_SOUNDSENTRYON: SOUNDSENTRY_FLAGS = 1u32;
pub const SSGF_DISPLAY: SOUND_SENTRY_GRAPHICS_EFFECT = 3u32;
pub const SSGF_NONE: SOUND_SENTRY_GRAPHICS_EFFECT = 0u32;
pub const SSTF_BORDER: SOUNDSENTRY_TEXT_EFFECT = 2u32;
pub const SSTF_CHARS: SOUNDSENTRY_TEXT_EFFECT = 1u32;
pub const SSTF_DISPLAY: SOUNDSENTRY_TEXT_EFFECT = 3u32;
pub const SSTF_NONE: SOUNDSENTRY_TEXT_EFFECT = 0u32;
pub const SSWF_CUSTOM: SOUNDSENTRY_WINDOWS_EFFECT = 4u32;
pub const SSWF_DISPLAY: SOUNDSENTRY_WINDOWS_EFFECT = 3u32;
pub const SSWF_NONE: SOUNDSENTRY_WINDOWS_EFFECT = 0u32;
pub const SSWF_TITLE: SOUNDSENTRY_WINDOWS_EFFECT = 1u32;
pub const SSWF_WINDOW: SOUNDSENTRY_WINDOWS_EFFECT = 2u32;
pub const STATE_SYSTEM_HASPOPUP: u32 = 1073741824u32;
pub const STATE_SYSTEM_NORMAL: u32 = 0u32;
pub const SayAsInterpretAs_Address: SayAsInterpretAs = 11i32;
pub const SayAsInterpretAs_Alphanumeric: SayAsInterpretAs = 12i32;
pub const SayAsInterpretAs_Cardinal: SayAsInterpretAs = 2i32;
pub const SayAsInterpretAs_Currency: SayAsInterpretAs = 8i32;
pub const SayAsInterpretAs_Date: SayAsInterpretAs = 5i32;
pub const SayAsInterpretAs_Date_DayMonth: SayAsInterpretAs = 20i32;
pub const SayAsInterpretAs_Date_DayMonthYear: SayAsInterpretAs = 16i32;
pub const SayAsInterpretAs_Date_MonthDay: SayAsInterpretAs = 21i32;
pub const SayAsInterpretAs_Date_MonthDayYear: SayAsInterpretAs = 15i32;
pub const SayAsInterpretAs_Date_MonthYear: SayAsInterpretAs = 19i32;
pub const SayAsInterpretAs_Date_Year: SayAsInterpretAs = 22i32;
pub const SayAsInterpretAs_Date_YearMonth: SayAsInterpretAs = 18i32;
pub const SayAsInterpretAs_Date_YearMonthDay: SayAsInterpretAs = 17i32;
pub const SayAsInterpretAs_Media: SayAsInterpretAs = 14i32;
pub const SayAsInterpretAs_Name: SayAsInterpretAs = 13i32;
pub const SayAsInterpretAs_Net: SayAsInterpretAs = 9i32;
pub const SayAsInterpretAs_None: SayAsInterpretAs = 0i32;
pub const SayAsInterpretAs_Number: SayAsInterpretAs = 4i32;
pub const SayAsInterpretAs_Ordinal: SayAsInterpretAs = 3i32;
pub const SayAsInterpretAs_Spell: SayAsInterpretAs = 1i32;
pub const SayAsInterpretAs_Telephone: SayAsInterpretAs = 7i32;
pub const SayAsInterpretAs_Time: SayAsInterpretAs = 6i32;
pub const SayAsInterpretAs_Time_HoursMinutes12: SayAsInterpretAs = 24i32;
pub const SayAsInterpretAs_Time_HoursMinutes24: SayAsInterpretAs = 26i32;
pub const SayAsInterpretAs_Time_HoursMinutesSeconds12: SayAsInterpretAs = 23i32;
pub const SayAsInterpretAs_Time_HoursMinutesSeconds24: SayAsInterpretAs = 25i32;
pub const SayAsInterpretAs_Url: SayAsInterpretAs = 10i32;
pub const ScrollAmount_LargeDecrement: ScrollAmount = 0i32;
pub const ScrollAmount_LargeIncrement: ScrollAmount = 3i32;
pub const ScrollAmount_NoAmount: ScrollAmount = 2i32;
pub const ScrollAmount_SmallDecrement: ScrollAmount = 1i32;
pub const ScrollAmount_SmallIncrement: ScrollAmount = 4i32;
pub const ScrollBar_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdaf34b36_5065_4946_b22f_92595fc0751a);
pub const ScrollItem_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4591d005_a803_4d5c_b4d5_8d2800f906a7);
pub const Scroll_HorizontalScrollPercent_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc7c13c0e_eb21_47ff_acc4_b5a3350f5191);
pub const Scroll_HorizontalViewSize_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x70c2e5d4_fcb0_4713_a9aa_af92ff79e4cd);
pub const Scroll_HorizontallyScrollable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8b925147_28cd_49ae_bd63_f44118d2e719);
pub const Scroll_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x895fa4b4_759d_4c50_8e15_03460672003c);
pub const Scroll_VerticalScrollPercent_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6c8d7099_b2a8_4948_bff7_3cf9058bfefb);
pub const Scroll_VerticalViewSize_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xde6a2e22_d8c7_40c5_83ba_e5f681d53108);
pub const Scroll_VerticallyScrollable_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x89164798_0068_4315_b89a_1e7cfbbc3dfc);
pub const Selection2_CurrentSelectedItem_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x34257c26_83b5_41a6_939c_ae841c136236);
pub const Selection2_FirstSelectedItem_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcc24ea67_369c_4e55_9ff7_38da69540c29);
pub const Selection2_ItemCount_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbb49eb9f_456d_4048_b591_9c2026b84636);
pub const Selection2_LastSelectedItem_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcf7bda90_2d83_49f8_860c_9ce394cf89b4);
pub const SelectionItem_ElementAddedToSelectionEvent_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3c822dd1_c407_4dba_91dd_79d4aed0aec6);
pub const SelectionItem_ElementRemovedFromSelectionEvent_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x097fa8a9_7079_41af_8b9c_0934d8305e5c);
pub const SelectionItem_ElementSelectedEvent_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb9c7dbfb_4ebe_4532_aaf4_008cf647233c);
pub const SelectionItem_IsSelected_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf122835f_cd5f_43df_b79d_4b849e9e6020);
pub const SelectionItem_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9bc64eeb_87c7_4b28_94bb_4d9fa437b6ef);
pub const SelectionItem_SelectionContainer_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa4365b6e_9c1e_4b63_8b53_c2421dd1e8fb);
pub const Selection_CanSelectMultiple_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x49d73da5_c883_4500_883d_8fcf8daf6cbe);
pub const Selection_InvalidatedEvent_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcac14904_16b4_4b53_8e47_4cb1df267bb7);
pub const Selection_IsSelectionRequired_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb1ae4422_63fe_44e7_a5a5_a738c829b19a);
pub const Selection_Pattern2_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfba25cab_ab98_49f7_a7dc_fe539dc15be7);
pub const Selection_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x66e3b7e8_d821_4d25_8761_435d2c8b253f);
pub const Selection_Selection_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xaa6dc2a2_0e2b_4d38_96d5_34e470b81853);
pub const SemanticZoom_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5fd34a43_061e_42c8_b589_9dccf74bc43a);
pub const Separator_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8767eba3_2a63_4ab0_ac8d_aa50e23de978);
pub const SizeOfSet_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1600d33c_3b9f_4369_9431_aa293f344cf1);
pub const Size_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2b5f761d_f885_4404_973f_9b1d98e36d8f);
pub const Slider_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb033c24b_3b35_4cea_b609_763682fa660b);
pub const Spinner_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x60cc4b38_3cb1_4161_b442_c6b726c17825);
pub const SplitButton_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7011f01f_4ace_4901_b461_920a6f1ca650);
pub const SpreadsheetItem_AnnotationObjects_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa3194c38_c9bc_4604_9396_ae3f9f457f7b);
pub const SpreadsheetItem_AnnotationTypes_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc70c51d0_d602_4b45_afbc_b4712b96d72b);
pub const SpreadsheetItem_Formula_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe602e47d_1b47_4bea_87cf_3b0b0b5c15b6);
pub const SpreadsheetItem_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x32cf83ff_f1a8_4a8c_8658_d47ba74e20ba);
pub const Spreadsheet_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6a5b24c9_9d1e_4b85_9e44_c02e3169b10b);
pub const StatusBar_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd45e7d1b_5873_475f_95a4_0433e1f1b00a);
pub const StructureChangeType_ChildAdded: StructureChangeType = 0i32;
pub const StructureChangeType_ChildRemoved: StructureChangeType = 1i32;
pub const StructureChangeType_ChildrenBulkAdded: StructureChangeType = 3i32;
pub const StructureChangeType_ChildrenBulkRemoved: StructureChangeType = 4i32;
pub const StructureChangeType_ChildrenInvalidated: StructureChangeType = 2i32;
pub const StructureChangeType_ChildrenReordered: StructureChangeType = 5i32;
pub const StructureChanged_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x59977961_3edd_4b11_b13b_676b2a2a6ca9);
pub const StructuredMarkup_CompositionComplete_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc48a3c17_677a_4047_a68d_fc1257528aef);
pub const StructuredMarkup_Deleted_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf9d0a020_e1c1_4ecf_b9aa_52efde7e41e1);
pub const StructuredMarkup_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xabbd0878_8665_4f5c_94fc_36e7d8bb706b);
pub const StructuredMarkup_SelectionChanged_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa7c815f7_ff9f_41c7_a3a7_ab6cbfdb4903);
pub const StyleId_BulletedList: UIA_STYLE_ID = 70015u32;
pub const StyleId_BulletedList_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5963ed64_6426_4632_8caf_a32ad402d91a);
pub const StyleId_Custom: UIA_STYLE_ID = 70000u32;
pub const StyleId_Custom_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xef2edd3e_a999_4b7c_a378_09bbd52a3516);
pub const StyleId_Emphasis: UIA_STYLE_ID = 70013u32;
pub const StyleId_Emphasis_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xca6e7dbe_355e_4820_95a0_925f041d3470);
pub const StyleId_Heading1: UIA_STYLE_ID = 70001u32;
pub const StyleId_Heading1_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7f7e8f69_6866_4621_930c_9a5d0ca5961c);
pub const StyleId_Heading2: UIA_STYLE_ID = 70002u32;
pub const StyleId_Heading2_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbaa9b241_5c69_469d_85ad_474737b52b14);
pub const StyleId_Heading3: UIA_STYLE_ID = 70003u32;
pub const StyleId_Heading3_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbf8be9d2_d8b8_4ec5_8c52_9cfb0d035970);
pub const StyleId_Heading4: UIA_STYLE_ID = 70004u32;
pub const StyleId_Heading4_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8436ffc0_9578_45fc_83a4_ff40053315dd);
pub const StyleId_Heading5: UIA_STYLE_ID = 70005u32;
pub const StyleId_Heading5_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x909f424d_0dbf_406e_97bb_4e773d9798f7);
pub const StyleId_Heading6: UIA_STYLE_ID = 70006u32;
pub const StyleId_Heading6_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x89d23459_5d5b_4824_a420_11d3ed82e40f);
pub const StyleId_Heading7: UIA_STYLE_ID = 70007u32;
pub const StyleId_Heading7_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa3790473_e9ae_422d_b8e3_3b675c6181a4);
pub const StyleId_Heading8: UIA_STYLE_ID = 70008u32;
pub const StyleId_Heading8_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2bc14145_a40c_4881_84ae_f2235685380c);
pub const StyleId_Heading9: UIA_STYLE_ID = 70009u32;
pub const StyleId_Heading9_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc70d9133_bb2a_43d3_8ac6_33657884b0f0);
pub const StyleId_Normal: UIA_STYLE_ID = 70012u32;
pub const StyleId_Normal_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcd14d429_e45e_4475_a1c5_7f9e6be96eba);
pub const StyleId_NumberedList: UIA_STYLE_ID = 70016u32;
pub const StyleId_NumberedList_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1e96dbd5_64c3_43d0_b1ee_b53b06e3eddf);
pub const StyleId_Quote: UIA_STYLE_ID = 70014u32;
pub const StyleId_Quote_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5d1c21ea_8195_4f6c_87ea_5dabece64c1d);
pub const StyleId_Subtitle: UIA_STYLE_ID = 70011u32;
pub const StyleId_Subtitle_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb5d9fc17_5d6f_4420_b439_7cb19ad434e2);
pub const StyleId_Title: UIA_STYLE_ID = 70010u32;
pub const StyleId_Title_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x15d8201a_ffcf_481f_b0a1_30b63be98f07);
pub const Styles_ExtendedProperties_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf451cda0_ba0a_4681_b0b0_0dbdb53e58f3);
pub const Styles_FillColor_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x63eff97a_a1c5_4b1d_84eb_b765f2edd632);
pub const Styles_FillPatternColor_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x939a59fe_8fbd_4e75_a271_ac4595195163);
pub const Styles_FillPatternStyle_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x81cf651f_482b_4451_a30a_e1545e554fb8);
pub const Styles_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1ae62655_da72_4d60_a153_e5aa6988e3bf);
pub const Styles_Shape_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc71a23f8_778c_400d_8458_3b543e526984);
pub const Styles_StyleId_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xda82852f_3817_4233_82af_02279e72cc77);
pub const Styles_StyleName_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1c12b035_05d1_4f55_9e8e_1489f3ff550d);
pub const SupportedTextSelection_Multiple: SupportedTextSelection = 2i32;
pub const SupportedTextSelection_None: SupportedTextSelection = 0i32;
pub const SupportedTextSelection_Single: SupportedTextSelection = 1i32;
pub const SynchronizedInputType_KeyDown: SynchronizedInputType = 2i32;
pub const SynchronizedInputType_KeyUp: SynchronizedInputType = 1i32;
pub const SynchronizedInputType_LeftMouseDown: SynchronizedInputType = 8i32;
pub const SynchronizedInputType_LeftMouseUp: SynchronizedInputType = 4i32;
pub const SynchronizedInputType_RightMouseDown: SynchronizedInputType = 32i32;
pub const SynchronizedInputType_RightMouseUp: SynchronizedInputType = 16i32;
pub const SynchronizedInput_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x05c288a6_c47b_488b_b653_33977a551b8b);
pub const SystemAlert_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd271545d_7a3a_47a7_8474_81d29a2451c9);
pub const TabItem_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2c6a634f_921b_4e6e_b26e_08fcb0798f4c);
pub const Tab_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x38cd1f2d_337a_4bd2_a5e3_adb469e30bd3);
pub const TableItem_ColumnHeaderItems_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x967a56a3_74b6_431e_8de6_99c411031c58);
pub const TableItem_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdf1343bd_1888_4a29_a50c_b92e6de37f6f);
pub const TableItem_RowHeaderItems_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb3f853a0_0574_4cd8_bcd7_ed5923572d97);
pub const Table_ColumnHeaders_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xaff1d72b_968d_42b1_b459_150b299da664);
pub const Table_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x773bfa0e_5bc4_4deb_921b_de7b3206229e);
pub const Table_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc415218e_a028_461e_aa92_8f925cf79351);
pub const Table_RowHeaders_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd9e35b87_6eb8_4562_aac6_a8a9075236a8);
pub const Table_RowOrColumnMajor_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x83be75c3_29fe_4a30_85e1_2a6277fd106e);
pub const TextChild_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7533cab7_3bfe_41ef_9e85_e2638cbe169e);
pub const TextDecorationLineStyle_Dash: TextDecorationLineStyle = 5i32;
pub const TextDecorationLineStyle_DashDot: TextDecorationLineStyle = 6i32;
pub const TextDecorationLineStyle_DashDotDot: TextDecorationLineStyle = 7i32;
pub const TextDecorationLineStyle_Dot: TextDecorationLineStyle = 4i32;
pub const TextDecorationLineStyle_Double: TextDecorationLineStyle = 3i32;
pub const TextDecorationLineStyle_DoubleWavy: TextDecorationLineStyle = 11i32;
pub const TextDecorationLineStyle_LongDash: TextDecorationLineStyle = 13i32;
pub const TextDecorationLineStyle_None: TextDecorationLineStyle = 0i32;
pub const TextDecorationLineStyle_Other: TextDecorationLineStyle = -1i32;
pub const TextDecorationLineStyle_Single: TextDecorationLineStyle = 1i32;
pub const TextDecorationLineStyle_ThickDash: TextDecorationLineStyle = 14i32;
pub const TextDecorationLineStyle_ThickDashDot: TextDecorationLineStyle = 15i32;
pub const TextDecorationLineStyle_ThickDashDotDot: TextDecorationLineStyle = 16i32;
pub const TextDecorationLineStyle_ThickDot: TextDecorationLineStyle = 17i32;
pub const TextDecorationLineStyle_ThickLongDash: TextDecorationLineStyle = 18i32;
pub const TextDecorationLineStyle_ThickSingle: TextDecorationLineStyle = 9i32;
pub const TextDecorationLineStyle_ThickWavy: TextDecorationLineStyle = 12i32;
pub const TextDecorationLineStyle_Wavy: TextDecorationLineStyle = 8i32;
pub const TextDecorationLineStyle_WordsOnly: TextDecorationLineStyle = 2i32;
pub const TextEditChangeType_AutoComplete: TextEditChangeType = 4i32;
pub const TextEditChangeType_AutoCorrect: TextEditChangeType = 1i32;
pub const TextEditChangeType_Composition: TextEditChangeType = 2i32;
pub const TextEditChangeType_CompositionFinalized: TextEditChangeType = 3i32;
pub const TextEditChangeType_None: TextEditChangeType = 0i32;
pub const TextEdit_ConversionTargetChanged_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3388c183_ed4f_4c8b_9baa_364d51d8847f);
pub const TextEdit_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x69f3ff89_5af9_4c75_9340_f2de292e4591);
pub const TextEdit_TextChanged_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x120b0308_ec22_4eb8_9c98_9867cda1b165);
pub const TextPatternRangeEndpoint_End: TextPatternRangeEndpoint = 1i32;
pub const TextPatternRangeEndpoint_Start: TextPatternRangeEndpoint = 0i32;
pub const TextUnit_Character: TextUnit = 0i32;
pub const TextUnit_Document: TextUnit = 6i32;
pub const TextUnit_Format: TextUnit = 1i32;
pub const TextUnit_Line: TextUnit = 3i32;
pub const TextUnit_Page: TextUnit = 5i32;
pub const TextUnit_Paragraph: TextUnit = 4i32;
pub const TextUnit_Word: TextUnit = 2i32;
pub const Text_AfterParagraphSpacing_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x588cbb38_e62f_497c_b5d1_ccdf0ee823d8);
pub const Text_AfterSpacing_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x588cbb38_e62f_497c_b5d1_ccdf0ee823d8);
pub const Text_AnimationStyle_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x628209f0_7c9a_4d57_be64_1f1836571ff5);
pub const Text_AnnotationObjects_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xff41cf68_e7ab_40b9_8c72_72a8ed94017d);
pub const Text_AnnotationTypes_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xad2eb431_ee4e_4be1_a7ba_5559155a73ef);
pub const Text_BackgroundColor_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfdc49a07_583d_4f17_ad27_77fc832a3c0b);
pub const Text_BeforeParagraphSpacing_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbe7b0ab1_c822_4a24_85e9_c8f2650fc79c);
pub const Text_BeforeSpacing_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbe7b0ab1_c822_4a24_85e9_c8f2650fc79c);
pub const Text_BulletStyle_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc1097c90_d5c4_4237_9781_3bec8ba54e48);
pub const Text_CapStyle_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfb059c50_92cc_49a5_ba8f_0aa872bba2f3);
pub const Text_CaretBidiMode_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x929ee7a6_51d3_4715_96dc_b694fa24a168);
pub const Text_CaretPosition_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb227b131_9889_4752_a91b_733efdc5c5a0);
pub const Text_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xae9772dc_d331_4f09_be20_7e6dfaf07b0a);
pub const Text_Culture_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc2025af9_a42d_4ced_a1fb_c6746315222e);
pub const Text_FontName_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x64e63ba8_f2e5_476e_a477_1734feaaf726);
pub const Text_FontSize_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdc5eeeff_0506_4673_93f2_377e4a8e01f1);
pub const Text_FontWeight_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6fc02359_b316_4f5f_b401_f1ce55741853);
pub const Text_ForegroundColor_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x72d1c95d_5e60_471a_96b1_6c1b3b77a436);
pub const Text_HorizontalTextAlignment_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x04ea6161_fba3_477a_952a_bb326d026a5b);
pub const Text_IndentationFirstLine_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x206f9ad5_c1d3_424a_8182_6da9a7f3d632);
pub const Text_IndentationLeading_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5cf66bac_2d45_4a4b_b6c9_f7221d2815b0);
pub const Text_IndentationTrailing_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x97ff6c0f_1ce4_408a_b67b_94d83eb69bf2);
pub const Text_IsActive_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf5a4e533_e1b8_436b_935d_b57aa3f558c4);
pub const Text_IsHidden_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x360182fb_bdd7_47f6_ab69_19e33f8a3344);
pub const Text_IsItalic_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfce12a56_1336_4a34_9663_1bab47239320);
pub const Text_IsReadOnly_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa738156b_ca3e_495e_9514_833c440feb11);
pub const Text_IsSubscript_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf0ead858_8f53_413c_873f_1a7d7f5e0de4);
pub const Text_IsSuperscript_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xda706ee4_b3aa_4645_a41f_cd25157dea76);
pub const Text_LineSpacing_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x63ff70ae_d943_4b47_8ab7_a7a033d3214b);
pub const Text_Link_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb38ef51d_9e8d_4e46_9144_56ebe177329b);
pub const Text_MarginBottom_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7ee593c4_72b4_4cac_9271_3ed24b0e4d42);
pub const Text_MarginLeading_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9e9242d0_5ed0_4900_8e8a_eecc03835afc);
pub const Text_MarginTop_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x683d936f_c9b9_4a9a_b3d9_d20d33311e2a);
pub const Text_MarginTrailing_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xaf522f98_999d_40af_a5b2_0169d0342002);
pub const Text_OutlineStyles_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5b675b27_db89_46fe_970c_614d523bb97d);
pub const Text_OverlineColor_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x83ab383a_fd43_40da_ab3e_ecf8165cbb6d);
pub const Text_OverlineStyle_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0a234d66_617e_427f_871d_e1ff1e0c213f);
pub const Text_Pattern2_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x498479a2_5b22_448d_b6e4_647490860698);
pub const Text_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8615f05d_7de5_44fd_a679_2ca4b46033a8);
pub const Text_SayAsInterpretAs_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb38ad6ac_eee1_4b6e_88cc_014cefa93fcb);
pub const Text_SelectionActiveEnd_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1f668cc3_9bbf_416b_b0a2_f89f86f6612c);
pub const Text_StrikethroughColor_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbfe15a18_8c41_4c5a_9a0b_04af0e07f487);
pub const Text_StrikethroughStyle_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x72913ef1_da00_4f01_899c_ac5a8577a307);
pub const Text_StyleId_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x14c300de_c32b_449b_ab7c_b0e0789aea5d);
pub const Text_StyleName_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x22c9e091_4d66_45d8_a828_737bab4c98a7);
pub const Text_Tabs_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2e68d00b_92fe_42d8_899a_a784aa4454a1);
pub const Text_TextChangedEvent_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4a342082_f483_48c4_ac11_a84b435e2a84);
pub const Text_TextFlowDirections_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8bdf8739_f420_423e_af77_20a5d973a907);
pub const Text_TextSelectionChangedEvent_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x918edaa1_71b3_49ae_9741_79beb8d358f3);
pub const Text_UnderlineColor_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbfa12c73_fde2_4473_bf64_1036d6aa0f45);
pub const Text_UnderlineStyle_Attribute_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5f3b21c0_ede4_44bd_9c36_3853038cbfeb);
pub const Thumb_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x701ca877_e310_4dd6_b644_797e4faea213);
pub const TitleBar_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x98aa55bf_3bb0_4b65_836e_2ea30dbc171f);
pub const ToggleState_Indeterminate: ToggleState = 2i32;
pub const ToggleState_Off: ToggleState = 0i32;
pub const ToggleState_On: ToggleState = 1i32;
pub const Toggle_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0b419760_e2f4_43ff_8c5f_9457c82b56e9);
pub const Toggle_ToggleState_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb23cdc52_22c2_4c6c_9ded_f5c422479ede);
pub const ToolBar_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8f06b751_e182_4e98_8893_2284543a7dce);
pub const ToolTipClosed_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x276d71ef_24a9_49b6_8e97_da98b401bbcd);
pub const ToolTipOpened_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3f4b97ff_2edc_451d_bca4_95a3188d5b03);
pub const ToolTip_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x05ddc6d1_2137_4768_98ea_73f52f7134f3);
pub const Tranform_Pattern2_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8afcfd07_a369_44de_988b_2f7ff49fb8a8);
pub const Transform2_CanZoom_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf357e890_a756_4359_9ca6_86702bf8f381);
pub const Transform2_ZoomLevel_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xeee29f1a_f4a2_4b5b_ac65_95cf93283387);
pub const Transform2_ZoomMaximum_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x42ab6b77_ceb0_4eca_b82a_6cfa5fa1fc08);
pub const Transform2_ZoomMinimum_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x742ccc16_4ad1_4e07_96fe_b122c6e6b22b);
pub const Transform_CanMove_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1b75824d_208b_4fdf_bccd_f1f4e5741f4f);
pub const Transform_CanResize_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbb98dca5_4c1a_41d4_a4f6_ebc128644180);
pub const Transform_CanRotate_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x10079b48_3849_476f_ac96_44a95c8440d9);
pub const Transform_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x24b46fdb_587e_49f1_9c4a_d8e98b664b7b);
pub const TreeItem_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x62c9feb9_8ffc_4878_a3a4_96b030315c18);
pub const TreeScope_Ancestors: TreeScope = 16i32;
pub const TreeScope_Children: TreeScope = 2i32;
pub const TreeScope_Descendants: TreeScope = 4i32;
pub const TreeScope_Element: TreeScope = 1i32;
pub const TreeScope_None: TreeScope = 0i32;
pub const TreeScope_Parent: TreeScope = 8i32;
pub const TreeScope_Subtree: TreeScope = 7i32;
pub const TreeTraversalOptions_Default: TreeTraversalOptions = 0i32;
pub const TreeTraversalOptions_LastToFirstOrder: TreeTraversalOptions = 2i32;
pub const TreeTraversalOptions_PostOrder: TreeTraversalOptions = 1i32;
pub const Tree_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7561349c_d241_43f4_9908_b5f091bee611);
pub const UIA_AcceleratorKeyPropertyId: UIA_PROPERTY_ID = 30006u32;
pub const UIA_AccessKeyPropertyId: UIA_PROPERTY_ID = 30007u32;
pub const UIA_ActiveTextPositionChangedEventId: UIA_EVENT_ID = 20036i32;
pub const UIA_AfterParagraphSpacingAttributeId: UIA_TEXTATTRIBUTE_ID = 40042u32;
pub const UIA_AnimationStyleAttributeId: UIA_TEXTATTRIBUTE_ID = 40000u32;
pub const UIA_AnnotationAnnotationTypeIdPropertyId: UIA_PROPERTY_ID = 30113u32;
pub const UIA_AnnotationAnnotationTypeNamePropertyId: UIA_PROPERTY_ID = 30114u32;
pub const UIA_AnnotationAuthorPropertyId: UIA_PROPERTY_ID = 30115u32;
pub const UIA_AnnotationDateTimePropertyId: UIA_PROPERTY_ID = 30116u32;
pub const UIA_AnnotationObjectsAttributeId: UIA_TEXTATTRIBUTE_ID = 40032u32;
pub const UIA_AnnotationObjectsPropertyId: UIA_PROPERTY_ID = 30156u32;
pub const UIA_AnnotationPatternId: UIA_PATTERN_ID = 10023u32;
pub const UIA_AnnotationTargetPropertyId: UIA_PROPERTY_ID = 30117u32;
pub const UIA_AnnotationTypesAttributeId: UIA_TEXTATTRIBUTE_ID = 40031u32;
pub const UIA_AnnotationTypesPropertyId: UIA_PROPERTY_ID = 30155u32;
pub const UIA_AppBarControlTypeId: UIA_CONTROLTYPE_ID = 50040u32;
pub const UIA_AriaPropertiesPropertyId: UIA_PROPERTY_ID = 30102u32;
pub const UIA_AriaRolePropertyId: UIA_PROPERTY_ID = 30101u32;
pub const UIA_AsyncContentLoadedEventId: UIA_EVENT_ID = 20006i32;
pub const UIA_AutomationFocusChangedEventId: UIA_EVENT_ID = 20005i32;
pub const UIA_AutomationIdPropertyId: UIA_PROPERTY_ID = 30011u32;
pub const UIA_AutomationPropertyChangedEventId: UIA_EVENT_ID = 20004i32;
pub const UIA_BackgroundColorAttributeId: UIA_TEXTATTRIBUTE_ID = 40001u32;
pub const UIA_BeforeParagraphSpacingAttributeId: UIA_TEXTATTRIBUTE_ID = 40041u32;
pub const UIA_BoundingRectanglePropertyId: UIA_PROPERTY_ID = 30001u32;
pub const UIA_BulletStyleAttributeId: UIA_TEXTATTRIBUTE_ID = 40002u32;
pub const UIA_ButtonControlTypeId: UIA_CONTROLTYPE_ID = 50000u32;
pub const UIA_CalendarControlTypeId: UIA_CONTROLTYPE_ID = 50001u32;
pub const UIA_CapStyleAttributeId: UIA_TEXTATTRIBUTE_ID = 40003u32;
pub const UIA_CaretBidiModeAttributeId: UIA_TEXTATTRIBUTE_ID = 40039u32;
pub const UIA_CaretPositionAttributeId: UIA_TEXTATTRIBUTE_ID = 40038u32;
pub const UIA_CenterPointPropertyId: UIA_PROPERTY_ID = 30165u32;
pub const UIA_ChangesEventId: UIA_EVENT_ID = 20034i32;
pub const UIA_CheckBoxControlTypeId: UIA_CONTROLTYPE_ID = 50002u32;
pub const UIA_ClassNamePropertyId: UIA_PROPERTY_ID = 30012u32;
pub const UIA_ClickablePointPropertyId: UIA_PROPERTY_ID = 30014u32;
pub const UIA_ComboBoxControlTypeId: UIA_CONTROLTYPE_ID = 50003u32;
pub const UIA_ControlTypePropertyId: UIA_PROPERTY_ID = 30003u32;
pub const UIA_ControllerForPropertyId: UIA_PROPERTY_ID = 30104u32;
pub const UIA_CultureAttributeId: UIA_TEXTATTRIBUTE_ID = 40004u32;
pub const UIA_CulturePropertyId: UIA_PROPERTY_ID = 30015u32;
pub const UIA_CustomControlTypeId: UIA_CONTROLTYPE_ID = 50025u32;
pub const UIA_CustomLandmarkTypeId: UIA_LANDMARKTYPE_ID = 80000u32;
pub const UIA_CustomNavigationPatternId: UIA_PATTERN_ID = 10033u32;
pub const UIA_DataGridControlTypeId: UIA_CONTROLTYPE_ID = 50028u32;
pub const UIA_DataItemControlTypeId: UIA_CONTROLTYPE_ID = 50029u32;
pub const UIA_DescribedByPropertyId: UIA_PROPERTY_ID = 30105u32;
pub const UIA_DockDockPositionPropertyId: UIA_PROPERTY_ID = 30069u32;
pub const UIA_DockPatternId: UIA_PATTERN_ID = 10011u32;
pub const UIA_DocumentControlTypeId: UIA_CONTROLTYPE_ID = 50030u32;
pub const UIA_DragDropEffectPropertyId: UIA_PROPERTY_ID = 30139u32;
pub const UIA_DragDropEffectsPropertyId: UIA_PROPERTY_ID = 30140u32;
pub const UIA_DragGrabbedItemsPropertyId: UIA_PROPERTY_ID = 30144u32;
pub const UIA_DragIsGrabbedPropertyId: UIA_PROPERTY_ID = 30138u32;
pub const UIA_DragPatternId: UIA_PATTERN_ID = 10030u32;
pub const UIA_Drag_DragCancelEventId: UIA_EVENT_ID = 20027i32;
pub const UIA_Drag_DragCompleteEventId: UIA_EVENT_ID = 20028i32;
pub const UIA_Drag_DragStartEventId: UIA_EVENT_ID = 20026i32;
pub const UIA_DropTargetDropTargetEffectPropertyId: UIA_PROPERTY_ID = 30142u32;
pub const UIA_DropTargetDropTargetEffectsPropertyId: UIA_PROPERTY_ID = 30143u32;
pub const UIA_DropTargetPatternId: UIA_PATTERN_ID = 10031u32;
pub const UIA_DropTarget_DragEnterEventId: UIA_EVENT_ID = 20029i32;
pub const UIA_DropTarget_DragLeaveEventId: UIA_EVENT_ID = 20030i32;
pub const UIA_DropTarget_DroppedEventId: UIA_EVENT_ID = 20031i32;
pub const UIA_E_ELEMENTNOTAVAILABLE: u32 = 2147746305u32;
pub const UIA_E_ELEMENTNOTENABLED: u32 = 2147746304u32;
pub const UIA_E_INVALIDOPERATION: u32 = 2148734217u32;
pub const UIA_E_NOCLICKABLEPOINT: u32 = 2147746306u32;
pub const UIA_E_NOTSUPPORTED: u32 = 2147746308u32;
pub const UIA_E_PROXYASSEMBLYNOTLOADED: u32 = 2147746307u32;
pub const UIA_E_TIMEOUT: u32 = 2148734213u32;
pub const UIA_EditControlTypeId: UIA_CONTROLTYPE_ID = 50004u32;
pub const UIA_ExpandCollapseExpandCollapseStatePropertyId: UIA_PROPERTY_ID = 30070u32;
pub const UIA_ExpandCollapsePatternId: UIA_PATTERN_ID = 10005u32;
pub const UIA_FillColorPropertyId: UIA_PROPERTY_ID = 30160u32;
pub const UIA_FillTypePropertyId: UIA_PROPERTY_ID = 30162u32;
pub const UIA_FlowsFromPropertyId: UIA_PROPERTY_ID = 30148u32;
pub const UIA_FlowsToPropertyId: UIA_PROPERTY_ID = 30106u32;
pub const UIA_FontNameAttributeId: UIA_TEXTATTRIBUTE_ID = 40005u32;
pub const UIA_FontSizeAttributeId: UIA_TEXTATTRIBUTE_ID = 40006u32;
pub const UIA_FontWeightAttributeId: UIA_TEXTATTRIBUTE_ID = 40007u32;
pub const UIA_ForegroundColorAttributeId: UIA_TEXTATTRIBUTE_ID = 40008u32;
pub const UIA_FormLandmarkTypeId: UIA_LANDMARKTYPE_ID = 80001u32;
pub const UIA_FrameworkIdPropertyId: UIA_PROPERTY_ID = 30024u32;
pub const UIA_FullDescriptionPropertyId: UIA_PROPERTY_ID = 30159u32;
pub const UIA_GridColumnCountPropertyId: UIA_PROPERTY_ID = 30063u32;
pub const UIA_GridItemColumnPropertyId: UIA_PROPERTY_ID = 30065u32;
pub const UIA_GridItemColumnSpanPropertyId: UIA_PROPERTY_ID = 30067u32;
pub const UIA_GridItemContainingGridPropertyId: UIA_PROPERTY_ID = 30068u32;
pub const UIA_GridItemPatternId: UIA_PATTERN_ID = 10007u32;
pub const UIA_GridItemRowPropertyId: UIA_PROPERTY_ID = 30064u32;
pub const UIA_GridItemRowSpanPropertyId: UIA_PROPERTY_ID = 30066u32;
pub const UIA_GridPatternId: UIA_PATTERN_ID = 10006u32;
pub const UIA_GridRowCountPropertyId: UIA_PROPERTY_ID = 30062u32;
pub const UIA_GroupControlTypeId: UIA_CONTROLTYPE_ID = 50026u32;
pub const UIA_HasKeyboardFocusPropertyId: UIA_PROPERTY_ID = 30008u32;
pub const UIA_HeaderControlTypeId: UIA_CONTROLTYPE_ID = 50034u32;
pub const UIA_HeaderItemControlTypeId: UIA_CONTROLTYPE_ID = 50035u32;
pub const UIA_HeadingLevelPropertyId: UIA_PROPERTY_ID = 30173u32;
pub const UIA_HelpTextPropertyId: UIA_PROPERTY_ID = 30013u32;
pub const UIA_HorizontalTextAlignmentAttributeId: UIA_TEXTATTRIBUTE_ID = 40009u32;
pub const UIA_HostedFragmentRootsInvalidatedEventId: UIA_EVENT_ID = 20025i32;
pub const UIA_HyperlinkControlTypeId: UIA_CONTROLTYPE_ID = 50005u32;
pub const UIA_IAFP_DEFAULT: u32 = 0u32;
pub const UIA_IAFP_UNWRAP_BRIDGE: u32 = 1u32;
pub const UIA_ImageControlTypeId: UIA_CONTROLTYPE_ID = 50006u32;
pub const UIA_IndentationFirstLineAttributeId: UIA_TEXTATTRIBUTE_ID = 40010u32;
pub const UIA_IndentationLeadingAttributeId: UIA_TEXTATTRIBUTE_ID = 40011u32;
pub const UIA_IndentationTrailingAttributeId: UIA_TEXTATTRIBUTE_ID = 40012u32;
pub const UIA_InputDiscardedEventId: UIA_EVENT_ID = 20022i32;
pub const UIA_InputReachedOtherElementEventId: UIA_EVENT_ID = 20021i32;
pub const UIA_InputReachedTargetEventId: UIA_EVENT_ID = 20020i32;
pub const UIA_InvokePatternId: UIA_PATTERN_ID = 10000u32;
pub const UIA_Invoke_InvokedEventId: UIA_EVENT_ID = 20009i32;
pub const UIA_IsActiveAttributeId: UIA_TEXTATTRIBUTE_ID = 40036u32;
pub const UIA_IsAnnotationPatternAvailablePropertyId: UIA_PROPERTY_ID = 30118u32;
pub const UIA_IsContentElementPropertyId: UIA_PROPERTY_ID = 30017u32;
pub const UIA_IsControlElementPropertyId: UIA_PROPERTY_ID = 30016u32;
pub const UIA_IsCustomNavigationPatternAvailablePropertyId: UIA_PROPERTY_ID = 30151u32;
pub const UIA_IsDataValidForFormPropertyId: UIA_PROPERTY_ID = 30103u32;
pub const UIA_IsDialogPropertyId: UIA_PROPERTY_ID = 30174u32;
pub const UIA_IsDockPatternAvailablePropertyId: UIA_PROPERTY_ID = 30027u32;
pub const UIA_IsDragPatternAvailablePropertyId: UIA_PROPERTY_ID = 30137u32;
pub const UIA_IsDropTargetPatternAvailablePropertyId: UIA_PROPERTY_ID = 30141u32;
pub const UIA_IsEnabledPropertyId: UIA_PROPERTY_ID = 30010u32;
pub const UIA_IsExpandCollapsePatternAvailablePropertyId: UIA_PROPERTY_ID = 30028u32;
pub const UIA_IsGridItemPatternAvailablePropertyId: UIA_PROPERTY_ID = 30029u32;
pub const UIA_IsGridPatternAvailablePropertyId: UIA_PROPERTY_ID = 30030u32;
pub const UIA_IsHiddenAttributeId: UIA_TEXTATTRIBUTE_ID = 40013u32;
pub const UIA_IsInvokePatternAvailablePropertyId: UIA_PROPERTY_ID = 30031u32;
pub const UIA_IsItalicAttributeId: UIA_TEXTATTRIBUTE_ID = 40014u32;
pub const UIA_IsItemContainerPatternAvailablePropertyId: UIA_PROPERTY_ID = 30108u32;
pub const UIA_IsKeyboardFocusablePropertyId: UIA_PROPERTY_ID = 30009u32;
pub const UIA_IsLegacyIAccessiblePatternAvailablePropertyId: UIA_PROPERTY_ID = 30090u32;
pub const UIA_IsMultipleViewPatternAvailablePropertyId: UIA_PROPERTY_ID = 30032u32;
pub const UIA_IsObjectModelPatternAvailablePropertyId: UIA_PROPERTY_ID = 30112u32;
pub const UIA_IsOffscreenPropertyId: UIA_PROPERTY_ID = 30022u32;
pub const UIA_IsPasswordPropertyId: UIA_PROPERTY_ID = 30019u32;
pub const UIA_IsPeripheralPropertyId: UIA_PROPERTY_ID = 30150u32;
pub const UIA_IsRangeValuePatternAvailablePropertyId: UIA_PROPERTY_ID = 30033u32;
pub const UIA_IsReadOnlyAttributeId: UIA_TEXTATTRIBUTE_ID = 40015u32;
pub const UIA_IsRequiredForFormPropertyId: UIA_PROPERTY_ID = 30025u32;
pub const UIA_IsScrollItemPatternAvailablePropertyId: UIA_PROPERTY_ID = 30035u32;
pub const UIA_IsScrollPatternAvailablePropertyId: UIA_PROPERTY_ID = 30034u32;
pub const UIA_IsSelectionItemPatternAvailablePropertyId: UIA_PROPERTY_ID = 30036u32;
pub const UIA_IsSelectionPattern2AvailablePropertyId: UIA_PROPERTY_ID = 30168u32;
pub const UIA_IsSelectionPatternAvailablePropertyId: UIA_PROPERTY_ID = 30037u32;
pub const UIA_IsSpreadsheetItemPatternAvailablePropertyId: UIA_PROPERTY_ID = 30132u32;
pub const UIA_IsSpreadsheetPatternAvailablePropertyId: UIA_PROPERTY_ID = 30128u32;
pub const UIA_IsStylesPatternAvailablePropertyId: UIA_PROPERTY_ID = 30127u32;
pub const UIA_IsSubscriptAttributeId: UIA_TEXTATTRIBUTE_ID = 40016u32;
pub const UIA_IsSuperscriptAttributeId: UIA_TEXTATTRIBUTE_ID = 40017u32;
pub const UIA_IsSynchronizedInputPatternAvailablePropertyId: UIA_PROPERTY_ID = 30110u32;
pub const UIA_IsTableItemPatternAvailablePropertyId: UIA_PROPERTY_ID = 30039u32;
pub const UIA_IsTablePatternAvailablePropertyId: UIA_PROPERTY_ID = 30038u32;
pub const UIA_IsTextChildPatternAvailablePropertyId: UIA_PROPERTY_ID = 30136u32;
pub const UIA_IsTextEditPatternAvailablePropertyId: UIA_PROPERTY_ID = 30149u32;
pub const UIA_IsTextPattern2AvailablePropertyId: UIA_PROPERTY_ID = 30119u32;
pub const UIA_IsTextPatternAvailablePropertyId: UIA_PROPERTY_ID = 30040u32;
pub const UIA_IsTogglePatternAvailablePropertyId: UIA_PROPERTY_ID = 30041u32;
pub const UIA_IsTransformPattern2AvailablePropertyId: UIA_PROPERTY_ID = 30134u32;
pub const UIA_IsTransformPatternAvailablePropertyId: UIA_PROPERTY_ID = 30042u32;
pub const UIA_IsValuePatternAvailablePropertyId: UIA_PROPERTY_ID = 30043u32;
pub const UIA_IsVirtualizedItemPatternAvailablePropertyId: UIA_PROPERTY_ID = 30109u32;
pub const UIA_IsWindowPatternAvailablePropertyId: UIA_PROPERTY_ID = 30044u32;
pub const UIA_ItemContainerPatternId: UIA_PATTERN_ID = 10019u32;
pub const UIA_ItemStatusPropertyId: UIA_PROPERTY_ID = 30026u32;
pub const UIA_ItemTypePropertyId: UIA_PROPERTY_ID = 30021u32;
pub const UIA_LabeledByPropertyId: UIA_PROPERTY_ID = 30018u32;
pub const UIA_LandmarkTypePropertyId: UIA_PROPERTY_ID = 30157u32;
pub const UIA_LayoutInvalidatedEventId: UIA_EVENT_ID = 20008i32;
pub const UIA_LegacyIAccessibleChildIdPropertyId: UIA_PROPERTY_ID = 30091u32;
pub const UIA_LegacyIAccessibleDefaultActionPropertyId: UIA_PROPERTY_ID = 30100u32;
pub const UIA_LegacyIAccessibleDescriptionPropertyId: UIA_PROPERTY_ID = 30094u32;
pub const UIA_LegacyIAccessibleHelpPropertyId: UIA_PROPERTY_ID = 30097u32;
pub const UIA_LegacyIAccessibleKeyboardShortcutPropertyId: UIA_PROPERTY_ID = 30098u32;
pub const UIA_LegacyIAccessibleNamePropertyId: UIA_PROPERTY_ID = 30092u32;
pub const UIA_LegacyIAccessiblePatternId: UIA_PATTERN_ID = 10018u32;
pub const UIA_LegacyIAccessibleRolePropertyId: UIA_PROPERTY_ID = 30095u32;
pub const UIA_LegacyIAccessibleSelectionPropertyId: UIA_PROPERTY_ID = 30099u32;
pub const UIA_LegacyIAccessibleStatePropertyId: UIA_PROPERTY_ID = 30096u32;
pub const UIA_LegacyIAccessibleValuePropertyId: UIA_PROPERTY_ID = 30093u32;
pub const UIA_LevelPropertyId: UIA_PROPERTY_ID = 30154u32;
pub const UIA_LineSpacingAttributeId: UIA_TEXTATTRIBUTE_ID = 40040u32;
pub const UIA_LinkAttributeId: UIA_TEXTATTRIBUTE_ID = 40035u32;
pub const UIA_ListControlTypeId: UIA_CONTROLTYPE_ID = 50008u32;
pub const UIA_ListItemControlTypeId: UIA_CONTROLTYPE_ID = 50007u32;
pub const UIA_LiveRegionChangedEventId: UIA_EVENT_ID = 20024i32;
pub const UIA_LiveSettingPropertyId: UIA_PROPERTY_ID = 30135u32;
pub const UIA_LocalizedControlTypePropertyId: UIA_PROPERTY_ID = 30004u32;
pub const UIA_LocalizedLandmarkTypePropertyId: UIA_PROPERTY_ID = 30158u32;
pub const UIA_MainLandmarkTypeId: UIA_LANDMARKTYPE_ID = 80002u32;
pub const UIA_MarginBottomAttributeId: UIA_TEXTATTRIBUTE_ID = 40018u32;
pub const UIA_MarginLeadingAttributeId: UIA_TEXTATTRIBUTE_ID = 40019u32;
pub const UIA_MarginTopAttributeId: UIA_TEXTATTRIBUTE_ID = 40020u32;
pub const UIA_MarginTrailingAttributeId: UIA_TEXTATTRIBUTE_ID = 40021u32;
pub const UIA_MenuBarControlTypeId: UIA_CONTROLTYPE_ID = 50010u32;
pub const UIA_MenuClosedEventId: UIA_EVENT_ID = 20007i32;
pub const UIA_MenuControlTypeId: UIA_CONTROLTYPE_ID = 50009u32;
pub const UIA_MenuItemControlTypeId: UIA_CONTROLTYPE_ID = 50011u32;
pub const UIA_MenuModeEndEventId: UIA_EVENT_ID = 20019i32;
pub const UIA_MenuModeStartEventId: UIA_EVENT_ID = 20018i32;
pub const UIA_MenuOpenedEventId: UIA_EVENT_ID = 20003i32;
pub const UIA_MultipleViewCurrentViewPropertyId: UIA_PROPERTY_ID = 30071u32;
pub const UIA_MultipleViewPatternId: UIA_PATTERN_ID = 10008u32;
pub const UIA_MultipleViewSupportedViewsPropertyId: UIA_PROPERTY_ID = 30072u32;
pub const UIA_NamePropertyId: UIA_PROPERTY_ID = 30005u32;
pub const UIA_NativeWindowHandlePropertyId: UIA_PROPERTY_ID = 30020u32;
pub const UIA_NavigationLandmarkTypeId: UIA_LANDMARKTYPE_ID = 80003u32;
pub const UIA_NotificationEventId: UIA_EVENT_ID = 20035i32;
pub const UIA_ObjectModelPatternId: UIA_PATTERN_ID = 10022u32;
pub const UIA_OptimizeForVisualContentPropertyId: UIA_PROPERTY_ID = 30111u32;
pub const UIA_OrientationPropertyId: UIA_PROPERTY_ID = 30023u32;
pub const UIA_OutlineColorPropertyId: UIA_PROPERTY_ID = 30161u32;
pub const UIA_OutlineStylesAttributeId: UIA_TEXTATTRIBUTE_ID = 40022u32;
pub const UIA_OutlineThicknessPropertyId: UIA_PROPERTY_ID = 30164u32;
pub const UIA_OverlineColorAttributeId: UIA_TEXTATTRIBUTE_ID = 40023u32;
pub const UIA_OverlineStyleAttributeId: UIA_TEXTATTRIBUTE_ID = 40024u32;
pub const UIA_PFIA_DEFAULT: u32 = 0u32;
pub const UIA_PFIA_UNWRAP_BRIDGE: u32 = 1u32;
pub const UIA_PaneControlTypeId: UIA_CONTROLTYPE_ID = 50033u32;
pub const UIA_PositionInSetPropertyId: UIA_PROPERTY_ID = 30152u32;
pub const UIA_ProcessIdPropertyId: UIA_PROPERTY_ID = 30002u32;
pub const UIA_ProgressBarControlTypeId: UIA_CONTROLTYPE_ID = 50012u32;
pub const UIA_ProviderDescriptionPropertyId: UIA_PROPERTY_ID = 30107u32;
pub const UIA_RadioButtonControlTypeId: UIA_CONTROLTYPE_ID = 50013u32;
pub const UIA_RangeValueIsReadOnlyPropertyId: UIA_PROPERTY_ID = 30048u32;
pub const UIA_RangeValueLargeChangePropertyId: UIA_PROPERTY_ID = 30051u32;
pub const UIA_RangeValueMaximumPropertyId: UIA_PROPERTY_ID = 30050u32;
pub const UIA_RangeValueMinimumPropertyId: UIA_PROPERTY_ID = 30049u32;
pub const UIA_RangeValuePatternId: UIA_PATTERN_ID = 10003u32;
pub const UIA_RangeValueSmallChangePropertyId: UIA_PROPERTY_ID = 30052u32;
pub const UIA_RangeValueValuePropertyId: UIA_PROPERTY_ID = 30047u32;
pub const UIA_RotationPropertyId: UIA_PROPERTY_ID = 30166u32;
pub const UIA_RuntimeIdPropertyId: UIA_PROPERTY_ID = 30000u32;
pub const UIA_SayAsInterpretAsAttributeId: UIA_TEXTATTRIBUTE_ID = 40043u32;
pub const UIA_SayAsInterpretAsMetadataId: UIA_METADATA_ID = 100000u32;
pub const UIA_ScrollBarControlTypeId: UIA_CONTROLTYPE_ID = 50014u32;
pub const UIA_ScrollHorizontalScrollPercentPropertyId: UIA_PROPERTY_ID = 30053u32;
pub const UIA_ScrollHorizontalViewSizePropertyId: UIA_PROPERTY_ID = 30054u32;
pub const UIA_ScrollHorizontallyScrollablePropertyId: UIA_PROPERTY_ID = 30057u32;
pub const UIA_ScrollItemPatternId: UIA_PATTERN_ID = 10017u32;
pub const UIA_ScrollPatternId: UIA_PATTERN_ID = 10004u32;
pub const UIA_ScrollPatternNoScroll: f64 = -1f64;
pub const UIA_ScrollVerticalScrollPercentPropertyId: UIA_PROPERTY_ID = 30055u32;
pub const UIA_ScrollVerticalViewSizePropertyId: UIA_PROPERTY_ID = 30056u32;
pub const UIA_ScrollVerticallyScrollablePropertyId: UIA_PROPERTY_ID = 30058u32;
pub const UIA_SearchLandmarkTypeId: UIA_LANDMARKTYPE_ID = 80004u32;
pub const UIA_Selection2CurrentSelectedItemPropertyId: UIA_PROPERTY_ID = 30171u32;
pub const UIA_Selection2FirstSelectedItemPropertyId: UIA_PROPERTY_ID = 30169u32;
pub const UIA_Selection2ItemCountPropertyId: UIA_PROPERTY_ID = 30172u32;
pub const UIA_Selection2LastSelectedItemPropertyId: UIA_PROPERTY_ID = 30170u32;
pub const UIA_SelectionActiveEndAttributeId: UIA_TEXTATTRIBUTE_ID = 40037u32;
pub const UIA_SelectionCanSelectMultiplePropertyId: UIA_PROPERTY_ID = 30060u32;
pub const UIA_SelectionIsSelectionRequiredPropertyId: UIA_PROPERTY_ID = 30061u32;
pub const UIA_SelectionItemIsSelectedPropertyId: UIA_PROPERTY_ID = 30079u32;
pub const UIA_SelectionItemPatternId: UIA_PATTERN_ID = 10010u32;
pub const UIA_SelectionItemSelectionContainerPropertyId: UIA_PROPERTY_ID = 30080u32;
pub const UIA_SelectionItem_ElementAddedToSelectionEventId: UIA_EVENT_ID = 20010i32;
pub const UIA_SelectionItem_ElementRemovedFromSelectionEventId: UIA_EVENT_ID = 20011i32;
pub const UIA_SelectionItem_ElementSelectedEventId: UIA_EVENT_ID = 20012i32;
pub const UIA_SelectionPattern2Id: UIA_PATTERN_ID = 10034u32;
pub const UIA_SelectionPatternId: UIA_PATTERN_ID = 10001u32;
pub const UIA_SelectionSelectionPropertyId: UIA_PROPERTY_ID = 30059u32;
pub const UIA_Selection_InvalidatedEventId: UIA_EVENT_ID = 20013i32;
pub const UIA_SemanticZoomControlTypeId: UIA_CONTROLTYPE_ID = 50039u32;
pub const UIA_SeparatorControlTypeId: UIA_CONTROLTYPE_ID = 50038u32;
pub const UIA_SizeOfSetPropertyId: UIA_PROPERTY_ID = 30153u32;
pub const UIA_SizePropertyId: UIA_PROPERTY_ID = 30167u32;
pub const UIA_SliderControlTypeId: UIA_CONTROLTYPE_ID = 50015u32;
pub const UIA_SpinnerControlTypeId: UIA_CONTROLTYPE_ID = 50016u32;
pub const UIA_SplitButtonControlTypeId: UIA_CONTROLTYPE_ID = 50031u32;
pub const UIA_SpreadsheetItemAnnotationObjectsPropertyId: UIA_PROPERTY_ID = 30130u32;
pub const UIA_SpreadsheetItemAnnotationTypesPropertyId: UIA_PROPERTY_ID = 30131u32;
pub const UIA_SpreadsheetItemFormulaPropertyId: UIA_PROPERTY_ID = 30129u32;
pub const UIA_SpreadsheetItemPatternId: UIA_PATTERN_ID = 10027u32;
pub const UIA_SpreadsheetPatternId: UIA_PATTERN_ID = 10026u32;
pub const UIA_StatusBarControlTypeId: UIA_CONTROLTYPE_ID = 50017u32;
pub const UIA_StrikethroughColorAttributeId: UIA_TEXTATTRIBUTE_ID = 40025u32;
pub const UIA_StrikethroughStyleAttributeId: UIA_TEXTATTRIBUTE_ID = 40026u32;
pub const UIA_StructureChangedEventId: UIA_EVENT_ID = 20002i32;
pub const UIA_StyleIdAttributeId: UIA_TEXTATTRIBUTE_ID = 40034u32;
pub const UIA_StyleNameAttributeId: UIA_TEXTATTRIBUTE_ID = 40033u32;
pub const UIA_StylesExtendedPropertiesPropertyId: UIA_PROPERTY_ID = 30126u32;
pub const UIA_StylesFillColorPropertyId: UIA_PROPERTY_ID = 30122u32;
pub const UIA_StylesFillPatternColorPropertyId: UIA_PROPERTY_ID = 30125u32;
pub const UIA_StylesFillPatternStylePropertyId: UIA_PROPERTY_ID = 30123u32;
pub const UIA_StylesPatternId: UIA_PATTERN_ID = 10025u32;
pub const UIA_StylesShapePropertyId: UIA_PROPERTY_ID = 30124u32;
pub const UIA_StylesStyleIdPropertyId: UIA_PROPERTY_ID = 30120u32;
pub const UIA_StylesStyleNamePropertyId: UIA_PROPERTY_ID = 30121u32;
pub const UIA_SummaryChangeId: UIA_CHANGE_ID = 90000u32;
pub const UIA_SynchronizedInputPatternId: UIA_PATTERN_ID = 10021u32;
pub const UIA_SystemAlertEventId: UIA_EVENT_ID = 20023i32;
pub const UIA_TabControlTypeId: UIA_CONTROLTYPE_ID = 50018u32;
pub const UIA_TabItemControlTypeId: UIA_CONTROLTYPE_ID = 50019u32;
pub const UIA_TableColumnHeadersPropertyId: UIA_PROPERTY_ID = 30082u32;
pub const UIA_TableControlTypeId: UIA_CONTROLTYPE_ID = 50036u32;
pub const UIA_TableItemColumnHeaderItemsPropertyId: UIA_PROPERTY_ID = 30085u32;
pub const UIA_TableItemPatternId: UIA_PATTERN_ID = 10013u32;
pub const UIA_TableItemRowHeaderItemsPropertyId: UIA_PROPERTY_ID = 30084u32;
pub const UIA_TablePatternId: UIA_PATTERN_ID = 10012u32;
pub const UIA_TableRowHeadersPropertyId: UIA_PROPERTY_ID = 30081u32;
pub const UIA_TableRowOrColumnMajorPropertyId: UIA_PROPERTY_ID = 30083u32;
pub const UIA_TabsAttributeId: UIA_TEXTATTRIBUTE_ID = 40027u32;
pub const UIA_TextChildPatternId: UIA_PATTERN_ID = 10029u32;
pub const UIA_TextControlTypeId: UIA_CONTROLTYPE_ID = 50020u32;
pub const UIA_TextEditPatternId: UIA_PATTERN_ID = 10032u32;
pub const UIA_TextEdit_ConversionTargetChangedEventId: UIA_EVENT_ID = 20033i32;
pub const UIA_TextEdit_TextChangedEventId: UIA_EVENT_ID = 20032i32;
pub const UIA_TextFlowDirectionsAttributeId: UIA_TEXTATTRIBUTE_ID = 40028u32;
pub const UIA_TextPattern2Id: UIA_PATTERN_ID = 10024u32;
pub const UIA_TextPatternId: UIA_PATTERN_ID = 10014u32;
pub const UIA_Text_TextChangedEventId: UIA_EVENT_ID = 20015i32;
pub const UIA_Text_TextSelectionChangedEventId: UIA_EVENT_ID = 20014i32;
pub const UIA_ThumbControlTypeId: UIA_CONTROLTYPE_ID = 50027u32;
pub const UIA_TitleBarControlTypeId: UIA_CONTROLTYPE_ID = 50037u32;
pub const UIA_TogglePatternId: UIA_PATTERN_ID = 10015u32;
pub const UIA_ToggleToggleStatePropertyId: UIA_PROPERTY_ID = 30086u32;
pub const UIA_ToolBarControlTypeId: UIA_CONTROLTYPE_ID = 50021u32;
pub const UIA_ToolTipClosedEventId: UIA_EVENT_ID = 20001i32;
pub const UIA_ToolTipControlTypeId: UIA_CONTROLTYPE_ID = 50022u32;
pub const UIA_ToolTipOpenedEventId: UIA_EVENT_ID = 20000i32;
pub const UIA_Transform2CanZoomPropertyId: UIA_PROPERTY_ID = 30133u32;
pub const UIA_Transform2ZoomLevelPropertyId: UIA_PROPERTY_ID = 30145u32;
pub const UIA_Transform2ZoomMaximumPropertyId: UIA_PROPERTY_ID = 30147u32;
pub const UIA_Transform2ZoomMinimumPropertyId: UIA_PROPERTY_ID = 30146u32;
pub const UIA_TransformCanMovePropertyId: UIA_PROPERTY_ID = 30087u32;
pub const UIA_TransformCanResizePropertyId: UIA_PROPERTY_ID = 30088u32;
pub const UIA_TransformCanRotatePropertyId: UIA_PROPERTY_ID = 30089u32;
pub const UIA_TransformPattern2Id: UIA_PATTERN_ID = 10028u32;
pub const UIA_TransformPatternId: UIA_PATTERN_ID = 10016u32;
pub const UIA_TreeControlTypeId: UIA_CONTROLTYPE_ID = 50023u32;
pub const UIA_TreeItemControlTypeId: UIA_CONTROLTYPE_ID = 50024u32;
pub const UIA_UnderlineColorAttributeId: UIA_TEXTATTRIBUTE_ID = 40029u32;
pub const UIA_UnderlineStyleAttributeId: UIA_TEXTATTRIBUTE_ID = 40030u32;
pub const UIA_ValueIsReadOnlyPropertyId: UIA_PROPERTY_ID = 30046u32;
pub const UIA_ValuePatternId: UIA_PATTERN_ID = 10002u32;
pub const UIA_ValueValuePropertyId: UIA_PROPERTY_ID = 30045u32;
pub const UIA_VirtualizedItemPatternId: UIA_PATTERN_ID = 10020u32;
pub const UIA_VisualEffectsPropertyId: UIA_PROPERTY_ID = 30163u32;
pub const UIA_WindowCanMaximizePropertyId: UIA_PROPERTY_ID = 30073u32;
pub const UIA_WindowCanMinimizePropertyId: UIA_PROPERTY_ID = 30074u32;
pub const UIA_WindowControlTypeId: UIA_CONTROLTYPE_ID = 50032u32;
pub const UIA_WindowIsModalPropertyId: UIA_PROPERTY_ID = 30077u32;
pub const UIA_WindowIsTopmostPropertyId: UIA_PROPERTY_ID = 30078u32;
pub const UIA_WindowPatternId: UIA_PATTERN_ID = 10009u32;
pub const UIA_WindowWindowInteractionStatePropertyId: UIA_PROPERTY_ID = 30076u32;
pub const UIA_WindowWindowVisualStatePropertyId: UIA_PROPERTY_ID = 30075u32;
pub const UIA_Window_WindowClosedEventId: UIA_EVENT_ID = 20017i32;
pub const UIA_Window_WindowOpenedEventId: UIA_EVENT_ID = 20016i32;
pub const UIAutomationType_Array: UIAutomationType = 65536i32;
pub const UIAutomationType_Bool: UIAutomationType = 2i32;
pub const UIAutomationType_BoolArray: UIAutomationType = 65538i32;
pub const UIAutomationType_Double: UIAutomationType = 4i32;
pub const UIAutomationType_DoubleArray: UIAutomationType = 65540i32;
pub const UIAutomationType_Element: UIAutomationType = 7i32;
pub const UIAutomationType_ElementArray: UIAutomationType = 65543i32;
pub const UIAutomationType_Int: UIAutomationType = 1i32;
pub const UIAutomationType_IntArray: UIAutomationType = 65537i32;
pub const UIAutomationType_Out: UIAutomationType = 131072i32;
pub const UIAutomationType_OutBool: UIAutomationType = 131074i32;
pub const UIAutomationType_OutBoolArray: UIAutomationType = 196610i32;
pub const UIAutomationType_OutDouble: UIAutomationType = 131076i32;
pub const UIAutomationType_OutDoubleArray: UIAutomationType = 196612i32;
pub const UIAutomationType_OutElement: UIAutomationType = 131079i32;
pub const UIAutomationType_OutElementArray: UIAutomationType = 196615i32;
pub const UIAutomationType_OutInt: UIAutomationType = 131073i32;
pub const UIAutomationType_OutIntArray: UIAutomationType = 196609i32;
pub const UIAutomationType_OutPoint: UIAutomationType = 131077i32;
pub const UIAutomationType_OutPointArray: UIAutomationType = 196613i32;
pub const UIAutomationType_OutRect: UIAutomationType = 131078i32;
pub const UIAutomationType_OutRectArray: UIAutomationType = 196614i32;
pub const UIAutomationType_OutString: UIAutomationType = 131075i32;
pub const UIAutomationType_OutStringArray: UIAutomationType = 196611i32;
pub const UIAutomationType_Point: UIAutomationType = 5i32;
pub const UIAutomationType_PointArray: UIAutomationType = 65541i32;
pub const UIAutomationType_Rect: UIAutomationType = 6i32;
pub const UIAutomationType_RectArray: UIAutomationType = 65542i32;
pub const UIAutomationType_String: UIAutomationType = 3i32;
pub const UIAutomationType_StringArray: UIAutomationType = 65539i32;
pub const UiaAppendRuntimeId: u32 = 3u32;
pub const UiaRootObjectId: i32 = -25i32;
pub const Value_IsReadOnly_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xeb090f30_e24c_4799_a705_0d247bc037f8);
pub const Value_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x17faad9e_c877_475b_b933_77332779b637);
pub const Value_Value_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe95f5e64_269f_4a85_ba99_4092c3ea2986);
pub const VirtualizedItem_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf510173e_2e71_45e9_a6e5_62f6ed8289d5);
pub const VisualEffects_Bevel: VisualEffects = 16i32;
pub const VisualEffects_Glow: VisualEffects = 4i32;
pub const VisualEffects_None: VisualEffects = 0i32;
pub const VisualEffects_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe61a8565_aad9_46d7_9e70_4e8a8420d420);
pub const VisualEffects_Reflection: VisualEffects = 2i32;
pub const VisualEffects_Shadow: VisualEffects = 1i32;
pub const VisualEffects_SoftEdges: VisualEffects = 8i32;
pub const WindowInteractionState_BlockedByModalWindow: WindowInteractionState = 3i32;
pub const WindowInteractionState_Closing: WindowInteractionState = 1i32;
pub const WindowInteractionState_NotResponding: WindowInteractionState = 4i32;
pub const WindowInteractionState_ReadyForUserInteraction: WindowInteractionState = 2i32;
pub const WindowInteractionState_Running: WindowInteractionState = 0i32;
pub const WindowVisualState_Maximized: WindowVisualState = 1i32;
pub const WindowVisualState_Minimized: WindowVisualState = 2i32;
pub const WindowVisualState_Normal: WindowVisualState = 0i32;
pub const Window_CanMaximize_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x64fff53f_635d_41c1_950c_cb5adfbe28e3);
pub const Window_CanMinimize_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb73b4625_5988_4b97_b4c2_a6fe6e78c8c6);
pub const Window_Control_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe13a7242_f462_4f4d_aec1_53b28d6c3290);
pub const Window_IsModal_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xff4e6892_37b9_4fca_8532_ffe674ecfeed);
pub const Window_IsTopmost_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xef7d85d3_0937_4962_9241_b62345f24041);
pub const Window_Pattern_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x27901735_c760_4994_ad11_5919e606b110);
pub const Window_WindowClosed_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xedf141f8_fa67_4e22_bbf7_944e05735ee2);
pub const Window_WindowInteractionState_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4fed26a4_0455_4fa2_b21c_c4da2db1ff9c);
pub const Window_WindowOpened_Event_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd3e81d06_de45_4f2f_9633_de9e02fb65af);
pub const Window_WindowVisualState_Property_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4ab7905f_e860_453e_a30a_f6431e5daad5);
pub const ZoomUnit_LargeDecrement: ZoomUnit = 1i32;
pub const ZoomUnit_LargeIncrement: ZoomUnit = 3i32;
pub const ZoomUnit_NoAmount: ZoomUnit = 0i32;
pub const ZoomUnit_SmallDecrement: ZoomUnit = 2i32;
pub const ZoomUnit_SmallIncrement: ZoomUnit = 4i32;
pub type ACC_UTILITY_STATE_FLAGS = u32;
pub type ActiveEnd = i32;
pub type AnimationStyle = i32;
pub type AnnoScope = i32;
pub type AsyncContentLoadedState = i32;
pub type AutomationElementMode = i32;
pub type AutomationIdentifierType = i32;
pub type BulletStyle = i32;
pub type CapStyle = i32;
pub type CaretBidiMode = i32;
pub type CaretPosition = i32;
pub type CoalesceEventsOptions = i32;
pub type ConditionType = i32;
pub type ConnectionRecoveryBehaviorOptions = i32;
pub type DockPosition = i32;
pub type EventArgsType = i32;
pub type ExpandCollapseState = i32;
pub type FillType = i32;
pub type FlowDirections = i32;
pub type HIGHCONTRASTW_FLAGS = u32;
pub type HorizontalTextAlignment = i32;
pub type LiveSetting = i32;
pub type NavigateDirection = i32;
pub type NormalizeState = i32;
pub type NotificationKind = i32;
pub type NotificationProcessing = i32;
pub type OrientationType = i32;
pub type OutlineStyles = i32;
pub type PropertyConditionFlags = i32;
pub type ProviderOptions = i32;
pub type ProviderType = i32;
pub type RowOrColumnMajor = i32;
pub type SERIALKEYS_FLAGS = u32;
pub type SOUNDSENTRY_FLAGS = u32;
pub type SOUNDSENTRY_TEXT_EFFECT = u32;
pub type SOUNDSENTRY_WINDOWS_EFFECT = u32;
pub type SOUND_SENTRY_GRAPHICS_EFFECT = u32;
pub type STICKYKEYS_FLAGS = u32;
pub type SayAsInterpretAs = i32;
pub type ScrollAmount = i32;
pub type StructureChangeType = i32;
pub type SupportedTextSelection = i32;
pub type SynchronizedInputType = i32;
pub type TextDecorationLineStyle = i32;
pub type TextEditChangeType = i32;
pub type TextPatternRangeEndpoint = i32;
pub type TextUnit = i32;
pub type ToggleState = i32;
pub type TreeScope = i32;
pub type TreeTraversalOptions = i32;
pub type UIA_ANNOTATIONTYPE = u32;
pub type UIA_CHANGE_ID = u32;
pub type UIA_CONTROLTYPE_ID = u32;
pub type UIA_EVENT_ID = i32;
pub type UIA_HEADINGLEVEL_ID = u32;
pub type UIA_LANDMARKTYPE_ID = u32;
pub type UIA_METADATA_ID = u32;
pub type UIA_PATTERN_ID = u32;
pub type UIA_PROPERTY_ID = u32;
pub type UIA_STYLE_ID = u32;
pub type UIA_TEXTATTRIBUTE_ID = u32;
pub type UIAutomationType = i32;
pub type VisualEffects = i32;
pub type WindowInteractionState = i32;
pub type WindowVisualState = i32;
pub type ZoomUnit = i32;
#[repr(C)]
pub struct ACCESSTIMEOUT {
    pub cbSize: u32,
    pub dwFlags: u32,
    pub iTimeOutMSec: u32,
}
impl ::core::marker::Copy for ACCESSTIMEOUT {}
impl ::core::clone::Clone for ACCESSTIMEOUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ExtendedProperty {
    pub PropertyName: ::windows_sys::core::BSTR,
    pub PropertyValue: ::windows_sys::core::BSTR,
}
impl ::core::marker::Copy for ExtendedProperty {}
impl ::core::clone::Clone for ExtendedProperty {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILTERKEYS {
    pub cbSize: u32,
    pub dwFlags: u32,
    pub iWaitMSec: u32,
    pub iDelayMSec: u32,
    pub iRepeatMSec: u32,
    pub iBounceMSec: u32,
}
impl ::core::marker::Copy for FILTERKEYS {}
impl ::core::clone::Clone for FILTERKEYS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct HIGHCONTRASTA {
    pub cbSize: u32,
    pub dwFlags: HIGHCONTRASTW_FLAGS,
    pub lpszDefaultScheme: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for HIGHCONTRASTA {}
impl ::core::clone::Clone for HIGHCONTRASTA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct HIGHCONTRASTW {
    pub cbSize: u32,
    pub dwFlags: HIGHCONTRASTW_FLAGS,
    pub lpszDefaultScheme: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for HIGHCONTRASTW {}
impl ::core::clone::Clone for HIGHCONTRASTW {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HUIAEVENT = isize;
pub type HUIANODE = isize;
pub type HUIAPATTERNOBJECT = isize;
pub type HUIATEXTRANGE = isize;
pub type HWINEVENTHOOK = isize;
#[repr(C)]
pub struct MOUSEKEYS {
    pub cbSize: u32,
    pub dwFlags: u32,
    pub iMaxSpeed: u32,
    pub iTimeToMaxSpeed: u32,
    pub iCtrlSpeed: u32,
    pub dwReserved1: u32,
    pub dwReserved2: u32,
}
impl ::core::marker::Copy for MOUSEKEYS {}
impl ::core::clone::Clone for MOUSEKEYS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MSAAMENUINFO {
    pub dwMSAASignature: u32,
    pub cchWText: u32,
    pub pszWText: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for MSAAMENUINFO {}
impl ::core::clone::Clone for MSAAMENUINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SERIALKEYSA {
    pub cbSize: u32,
    pub dwFlags: SERIALKEYS_FLAGS,
    pub lpszActivePort: ::windows_sys::core::PSTR,
    pub lpszPort: ::windows_sys::core::PSTR,
    pub iBaudRate: u32,
    pub iPortState: u32,
    pub iActive: u32,
}
impl ::core::marker::Copy for SERIALKEYSA {}
impl ::core::clone::Clone for SERIALKEYSA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SERIALKEYSW {
    pub cbSize: u32,
    pub dwFlags: SERIALKEYS_FLAGS,
    pub lpszActivePort: ::windows_sys::core::PWSTR,
    pub lpszPort: ::windows_sys::core::PWSTR,
    pub iBaudRate: u32,
    pub iPortState: u32,
    pub iActive: u32,
}
impl ::core::marker::Copy for SERIALKEYSW {}
impl ::core::clone::Clone for SERIALKEYSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SOUNDSENTRYA {
    pub cbSize: u32,
    pub dwFlags: SOUNDSENTRY_FLAGS,
    pub iFSTextEffect: SOUNDSENTRY_TEXT_EFFECT,
    pub iFSTextEffectMSec: u32,
    pub iFSTextEffectColorBits: u32,
    pub iFSGrafEffect: SOUND_SENTRY_GRAPHICS_EFFECT,
    pub iFSGrafEffectMSec: u32,
    pub iFSGrafEffectColor: u32,
    pub iWindowsEffect: SOUNDSENTRY_WINDOWS_EFFECT,
    pub iWindowsEffectMSec: u32,
    pub lpszWindowsEffectDLL: ::windows_sys::core::PSTR,
    pub iWindowsEffectOrdinal: u32,
}
impl ::core::marker::Copy for SOUNDSENTRYA {}
impl ::core::clone::Clone for SOUNDSENTRYA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SOUNDSENTRYW {
    pub cbSize: u32,
    pub dwFlags: SOUNDSENTRY_FLAGS,
    pub iFSTextEffect: SOUNDSENTRY_TEXT_EFFECT,
    pub iFSTextEffectMSec: u32,
    pub iFSTextEffectColorBits: u32,
    pub iFSGrafEffect: SOUND_SENTRY_GRAPHICS_EFFECT,
    pub iFSGrafEffectMSec: u32,
    pub iFSGrafEffectColor: u32,
    pub iWindowsEffect: SOUNDSENTRY_WINDOWS_EFFECT,
    pub iWindowsEffectMSec: u32,
    pub lpszWindowsEffectDLL: ::windows_sys::core::PWSTR,
    pub iWindowsEffectOrdinal: u32,
}
impl ::core::marker::Copy for SOUNDSENTRYW {}
impl ::core::clone::Clone for SOUNDSENTRYW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct STICKYKEYS {
    pub cbSize: u32,
    pub dwFlags: STICKYKEYS_FLAGS,
}
impl ::core::marker::Copy for STICKYKEYS {}
impl ::core::clone::Clone for STICKYKEYS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TOGGLEKEYS {
    pub cbSize: u32,
    pub dwFlags: u32,
}
impl ::core::marker::Copy for TOGGLEKEYS {}
impl ::core::clone::Clone for TOGGLEKEYS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UIAutomationEventInfo {
    pub guid: ::windows_sys::core::GUID,
    pub pProgrammaticName: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for UIAutomationEventInfo {}
impl ::core::clone::Clone for UIAutomationEventInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct UIAutomationMethodInfo {
    pub pProgrammaticName: ::windows_sys::core::PCWSTR,
    pub doSetFocus: super::super::Foundation::BOOL,
    pub cInParameters: u32,
    pub cOutParameters: u32,
    pub pParameterTypes: *mut UIAutomationType,
    pub pParameterNames: *const ::windows_sys::core::PCWSTR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for UIAutomationMethodInfo {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for UIAutomationMethodInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UIAutomationParameter {
    pub r#type: UIAutomationType,
    pub pData: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for UIAutomationParameter {}
impl ::core::clone::Clone for UIAutomationParameter {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct UIAutomationPatternInfo {
    pub guid: ::windows_sys::core::GUID,
    pub pProgrammaticName: ::windows_sys::core::PCWSTR,
    pub providerInterfaceId: ::windows_sys::core::GUID,
    pub clientInterfaceId: ::windows_sys::core::GUID,
    pub cProperties: u32,
    pub pProperties: *mut UIAutomationPropertyInfo,
    pub cMethods: u32,
    pub pMethods: *mut UIAutomationMethodInfo,
    pub cEvents: u32,
    pub pEvents: *mut UIAutomationEventInfo,
    pub pPatternHandler: IUIAutomationPatternHandler,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for UIAutomationPatternInfo {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for UIAutomationPatternInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UIAutomationPropertyInfo {
    pub guid: ::windows_sys::core::GUID,
    pub pProgrammaticName: ::windows_sys::core::PCWSTR,
    pub r#type: UIAutomationType,
}
impl ::core::marker::Copy for UIAutomationPropertyInfo {}
impl ::core::clone::Clone for UIAutomationPropertyInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UiaAndOrCondition {
    pub ConditionType: ConditionType,
    pub ppConditions: *mut *mut UiaCondition,
    pub cConditions: i32,
}
impl ::core::marker::Copy for UiaAndOrCondition {}
impl ::core::clone::Clone for UiaAndOrCondition {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UiaAsyncContentLoadedEventArgs {
    pub Type: EventArgsType,
    pub EventId: i32,
    pub AsyncContentLoadedState: AsyncContentLoadedState,
    pub PercentComplete: f64,
}
impl ::core::marker::Copy for UiaAsyncContentLoadedEventArgs {}
impl ::core::clone::Clone for UiaAsyncContentLoadedEventArgs {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UiaCacheRequest {
    pub pViewCondition: *mut UiaCondition,
    pub Scope: TreeScope,
    pub pProperties: *mut i32,
    pub cProperties: i32,
    pub pPatterns: *mut i32,
    pub cPatterns: i32,
    pub automationElementMode: AutomationElementMode,
}
impl ::core::marker::Copy for UiaCacheRequest {}
impl ::core::clone::Clone for UiaCacheRequest {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
pub struct UiaChangeInfo {
    pub uiaId: i32,
    pub payload: super::super::System::Variant::VARIANT,
    pub extraInfo: super::super::System::Variant::VARIANT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for UiaChangeInfo {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for UiaChangeInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
pub struct UiaChangesEventArgs {
    pub Type: EventArgsType,
    pub EventId: i32,
    pub EventIdCount: i32,
    pub pUiaChanges: *mut UiaChangeInfo,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for UiaChangesEventArgs {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for UiaChangesEventArgs {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UiaCondition {
    pub ConditionType: ConditionType,
}
impl ::core::marker::Copy for UiaCondition {}
impl ::core::clone::Clone for UiaCondition {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UiaEventArgs {
    pub Type: EventArgsType,
    pub EventId: i32,
}
impl ::core::marker::Copy for UiaEventArgs {}
impl ::core::clone::Clone for UiaEventArgs {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct UiaFindParams {
    pub MaxDepth: i32,
    pub FindFirst: super::super::Foundation::BOOL,
    pub ExcludeRoot: super::super::Foundation::BOOL,
    pub pFindCondition: *mut UiaCondition,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for UiaFindParams {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for UiaFindParams {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UiaNotCondition {
    pub ConditionType: ConditionType,
    pub pCondition: *mut UiaCondition,
}
impl ::core::marker::Copy for UiaNotCondition {}
impl ::core::clone::Clone for UiaNotCondition {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UiaPoint {
    pub x: f64,
    pub y: f64,
}
impl ::core::marker::Copy for UiaPoint {}
impl ::core::clone::Clone for UiaPoint {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
pub struct UiaPropertyChangedEventArgs {
    pub Type: EventArgsType,
    pub EventId: UIA_EVENT_ID,
    pub PropertyId: i32,
    pub OldValue: super::super::System::Variant::VARIANT,
    pub NewValue: super::super::System::Variant::VARIANT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for UiaPropertyChangedEventArgs {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for UiaPropertyChangedEventArgs {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
pub struct UiaPropertyCondition {
    pub ConditionType: ConditionType,
    pub PropertyId: UIA_PROPERTY_ID,
    pub Value: super::super::System::Variant::VARIANT,
    pub Flags: PropertyConditionFlags,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for UiaPropertyCondition {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for UiaPropertyCondition {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UiaRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}
impl ::core::marker::Copy for UiaRect {}
impl ::core::clone::Clone for UiaRect {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UiaStructureChangedEventArgs {
    pub Type: EventArgsType,
    pub EventId: i32,
    pub StructureChangeType: StructureChangeType,
    pub pRuntimeId: *mut i32,
    pub cRuntimeIdLen: i32,
}
impl ::core::marker::Copy for UiaStructureChangedEventArgs {}
impl ::core::clone::Clone for UiaStructureChangedEventArgs {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct UiaTextEditTextChangedEventArgs {
    pub Type: EventArgsType,
    pub EventId: i32,
    pub TextEditChangeType: TextEditChangeType,
    pub pTextChange: *mut super::super::System::Com::SAFEARRAY,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for UiaTextEditTextChangedEventArgs {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for UiaTextEditTextChangedEventArgs {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UiaWindowClosedEventArgs {
    pub Type: EventArgsType,
    pub EventId: i32,
    pub pRuntimeId: *mut i32,
    pub cRuntimeIdLen: i32,
}
impl ::core::marker::Copy for UiaWindowClosedEventArgs {}
impl ::core::clone::Clone for UiaWindowClosedEventArgs {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
pub type LPFNACCESSIBLECHILDREN = ::core::option::Option<unsafe extern "system" fn(pacccontainer: IAccessible, ichildstart: i32, cchildren: i32, rgvarchildren: *mut super::super::System::Variant::VARIANT, pcobtained: *mut i32) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
pub type LPFNACCESSIBLEOBJECTFROMPOINT = ::core::option::Option<unsafe extern "system" fn(ptscreen: super::super::Foundation::POINT, ppacc: *mut IAccessible, pvarchild: *mut super::super::System::Variant::VARIANT) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LPFNACCESSIBLEOBJECTFROMWINDOW = ::core::option::Option<unsafe extern "system" fn(hwnd: super::super::Foundation::HWND, dwid: u32, riid: *const ::windows_sys::core::GUID, ppvobject: *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LPFNCREATESTDACCESSIBLEOBJECT = ::core::option::Option<unsafe extern "system" fn(hwnd: super::super::Foundation::HWND, idobject: i32, riid: *const ::windows_sys::core::GUID, ppvobject: *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LPFNLRESULTFROMOBJECT = ::core::option::Option<unsafe extern "system" fn(riid: *const ::windows_sys::core::GUID, wparam: super::super::Foundation::WPARAM, punk: ::windows_sys::core::IUnknown) -> super::super::Foundation::LRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LPFNOBJECTFROMLRESULT = ::core::option::Option<unsafe extern "system" fn(lresult: super::super::Foundation::LRESULT, riid: *const ::windows_sys::core::GUID, wparam: super::super::Foundation::WPARAM, ppvobject: *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub type UiaEventCallback = ::core::option::Option<unsafe extern "system" fn(pargs: *mut UiaEventArgs, prequesteddata: *mut super::super::System::Com::SAFEARRAY, ptreestructure: ::windows_sys::core::BSTR) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub type UiaProviderCallback = ::core::option::Option<unsafe extern "system" fn(hwnd: super::super::Foundation::HWND, providertype: ProviderType) -> *mut super::super::System::Com::SAFEARRAY>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type WINEVENTPROC = ::core::option::Option<unsafe extern "system" fn(hwineventhook: HWINEVENTHOOK, event: u32, hwnd: super::super::Foundation::HWND, idobject: i32, idchild: i32, ideventthread: u32, dwmseventtime: u32) -> ()>;
