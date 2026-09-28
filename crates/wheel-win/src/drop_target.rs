/// OLE IDropTarget implementation for the overlay window.
///
/// # Safety
/// This module uses COM/OLE unsafe code. All unsafe is contained here.
///
/// # Key design decisions
/// - We return DROPEFFECT_COPY always to prevent Explorer from moving/deleting files.
/// - DragEnter inspects CF_HDROP to extract file paths.

#[cfg(windows)]
mod windows_impl {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;
    use std::sync::Arc;

    use windows::core::{implement, Ref, Result as WinResult};
    use windows::Win32::Foundation::{HWND, POINTL};
    use windows::Win32::System::Com::{
        IDataObject, FORMATETC, DVASPECT_CONTENT, TYMED_HGLOBAL,
    };
    use windows::Win32::System::Ole::{
        IDropTarget, IDropTarget_Impl, RegisterDragDrop, RevokeDragDrop, ReleaseStgMedium,
        DROPEFFECT_COPY, DROPEFFECT_NONE, DROPEFFECT, OleInitialize,
    };
    use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
    use tracing::debug;

    pub type DropCallback = Box<dyn Fn(DropEvent) + Send + Sync + 'static>;

    #[derive(Debug, Clone)]
    pub enum DropEvent {
        Enter { files: Vec<PathBuf>, x: i32, y: i32 },
        Over { x: i32, y: i32 },
        Leave,
        Drop { files: Vec<PathBuf>, x: i32, y: i32 },
    }

    /// CF_HDROP format number (15)
    const CF_HDROP_VALUE: u16 = 15;

    unsafe fn extract_paths_from_data_object(data_object: &IDataObject) -> Vec<PathBuf> {
        let mut paths = Vec::new();

        let fmt = FORMATETC {
            cfFormat: CF_HDROP_VALUE,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };

        if let Ok(mut medium) = data_object.GetData(&fmt) {
            let hglobal = medium.u.hGlobal;
            let hdrop = HDROP(hglobal.0);
            let count = DragQueryFileW(hdrop, u32::MAX, None);

            for i in 0..count {
                let needed = DragQueryFileW(hdrop, i, None) as usize + 1;
                let mut buf: Vec<u16> = vec![0u16; needed];
                let written = DragQueryFileW(hdrop, i, Some(&mut buf)) as usize;
                if written > 0 {
                    let s = OsString::from_wide(&buf[..written]);
                    paths.push(PathBuf::from(s));
                }
            }

            // Release the STGMEDIUM
            ReleaseStgMedium(&mut medium as *mut _);
        }

        paths
    }

    #[implement(IDropTarget)]
    pub struct WheelDropTarget {
        hwnd: HWND,
        callback: Arc<DropCallback>,
    }

    impl WheelDropTarget {
        pub fn new(hwnd: HWND, callback: DropCallback) -> Self {
            Self {
                hwnd,
                callback: Arc::new(callback),
            }
        }

        fn to_client_pt(&self, pt: &POINTL) -> (i32, i32) {
            use windows::Win32::Graphics::Gdi::ScreenToClient;
            use windows::Win32::Foundation::POINT;
            let mut p = POINT { x: pt.x, y: pt.y };
            unsafe {
                let _ = ScreenToClient(self.hwnd, &mut p);
            }
            (p.x, p.y)
        }
    }

    impl IDropTarget_Impl for WheelDropTarget_Impl {
        fn DragEnter(
            &self,
            pdataobj: Ref<'_, IDataObject>,
            _grfkeystate: MODIFIERKEYS_FLAGS,
            pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> WinResult<()> {
            unsafe {
                if let Some(data_obj) = pdataobj.as_ref() {
                    let files = extract_paths_from_data_object(data_obj);
                    let (cx, cy) = self.to_client_pt(pt);
                    debug!("DragEnter: {} files at client ({}, {})", files.len(), cx, cy);

                    if !files.is_empty() {
                        (self.callback)(DropEvent::Enter {
                            files,
                            x: cx,
                            y: cy,
                        });
                        *pdweffect = DROPEFFECT_COPY;
                    } else {
                        *pdweffect = DROPEFFECT_NONE;
                    }
                }
            }
            Ok(())
        }

        fn DragOver(
            &self,
            _grfkeystate: MODIFIERKEYS_FLAGS,
            pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> WinResult<()> {
            let (cx, cy) = self.to_client_pt(pt);
            (self.callback)(DropEvent::Over { x: cx, y: cy });
            unsafe { *pdweffect = DROPEFFECT_COPY; }
            Ok(())
        }

        fn DragLeave(&self) -> WinResult<()> {
            (self.callback)(DropEvent::Leave);
            Ok(())
        }

        fn Drop(
            &self,
            pdataobj: Ref<'_, IDataObject>,
            _grfkeystate: MODIFIERKEYS_FLAGS,
            pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> WinResult<()> {
            unsafe {
                if let Some(data_obj) = pdataobj.as_ref() {
                    let files = extract_paths_from_data_object(data_obj);
                    let (cx, cy) = self.to_client_pt(pt);
                    debug!("Drop: {} files at client ({}, {})", files.len(), cx, cy);
                    (self.callback)(DropEvent::Drop {
                        files,
                        x: cx,
                        y: cy,
                    });
                    *pdweffect = DROPEFFECT_COPY;
                }
            }
            Ok(())
        }
    }

    pub fn register_drop_target(
        hwnd: isize,
        callback: DropCallback,
    ) -> anyhow::Result<DropTargetHandle> {
        unsafe {
            let _ = OleInitialize(None);
            let hwnd = HWND(hwnd as *mut std::ffi::c_void);
            let target = WheelDropTarget::new(hwnd, callback);
            let itarget: IDropTarget = target.into();
            RegisterDragDrop(hwnd, &itarget)
                .map_err(|e| anyhow::anyhow!("RegisterDragDrop failed: {:?}", e))?;
            Ok(DropTargetHandle { hwnd: hwnd.0 as isize })
        }
    }

    pub struct DropTargetHandle {
        hwnd: isize,
    }

    impl Drop for DropTargetHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = RevokeDragDrop(HWND(self.hwnd as *mut std::ffi::c_void));
            }
        }
    }
}

#[cfg(windows)]
pub use windows_impl::{DropCallback, DropEvent, register_drop_target, DropTargetHandle};

#[cfg(not(windows))]
pub type DropCallback = Box<dyn Fn(()) + Send + Sync + 'static>;
#[cfg(not(windows))]
pub struct DropTargetHandle;
#[cfg(not(windows))]
pub fn register_drop_target(_hwnd: isize, _callback: DropCallback) -> anyhow::Result<DropTargetHandle> {
    Ok(DropTargetHandle)
}
