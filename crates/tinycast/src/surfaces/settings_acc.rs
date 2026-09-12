//! MSAA/UIA provider for the custom-drawn Settings window.

use windows::core::{implement, IUnknown, Interface, BSTR, VARIANT};
use windows::Win32::Foundation::{
    DISP_E_MEMBERNOTFOUND, E_INVALIDARG, E_NOTIMPL, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Com::{IDispatch, ITypeInfo, DISPATCH_FLAGS, DISPPARAMS, EXCEPINFO};
use windows::Win32::UI::Accessibility::{
    CreateStdAccessibleObject, IAccessible, IAccessible_Impl, LresultFromObject,
    UiaProviderFromIAccessible, UiaReturnRawElementProvider, UiaRootObjectId, ROLE_SYSTEM_WINDOW,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, PostMessageW, WM_APP};

use super::focus_items;
use super::inner_from;
use crate::features::settings::focus::{acc_role_id, acc_state, acc_value, default_action};
use crate::platform::screens::dip_scalar_to_px;

pub const WM_SETTINGS_ACC_INVOKE: u32 = WM_APP + 40;
const OBJID_CLIENT: i32 = -4;
const OBJID_WINDOW: i32 = 0;

#[implement(IAccessible)]
struct SettingsAcc {
    hwnd: isize,
}

fn hwnd_of(raw: isize) -> HWND {
    HWND(raw as *mut core::ffi::c_void)
}

fn child_id(var: &VARIANT) -> windows::core::Result<i32> {
    if var.is_empty() {
        return Ok(0);
    }
    i32::try_from(var)
}

fn snapshot(hwnd: HWND) -> Vec<crate::features::settings::focus::FocusItem> {
    unsafe {
        inner_from(hwnd)
            .map(|inner| focus_items(hwnd, inner))
            .unwrap_or_default()
    }
}

fn item_at(
    hwnd: HWND,
    child: i32,
) -> windows::core::Result<crate::features::settings::focus::FocusItem> {
    if child <= 0 {
        return Err(E_INVALIDARG.into());
    }
    snapshot(hwnd)
        .into_iter()
        .nth((child as usize).saturating_sub(1))
        .ok_or_else(|| E_INVALIDARG.into())
}

fn not_found<T>() -> windows::core::Result<T> {
    Err(DISP_E_MEMBERNOTFOUND.into())
}

impl windows::Win32::System::Com::IDispatch_Impl for SettingsAcc_Impl {
    fn GetTypeInfoCount(&self) -> windows::core::Result<u32> {
        Ok(0)
    }

    fn GetTypeInfo(&self, _itinfo: u32, _lcid: u32) -> windows::core::Result<ITypeInfo> {
        Err(E_NOTIMPL.into())
    }

    fn GetIDsOfNames(
        &self,
        _riid: *const windows::core::GUID,
        _rgsznames: *const windows::core::PCWSTR,
        _cnames: u32,
        _lcid: u32,
        _rgdispid: *mut i32,
    ) -> windows::core::Result<()> {
        Err(DISP_E_MEMBERNOTFOUND.into())
    }

    fn Invoke(
        &self,
        _dispidmember: i32,
        _riid: *const windows::core::GUID,
        _lcid: u32,
        _wflags: DISPATCH_FLAGS,
        _pdispparams: *const DISPPARAMS,
        _pvarresult: *mut VARIANT,
        _pexcepinfo: *mut EXCEPINFO,
        _puargerr: *mut u32,
    ) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }
}

impl IAccessible_Impl for SettingsAcc_Impl {
    fn accParent(&self) -> windows::core::Result<IDispatch> {
        let hwnd = hwnd_of(self.hwnd);
        let mut pv: *mut core::ffi::c_void = std::ptr::null_mut();
        unsafe {
            CreateStdAccessibleObject(hwnd, OBJID_WINDOW, &IAccessible::IID, &mut pv)?;
        }
        if pv.is_null() {
            return Err(E_NOTIMPL.into());
        }
        let unk = unsafe { IUnknown::from_raw(pv) };
        unk.cast()
    }

    fn accChildCount(&self) -> windows::core::Result<i32> {
        Ok(snapshot(hwnd_of(self.hwnd)).len() as i32)
    }

    fn get_accChild(&self, varchild: &VARIANT) -> windows::core::Result<IDispatch> {
        let id = child_id(varchild)?;
        if id == 0 {
            return unsafe { self.cast() };
        }
        not_found()
    }

    fn get_accName(&self, varchild: &VARIANT) -> windows::core::Result<BSTR> {
        let id = child_id(varchild)?;
        if id == 0 {
            return Ok(BSTR::from("Settings"));
        }
        let item = item_at(hwnd_of(self.hwnd), id)?;
        if item.secret {
            return not_found();
        }
        Ok(BSTR::from(item.name.as_str()))
    }

    fn get_accValue(&self, varchild: &VARIANT) -> windows::core::Result<BSTR> {
        let id = child_id(varchild)?;
        if id == 0 {
            return not_found();
        }
        let item = item_at(hwnd_of(self.hwnd), id)?;
        acc_value(&item)
            .map(|v| BSTR::from(v.as_str()))
            .ok_or_else(|| DISP_E_MEMBERNOTFOUND.into())
    }

    fn get_accDescription(&self, _varchild: &VARIANT) -> windows::core::Result<BSTR> {
        not_found()
    }

    fn get_accRole(&self, varchild: &VARIANT) -> windows::core::Result<VARIANT> {
        let id = child_id(varchild)?;
        if id == 0 {
            return Ok(VARIANT::from(ROLE_SYSTEM_WINDOW as i32));
        }
        let item = item_at(hwnd_of(self.hwnd), id)?;
        Ok(VARIANT::from(acc_role_id(item.role)))
    }

    fn get_accState(&self, varchild: &VARIANT) -> windows::core::Result<VARIANT> {
        let id = child_id(varchild)?;
        let hwnd = hwnd_of(self.hwnd);
        if id == 0 {
            return Ok(VARIANT::from(0x0010_0000i32));
        }
        let focused = unsafe { inner_from(hwnd).and_then(|inner| (*inner).focus) };
        let item = item_at(hwnd, id)?;
        Ok(VARIANT::from(acc_state(
            &item,
            focused == Some((id as usize) - 1),
        )))
    }

    fn get_accHelp(&self, _varchild: &VARIANT) -> windows::core::Result<BSTR> {
        not_found()
    }

    fn get_accHelpTopic(
        &self,
        _pszhelpfile: *mut BSTR,
        _varchild: &VARIANT,
    ) -> windows::core::Result<i32> {
        Err(E_NOTIMPL.into())
    }

    fn get_accKeyboardShortcut(&self, _varchild: &VARIANT) -> windows::core::Result<BSTR> {
        not_found()
    }

    fn accFocus(&self) -> windows::core::Result<VARIANT> {
        let hwnd = hwnd_of(self.hwnd);
        let focused = unsafe { inner_from(hwnd).and_then(|inner| (*inner).focus) };
        Ok(VARIANT::from(focused.map(|i| (i as i32) + 1).unwrap_or(0)))
    }

    fn accSelection(&self) -> windows::core::Result<VARIANT> {
        self.accFocus()
    }

    fn get_accDefaultAction(&self, varchild: &VARIANT) -> windows::core::Result<BSTR> {
        let id = child_id(varchild)?;
        if id == 0 {
            return not_found();
        }
        let item = item_at(hwnd_of(self.hwnd), id)?;
        Ok(BSTR::from(default_action(item.role)))
    }

    fn accSelect(&self, _flagsselect: i32, varchild: &VARIANT) -> windows::core::Result<()> {
        let id = child_id(varchild)?;
        if id <= 0 {
            return Ok(());
        }
        let hwnd = hwnd_of(self.hwnd);
        unsafe {
            if let Some(inner) = inner_from(hwnd) {
                (*inner).focus = Some((id as usize) - 1);
            }
        }
        Ok(())
    }

    fn accLocation(
        &self,
        pxleft: *mut i32,
        pytop: *mut i32,
        pcxwidth: *mut i32,
        pcyheight: *mut i32,
        varchild: &VARIANT,
    ) -> windows::core::Result<()> {
        let hwnd = hwnd_of(self.hwnd);
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        let mut origin = POINT::default();
        unsafe {
            let _ = ClientToScreen(hwnd, &mut origin);
        }
        let id = child_id(varchild)?;
        let (x, y, w, h) = if id == 0 {
            let mut rc = windows::Win32::Foundation::RECT::default();
            unsafe {
                let _ = GetClientRect(hwnd, &mut rc);
            }
            (origin.x, origin.y, rc.right - rc.left, rc.bottom - rc.top)
        } else {
            let item = item_at(hwnd, id)?;
            (
                origin.x + dip_scalar_to_px(item.rect.x, dpi),
                origin.y + dip_scalar_to_px(item.rect.y, dpi),
                dip_scalar_to_px(item.rect.w, dpi),
                dip_scalar_to_px(item.rect.h, dpi),
            )
        };
        unsafe {
            if !pxleft.is_null() {
                *pxleft = x;
            }
            if !pytop.is_null() {
                *pytop = y;
            }
            if !pcxwidth.is_null() {
                *pcxwidth = w;
            }
            if !pcyheight.is_null() {
                *pcyheight = h;
            }
        }
        Ok(())
    }

    fn accNavigate(&self, navdir: i32, varstart: &VARIANT) -> windows::core::Result<VARIANT> {
        let start = child_id(varstart)?;
        let items = snapshot(hwnd_of(self.hwnd));
        if items.is_empty() {
            return not_found();
        }
        let idx = if start <= 0 { 0 } else { (start as usize) - 1 };
        let next = match navdir {
            0x5 => idx.saturating_add(1),         // NAVDIR_NEXT
            0x4 => idx.saturating_sub(1),         // NAVDIR_PREVIOUS
            0x2 => items.len().saturating_sub(1), // NAVDIR_LAST
            _ => 0,                               // FIRST / DOWN / etc.
        };
        if next < items.len() {
            Ok(VARIANT::from((next as i32) + 1))
        } else {
            not_found()
        }
    }

    fn accHitTest(&self, xleft: i32, ytop: i32) -> windows::core::Result<VARIANT> {
        let hwnd = hwnd_of(self.hwnd);
        let mut origin = POINT::default();
        unsafe {
            let _ = ClientToScreen(hwnd, &mut origin);
        }
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        let x = px_to_dip(xleft - origin.x, dpi);
        let y = px_to_dip(ytop - origin.y, dpi);
        for (i, item) in snapshot(hwnd).iter().enumerate() {
            if x >= item.rect.x
                && x < item.rect.x + item.rect.w
                && y >= item.rect.y
                && y < item.rect.y + item.rect.h
            {
                return Ok(VARIANT::from((i as i32) + 1));
            }
        }
        Ok(VARIANT::from(0i32))
    }

    fn accDoDefaultAction(&self, varchild: &VARIANT) -> windows::core::Result<()> {
        let id = child_id(varchild)?;
        if id <= 0 {
            return Ok(());
        }
        let hwnd = hwnd_of(self.hwnd);
        unsafe {
            let _ = PostMessageW(
                hwnd,
                WM_SETTINGS_ACC_INVOKE,
                WPARAM((id as usize) - 1),
                LPARAM(0),
            );
        }
        Ok(())
    }

    fn put_accName(&self, _varchild: &VARIANT, _szname: &BSTR) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn put_accValue(&self, _varchild: &VARIANT, _szvalue: &BSTR) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }
}

pub unsafe fn handle_getobject(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let id = lparam.0 as i32;
    if id != OBJID_CLIENT && id != UiaRootObjectId {
        return LRESULT(0);
    }
    let acc: IAccessible = SettingsAcc {
        hwnd: hwnd.0 as isize,
    }
    .into();
    if id == UiaRootObjectId {
        if let Ok(provider) = UiaProviderFromIAccessible(&acc, 0, 0) {
            return UiaReturnRawElementProvider(hwnd, wparam, lparam, &provider);
        }
    }
    LresultFromObject(&IAccessible::IID, wparam, &acc)
}

fn px_to_dip(px: i32, dpi: u32) -> f32 {
    let dpi = if dpi == 0 { 96.0 } else { dpi as f32 };
    px as f32 * 96.0 / dpi
}
